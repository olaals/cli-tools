use std::error::Error;
use std::path::PathBuf;
use std::str::FromStr;

use watchdag::config::load_and_validate;
use watchdag::engine::{TriggerQueue, TriggerWhileRunningBehaviour};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn configs_behaviour_toml_drives_queue_config() -> TestResult {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest.join("examples/configs-behaviour.toml"))?;

    assert_eq!(cfg.config.triggered_while_running_behaviour, "queue");
    assert_eq!(cfg.config.queue_length, 1);

    let behaviour = TriggerWhileRunningBehaviour::from_str(
        &cfg.config.triggered_while_running_behaviour,
    )?;

    let q = TriggerQueue::new(behaviour, cfg.config.queue_length);
    assert!(matches!(q.behaviour(), TriggerWhileRunningBehaviour::Queue));
    assert!(q.is_empty());

    Ok(())
}

#[test]
fn queue_mode_merges_triggers_into_single_batch() -> TestResult {
    let mut q = TriggerQueue::new(TriggerWhileRunningBehaviour::Queue, 2);

    q.record_trigger("A");
    q.record_trigger("B");
    q.record_trigger("A");

    let mut items = q.drain_pending();
    items.sort();
    assert_eq!(items, vec!["A".to_string(), "B".to_string()]);
    assert!(q.is_empty());

    Ok(())
}

#[test]
fn cancel_mode_keeps_only_latest_trigger() -> TestResult {
    let mut q = TriggerQueue::new(TriggerWhileRunningBehaviour::Cancel, 3);

    q.record_trigger("A");
    q.record_trigger("B");

    let tasks = q.drain_pending();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0], "B");

    Ok(())
}
