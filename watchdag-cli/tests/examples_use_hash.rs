mod common;
use crate::common::builders::{ConfigFileBuilder, TaskConfigBuilder};
use crate::common::init_tracing;

use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;
use tokio::time::{sleep, timeout, Duration};

use watchdag::config::{
    load_and_validate, ConfigFile, TaskConfig,
};
use watchdag::engine::{RuntimeEvent, TriggerReason};
use watchdag::types::HashStorageMode;
use watchdag::watch::{
    compute_hash_for_paths, spawn_watcher,
};

type TestResult = Result<(), Box<dyn Error>>;

/// Build a synthetic in-memory config to avoid loading from TOML.
///
/// Layout:
/// - [default]
///     watch     = ["src/**/*.rs"]
///     exclude   = ["src/tmp/**"]
///     use_hash  = true
/// - [task.A]
///     cmd       = "echo A"
///     use_hash  = <inherits true>
/// - [task.B]
///     cmd       = "echo B"
///     use_hash  = false
fn synthetic_use_hash_config() -> ConfigFile {
    ConfigFileBuilder::new()
        .with_global_watch("src/**/*.rs")
        .with_global_exclude("src/tmp/**")
        .with_default_use_hash(true)
        .with_task("A", TaskConfigBuilder::new("echo A").build())
        .with_task("B", TaskConfigBuilder::new("echo B").use_hash(false).build())
        .build()
}

#[test]
fn config_use_hash_defaults_and_overrides() -> TestResult {
    init_tracing();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest_dir.join("examples/use-hash.toml"))?;

    let default_use_hash = cfg.default.use_hash.unwrap_or(false);

    let a: &TaskConfig = cfg.task.get("A").unwrap();
    let b: &TaskConfig = cfg.task.get("B").unwrap();

    assert!(a.effective_use_hash(default_use_hash));
    assert!(!b.effective_use_hash(default_use_hash));

    Ok(())
}

#[test]
fn compute_hash_is_order_insensitive_and_tracks_content_changes() -> TestResult {
    init_tracing();

    let dir = tempdir()?;
    let f1 = dir.path().join("a.txt");
    let f2 = dir.path().join("b.txt");

    fs::write(&f1, "hello")?;
    fs::write(&f2, "world")?;

    let h1 = compute_hash_for_paths([&f1, &f2])?;
    let h2 = compute_hash_for_paths([&f2, &f1])?;
    assert_eq!(h1, h2);

    fs::write(&f1, "HELLO")?;
    let h3 = compute_hash_for_paths([&f1, &f2])?;
    assert_ne!(h1, h3);

    Ok(())
}

/// In-memory config + watch profiles:
/// - Asserts that `use_hash` is correctly propagated from Default/task into
///   `TaskWatchProfile`.
/// - Also checks that watch + exclude patterns behave as expected.
/// This does *not* load any TOML file.
#[test]
fn in_memory_config_builds_profiles_with_correct_use_hash_and_patterns() -> TestResult {
    init_tracing();

    let cfg = synthetic_use_hash_config();
    let (_defaults, profiles) = watchdag::watch::build_profiles_from_config(&cfg)?;

    let a = profiles.iter().find(|p| p.name() == "A").unwrap();
    let b = profiles.iter().find(|p| p.name() == "B").unwrap();

    // A inherits use_hash = true from default, B overrides to false.
    assert!(a.use_hash(), "A should inherit use_hash = true from defaults");
    assert!(!b.use_hash(), "B should override use_hash = false");

    // Both tasks watch Rust files under src/
    assert!(a.matches("src/main.rs"));
    assert!(b.matches("src/main.rs"));

    // Both should respect the global exclude "src/tmp/**"
    assert!(!a.matches("src/tmp/generated.rs"));
    assert!(!b.matches("src/tmp/generated.rs"));

    Ok(())
}

#[tokio::test]
async fn file_change_without_content_change_only_triggers_non_hashing_task() {
    init_tracing();

    // Load the real use-hash example so the test tracks the README.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest_dir.join("examples/use-hash.toml")).unwrap();

    // Sanity-check assumptions about the config:
    // - [default].use_hash = true
    // - task A inherits use_hash = true
    // - task B overrides use_hash = false
    let default_use_hash = cfg.default.use_hash.unwrap_or(false);
    let a: &TaskConfig = cfg.task.get("A").expect("task A");
    let b: &TaskConfig = cfg.task.get("B").expect("task B");
    assert!(a.effective_use_hash(default_use_hash));
    assert!(!b.effective_use_hash(default_use_hash));

    let profiles = watchdag::watch::build_profiles_from_config(&cfg).unwrap().1;

    // Watch a fresh temporary root so test I/O is isolated.
    let temp_dir = tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();

    // Match the "texts/**.txt" pattern from examples/use-hash.toml.
    std::fs::create_dir_all(root.join("texts")).unwrap();
    let watched_file = root.join("texts").join("input.txt");

    let (runtime_tx, mut runtime_rx) = tokio::sync::mpsc::channel::<RuntimeEvent>(32);

    let _watcher = spawn_watcher(&root, profiles, runtime_tx, HashStorageMode::File).unwrap();

    // Give the OS watcher a brief moment to start.
    sleep(Duration::from_millis(100)).await;

    // First write: new content -> both A and B should be triggered.
    tokio::fs::write(&watched_file, "hello").await.unwrap();
    sleep(Duration::from_millis(50)).await;

    let first_tasks = collect_triggered_filewatch_tasks(&mut runtime_rx).await;
    assert!(
        first_tasks.contains("A"),
        "first change: task A should be triggered"
    );
    assert!(
        first_tasks.contains("B"),
        "first change: task B should be triggered"
    );

    // Second write with identical contents:
    // Desired behaviour for use_hash:
    // - A (use_hash = true) should *not* trigger again.
    // - B (use_hash = false) should still trigger.
    tokio::fs::write(&watched_file, "hello").await.unwrap();
    sleep(Duration::from_millis(50)).await;

    let second_tasks = collect_triggered_filewatch_tasks(&mut runtime_rx).await;

    assert!(
        !second_tasks.contains("A"),
        "second change with identical content: task A (use_hash=true) should NOT be triggered again"
    );
    assert!(
        second_tasks.contains("B"),
        "second change with identical content: task B (use_hash=false) should still be triggered"
    );
}

#[tokio::test]
async fn memory_storage_mode_works_same_as_file_within_run() {
    init_tracing();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest_dir.join("examples/use-hash.toml")).unwrap();
    let profiles = watchdag::watch::build_profiles_from_config(&cfg).unwrap().1;

    let temp_dir = tempdir().unwrap();
    let root = temp_dir.path().to_path_buf();

    std::fs::create_dir_all(root.join("texts")).unwrap();
    let watched_file = root.join("texts").join("input.txt");

    let (runtime_tx, mut runtime_rx) = tokio::sync::mpsc::channel::<RuntimeEvent>(32);

    // Use Memory mode
    let _watcher = spawn_watcher(&root, profiles, runtime_tx, HashStorageMode::Memory).unwrap();

    sleep(Duration::from_millis(100)).await;

    // 1. Write "hello" -> triggers A and B
    tokio::fs::write(&watched_file, "hello").await.unwrap();
    sleep(Duration::from_millis(50)).await;
    let tasks = collect_triggered_filewatch_tasks(&mut runtime_rx).await;
    assert!(tasks.contains("A"));
    assert!(tasks.contains("B"));

    // 2. Write "hello" again -> A should NOT trigger (hash match in memory)
    tokio::fs::write(&watched_file, "hello").await.unwrap();
    sleep(Duration::from_millis(50)).await;
    let tasks = collect_triggered_filewatch_tasks(&mut runtime_rx).await;
    assert!(!tasks.contains("A"));
    assert!(tasks.contains("B"));
}

async fn collect_triggered_filewatch_tasks(
    rx: &mut tokio::sync::mpsc::Receiver<RuntimeEvent>,
) -> HashSet<String> {
    let mut tasks = HashSet::new();

    loop {
        match timeout(Duration::from_millis(500), rx.recv()).await {
            Ok(Some(RuntimeEvent::TaskTriggered { task, reason }))
                if reason == TriggerReason::FileWatch =>
            {
                tasks.insert(task);
            }
            Ok(Some(_)) => {
                // Ignore other event types (shouldn't happen from watcher).
            }
            Ok(None) | Err(_) => {
                // Channel closed or no more events within the timeout window.
                break;
            }
        }
    }

    tasks
}