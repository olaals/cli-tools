// src/dag/graph.rs

use std::collections::HashMap;

use crate::config::model::ConfigFile;

/// Internal node structure: stores immediate deps and dependents.
#[derive(Debug, Clone)]
struct DagNode {
    /// Direct dependencies: tasks that must progress before this one can run.
    deps: Vec<String>,
    /// Direct dependents: tasks that depend on this one.
    dependents: Vec<String>,
}

/// Simple in-memory DAG representation keyed by task name.
///
/// This is intentionally lightweight; we already validate acyclicity in
/// `config::validate`, so here we just keep adjacency information for
/// scheduling and diagnostics.
#[derive(Debug, Clone)]
pub struct DagGraph {
    nodes: HashMap<String, DagNode>,
}

impl DagGraph {
    /// Build a DAG from a validated [`ConfigFile`].
    ///
    /// Assumes that:
    /// - all `after` references are valid
    /// - there are no cycles
    pub fn from_config(cfg: &ConfigFile) -> Self {
        let mut nodes: HashMap<String, DagNode> = HashMap::new();

        // First pass: create nodes with their dependency lists.
        for (name, task) in cfg.task.iter() {
            nodes.insert(
                name.clone(),
                DagNode {
                    deps: task.after.clone(),
                    dependents: Vec::new(),
                },
            );
        }

        // Second pass: populate dependents based on deps.
        let task_names: Vec<String> = nodes.keys().cloned().collect();
        for task_name in task_names {
            // clone to avoid borrowing issues while mutating
            let deps = nodes
                .get(&task_name)
                .map(|n| n.deps.clone())
                .unwrap_or_default();

            for dep in deps {
                if let Some(dep_node) = nodes.get_mut(&dep) {
                    dep_node.dependents.push(task_name.clone());
                }
            }
        }

        Self { nodes }
    }

    /// Return all task names.
    pub fn tasks(&self) -> impl Iterator<Item = &str> {
        self.nodes.keys().map(|s| s.as_str())
    }

    /// Immediate dependencies of a task (the tasks listed in its `after`).
    pub fn dependencies_of(&self, name: &str) -> &[String] {
        self.nodes
            .get(name)
            .map(|n| n.deps.as_slice())
            .unwrap_or(&[])
    }

    /// Immediate dependents of a task (tasks that list this one in their `after`).
    pub fn dependents_of(&self, name: &str) -> &[String] {
        self.nodes
            .get(name)
            .map(|n| n.dependents.as_slice())
            .unwrap_or(&[])
    }
}
