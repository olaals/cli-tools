// src/engine/runtime.rs

use std::fmt;

use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::dag::ScheduledTask;
use crate::errors::Result;
use crate::exec::ExecutorBackend;

use super::core::CoreRuntime;
use super::{CoreCommand, RuntimeEvent};

/// Drives the DAG scheduler in response to `RuntimeEvent`s,
/// and delegates actual command execution to an `ExecutorBackend`.
///
/// This is a pure IO shell around `CoreRuntime`, which contains all the
/// runtime semantics. This struct handles async IO: reading events from
/// channels and dispatching tasks to the executor.
pub struct Runtime<E: ExecutorBackend> {
    core: CoreRuntime,
    event_rx: mpsc::Receiver<RuntimeEvent>,
    executor: E,
}

impl<E: ExecutorBackend> fmt::Debug for Runtime<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Runtime")
            .field("core", &self.core)
            .finish_non_exhaustive()
    }
}

impl<E: ExecutorBackend> Runtime<E> {
    pub fn new(
        core: CoreRuntime,
        event_rx: mpsc::Receiver<RuntimeEvent>,
        executor: E,
    ) -> Self {
        Self {
            core,
            event_rx,
            executor,
        }
    }

    /// Main event loop.
    ///
    /// - Consumes `RuntimeEvent`s from `event_rx`.
    /// - Feeds them into the core runtime.
    /// - Executes commands returned by the core (spawn tasks, exit, etc).
    pub async fn run(mut self) -> Result<()> {
        info!("watchdag runtime started");

        loop {
            let event = match self.event_rx.recv().await {
                Some(e) => e,
                None => {
                    info!("runtime event channel closed; exiting");
                    break;
                }
            };

            debug!(?event, "runtime received event");

            // Feed the event into the pure core and get commands back.
            let step = self.core.step(event);

            // Execute the commands.
            for command in step.commands {
                self.execute_command(command).await?;
            }

            // If the core says to stop, break out of the loop.
            if !step.keep_running {
                info!("core requested exit; stopping runtime");
                break;
            }
        }

        info!("runtime exiting");
        Ok(())
    }

    /// Execute a single command from the core.
    async fn execute_command(&mut self, command: CoreCommand) -> Result<()> {
        match command {
            CoreCommand::DispatchTasks(tasks) => {
                self.spawn_ready(tasks).await?;
            }
            CoreCommand::RequestExit => {
                // The core wants to exit. We could set a flag here, but
                // the core already returns keep_running=false in this case,
                // so this command is somewhat redundant. We'll just log it.
                info!("core issued RequestExit command");
            }
        }
        Ok(())
    }

    async fn spawn_ready(&mut self, tasks: Vec<ScheduledTask>) -> Result<()> {
        if tasks.is_empty() {
            return Ok(());
        }

        let names: Vec<_> = tasks.iter().map(|t| t.name.as_str()).collect();
        let run_ids: Vec<_> = tasks.iter().map(|t| t.run_id).collect();
        debug!(?names, ?run_ids, "spawning ready tasks");

        self.executor.spawn_ready_tasks(tasks).await
    }
}
