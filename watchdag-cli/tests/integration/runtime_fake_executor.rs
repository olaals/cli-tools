// tests/runtime_fake_executor.rs

use watchdag_test_utils::init_tracing;
use watchdag_test_utils::builders::{ConfigFileBuilder, TaskConfigBuilder};
use watchdag_test_utils::fake_executor::FakeExecutor;

use std::error::Error;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

use watchdag::config::ConfigFile;
use watchdag::dag::Scheduler;
use watchdag::engine::{
    CoreRuntime, Runtime, RuntimeEvent, RuntimeOptions, TriggerReason,
    TriggerWhileRunningBehaviour,
};

type TestResult = Result<(), Box<dyn Error>>;

/// Very simple chain: A -> B
fn simple_chain_config() -> ConfigFile {
    ConfigFileBuilder::new()
        .with_task("A", TaskConfigBuilder::new("echo A").build())
        .with_task("B", TaskConfigBuilder::new("echo B").after("A").build())
        .build()
}

#[tokio::test]
async fn runtime_with_fake_executor_runs_simple_chain() -> TestResult {
    init_tracing();

    let cfg = simple_chain_config();
    let scheduler = Scheduler::from_config(&cfg);

    let behaviour = TriggerWhileRunningBehaviour::Queue;
    let queue_length = 1;
    let options = RuntimeOptions {
        exit_when_idle: true,
    };

    let (rt_tx, rt_rx) = mpsc::channel::<RuntimeEvent>(16);

    let executed = Arc::new(Mutex::new(Vec::new()));
    let executor = FakeExecutor::new(rt_tx.clone(), executed.clone());

    // Seed initial manual trigger for A before starting the runtime loop.
    rt_tx
        .send(RuntimeEvent::TaskTriggered {
            task: "A".to_string(),
            reason: TriggerReason::Manual,
        })
        .await?;

    let core = CoreRuntime::new(scheduler, behaviour, queue_length, options);
    let runtime = Runtime::new(core, rt_rx, executor);

    // Enforce an upper bound on how long this test may run.
    let run_result = timeout(Duration::from_secs(3), runtime.run()).await;

    match run_result {
        Ok(Ok(())) => {
            // Runtime finished normally within the timeout.
        }
        Ok(Err(e)) => {
            // Runtime returned an error.
            return Err(e.into());
        }
        Err(_) => {
            // Timeout elapsed: treat as test failure instead of hanging.
            panic!("runtime did not finish within 3 seconds");
        }
    }

    let tasks_run = executed.lock().unwrap().clone();
    assert_eq!(tasks_run, vec!["A".to_string(), "B".to_string()]);

    Ok(())
}
