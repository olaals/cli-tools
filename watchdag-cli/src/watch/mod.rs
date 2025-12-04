// src/watch/mod.rs

//! File watching and change detection.
//!
//! This module is responsible for:
//! - Compiling `watch` / `exclude` glob patterns per task.
//! - Wiring up a cross-platform filesystem watcher (`notify`).
//! - (Optionally) supporting content hashing to avoid re-running tasks when
//!   watched files haven't actually changed.
//!
//! It does **not** know about the DAG or task dependencies; it only turns
//! filesystem changes into task-level triggers.

pub mod dag_filter;
pub mod event_handler;
pub mod hash;
pub mod path_utils;
pub mod patterns;
pub mod watcher;
pub mod cache;

pub use patterns::{
    build_profiles_from_config, build_task_watch_profiles, RawTaskPatternSpec,
    TaskWatchProfile, WatchDefaults,
};
pub use watcher::{spawn_watcher, WatcherHandle};
pub use hash::{
    compute_hash_for_paths, FileHashStore, HashStore, MemoryHashStore,
    HASH_FILE_PATH,
};
