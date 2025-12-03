use std::error::Error;
use std::path::PathBuf;

use watchdag::config::load_and_validate;
use watchdag::watch::{build_task_watch_profiles, RawTaskPatternSpec, WatchDefaults};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn task_first_appends_default_watch_and_exclude() -> TestResult {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("examples/include-default.toml");

    let cfg = load_and_validate(&path)?;
    let defaults = WatchDefaults {
        watch: cfg.default.watch.clone(),
        exclude: cfg.default.exclude.clone(),
    };

    let specs: Vec<RawTaskPatternSpec> = cfg
        .task
        .iter()
        .map(|(name, t)| RawTaskPatternSpec {
            name: name.clone(),
            watch: t.watch.clone(),
            exclude: t.exclude.clone(),
            append_default_watch: t.append_default_watch,
            append_default_exclude: t.append_default_exclude,
        })
        .collect();

    let profiles = build_task_watch_profiles(&defaults, &specs)?;

    let first = profiles.iter().find(|p| p.name() == "first").unwrap();
    let second = profiles.iter().find(|p| p.name() == "second").unwrap();

    assert!(first.matches("src/a/file.py"));
    assert!(first.matches("scripts/deploy.sh"));
    assert!(!first.matches("src/a/tests/test_mod.py"));
    assert!(!first.matches("src/foo.tmp.py"));

    assert!(second.matches("scripts/deploy.sh"));
    assert!(second.matches("src/a/tests/test_mod.py"));

    Ok(())
}
