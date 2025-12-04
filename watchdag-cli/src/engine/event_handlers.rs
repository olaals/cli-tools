// src/engine/event_handlers.rs

//! Event handling logic for the core runtime.

use std::collections::HashSet;

use crate::dag::{Scheduler, ScheduledTask};
use crate::engine::queue::TriggerQueue;
use crate::engine::{RuntimeOptions, TaskName, TaskOutcome, TriggerReason};
use crate::dag::TaskRunState;

/// Command produced by the pure core, to be executed by the outer IO shell.
#[derive(Debug, Clone)]
pub enum CoreCommand {
    /// Send these tasks to the executor.
    DispatchTasks(Vec<ScheduledTask>),
    /// Request that the process exits (used for `--once` when idle).
    RequestExit,
}

/// Decision returned by the core after handling a single `RuntimeEvent`.
#[derive(Debug, Clone)]
pub struct CoreStep {
    /// Commands the IO shell should execute (send tasks, start new run, exit).
    pub commands: Vec<CoreCommand>,
    /// Whether the outer runtime loop should keep running.
    pub keep_running: bool,
}

/// Handle a task trigger event.
///
/// - If the scheduler is idle, we start a new run and seed it with
///   this trigger plus anything that was already queued.
/// - If a run is active:
///   - If `task` is already participating in this run, we honour the
///     queue/cancel semantics and record it for a *future* run.
///   - If `task` is *not* in the current run, we MERGE it into the
///     current run by calling `handle_trigger` immediately. This means
///     unrelated roots share the same run_id and can run in parallel.
pub fn handle_task_trigger(
    scheduler: &mut Scheduler,
    queue: &mut TriggerQueue,
    task: TaskName,
    _reason: TriggerReason,
) -> CoreStep {
    let mut commands = Vec::new();

    if scheduler.is_idle() {
        // We're starting a new DAG run. Combine this trigger with anything that
        // was already queued (e.g. from a previous run completion).
        let mut triggers: HashSet<TaskName> =
            queue.drain_pending().into_iter().collect();
        triggers.insert(task);

        let mut step =
            start_new_run_from_triggers(scheduler, triggers.into_iter().collect());
        commands.append(&mut step.commands);

        return CoreStep {
            commands,
            keep_running: true,
        };
    }

    // DAG currently running.
    match scheduler.run_state_of(&task) {
        None => {
            // Unknown task (shouldn't happen with validated config).
            // Ignore the trigger.
        }
        Some(TaskRunState::NotInRun) => {
            // New root for this run: merge its DAG component into the
            // active run immediately.
            let newly_ready = scheduler.handle_trigger(&task);
            if !newly_ready.is_empty() {
                commands.push(CoreCommand::DispatchTasks(newly_ready));
            }
        }
        Some(_already_in_run) => {
            // Task is already participating in the current run; keep
            // queue/cancel semantics for re-triggers.
            queue.record_trigger(&task);
        }
    }

    CoreStep {
        commands,
        keep_running: true,
    }
}

/// Handle a task progress event.
pub fn handle_task_progress(
    scheduler: &mut Scheduler,
    queue: &mut TriggerQueue,
    task: TaskName,
) -> CoreStep {
    let mut commands = Vec::new();

    let newly_ready = scheduler.handle_progress(&task);
    if !newly_ready.is_empty() {
        commands.push(CoreCommand::DispatchTasks(newly_ready));
    }

    // If the scheduler is idle after processing progress, we may need to
    // start a queued run.
    let mut queued_cmds = maybe_start_queued_run(scheduler, queue);
    commands.append(&mut queued_cmds);

    CoreStep {
        commands,
        keep_running: true,
    }
}

/// Handle a task completion event.
pub fn handle_task_completion(
    scheduler: &mut Scheduler,
    queue: &mut TriggerQueue,
    options: &RuntimeOptions,
    task: TaskName,
    outcome: TaskOutcome,
) -> CoreStep {
    let mut commands = Vec::new();

    let newly_ready = scheduler.handle_completion(&task, outcome);
    if !newly_ready.is_empty() {
        commands.push(CoreCommand::DispatchTasks(newly_ready));
    }

    let mut queued_cmds = maybe_start_queued_run(scheduler, queue);
    commands.append(&mut queued_cmds);

    // In `--once` mode, we can exit when the DAG is idle and there are no
    // pending triggers in the queue.
    let mut keep_running = true;
    if options.exit_when_idle
        && scheduler.is_idle()
        && queue.is_empty()
    {
        keep_running = false;
        commands.push(CoreCommand::RequestExit);
    }

    CoreStep {
        commands,
        keep_running,
    }
}

/// Convenience for seeding a new run from initial root triggers.
///
/// This mirrors the async runtime's logic, but is pure and returns
/// commands instead of performing IO.
pub fn start_new_run_from_triggers(
    scheduler: &mut Scheduler,
    triggers: Vec<TaskName>,
) -> CoreStep {
    let mut commands = Vec::new();

    if triggers.is_empty() {
        return CoreStep {
            commands,
            keep_running: true,
        };
    }

    scheduler.start_new_run();

    let mut all_ready = Vec::new();
    for task in triggers {
        let newly_ready = scheduler.handle_trigger(&task);
        all_ready.extend(newly_ready);
    }

    if !all_ready.is_empty() {
        commands.push(CoreCommand::DispatchTasks(all_ready));
    }

    CoreStep {
        commands,
        keep_running: true,
    }
}

/// If the scheduler is idle and there are queued triggers, start a new run.
fn maybe_start_queued_run(
    scheduler: &mut Scheduler,
    queue: &mut TriggerQueue,
) -> Vec<CoreCommand> {
    let mut commands = Vec::new();

    if !scheduler.is_idle() {
        return commands;
    }

    let triggers = queue.drain_pending();
    if triggers.is_empty() {
        return commands;
    }

    let step = start_new_run_from_triggers(scheduler, triggers);
    commands.extend(step.commands);

    commands
}
