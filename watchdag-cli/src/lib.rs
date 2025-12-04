// src/lib.rs

pub mod cli;
pub mod config;
pub mod dag;
pub mod engine;
pub mod errors;
pub mod exec;
pub mod logging;
pub mod watch;
pub mod types;

use std::path::{Path, PathBuf};

use anyhow::Result;
use tokio::sync::mpsc;
use tracing::{debug, info};

use crate::cli::CliArgs;
use crate::config::loader::load_and_validate;
use crate::config::model::ConfigFile;
use crate::dag::DagGraph;
use crate::dag::Scheduler;
use crate::engine::{
    CoreRuntime, Runtime, RuntimeEvent, RuntimeOptions, TriggerReason,
};
use crate::exec::RealExecutorBackend;

/// High-level entry point used by `main.rs`.
///
/// This wires together:
/// - config loading
/// - scheduler / queue / runtime
/// - executor
/// - (optional) file watcher
/// - Ctrl-C handling
pub async fn run(args: CliArgs) -> Result<()> {
    let config_path = PathBuf::from(&args.config);
    let cfg = load_and_validate(&config_path)?;

    if args.dry_run {
        print_dry_run(&cfg);
        return Ok(());
    }

    // DAG + scheduler.
    let scheduler = Scheduler::from_config(&cfg);

    // Queue behaviour from [config].
    let behaviour = cfg.config.triggered_while_running_behaviour;
    let queue_length = cfg.config.queue_length;

    // Runtime event channel.
    let (rt_tx, rt_rx) = mpsc::channel::<RuntimeEvent>(64);

    // Process executor backend (real implementation in production).
    let executor = RealExecutorBackend::new(rt_tx.clone());

    // Optional file watcher (disabled in --once mode).
    let _watcher_handle = if !args.once {
        let (_defaults, profiles) = crate::watch::build_profiles_from_config(&cfg)?;
        let root_dir = config_root_dir(&config_path);
        Some(crate::watch::spawn_watcher(
            root_dir,
            profiles,
            rt_tx.clone(),
            cfg.config.hash_storage_mode,
        )?)
    } else {
        None
    };

    // Ctrl-C → graceful shutdown.
    {
        let tx = rt_tx.clone();
        tokio::spawn(async move {
            if let Err(e) = tokio::signal::ctrl_c().await {
                eprintln!("failed to listen for Ctrl+C: {e}");
                return;
            }
            let _ = tx.send(RuntimeEvent::ShutdownRequested).await;
        });
    }

    // Seed initial triggers from DAG roots.
    let roots = root_tasks(&cfg);
    info!(?roots, "initial DAG roots to trigger at startup");

    for task in roots {
        rt_tx
            .send(RuntimeEvent::TaskTriggered {
                task,
                reason: TriggerReason::Manual,
            })
            .await?;
    }

    let options = RuntimeOptions {
        exit_when_idle: args.once,
    };

    // Construct the pure core runtime (single source of truth for semantics).
    let core = CoreRuntime::new(scheduler, behaviour, queue_length, options);

    // Construct the async IO shell around the core.
    let runtime = Runtime::new(core, rt_rx, executor);
    runtime.run().await
}

/// Figure out a sensible project root for watching.
///
/// - If the config path has a non-empty parent (e.g. "configs/Watchdag.toml"),
///   we use that directory.
/// - If it's just a bare filename like "Watchdag.toml" (parent = ""),
///   we fall back to the current working directory "."
fn config_root_dir(config_path: &Path) -> PathBuf {
    match config_path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    }
}

/// Compute DAG roots (tasks with no `after = [...]` dependencies).
fn root_tasks(cfg: &ConfigFile) -> Vec<String> {
    let graph = DagGraph::from_config(cfg);
    graph
        .tasks()
        .filter(|name| graph.dependencies_of(name).is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Simple dry-run output: print tasks, deps and commands.
fn print_dry_run(cfg: &ConfigFile) {
    println!("watchdag dry-run");
    println!(
        "  config.triggered_while_running_behaviour = {:?}",
        cfg.config.triggered_while_running_behaviour
    );
    println!("  config.queue_length = {}", cfg.config.queue_length);
    println!();

    println!("tasks ({}):", cfg.task.len());
    for (name, task) in cfg.task.iter() {
        println!("  - {name}");
        println!("      cmd: {}", task.cmd);
        if !task.after.is_empty() {
            println!("      after: {:?}", task.after);
        }
        if let Some(ref watch) = task.watch {
            if !watch.is_empty() {
                println!("      watch: {:?}", watch);
            }
        }
        if let Some(ref exclude) = task.exclude {
            if !exclude.is_empty() {
                println!("      exclude: {:?}", exclude);
            }
        }
        if let Some(use_hash) = task.use_hash {
            println!("      use_hash: {use_hash}");
        }
        if task.long_lived {
            println!("      long_lived: true");
        }
        if let Some(ref s) = task.progress_on_stdout {
            println!("      progress_on_stdout: {s}");
        }
        if let Some(ref s) = task.trigger_on_stdout {
            println!("      trigger_on_stdout: {s}");
        }
        if let Some(ref s) = task.progress_on_time {
            println!("      progress_on_time: {s}");
        }
    }

    debug!("dry-run complete (no execution)");
}
