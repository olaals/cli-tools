use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use watchdag::engine::{RuntimeEvent, TriggerReason};
use watchdag::watch::cache::FileCache;
use watchdag::watch::event_handler::process_file_change;
use watchdag::watch::hash::{HashStore, MemoryHashStore};
use watchdag::watch::patterns::{RawTaskPatternSpec, build_task_watch_profiles, WatchDefaults};

fn make_hash_store() -> Arc<Mutex<Box<dyn HashStore>>> {
    Arc::new(Mutex::new(Box::new(MemoryHashStore::new())))
}

fn make_file_cache() -> Arc<Mutex<FileCache>> {
    Arc::new(Mutex::new(FileCache::new()))
}

#[tokio::test]
async fn test_dag_filtering_root_only() {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
    // DAG: A -> B -> C
    // Patterns: A and B match "src/common.rs".
    // Expected: Only A triggers (because A is an ancestor of B).

    let defaults = WatchDefaults::default();
    let specs = vec![
        RawTaskPatternSpec::new("A", Some(vec!["src/common.rs".into()]), None, false, false, false),
        RawTaskPatternSpec {
            name: "B".to_string(),
            watch: Some(vec!["src/common.rs".into()]),
            exclude: None,
            append_default_watch: false,
            append_default_exclude: false,
            use_hash: false,
            deps: vec!["A".to_string()], // B depends on A
        },
        RawTaskPatternSpec {
            name: "C".to_string(),
            watch: None,
            exclude: None,
            append_default_watch: false,
            append_default_exclude: false,
            use_hash: false,
            deps: vec!["B".to_string()], // C depends on B
        },
    ];

    let profiles = build_task_watch_profiles(&defaults, &specs).unwrap();
    let profiles = Arc::new(profiles);

    // Build dep_map manually as watcher does
    let mut dep_map = HashMap::new();
    dep_map.insert("A".to_string(), vec![]);
    dep_map.insert("B".to_string(), vec!["A".to_string()]);
    dep_map.insert("C".to_string(), vec!["B".to_string()]);
    let dep_map = Arc::new(dep_map);

    let (tx, mut rx) = mpsc::channel(10);
    let hash_store = make_hash_store();
    let file_cache = make_file_cache();
    let root = PathBuf::from("/tmp/test"); // Dummy root, won't be read because use_hash=false

    // Simulate change to "src/common.rs"
    let path = root.join("src/common.rs");
    process_file_change(&root, &path, &profiles, &dep_map, &tx, hash_store, file_cache).await;

    // Expect exactly one trigger for A
    let event = rx.recv().await.expect("should receive event");
    match event {
        RuntimeEvent::TaskTriggered { task, reason } => {
            assert_eq!(task, "A");
            assert_eq!(reason, TriggerReason::FileWatch);
        }
        _ => panic!("unexpected event: {:?}", event),
    }

    // Should be no more events
    assert!(tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await.is_err());
    }).await.expect("test timed out");
}

#[tokio::test]
async fn test_dag_filtering_common_files_subset() {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
    // DAG: A -> B -> C
    // Patterns: B and C match "src/lib.rs". A does not.
    // Expected: Only B triggers (because B is ancestor of C, and A is not involved).

    let defaults = WatchDefaults::default();
    let specs = vec![
        RawTaskPatternSpec::new("A", Some(vec!["src/main.rs".into()]), None, false, false, false),
        RawTaskPatternSpec {
            name: "B".to_string(),
            watch: Some(vec!["src/lib.rs".into()]),
            exclude: None,
            append_default_watch: false,
            append_default_exclude: false,
            use_hash: false,
            deps: vec!["A".to_string()],
        },
        RawTaskPatternSpec {
            name: "C".to_string(),
            watch: Some(vec!["src/lib.rs".into()]),
            exclude: None,
            append_default_watch: false,
            append_default_exclude: false,
            use_hash: false,
            deps: vec!["B".to_string()],
        },
    ];

    let profiles = build_task_watch_profiles(&defaults, &specs).unwrap();
    let profiles = Arc::new(profiles);

    let mut dep_map = HashMap::new();
    dep_map.insert("A".to_string(), vec![]);
    dep_map.insert("B".to_string(), vec!["A".to_string()]);
    dep_map.insert("C".to_string(), vec!["B".to_string()]);
    let dep_map = Arc::new(dep_map);

    let (tx, mut rx) = mpsc::channel(10);
    let hash_store = make_hash_store();
    let file_cache = make_file_cache();
    let root = PathBuf::from("/tmp/test");

    // Simulate change to "src/lib.rs"
    let path = root.join("src/lib.rs");
    process_file_change(&root, &path, &profiles, &dep_map, &tx, hash_store, file_cache).await;

    // Expect exactly one trigger for B
    let event = rx.recv().await.expect("should receive event");
    match event {
        RuntimeEvent::TaskTriggered { task, reason } => {
            assert_eq!(task, "B");
            assert_eq!(reason, TriggerReason::FileWatch);
        }
        _ => panic!("unexpected event: {:?}", event),
    }

    assert!(tokio::time::timeout(std::time::Duration::from_millis(50), rx.recv()).await.is_err());
    }).await.expect("test timed out");
}
