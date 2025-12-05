use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;
use watchdag::dag::ScheduledTask;
use watchdag::engine::{RuntimeEvent, TaskOutcome};
use watchdag::exec::ExecutorBackend;
use watchdag::errors::Result;

/// A fake executor that:
/// - records which tasks were "run"
/// - immediately reports TaskCompleted(Success) for each scheduled task.
pub struct FakeExecutor {
    runtime_tx: mpsc::Sender<RuntimeEvent>,
    executed: Arc<Mutex<Vec<String>>>,
}

impl FakeExecutor {
    pub fn new(
        runtime_tx: mpsc::Sender<RuntimeEvent>,
        executed: Arc<Mutex<Vec<String>>>,
    ) -> Self {
        Self { runtime_tx, executed }
    }
}

impl ExecutorBackend for FakeExecutor {
    fn spawn_ready_tasks(
        &mut self,
        tasks: Vec<ScheduledTask>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
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
                .await.map_err(anyhow::Error::from)?;
            }
            Ok(())
        })
    }
}
