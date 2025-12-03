// src/logging.rs

//! Logging setup for `watchdag` using `tracing` + `tracing-subscriber`.
//!
//! Priority for determining the log level:
//! 1. `--log-level` CLI flag (if provided)
//! 2. `WATCHDAG_LOG` environment variable (e.g. "info", "debug")
//! 3. default to `info`

use anyhow::Result;
use tracing_subscriber::fmt;

use crate::cli::LogLevel;

/// Initialise global logging subscriber.
///
/// Safe to call once at startup.
pub fn init_logging(cli_level: Option<LogLevel>) -> Result<()> {
    let level = match cli_level {
        Some(lvl) => level_from_log_level(lvl),
        None => std::env::var("WATCHDAG_LOG")
            .ok()
            .and_then(|s| parse_level_str(&s))
            .unwrap_or(tracing::Level::INFO),
    };

    // `init()` does not return a Result, so this cannot fail at runtime
    // (if called more than once, it will panic; we only call once in main).
    fmt()
        .with_max_level(level)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .init();

    Ok(())
}

fn level_from_log_level(lvl: LogLevel) -> tracing::Level {
    match lvl {
        LogLevel::Error => tracing::Level::ERROR,
        LogLevel::Warn => tracing::Level::WARN,
        LogLevel::Info => tracing::Level::INFO,
        LogLevel::Debug => tracing::Level::DEBUG,
        LogLevel::Trace => tracing::Level::TRACE,
    }
}

fn parse_level_str(s: &str) -> Option<tracing::Level> {
    match s.trim().to_lowercase().as_str() {
        "error" => Some(tracing::Level::ERROR),
        "warn" | "warning" => Some(tracing::Level::WARN),
        "info" => Some(tracing::Level::INFO),
        "debug" => Some(tracing::Level::DEBUG),
        "trace" => Some(tracing::Level::TRACE),
        _ => None,
    }
}
