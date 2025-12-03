// src/exec/mod.rs

//! Process execution layer.
//!
//! This module is responsible for actually running the commands defined in
//! the tasks, using `tokio::process::Command`, and reporting back to the
//! orchestration runtime via `RuntimeEvent`s.
//!
//! - [`command`] owns the "executor" loop which consumes `ScheduledTask`s and
//!   spawns processes.
//! - [`long_lived`] contains helpers for `progress_on_stdout`,
//!   `trigger_on_stdout`, and `progress_on_time` handling.

pub mod command;
pub mod long_lived;

pub use command::spawn_executor;
