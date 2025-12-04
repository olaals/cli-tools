// src/watch/event_handler.rs

//! Event processing logic for file system changes.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use crate::engine::{RuntimeEvent, TriggerReason};
use crate::watch::cache::FileCache;
use crate::watch::dag_filter::has_ancestor_in_matching;
use crate::watch::hash::{compute_aggregate_hash, HashStore};
use crate::watch::path_utils::relative_str;
use crate::watch::patterns::{collect_matching_files, TaskWatchProfile};

/// Process a single file change event and trigger appropriate tasks.
///
/// This function:
/// 1. Finds all tasks whose patterns match the changed path
/// 2. Applies DAG-aware filtering to trigger only root tasks
/// 3. Applies hash-based filtering if enabled
/// 4. Sends trigger events to the runtime
pub async fn process_file_change(
    root: &PathBuf,
    path: &PathBuf,
    profiles: &Arc<Vec<TaskWatchProfile>>,
    dep_map: &Arc<HashMap<String, Vec<String>>>,
    runtime_tx: &mpsc::Sender<RuntimeEvent>,
    hash_store: Arc<Mutex<Box<dyn HashStore>>>,
    file_cache: Arc<Mutex<FileCache>>,
) {
    let rel_str = match relative_str(root, path) {
        Some(s) => s,
        None => {
            warn!(
                "could not relativize path {:?} against root {:?}",
                path, root
            );
            return;
        }
    };

    debug!(?path, rel = %rel_str, "normalized event path");

    // 1) Find all tasks whose watch/exclude patterns match this path.
    let matching_profiles: Vec<&TaskWatchProfile> = profiles
        .iter()
        .filter(|p| p.matches(&rel_str))
        .collect();

    if matching_profiles.is_empty() {
        return;
    }

    // 2) Build a set of their names so we can check ancestors.
    let matching_names: HashSet<String> = matching_profiles
        .iter()
        .map(|p| p.name().to_string())
        .collect();

    // 3) Keep only those tasks that DO NOT have any ancestor
    //    also in `matching_names`. These are the "roots for
    //    this path".
    let mut root_profiles: Vec<&TaskWatchProfile> = Vec::new();
    for profile in matching_profiles {
        let name = profile.name();
        if !has_ancestor_in_matching(name, &matching_names, dep_map) {
            root_profiles.push(profile);
        }
    }

    // Nothing to do if everything was filtered out somehow.
    if root_profiles.is_empty() {
        return;
    }

    let trigger_names: Vec<&str> =
        root_profiles.iter().map(|p| p.name()).collect();
    debug!(
        rel = %rel_str,
        ?trigger_names,
        "DAG-aware filter: triggering only root tasks for this path"
    );

    // 4) For each selected task, apply optional hash-based
    //    content change detection and emit a trigger.
    for profile in root_profiles {
        if should_trigger_task(
            root,
            path,
            &rel_str,
            profile,
            Arc::clone(&hash_store),
            Arc::clone(&file_cache),
        )
        .await
        {
            let task_name = profile.name().to_string();
            debug!(
                task = %task_name,
                path = %rel_str,
                "watch match -> triggering task"
            );
            if let Err(err) = runtime_tx
                .send(RuntimeEvent::TaskTriggered {
                    task: task_name,
                    reason: TriggerReason::FileWatch,
                })
                .await
            {
                warn!("failed to send RuntimeEvent::TaskTriggered: {err}");
                // If the runtime channel is closed, there's no point
                // keeping the watcher loop alive.
                return;
            }
        }
    }
}

/// Check if a task should be triggered based on hash comparison.
///
/// Returns true if the task should be triggered, false if it should be skipped.
async fn should_trigger_task(
    root: &PathBuf,
    abs_path: &PathBuf,
    rel_path: &str,
    profile: &TaskWatchProfile,
    hash_store: Arc<Mutex<Box<dyn HashStore>>>,
    file_cache: Arc<Mutex<FileCache>>,
) -> bool {
    let task_name = profile.name().to_string();

    // If use_hash is not enabled, always trigger.
    if !profile.use_hash() {
        return true;
    }

    let root = root.clone();
    let abs_path = abs_path.clone();
    let profile = profile.clone();
    let rel_path = rel_path.to_string();

    tokio::task::spawn_blocking(move || {
        // If use_hash is enabled, only trigger when the aggregated
        // contents of all watched files for this task actually change.
        let files = match collect_matching_files(&root, &profile) {
            Ok(f) => f,
            Err(err) => {
                warn!(
                    task = %task_name,
                    error = %err,
                    "failed to collect watched files; triggering anyway"
                );
                return true;
            }
        };

        // Invalidate/update the cache for the changed file first.
        {
            let mut cache = match file_cache.lock() {
                Ok(g) => g,
                Err(_) => {
                    warn!("file cache mutex poisoned; triggering anyway");
                    return true;
                }
            };
            // We can just invalidate it, and get_or_compute will re-read it.
            // Or we can force update it now.
            cache.invalidate(&abs_path);
        }

        // Compute hashes for all files (using cache where possible).
        let mut file_hashes = Vec::with_capacity(files.len());
        {
            let mut cache = match file_cache.lock() {
                Ok(g) => g,
                Err(_) => {
                    warn!("file cache mutex poisoned; triggering anyway");
                    return true;
                }
            };

            for file_path in files {
                match cache.get_or_compute(&file_path) {
                    Ok(h) => file_hashes.push(h),
                    Err(err) => {
                        warn!(
                            task = %task_name,
                            file = ?file_path,
                            error = %err,
                            "failed to compute file hash; triggering anyway"
                        );
                        return true;
                    }
                }
            }
        }

        let new_hash = compute_aggregate_hash(&file_hashes);

        let mut store = match hash_store.lock() {
            Ok(guard) => guard,
            Err(_poisoned) => {
                warn!(task = %task_name, "hash store mutex poisoned; triggering anyway");
                return true;
            }
        };

        match store.load(&task_name) {
            Ok(Some(old_hash)) if old_hash == new_hash => {
                // Nice user-facing message on stdout
                // and a log entry when we skip due to unchanged hash.
                println!(
                    "[watchdag] Skipping task '{}' (watched content unchanged; last event path '{}')",
                    task_name, rel_path
                );
                info!(
                    task = %task_name,
                    path = %rel_path,
                    "hash unchanged; skipping trigger"
                );
                return false;
            }
            Ok(_) => {
                if let Err(err) = store.save(&task_name, &new_hash) {
                    warn!(
                        task = %task_name,
                        error = %err,
                        "failed to save task hash"
                    );
                }
            }
            Err(err) => {
                warn!(
                    task = %task_name,
                    error = %err,
                    "failed to load task hash; triggering anyway"
                );
            }
        }

        true
    })
    .await
    .unwrap_or(true) // If the blocking task panics, default to triggering.
}
