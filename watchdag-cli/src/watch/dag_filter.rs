// src/watch/dag_filter.rs

//! DAG-aware filtering logic for watch events.

use std::collections::{HashMap, HashSet};

/// Return true if `task` has any ancestor whose name is in `matching_names`.
///
/// Ancestors are followed transitively via the `after = [...]` dependency
/// lists encoded in `dep_map`.
pub fn has_ancestor_in_matching(
    task: &str,
    matching_names: &HashSet<String>,
    dep_map: &HashMap<String, Vec<String>>,
) -> bool {
    // Start from direct deps of `task` and walk upwards.
    let mut stack: Vec<String> =
        dep_map.get(task).cloned().unwrap_or_default();
    let mut visited: HashSet<String> = HashSet::new();

    while let Some(current) = stack.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }

        if matching_names.contains(&current) {
            return true;
        }

        if let Some(parents) = dep_map.get(&current) {
            for p in parents {
                stack.push(p.clone());
            }
        }
    }

    false
}
