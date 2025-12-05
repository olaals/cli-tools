// src/errors.rs

//! Crate-wide error aliases and helpers.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WatchdagError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Cycle detected in DAG: {0}")]
    DagCycle(String),

    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub use anyhow::Error;
pub type Result<T> = std::result::Result<T, WatchdagError>;
