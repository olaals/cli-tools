// src/exec/task_runner.rs

//! Individual task process runner.

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, oneshot};
use std::process::Stdio;
use tracing::{debug, error, info, warn};

use crate::dag::ScheduledTask;
use crate::engine::{RuntimeEvent, TaskOutcome};

/// Run a single task process, handling stdout/stderr and emitting
/// `TaskCompleted` events on success/failure.
///
/// - If the cancel channel fires (used for `rerun = true`), the child process
///   is killed and **no** `TaskCompleted` event is sent for that instance.
///   This avoids confusing the scheduler with completions from previous runs.
pub async fn run_task(
    task: ScheduledTask,
    runtime_tx: mpsc::Sender<RuntimeEvent>,
    cancel_rx: oneshot::Receiver<()>,
) {
    let task_name = task.name.clone();
    let run_id = task.run_id;
    if let Err(err) = run_task_inner(task, &runtime_tx, cancel_rx).await {
        error!(
            task = %task_name,
            run_id,
            error = %err,
            "task execution error"
        );
        let _ = runtime_tx
            .send(RuntimeEvent::TaskCompleted {
                task: task_name,
                outcome: TaskOutcome::Failed(-1),
            })
            .await;
    }
}

async fn run_task_inner(
    task: ScheduledTask,
    runtime_tx: &mpsc::Sender<RuntimeEvent>,
    mut cancel_rx: oneshot::Receiver<()>,
) -> Result<()> {
    info!(
        task = %task.name,
        run_id = task.run_id,
        cmd = %task.cmd,
        "starting task process"
    );

    // Build a shell command appropriate for the platform.
    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(&task.cmd);
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c").arg(&task.cmd);
        c
    };

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = cmd
        .spawn()
        .with_context(|| format!("spawning process for task '{}'", task.name))?;

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    // Attach long-lived / progress / trigger handlers to stdout.
    crate::exec::long_lived::setup_long_lived_handlers(&task, stdout, runtime_tx.clone());

    // Always consume stderr so buffers don't fill; log at debug.
    if let Some(stderr) = stderr {
        let task_name = task.name.clone();
        let run_id = task.run_id;
        tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                debug!(task = %task_name, run_id, "stderr: {}", line);
            }
        });
    }

    // Either the process exits on its own (normal case), or we receive a
    // cancellation request (for rerun=true when a new run is scheduled).
    tokio::select! {
        status_res = child.wait() => {
            let status = status_res.with_context(|| {
                format!("waiting for process of task '{}'", task.name)
            })?;

            let code = status.code().unwrap_or(-1);
            let outcome = if status.success() {
                TaskOutcome::Success
            } else {
                TaskOutcome::Failed(code)
            };

            info!(
                task = %task.name,
                run_id = task.run_id,
                exit_code = code,
                success = status.success(),
                "task process exited"
            );

            runtime_tx
                .send(RuntimeEvent::TaskCompleted {
                    task: task.name.clone(),
                    outcome,
                })
                .await
                .with_context(|| {
                    format!(
                        "sending TaskCompleted event for task '{}' to runtime",
                        task.name
                    )
                })?;
        }

        cancel = &mut cancel_rx => {
            match cancel {
                Ok(()) => {
                    info!(
                        task = %task.name,
                        run_id = task.run_id,
                        "cancellation requested for running task instance; killing process"
                    );
                    if let Err(e) = child.kill().await {
                        warn!(
                            task = %task.name,
                            run_id = task.run_id,
                            error = %e,
                            "failed to kill child process on cancellation"
                        );
                    }
                    // Do NOT send TaskCompleted for this cancelled instance.
                }
                Err(e) => {
                    debug!(
                        task = %task.name,
                        run_id = task.run_id,
                        error = %e,
                        "cancel channel closed without explicit cancellation"
                    );
                    // Child will be killed on drop due to kill_on_drop(true).
                }
            }
        }
    }

    Ok(())
}
