// src/exec/mod.rs

//! Process execution layer.
//!
//! This module is responsible for actually running the commands defined in
//! the tasks, using `tokio::process::Command`, and reporting back to the
//! orchestration runtime via `RuntimeEvent`s.
//!
//! - [`command`] re-exports the executor loop spawner.
//! - [`executor_loop`] owns the main executor loop which manages task processes.
//! - [`task_runner`] handles individual task process execution.
//! - [`long_lived`] contains helpers for `progress_on_stdout`,
//!   `trigger_on_stdout`, and `progress_on_time` handling.
//! - [`backend`] provides the `ExecutorBackend` trait and a concrete
//!   `RealExecutorBackend` that the runtime uses in production, and which
//!   tests can replace with a fake implementation.

pub mod backend;
pub mod command;
pub mod executor_loop;
pub mod long_lived;
pub mod task_runner;

pub use backend::{ExecutorBackend, RealExecutorBackend};
pub use command::spawn_executor;
