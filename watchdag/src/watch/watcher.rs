// src/watch/watcher.rs

use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::engine::{RuntimeEvent, TriggerReason};
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
pub fn spawn_watcher(
    root: impl Into<PathBuf>,
    profiles: Vec<TaskWatchProfile>,
    runtime_tx: mpsc::Sender<RuntimeEvent>,
) -> Result<WatcherHandle> {
    let root = root.into();
    let root = root
        .canonicalize()
        .unwrap_or_else(|_| root.clone()); // best-effort

    let profiles = Arc::new(profiles);

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
    tokio::spawn(async move {
        let mut runtime_tx = runtime_tx;

        while let Some(event) = event_rx.recv().await {
            debug!("received notify event: {:?}", event);

            for path in &event.paths {
                if let Some(rel_str) = relative_str(&async_root, path) {
                    for profile in async_profiles.iter() {
                        if profile.matches(&rel_str) {
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
                } else {
                    warn!(
                        "could not relativize path {:?} against root {:?}",
                        path, async_root
                    );
                }
            }
        }

        debug!("file watcher loop ended");
    });

    Ok(WatcherHandle { _inner: watcher })
}

/// Convert a path into a string relative to `root`, with forward slashes.
///
/// Returns `None` if the path is not under `root` and cannot be relativized.
fn relative_str(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let s = rel.to_string_lossy().replace('\\', "/");
    Some(s)
}
