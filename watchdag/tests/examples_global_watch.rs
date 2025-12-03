use std::error::Error;
use std::path::PathBuf;

use watchdag::config::load_and_validate;
use watchdag::watch::{build_task_watch_profiles, RawTaskPatternSpec, WatchDefaults};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn global_watch_and_exclude_apply_to_all_tasks() -> TestResult {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let config_path = manifest_dir.join("examples/global-watch.toml");

    let cfg = load_and_validate(&config_path)?;
    let defaults = WatchDefaults {
        watch: cfg.default.watch.clone(),
        exclude: cfg.default.exclude.clone(),
    };

    let specs: Vec<RawTaskPatternSpec> = cfg
        .task
        .iter()
        .map(|(name, task)| RawTaskPatternSpec {
            name: name.clone(),
            watch: task.watch.clone(),
            exclude: task.exclude.clone(),
            append_default_watch: task.append_default_watch,
            append_default_exclude: task.append_default_exclude,
        })
        .collect();

    let profiles = build_task_watch_profiles(&defaults, &specs)?;

    let src_py = "src/main.py";
    let matching = profiles.iter().filter(|p| p.matches(src_py)).count();
    assert_eq!(matching, cfg.task.len());

    let tmp_script = "scripts/foo.tmp.sh";
    assert!(!profiles.iter().any(|p| p.matches(tmp_script)));

    Ok(())
}
