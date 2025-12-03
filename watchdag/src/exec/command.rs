// src/exec/command.rs

use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::dag::scheduler::ScheduledTask;
use crate::engine::{RuntimeEvent, TaskOutcome};
use crate::exec::long_lived;

/// Spawn the background executor loop.
///
/// The returned `mpsc::Sender<ScheduledTask>` is what the runtime uses as
/// `exec_tx` in `engine::Runtime`. Each scheduled task is executed in its own
/// Tokio task, so multiple tasks can run in parallel.
pub fn spawn_executor(
    runtime_tx: mpsc::Sender<RuntimeEvent>,
) -> mpsc::Sender<ScheduledTask> {
    let (tx, mut rx) = mpsc::channel::<ScheduledTask>(32);

    tokio::spawn(async move {
        info!("executor loop started");
        while let Some(task) = rx.recv().await {
            let runtime_tx = runtime_tx.clone();
            tokio::spawn(async move {
                run_task(task, runtime_tx).await;
            });
        }
        info!("executor loop finished (channel closed)");
    });

    tx
}

/// Run a single task process, handling stdout/stderr and emitting
/// `TaskCompleted` events on success/failure.
///
/// All errors are converted into a failed completion event with exit code -1;
/// they are also logged via `tracing::error!`.
async fn run_task(task: ScheduledTask, runtime_tx: mpsc::Sender<RuntimeEvent>) {
    let task_name = task.name.clone();
    if let Err(err) = run_task_inner(task, &runtime_tx).await {
        error!(task = %task_name, error = %err, "task execution error");
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
) -> Result<()> {
    info!(task = %task.name, cmd = %task.cmd, "starting task process");

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
    long_lived::setup_long_lived_handlers(&task, stdout, runtime_tx.clone());

    // Always consume stderr so buffers don't fill; log at debug.
    if let Some(stderr) = stderr {
        let task_name = task.name.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr);
            let mut lines = reader.lines();

            while let Ok(Some(line)) = lines.next_line().await {
                debug!(task = %task_name, "stderr: {}", line);
            }
        });
    }

    // Wait for the child to exit.
    let status = child
        .wait()
        .await
        .with_context(|| format!("waiting for process of task '{}'", task.name))?;

    let code = status.code().unwrap_or(-1);
    let outcome = if status.success() {
        TaskOutcome::Success
    } else {
        TaskOutcome::Failed(code)
    };

    info!(
        task = %task.name,
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

    Ok(())
}
