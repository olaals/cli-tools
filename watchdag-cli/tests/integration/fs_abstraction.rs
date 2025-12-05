use std::collections::BTreeMap;
use std::path::PathBuf;

use watchdag::config::model::{ConfigFile, RawConfigFile, ConfigSection, DefaultSection, TaskConfig};
use watchdag::fs::mock::MockFileSystem;
use watchdag::watch::hash::compute_file_hash;
use watchdag::watch::patterns::{build_profiles_from_config, collect_matching_files};

#[test]
fn test_mock_fs_hashing() {
    let fs = MockFileSystem::new();
    fs.add_file("test.txt", b"hello world");

    let hash = compute_file_hash(&fs, &PathBuf::from("test.txt")).unwrap();
    // blake3 hash of "hello world"
    assert_eq!(hash, "d74981efa70a0c880b8d8c1985d075dbcbf679b99a5f9914e5aaf96b831a9e24");
}

#[test]
fn test_mock_fs_patterns() {
    let fs = MockFileSystem::new();
    fs.add_file("./src/main.rs", b"fn main() {}");
    fs.add_file("./src/lib.rs", b"pub fn lib() {}");
    fs.add_file("./README.md", b"# Readme");
    fs.add_file("./target/debug/watchdag", b"binary");

    let mut tasks = BTreeMap::new();
    tasks.insert(
        "build".to_string(),
        TaskConfig {
            cmd: "cargo build".to_string(),
            watch: Some(vec!["src/**/*.rs".to_string()]),
            exclude: None,
            append_default_watch: false,
            append_default_exclude: false,
            after: vec![],
            run_on_own_files_only: false,
            use_hash: None,
            long_lived: false,
            rerun: None,
            progress_on_stdout: None,
            trigger_on_stdout: None,
            progress_on_time: None,
        },
    );

    let raw_config = RawConfigFile {
        config: ConfigSection::default(),
        default: DefaultSection::default(),
        task: tasks,
    };

    let config = ConfigFile::try_from(raw_config).unwrap();

    let (_, profiles) = build_profiles_from_config(&config).unwrap();
    let profile = &profiles[0]; // "build" task

    let files = collect_matching_files(&fs, &PathBuf::from("."), profile).unwrap();
    
    // Should match src/main.rs and src/lib.rs
    assert_eq!(files.len(), 2);
    let mut file_names: Vec<String> = files
        .iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    file_names.sort();
    
    assert_eq!(file_names, vec!["./src/lib.rs", "./src/main.rs"]);
}
