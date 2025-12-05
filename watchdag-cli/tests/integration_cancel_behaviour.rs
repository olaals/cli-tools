mod common;
use crate::common::init_tracing;
use crate::common::builders::{ConfigFileBuilder, TaskConfigBuilder};

use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, Notify};
use tokio::time::timeout;

use watchdag::dag::Scheduler;
use watchdag::engine::{
    CoreRuntime, Runtime, RuntimeEvent, RuntimeOptions, TaskOutcome, TriggerReason,
    TriggerWhileRunningBehaviour,
};
use watchdag::exec::ExecutorBackend;

type TestResult = Result<(), Box<dyn Error>>;

/// A fake executor that allows us to control when tasks complete.
struct ControllableExecutor {
    runtime_tx: mpsc::Sender<RuntimeEvent>,
    /// Tasks that have been started.
    started_tasks: Arc<Mutex<Vec<String>>>,
    /// Signal to allow a task to complete.
    /// Map of task name -> Notify
    completion_signals: Arc<Mutex<std::collections::HashMap<String, Arc<Notify>>>>,
}

impl ControllableExecutor {
    fn new(runtime_tx: mpsc::Sender<RuntimeEvent>) -> Self {
        Self {
            runtime_tx,
            started_tasks: Arc::new(Mutex::new(Vec::new())),
            completion_signals: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    #[allow(dead_code)]
    fn get_started_tasks(&self) -> Vec<String> {
        self.started_tasks.lock().unwrap().clone()
    }

    #[allow(dead_code)]
    fn allow_completion(&self, task: &str) {
        let map = self.completion_signals.lock().unwrap();
        if let Some(notify) = map.get(task) {
            notify.notify_one();
        }
    }
}

impl ExecutorBackend for ControllableExecutor {
    fn spawn_ready_tasks(
        &mut self,
        tasks: Vec<watchdag::dag::ScheduledTask>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = watchdag::errors::Result<()>> + Send + '_>> {
        let tx = self.runtime_tx.clone();
        let started = self.started_tasks.clone();
        let signals = self.completion_signals.clone();

        Box::pin(async move {
            for task in tasks {
                let tx = tx.clone();
                let started = started.clone();
                let signals = signals.clone();
                
                // Spawn a background task so we don't block the runtime loop
                tokio::spawn(async move {
                    {
                        let mut guard = started.lock().unwrap();
                        guard.push(task.name.clone());
                    }

                    // Create a notify for this task if it doesn't exist
                    let notify = {
                        let mut map = signals.lock().unwrap();
                        map.entry(task.name.clone())
                            .or_insert_with(|| Arc::new(Notify::new()))
                            .clone()
                    };

                    // Wait for signal to complete
                    notify.notified().await;

                    let _ = tx.send(RuntimeEvent::TaskCompleted {
                        task: task.name.clone(),
                        outcome: TaskOutcome::Success,
                    })
                    .await;
                });
            }
            Ok(())
        })
    }
}

#[tokio::test]
async fn test_cancel_behaviour_last_wins() -> TestResult {
    init_tracing();

    // Config: A -> B
    // We will trigger A manually to start Run 1.
    // While A is running, we will trigger B, then A again.
    // With Cancel behavior, the queue should be cleared on each trigger.
    // So:
    // 1. Trigger B -> Queue = [B]
    // 2. Trigger A -> Queue = [A] (B is dropped)
    // After Run 1 finishes, Run 2 should start with A.

    let cfg = ConfigFileBuilder::new()
        .with_task("A", TaskConfigBuilder::new("echo A").build())
        .with_task("B", TaskConfigBuilder::new("echo B").after("A").build())
        .build();

    let scheduler = Scheduler::from_config(&cfg);
    let behaviour = TriggerWhileRunningBehaviour::Cancel;
    let queue_length = 5; // Large enough, but Cancel ignores it effectively for > 1 batch
    let options = RuntimeOptions {
        exit_when_idle: false, // We'll manually stop it
    };

    let (rt_tx, rt_rx) = mpsc::channel::<RuntimeEvent>(16);
    let executor = ControllableExecutor::new(rt_tx.clone());
    let executor_started = executor.started_tasks.clone();
    let executor_signals = executor.completion_signals.clone();

    // Start runtime in background
    let core = CoreRuntime::new(scheduler, behaviour, queue_length, options);
    let runtime = Runtime::new(core, rt_rx, executor);
    let runtime_handle = tokio::spawn(runtime.run());

    // 1. Start Run 1 (Trigger A)
    rt_tx.send(RuntimeEvent::TaskTriggered {
        task: "A".to_string(),
        reason: TriggerReason::Manual,
    }).await?;

    // Wait for A to start
    async fn wait_for_start(started: &Arc<Mutex<Vec<String>>>, task: &str) {
        for _ in 0..100 {
            {
                let guard = started.lock().unwrap();
                if guard.contains(&task.to_string()) {
                    return;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("Task {} did not start", task);
    }

    wait_for_start(&executor_started, "A").await;
    
    // Clear started tasks to track next run
    {
        let mut guard = executor_started.lock().unwrap();
        guard.clear();
    }

    // 2. While A is running (we haven't signaled completion yet), trigger B
    rt_tx.send(RuntimeEvent::TaskTriggered {
        task: "B".to_string(),
        reason: TriggerReason::Manual,
    }).await?;

    // Give it a moment to process
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 3. Trigger A again
    rt_tx.send(RuntimeEvent::TaskTriggered {
        task: "A".to_string(),
        reason: TriggerReason::Manual,
    }).await?;

    // Give it a moment to process
    tokio::time::sleep(Duration::from_millis(50)).await;

    // 4. Allow A (Run 1) to complete
    {
        let map = executor_signals.lock().unwrap();
        if let Some(notify) = map.get("A") {
            notify.notify_one();
        }
    }

    // Wait for A (Run 1) to finish and B (Run 1) to start?
    // Wait, A -> B. So after A finishes, B should start in Run 1.
    wait_for_start(&executor_started, "B").await;

    // Allow B (Run 1) to complete
    {
        let map = executor_signals.lock().unwrap();
        if let Some(notify) = map.get("B") {
            notify.notify_one();
        }
    }

    // Run 1 is now complete.
    // The runtime should now pick up the queued triggers.
    // We triggered B, then A.
    // "Cancel" behavior means "resetting queued batches to this task only".
    // So when we triggered B, queue became [B].
    // When we triggered A, queue became [A].
    // So Run 2 should start with A.

    // Clear started tasks
    {
        let mut guard = executor_started.lock().unwrap();
        guard.clear();
    }

    // Wait for Run 2 to start (A)
    wait_for_start(&executor_started, "A").await;

    // Verify that B did NOT start (it was dropped)
    {
        let guard = executor_started.lock().unwrap();
        assert!(guard.contains(&"A".to_string()));
        assert!(!guard.contains(&"B".to_string()));
    }

    // Cleanup
    rt_tx.send(RuntimeEvent::ShutdownRequested).await?;
    let _ = timeout(Duration::from_secs(1), runtime_handle).await;

    Ok(())
}
