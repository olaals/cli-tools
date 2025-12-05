// tests/run_on_own_files_only.rs

mod common;
use crate::common::init_tracing;
use crate::common::builders::{ConfigFileBuilder, TaskConfigBuilder};

use std::error::Error;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use watchdag::dag::Scheduler;
use watchdag::engine::{
    CoreRuntime, Runtime, RuntimeEvent, RuntimeOptions, TaskOutcome, TriggerReason,
    TriggerWhileRunningBehaviour,
};
use watchdag::exec::ExecutorBackend;

type TestResult = Result<(), Box<dyn Error>>;

struct FakeExecutor {
    runtime_tx: mpsc::Sender<RuntimeEvent>,
    executed: Arc<Mutex<Vec<String>>>,
}

impl FakeExecutor {
    fn new(
        runtime_tx: mpsc::Sender<RuntimeEvent>,
        executed: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self { runtime_tx, executed }
    }
}

impl ExecutorBackend for FakeExecutor {
    fn spawn_ready_tasks(
        &mut self,
        tasks: Vec<watchdag::dag::ScheduledTask>,
    ) -> Pin<
        Box<
            dyn Future<Output = watchdag::errors::Result<()>> + Send + '_,
        >,
    > {
        let tx = self.runtime_tx.clone();
        let executed = Arc::clone(&self.executed);

        Box::pin(async move {
            for t in tasks {
                {
                    let mut guard = executed.lock().unwrap();
                    guard.push(t.name.clone());
                }

                tx.send(RuntimeEvent::TaskCompleted {
                    task: t.name.clone(),
                    outcome: TaskOutcome::Success,
                })
                .await?;
            }
            Ok(())
        })
    }
}

#[tokio::test]
async fn test_run_on_own_files_only_skips_dependent() -> TestResult {
    crate::common::with_timeout(async {
        init_tracing();

    // A -> B
    // B has run_on_own_files_only = true
    let cfg = ConfigFileBuilder::new()
        .with_task("A", TaskConfigBuilder::new("echo A").build())
        .with_task(
            "B",
            TaskConfigBuilder::new("echo B")
                .after("A")
                .run_on_own_files_only(true)
                .build(),
        )
        .build();

    let scheduler = Scheduler::from_config(&cfg);
    let behaviour = TriggerWhileRunningBehaviour::Queue;
    let queue_length = 1;
    let options = RuntimeOptions {
        exit_when_idle: true,
    };

    let (rt_tx, rt_rx) = mpsc::channel::<RuntimeEvent>(16);
    let executed = Arc::new(Mutex::new(Vec::new()));
    let executor = FakeExecutor::new(rt_tx.clone(), executed.clone());

    // Trigger A only
    rt_tx
        .send(RuntimeEvent::TaskTriggered {
            task: "A".to_string(),
            reason: TriggerReason::Manual,
        })
        .await?;

    let core = CoreRuntime::new(scheduler, behaviour, queue_length, options);
    let runtime = Runtime::new(core, rt_rx, executor);
    runtime.run().await?;

    let executed_tasks = executed.lock().unwrap().clone();
    assert_eq!(executed_tasks, vec!["A"]); // B should be skipped

    Ok(())
    }).await
}

#[tokio::test]
async fn test_run_on_own_files_only_runs_if_triggered() -> TestResult {
    crate::common::with_timeout(async {
        init_tracing();

    // A, B (independent)
    // B has run_on_own_files_only = true
    let cfg = ConfigFileBuilder::new()
        .with_task("A", TaskConfigBuilder::new("echo A").build())
        .with_task(
            "B",
            TaskConfigBuilder::new("echo B")
                .run_on_own_files_only(true)
                .build(),
        )
        .build();

    let scheduler = Scheduler::from_config(&cfg);
    let behaviour = TriggerWhileRunningBehaviour::Queue;
    let queue_length = 1;
    let options = RuntimeOptions {
        exit_when_idle: true,
    };

    let (rt_tx, rt_rx) = mpsc::channel::<RuntimeEvent>(16);
    let executed = Arc::new(Mutex::new(Vec::new()));
    let executor = FakeExecutor::new(rt_tx.clone(), executed.clone());

    // Trigger B only
    rt_tx
        .send(RuntimeEvent::TaskTriggered {
            task: "B".to_string(),
            reason: TriggerReason::Manual,
        })
        .await?;

    let core = CoreRuntime::new(scheduler, behaviour, queue_length, options);
    let runtime = Runtime::new(core, rt_rx, executor);
    runtime.run().await?;

    let executed_tasks = executed.lock().unwrap().clone();
    assert_eq!(executed_tasks, vec!["B"]); // B runs because it was triggered

    Ok(())
    }).await
}

#[tokio::test]
async fn test_run_on_own_files_only_runs_if_both_triggered() -> TestResult {
    crate::common::with_timeout(async {
        init_tracing();

    // A -> B
    // B has run_on_own_files_only = true
    let cfg = ConfigFileBuilder::new()
        .with_task("A", TaskConfigBuilder::new("echo A").build())
        .with_task(
            "B",
            TaskConfigBuilder::new("echo B")
                .after("A")
                .run_on_own_files_only(true)
                .build(),
        )
        .build();

    let scheduler = Scheduler::from_config(&cfg);
    let behaviour = TriggerWhileRunningBehaviour::Queue;
    let queue_length = 1;
    let options = RuntimeOptions {
        exit_when_idle: true,
    };

    let (rt_tx, rt_rx) = mpsc::channel::<RuntimeEvent>(16);
    let executed = Arc::new(Mutex::new(Vec::new()));
    let executor = FakeExecutor::new(rt_tx.clone(), executed.clone());

    // Trigger A and B
    rt_tx.send(RuntimeEvent::TaskTriggered { task: "A".to_string(), reason: TriggerReason::Manual }).await?;
    rt_tx.send(RuntimeEvent::TaskTriggered { task: "B".to_string(), reason: TriggerReason::Manual }).await?;

    let core = CoreRuntime::new(scheduler, behaviour, queue_length, options);
    let runtime = Runtime::new(core, rt_rx, executor);
    runtime.run().await?;

    let executed_tasks = executed.lock().unwrap().clone();
    assert!(executed_tasks.contains(&"A".to_string()));
    assert!(executed_tasks.contains(&"B".to_string()));
    
    let pos_a = executed_tasks.iter().position(|x| x == "A").unwrap();
    let pos_b = executed_tasks.iter().position(|x| x == "B").unwrap();
    assert!(pos_a < pos_b);

    Ok(())
    }).await
}

#[tokio::test]
async fn test_default_behavior_runs_dependent_without_files() -> TestResult {
    crate::common::with_timeout(async {
        init_tracing();

    // A -> B
    // A watches "src/a.rs"
    // B watches nothing (default)
    // B has run_on_own_files_only = false (default)
    let cfg = ConfigFileBuilder::new()
        .with_task("A", TaskConfigBuilder::new("echo A").watch("src/a.rs").build())
        .with_task(
            "B",
            TaskConfigBuilder::new("echo B")
                .after("A")
                .build(),
        )
        .build();

    let scheduler = Scheduler::from_config(&cfg);
    let behaviour = TriggerWhileRunningBehaviour::Queue;
    let queue_length = 1;
    let options = RuntimeOptions {
        exit_when_idle: true,
    };

    let (rt_tx, rt_rx) = mpsc::channel::<RuntimeEvent>(16);
    let executed = Arc::new(Mutex::new(Vec::new()));
    let executor = FakeExecutor::new(rt_tx.clone(), executed.clone());

    // Trigger A only
    rt_tx
        .send(RuntimeEvent::TaskTriggered {
            task: "A".to_string(),
            reason: TriggerReason::Manual,
        })
        .await?;

    let core = CoreRuntime::new(scheduler, behaviour, queue_length, options);
    let runtime = Runtime::new(core, rt_rx, executor);
    runtime.run().await?;

    let executed_tasks = executed.lock().unwrap().clone();
    assert_eq!(executed_tasks, vec!["A", "B"]); // B should run because it's a dependent and run_on_own_files_only is false

    Ok(())
    }).await
}
