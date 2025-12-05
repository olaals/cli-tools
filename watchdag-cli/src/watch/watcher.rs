// src/watch/watcher.rs

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::engine::RuntimeEvent;
use crate::fs::{FileSystem, RealFileSystem};
use crate::types::HashStorageMode;
use crate::watch::cache::FileCache;
use crate::watch::event_handler::process_file_change;
use crate::watch::hash::{FileHashStore, HashStore, MemoryHashStore};
use crate::watch::patterns::TaskWatchProfile;

/// Handle for the filesystem watcher.
///
/// This exists mainly so the underlying `RecommendedWatcher` is kept alive for
/// as long as needed. Dropping this handle will stop file watching.
pub struct WatcherHandle {
    _inner: RecommendedWatcher,
}

impl std::fmt::Debug for WatcherHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WatcherHandle").finish()
    }
}

/// Spawn a filesystem watcher that observes the given `root` directory
/// recursively and sends `RuntimeEvent::TaskTriggered` for tasks whose
/// patterns match a changed path.
///
/// - `root` is the project root against which all glob patterns are evaluated.
/// - `profiles` is the compiled per-task pattern set.
/// - `runtime_tx` is the channel into the main runtime.
/// - `hash_storage_mode` determines where task hashes are stored.
pub fn spawn_watcher(
    root: impl Into<PathBuf>,
    profiles: Vec<TaskWatchProfile>,
    runtime_tx: mpsc::Sender<RuntimeEvent>,
    hash_storage_mode: HashStorageMode,
) -> Result<WatcherHandle> {
    let root = root.into();
    // Canonicalize once so we have a stable base path.
    let root = root.canonicalize().unwrap_or_else(|_| root.clone());

    let profiles = Arc::new(profiles);

    // Build a simple dependency map so we can reason about ancestors in the
    // watcher (for DAG-aware triggering).
    let dep_map: HashMap<String, Vec<String>> = profiles
        .iter()
        .map(|p| {
            (
                p.name().to_string(),
                p.deps().iter().cloned().collect::<Vec<_>>(),
            )
        })
        .collect();
    let dep_map = Arc::new(dep_map);

    // Channel from the blocking notify callback into the async world.
    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<Event>();

    // Closure called synchronously by notify whenever an event arrives.
    let mut watcher = RecommendedWatcher::new(
        {
            let event_tx = event_tx.clone();
            move |res: notify::Result<Event>| {
                match res {
                    Ok(event) => {
                        if let Err(err) = event_tx.send(event) {
                            // We can't log via tracing here easily, so fallback to stderr.
                            eprintln!("watchdag: failed to forward notify event: {err}");
                        }
                    }
                    Err(err) => {
                        eprintln!("watchdag: file watch error: {err}");
                    }
                }
            }
        },
        Config::default(),
    )?;

    watcher.watch(&root, RecursiveMode::Recursive)?;

    info!("file watcher started on {:?}", root);

    // Async task that consumes notify events and forwards task triggers to the runtime.
    let async_root = root.clone();
    let async_profiles = Arc::clone(&profiles);
    let async_dep_map = Arc::clone(&dep_map);
    let fs: Arc<dyn FileSystem> = Arc::new(RealFileSystem);

    let mut hash_store: Box<dyn HashStore> = match hash_storage_mode {
        HashStorageMode::File => Box::new(FileHashStore::new(async_root.clone(), fs.clone())),
        HashStorageMode::Memory => Box::new(MemoryHashStore::new()),
    };

    // Prune stale hashes (e.g. from renamed/removed tasks) at startup.
    let active_task_names: Vec<&str> = async_profiles.iter().map(|p| p.name()).collect();
    if let Err(e) = hash_store.prune(&active_task_names) {
        tracing::warn!("failed to prune stale hashes: {}", e);
    }

    tokio::spawn(async move {
        let file_cache = Arc::new(Mutex::new(FileCache::new()));
        let hash_store = Arc::new(Mutex::new(hash_store));

        while let Some(event) = event_rx.recv().await {
            debug!(?event, "received notify event");

            // We only care about events that modify content or create/remove files.
            // Filter out others if needed.
            // For now, we just process all paths in the event.
            for path in event.paths {
                process_file_change(
                    fs.clone(),
                    &async_root,
                    &path,
                    &async_profiles,
                    &async_dep_map,
                    &runtime_tx,
                    Arc::clone(&hash_store),
                    Arc::clone(&file_cache),
                )
                .await;
            }
        }
        debug!("watcher event loop finished");
    });

    Ok(WatcherHandle { _inner: watcher })
}
