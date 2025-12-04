// src/exec/backend.rs

//! Pluggable executor backend abstraction.
//!
//! The runtime talks to an `ExecutorBackend` instead of a raw mpsc sender.
//! This makes it easy to swap in a fake executor in tests while keeping the
//! production executor implementation in [`command`].
//!
//! - `RealExecutorBackend` is the default implementation used by `watchdag`.
//!   It wraps the existing `spawn_executor` loop and just forwards scheduled
//!   tasks over an mpsc channel.
//! - Tests can provide their own `ExecutorBackend` that, for example, records
//!   which tasks were scheduled and directly emits `TaskCompleted` events.

use std::future::Future;
use std::pin::Pin;

use tokio::sync::mpsc;

use crate::dag::ScheduledTask;
use crate::engine::RuntimeEvent;
use crate::errors::{Error, Result};

use super::command::spawn_executor;

/// Trait abstracting how scheduled tasks are executed.
///
/// Production code uses [`RealExecutorBackend`]; tests can provide their own
/// implementation that doesn't spawn real processes.
pub trait ExecutorBackend: Send {
    /// Dispatch the given tasks for execution.
    ///
    /// The implementation is free to:
    /// - spawn OS processes (production)
    /// - simulate completion and emit `RuntimeEvent`s (tests)
    fn spawn_ready_tasks(
        &mut self,
        tasks: Vec<ScheduledTask>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;
}

/// Real executor backend used in production.
///
/// Internally, this just wraps the existing executor loop in
/// [`spawn_executor`]. The runtime calls `spawn_ready_tasks`, which forwards
/// the tasks to the background executor via an mpsc channel.
pub struct RealExecutorBackend {
    tx: mpsc::Sender<ScheduledTask>,
}

impl RealExecutorBackend {
    /// Create a new real executor backend, wiring it to the given runtime
    /// event sender.
    ///
    /// This spawns the background executor loop immediately.
    pub fn new(runtime_tx: mpsc::Sender<RuntimeEvent>) -> Self {
        let tx = spawn_executor(runtime_tx);
        Self { tx }
    }
}

impl ExecutorBackend for RealExecutorBackend {
    fn spawn_ready_tasks(
        &mut self,
        tasks: Vec<ScheduledTask>,
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        // Clone the sender so the future doesn't borrow `self` across `await`.
        let tx = self.tx.clone();

        Box::pin(async move {
            for task in tasks {
                tx.send(task).await.map_err(Error::from)?;
            }
            Ok(())
        })
    }
}
