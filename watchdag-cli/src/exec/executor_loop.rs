// src/exec/executor_loop.rs

//! Main executor loop that manages running task processes.

use std::collections::HashMap;

use tokio::sync::{mpsc, oneshot};
use tracing::{debug, info};

use crate::dag::ScheduledTask;
use crate::engine::RuntimeEvent;
use crate::exec::task_runner::run_task;

/// Internal handle for a currently-running task process.
///
/// - `cancel` is used by the executor to request that the process be stopped
///   (used when `rerun = true` and a new run is scheduled).
/// - `handle` is the Tokio task that is actually running the command.
struct ActiveTask {
    cancel: Option<oneshot::Sender<()>>,
    handle: tokio::task::JoinHandle<()>,
}

/// Spawn the background executor loop.
///
/// The returned `mpsc::Sender<ScheduledTask>` is what the runtime (or
/// `RealExecutorBackend`) uses as `exec_tx`. Each scheduled task is executed
/// in its own Tokio task, and **per task name there will never be more than
/// one process running at the same time**:
///
/// - If a task is already running and `rerun = true`, the previous process is
///   cancelled (killed) before a new one is started.
/// - If a task is already running and `rerun = false`, the new scheduling
///   request is ignored.
pub fn spawn_executor(runtime_tx: mpsc::Sender<RuntimeEvent>) -> mpsc::Sender<ScheduledTask> {
    let (tx, mut rx) = mpsc::channel::<ScheduledTask>(32);

    tokio::spawn(async move {
        info!("executor loop started");

        // At most one ActiveTask per task name.
        let mut active: HashMap<String, ActiveTask> = HashMap::new();

        while let Some(task) = rx.recv().await {
            handle_scheduled_task(task, &mut active, &runtime_tx).await;
        }

        info!("executor loop finished (channel closed)");
    });

    tx
}

/// Handle a newly scheduled task.
async fn handle_scheduled_task(
    task: ScheduledTask,
    active: &mut HashMap<String, ActiveTask>,
    runtime_tx: &mpsc::Sender<RuntimeEvent>,
) {
    let name = task.name.clone();

    // See if we already have a running process for this task.
    if let Some(existing) = active.get_mut(&name) {
        if !existing.handle.is_finished() {
            if task.rerun {
                cancel_existing_task(&task, existing).await;
            } else {
                debug!(
                    task = %name,
                    run_id = task.run_id,
                    "task already running and rerun=false; ignoring new scheduling request"
                );

                // If the task is long-lived (e.g. a service) and we are configured NOT to restart it,
                // then the fact that it is already running satisfies the dependency for the new run.
                // We synthesize a TaskProgressed event so the scheduler marks it as DoneSuccess.
                if task.long_lived {
                    debug!(
                        task = %name,
                        run_id = task.run_id,
                        "synthesizing progress event for long-lived task"
                    );
                    let _ = runtime_tx
                        .send(RuntimeEvent::TaskProgressed { task: name.clone() })
                        .await;
                } else {
                    tracing::warn!(
                        task = %name,
                        run_id = task.run_id,
                        "task running, rerun=false, but NOT long_lived; scheduler may hang waiting for completion"
                    );
                }

                return;
            }
        }
    }

    // Create a fresh cancel channel and spawn the new process.
    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    let rt_tx = runtime_tx.clone();
    let task_clone = task.clone();
    let spawn_name = name.clone();

    let handle = tokio::spawn(async move {
        run_task(task_clone, rt_tx, cancel_rx).await;
        debug!(task = %spawn_name, "task runner future finished");
    });

    active.insert(
        name,
        ActiveTask {
            cancel: Some(cancel_tx),
            handle,
        },
    );
}

/// Cancel an existing running task.
async fn cancel_existing_task(task: &ScheduledTask, existing: &mut ActiveTask) {
    info!(
        task = %task.name,
        run_id = task.run_id,
        "rerun requested; cancelling previous process instance"
    );

    if let Some(cancel) = existing.cancel.take() {
        if cancel.send(()).is_err() {
            debug!(
                task = %task.name,
                run_id = task.run_id,
                "previous process already finished while cancelling"
            );
        }
    } else {
        debug!(
            task = %task.name,
            run_id = task.run_id,
            "no cancel sender present; process may already have been cancelled"
        );
    }
}
