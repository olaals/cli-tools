// tests/examples_long_lived.rs
mod common;
use crate::common::init_tracing;

use std::error::Error;
use std::path::PathBuf;

use tokio::time::{timeout, Duration};

use watchdag::config::load_and_validate;
use watchdag::dag::{ScheduledTask, Scheduler};
use watchdag::engine::{RuntimeEvent, TaskOutcome, TriggerReason};
use watchdag::exec::long_lived::setup_long_lived_handlers;

type TestResult = Result<(), Box<dyn Error>>;

/// Sanity-check that examples/long-lived.toml is wired the way the README describes.
#[test]
fn long_lived_example_config_is_parsed_correctly() -> TestResult {
    init_tracing();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest_dir.join("examples/long-lived.toml"))?;

    // Global defaults
    assert_eq!(
        cfg.default.watch,
        vec!["texts/**.txt".to_string(), "scripts/**/*.sh".to_string()]
    );
    assert_eq!(
        cfg.default.exclude,
        vec!["texts/**/*.tmp.txt".to_string()]
    );
    assert_eq!(cfg.default.use_hash, None);

    assert_eq!(cfg.task.len(), 2);

    let a = cfg.task.get("A").expect("task A must exist");
    assert!(a.long_lived, "A should be marked long_lived");
    assert_eq!(a.rerun, Some(false));
    assert_eq!(a.progress_on_stdout.as_deref(), Some("^hello"));
    assert!(a.progress_on_time.is_none());
    assert!(a.trigger_on_stdout.is_none());
    assert!(a.after.is_empty(), "A should have no dependencies");

    let b = cfg.task.get("B").expect("task B must exist");
    assert!(b.long_lived, "B should be marked long_lived");
    assert_eq!(b.rerun, Some(true));
    assert_eq!(b.progress_on_time.as_deref(), Some("3s"));
    assert!(b.progress_on_stdout.is_none());
    assert!(b.trigger_on_stdout.is_none());
    assert_eq!(b.after, vec!["A".to_string()]);

    Ok(())
}

/// Sanity-check that examples/long-lived-trigger.toml is wired as described.
#[test]
fn long_lived_trigger_example_config_is_parsed_correctly() -> TestResult {
    init_tracing();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest_dir.join("examples/long-lived-trigger.toml"))?;

    // No [default] section here, so defaults should be empty.
    assert!(cfg.default.watch.is_empty());
    assert!(cfg.default.exclude.is_empty());
    assert_eq!(cfg.default.use_hash, None);

    assert_eq!(cfg.task.len(), 2);

    let a = cfg.task.get("A").expect("task A must exist");
    assert!(a.long_lived);
    assert_eq!(a.rerun, Some(false));
    assert_eq!(a.trigger_on_stdout.as_deref(), Some("hello"));
    assert!(a.progress_on_stdout.is_none());
    assert!(a.progress_on_time.is_none());
    assert!(a.after.is_empty(), "A should have no dependencies");

    let b = cfg.task.get("B").expect("task B must exist");
    assert!(b.long_lived);
    assert_eq!(b.rerun, Some(true));
    assert_eq!(b.progress_on_time.as_deref(), Some("3s"));
    assert!(b.trigger_on_stdout.is_none());
    assert!(b.progress_on_stdout.is_none());
    assert_eq!(b.after, vec!["A".to_string()]);

    Ok(())
}

/// A long-lived root task A that reports progress should unblock B and allow the
/// run to finish even if A never sends a completion event.
///
/// This is the "logical completion" semantics for long-lived tasks:
/// - Runtime emits `TaskProgressed` for A (from stdout/time).
/// - Scheduler treats A as DoneSuccess for this run.
/// - B can run and the run becomes idle after B completes.
#[test]
fn progress_from_long_lived_unblocks_dependents_and_finishes_run() -> TestResult {
    init_tracing();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cfg = load_and_validate(manifest_dir.join("examples/long-lived.toml"))?;
    let mut scheduler = Scheduler::from_config(&cfg);

    // First run: trigger A (root).
    scheduler.start_new_run();
    let ready = scheduler.handle_trigger("A");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].name, "A");

    // Simulate "logical" progress from long-lived A (stdout/time), WITHOUT a
    // TaskCompleted event. This should unblock B.
    let ready_after_progress = scheduler.handle_progress("A");
    assert_eq!(
        ready_after_progress.len(),
        1,
        "progress on A should make exactly one task ready"
    );
    assert_eq!(ready_after_progress[0].name, "B");

    // When B completes successfully, the run should finish.
    let final_ready = scheduler.handle_completion("B", TaskOutcome::Success);
    assert!(
        final_ready.is_empty(),
        "no further tasks expected after B in this example"
    );
    assert!(
        scheduler.is_idle(),
        "run should be idle after B succeeds even though A never completed"
    );

    // Second run: A has a successful "progress" recorded in history, so
    // triggering B should not force A to re-run; B should be immediately runnable.
    scheduler.start_new_run();
    let ready_second_run = scheduler.handle_trigger("B");
    assert_eq!(
        ready_second_run.len(),
        1,
        "triggering B in a later run should schedule only B"
    );
    assert_eq!(ready_second_run[0].name, "B");

    Ok(())
}

/// Verify that `progress_on_time` yields a `RuntimeEvent::TaskProgressed`
/// after the configured duration, without depending on any external process.
///
/// This exercises:
/// - duration parsing ("50ms")
/// - timer-based progress emission
#[tokio::test]
async fn progress_on_time_emits_taskprogressed_event() -> TestResult {
    init_tracing();

    use tokio::sync::mpsc;

    let (tx, mut rx) = mpsc::channel::<RuntimeEvent>(4);

    let task = ScheduledTask {
        name: "timer-task".to_string(),
        cmd: "echo timer".to_string(),
        long_lived: true,
        rerun: true,
        progress_on_stdout: None,
        trigger_on_stdout: None,
        progress_on_time: Some("50ms".to_string()),
        use_hash: false,
        run_id: 0,
    };

    // No stdout pipe needed for pure time-based progress.
    setup_long_lived_handlers(&task, None, tx);

    let event = timeout(Duration::from_millis(500), rx.recv())
        .await
        .expect("timer-based progress should fire within 500ms")
        .expect("channel closed unexpectedly");

    match event {
        RuntimeEvent::TaskProgressed { task } => {
            assert_eq!(task, "timer-task");
        }
        other => panic!("expected TaskProgressed, got {:?}", other),
    }

    Ok(())
}

/// On Unix, verify that a long-lived command which prints a matching line to
/// stdout causes a `TaskProgressed` event, even though the command itself
/// continues running.
///
/// This checks:
/// - stdout regex matching for `progress_on_stdout`
/// - progress emission is independent of process exit
#[cfg(unix)]
#[tokio::test]
async fn progress_on_stdout_emits_taskprogressed_without_needing_command_exit() -> TestResult {
    use std::process::Stdio;
    use tokio::process::Command;
    use tokio::sync::mpsc;

    init_tracing();

    let (tx, mut rx) = mpsc::channel::<RuntimeEvent>(4);

    let task = ScheduledTask {
        name: "stdout-progress".to_string(),
        cmd: "sh -c 'echo hello; sleep 60'".to_string(),
        long_lived: true,
        rerun: true,
        progress_on_stdout: Some("hello".to_string()),
        trigger_on_stdout: None,
        progress_on_time: None,
        use_hash: false,
        run_id: 0,
    };

    // Spawn a child that prints "hello" and then sleeps for a long time.
    // We never wait for it to exit; we only care about the first line.
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("echo hello; sleep 60")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;
    let stdout = child
        .stdout
        .take()
        .expect("child must have stdout piped");

    setup_long_lived_handlers(&task, Some(stdout), tx);

    // We should see a TaskProgressed event fairly quickly after "hello" is printed.
    let event = timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("did not receive TaskProgressed within timeout")
        .expect("channel closed unexpectedly");

    match event {
        RuntimeEvent::TaskProgressed { task } => {
            assert_eq!(task, "stdout-progress");
        }
        other => panic!("expected TaskProgressed, got {:?}", other),
    }

    // The process should still be running (sleeping) at this point.
    let status = child.try_wait()?;
    assert!(
        status.is_none(),
        "child process should still be alive (long-lived)"
    );

    // Dropping `child` will kill it due to kill_on_drop(true).
    Ok(())
}

/// On Unix, verify that `trigger_on_stdout` emits `TaskTriggered` with the
/// `StdoutTrigger` reason when stdout matches the configured regex.
///
/// Again, the underlying process keeps running and the test does not wait
/// for it to exit.
#[cfg(unix)]
#[tokio::test]
async fn trigger_on_stdout_emits_tasktriggered_with_stdout_reason() -> TestResult {
    use std::process::Stdio;
    use tokio::process::Command;
    use tokio::sync::mpsc;

    init_tracing();

    let (tx, mut rx) = mpsc::channel::<RuntimeEvent>(4);

    let task = ScheduledTask {
        name: "stdout-trigger".to_string(),
        cmd: "sh -c 'echo world; sleep 60'".to_string(),
        long_lived: true,
        rerun: true,
        progress_on_stdout: None,
        trigger_on_stdout: Some("world".to_string()),
        progress_on_time: None,
        use_hash: false,
        run_id: 0,
    };

    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg("echo world; sleep 60")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);

    let mut child = cmd.spawn()?;
    let stdout = child
        .stdout
        .take()
        .expect("child must have stdout piped");

    setup_long_lived_handlers(&task, Some(stdout), tx);

    let event = timeout(Duration::from_secs(3), rx.recv())
        .await
        .expect("did not receive TaskTriggered within timeout")
        .expect("channel closed unexpectedly");

    match event {
        RuntimeEvent::TaskTriggered { task, reason } => {
            assert_eq!(task, "stdout-trigger");
            assert_eq!(reason, TriggerReason::StdoutTrigger);
        }
        other => panic!("expected TaskTriggered, got {:?}", other),
    }

    // Process should still be sleeping.
    let status = child.try_wait()?;
    assert!(
        status.is_none(),
        "child process should still be alive (long-lived)"
    );

    Ok(())
}
