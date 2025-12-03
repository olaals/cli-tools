// src/exec/long_lived.rs

use std::time::Duration;

use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStdout;
use tokio::sync::mpsc;
use tokio::time::sleep;
use tracing::{debug, warn};

use crate::dag::scheduler::ScheduledTask;
use crate::engine::{RuntimeEvent, TriggerReason};

/// Attach stdout / timer-based handlers for long-lived and progress-aware
/// tasks.
///
/// This is a fire-and-forget function: it spawns background Tokio tasks that
/// will emit `RuntimeEvent::TaskProgressed` and/or `RuntimeEvent::TaskTriggered`
/// as appropriate.
///
/// Notes:
/// - `progress_on_stdout` is used to mark the task as "logically done" in the
///   scheduler (even if the process keeps running).
/// - `trigger_on_stdout` is used to *trigger* further DAG runs (or the same
///   task again) based on log patterns.
/// - `progress_on_time` is used to mark logical completion after a fixed
///   duration, regardless of process state.
/// - These can be used for both `long_lived = true` and non-long-lived tasks.
pub fn setup_long_lived_handlers(
    task: &ScheduledTask,
    stdout: Option<ChildStdout>,
    runtime_tx: mpsc::Sender<RuntimeEvent>,
) {
    let has_stdout_logic =
        task.progress_on_stdout.is_some() || task.trigger_on_stdout.is_some();

    if has_stdout_logic {
        if let Some(stdout) = stdout {
            spawn_stdout_monitor(task, stdout, runtime_tx.clone());
        } else {
            warn!(
                task = %task.name,
                "progress_on_stdout/trigger_on_stdout configured but no stdout pipe available"
            );
        }
    } else if stdout.is_some() {
        // Even if we don't have any regex-based logic, consume stdout to avoid
        // filling OS buffers; log lines at debug.
        let stdout = stdout.unwrap();
        let task_name = task.name.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                debug!(task = %task_name, "stdout: {}", line);
            }
        });
    }

    if let Some(ref dur_str) = task.progress_on_time {
        match parse_duration(dur_str) {
            Ok(dur) => {
                let task_name = task.name.clone();
                tokio::spawn(async move {
                    sleep(dur).await;
                    debug!(
                        task = %task_name,
                        "progress_on_time elapsed; emitting TaskProgressed"
                    );
                    let _ = runtime_tx
                        .send(RuntimeEvent::TaskProgressed { task: task_name })
                        .await;
                });
            }
            Err(e) => {
                warn!(
                    task = %task.name,
                    duration = %dur_str,
                    error = %e,
                    "invalid progress_on_time duration; ignoring"
                );
            }
        }
    }
}

fn spawn_stdout_monitor(
    task: &ScheduledTask,
    stdout: ChildStdout,
    runtime_tx: mpsc::Sender<RuntimeEvent>,
) {
    let task_name = task.name.clone();

    let progress_regex = task
        .progress_on_stdout
        .as_ref()
        .and_then(|s| match Regex::new(s) {
            Ok(r) => Some(r),
            Err(e) => {
                warn!(
                    task = %task.name,
                    pattern = %s,
                    error = %e,
                    "invalid progress_on_stdout regex; ignoring"
                );
                None
            }
        });

    let trigger_regex = task
        .trigger_on_stdout
        .as_ref()
        .and_then(|s| match Regex::new(s) {
            Ok(r) => Some(r),
            Err(e) => {
                warn!(
                    task = %task.name,
                    pattern = %s,
                    error = %e,
                    "invalid trigger_on_stdout regex; ignoring"
                );
                None
            }
        });

    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            debug!(task = %task_name, "stdout: {}", line);

            if let Some(re) = &progress_regex {
                if re.is_match(&line) {
                    debug!(
                        task = %task_name,
                        "stdout matched progress_on_stdout; emitting TaskProgressed"
                    );
                    let _ = runtime_tx
                        .send(RuntimeEvent::TaskProgressed {
                            task: task_name.clone(),
                        })
                        .await;
                }
            }

            if let Some(re) = &trigger_regex {
                if re.is_match(&line) {
                    debug!(
                        task = %task_name,
                        "stdout matched trigger_on_stdout; emitting TaskTriggered"
                    );
                    let _ = runtime_tx
                        .send(RuntimeEvent::TaskTriggered {
                            task: task_name.clone(),
                            reason: TriggerReason::StdoutTrigger,
                        })
                        .await;
                }
            }
        }

        debug!(task = %task_name, "stdout monitor ended");
    });
}

/// Parse a simple duration string like `"3s"`, `"250ms"`, `"1m"`, `"2h"`.
///
/// This is intentionally minimal; it matches your examples (`"3s"`). You can
/// extend it later if you need more formats.
fn parse_duration(s: &str) -> Result<Duration, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("empty duration string".to_string());
    }

    // Find the boundary between digits and suffix.
    let idx = s
        .chars()
        .position(|c| !c.is_ascii_digit())
        .ok_or_else(|| "duration missing unit suffix".to_string())?;

    let (num_part, unit_part) = s.split_at(idx);
    let value: u64 = num_part
        .parse()
        .map_err(|e| format!("invalid duration number '{}': {}", num_part, e))?;
    let unit = unit_part.trim().to_lowercase();

    match unit.as_str() {
        "ms" => Ok(Duration::from_millis(value)),
        "s" => Ok(Duration::from_secs(value)),
        "m" => Ok(Duration::from_secs(value * 60)),
        "h" => Ok(Duration::from_secs(value * 60 * 60)),
        _ => Err(format!(
            "unsupported duration unit '{}'; expected ms, s, m, or h",
            unit
        )),
    }
}
