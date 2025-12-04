// src/cli.rs

//! CLI argument parsing using `clap`.
//!
//! NOTE: this expects `clap` to be built with the `derive` feature, e.g.:
//! `clap = { version = "4.5.53", features = ["derive"] }` in `Cargo.toml`.

use clap::{Parser, ValueEnum};

/// Command-line arguments for `watchdag`.
#[derive(Debug, Clone, Parser)]
#[command(
    name = "watchdag",
    version,
    about = "Run commands based on file changes and DAG dependencies.",
    long_about = None
)]
pub struct CliArgs {
    /// Path to the config file (TOML).
    ///
    /// Default: `Watchdag.toml` in the current working directory.
    #[arg(long, value_name = "PATH", default_value = "Watchdag.toml")]
    pub config: String,

    /// Run DAG once based on current state, no watching.
    #[arg(long)]
    pub once: bool,

    /// Run only a subgraph rooted at this task.
    ///
    /// NOTE: The precise semantics of this flag (e.g. which dependencies are
    /// considered satisfied) are a TODO for the scheduler/runtime. For now it
    /// is parsed and passed through but not fully enforced.
    #[arg(long, value_name = "NAME")]
    pub task: Option<String>,

    /// Logging level (error, warn, info, debug, trace).
    ///
    /// If omitted, `WATCHDAG_LOG` or a default level will be used.
    #[arg(long, value_enum, value_name = "LEVEL")]
    pub log_level: Option<LogLevel>,

    /// Parse + validate, print DAG, but don’t execute any commands.
    #[arg(long)]
    pub dry_run: bool,
}

/// Log level as exposed on the CLI.
#[derive(Debug, Copy, Clone, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Convenience wrapper around `CliArgs::parse()`.
pub fn parse() -> CliArgs {
    CliArgs::parse()
}
