// src/config/validate.rs

use petgraph::algo::toposort;
use petgraph::graphmap::DiGraphMap;

use crate::config::model::{ConfigFile, RawConfigFile};
use crate::errors::{Result, WatchdagError};

impl TryFrom<RawConfigFile> for ConfigFile {
    type Error = crate::errors::WatchdagError;

    fn try_from(raw: RawConfigFile) -> std::result::Result<Self, Self::Error> {
        validate_raw_config(&raw)?;
        Ok(ConfigFile::new_unchecked(raw.config, raw.default, raw.task))
    }
}

fn validate_raw_config(cfg: &RawConfigFile) -> Result<()> {
    ensure_has_tasks(cfg)?;
    validate_global_config(cfg)?;
    validate_task_dependencies(cfg)?;
    validate_dag(cfg)?;
    Ok(())
}

fn ensure_has_tasks(cfg: &RawConfigFile) -> Result<()> {
    if cfg.task.is_empty() {
        return Err(WatchdagError::ConfigError(
            "config must contain at least one [task.<name>] section".to_string(),
        ));
    }
    Ok(())
}

fn validate_global_config(cfg: &RawConfigFile) -> Result<()> {
    // triggered_while_running_behaviour is now strongly typed and validated
    // during deserialization, so we don't need to check it here.

    if cfg.config.queue_length == 0 {
        return Err(WatchdagError::ConfigError(
            "[config].queue_length must be >= 1 (got 0)".to_string(),
        ));
    }

    Ok(())
}

fn validate_task_dependencies(cfg: &RawConfigFile) -> Result<()> {
    for (name, task) in cfg.task.iter() {
        for dep in task.after.iter() {
            if !cfg.task.contains_key(dep) {
                return Err(WatchdagError::ConfigError(format!(
                    "task '{}' has unknown dependency '{}' in `after`",
                    name, dep
                )));
            }
            if dep == name {
                return Err(WatchdagError::ConfigError(format!(
                    "task '{}' cannot depend on itself in `after`",
                    name
                )));
            }
        }
    }
    Ok(())
}

fn validate_dag(cfg: &RawConfigFile) -> Result<()> {
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
            Err(WatchdagError::DagCycle(format!(
                "cycle detected in task DAG involving task '{}'",
                node
            )))
        }
    }
}
