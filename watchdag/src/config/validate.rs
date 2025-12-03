// src/config/validate.rs

use anyhow::{anyhow, Context, Result};
use petgraph::algo::toposort;
use petgraph::graphmap::DiGraphMap;

use crate::config::model::ConfigFile;
use crate::engine::TriggerWhileRunningBehaviour;

/// Run basic semantic validation against a loaded configuration.
///
/// This checks:
/// - there is at least one task
/// - `triggered_while_running_behaviour` is valid ("queue" or "cancel")
/// - `queue_length >= 1`
/// - all `after` dependencies refer to existing tasks
/// - the task graph has no cycles
///
/// It does **not**:
/// - fully validate regexes (`progress_on_stdout` / `trigger_on_stdout`)
/// - parse/validate duration strings (`progress_on_time`)
pub fn validate_config(cfg: &ConfigFile) -> Result<()> {
    ensure_has_tasks(cfg)?;
    validate_global_config(cfg)?;
    validate_task_dependencies(cfg)?;
    validate_dag(cfg)?;
    Ok(())
}

fn ensure_has_tasks(cfg: &ConfigFile) -> Result<()> {
    if cfg.task.is_empty() {
        return Err(anyhow!(
            "config must contain at least one [task.<name>] section"
        ));
    }
    Ok(())
}

fn validate_global_config(cfg: &ConfigFile) -> Result<()> {
    // Validate triggered_while_running_behaviour string.
    TriggerWhileRunningBehaviour::from_str(&cfg.config.triggered_while_running_behaviour)
        .map_err(|e| anyhow!(e))
        .context("invalid [config].triggered_while_running_behaviour")?;

    if cfg.config.queue_length == 0 {
        return Err(anyhow!(
            "[config].queue_length must be >= 1 (got 0)"
        ));
    }

    Ok(())
}

fn validate_task_dependencies(cfg: &ConfigFile) -> Result<()> {
    for (name, task) in cfg.task.iter() {
        for dep in task.after.iter() {
            if !cfg.task.contains_key(dep) {
                return Err(anyhow!(
                    "task '{}' has unknown dependency '{}' in `after`",
                    name,
                    dep
                ));
            }
            if dep == name {
                return Err(anyhow!(
                    "task '{}' cannot depend on itself in `after`",
                    name
                ));
            }
        }
    }
    Ok(())
}

fn validate_dag(cfg: &ConfigFile) -> Result<()> {
    // Build a simple petgraph graph from the tasks and their dependencies.
    //
    // Edge direction: dep -> task
    // For:
    //   [task.B]
    //   after = ["A"]
    // we add edge A -> B.
    let mut graph: DiGraphMap<&str, ()> = DiGraphMap::new();

    for name in cfg.task.keys() {
        graph.add_node(name.as_str());
    }

    for (name, task) in cfg.task.iter() {
        for dep in task.after.iter() {
            graph.add_edge(dep.as_str(), name.as_str(), ());
        }
    }

    // A topological sort will fail if there is a cycle.
    match toposort(&graph, None) {
        Ok(_order) => Ok(()),
        Err(cycle) => {
            let node = cycle.node_id();
            Err(anyhow!(
                "cycle detected in task DAG involving task '{}'",
                node
            ))
        }
    }
}

// Add a small helper so we can use `TriggerWhileRunningBehaviour::from_str`
// without importing `std::str::FromStr` at the call sites.
trait FromStrExt: Sized {
    fn from_str(s: &str) -> Result<Self, String>;
}

impl FromStrExt for TriggerWhileRunningBehaviour {
    fn from_str(s: &str) -> Result<Self, String> {
        std::str::FromStr::from_str(s)
    }
}
