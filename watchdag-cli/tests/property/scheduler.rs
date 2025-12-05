use std::collections::HashSet;
use proptest::prelude::*;
use watchdag::config::ConfigFile;
use watchdag::dag::Scheduler;
use watchdag::engine::TaskOutcome;
use watchdag_test_utils::builders::{ConfigFileBuilder, TaskConfigBuilder};

// Strategy to generate a valid DAG configuration.
// We ensure acyclicity by only allowing task N to depend on tasks 0..N-1.
fn dag_config_strategy(max_tasks: usize) -> impl Strategy<Value = ConfigFile> {
    (1..=max_tasks).prop_flat_map(|num_tasks| {
        // Generate a list of dependency lists.
        // Since we can't easily make the strategy depend on the index 'i' inside a vec combinator,
        // we'll generate a list of lists of random indices, and then sanitize them.
        let deps_strat = proptest::collection::vec(
            proptest::collection::vec(any::<usize>(), 0..num_tasks), // Inner list of potential deps
            num_tasks // Outer list length = num_tasks
        );
        
        deps_strat.prop_map(move |raw_deps| {
            let mut builder = ConfigFileBuilder::new();
            for (i, potential_deps) in raw_deps.into_iter().enumerate() {
                let name = format!("task_{}", i);
                let mut task_builder = TaskConfigBuilder::new(&format!("echo {}", name));
                
                // Sanitize dependencies: only allow deps < i
                let mut valid_deps = HashSet::new();
                for dep_idx in potential_deps {
                    if i > 0 {
                        valid_deps.insert(dep_idx % i);
                    }
                }
                
                for dep_idx in valid_deps {
                    task_builder = task_builder.after(&format!("task_{}", dep_idx));
                }
                builder = builder.with_task(&name, task_builder.build());
            }
            builder.build()
        })
    })
}

proptest! {
    #[test]
    #[ignore]
    fn test_scheduler_eventual_termination(
        cfg in dag_config_strategy(10),
        triggers in proptest::collection::vec(0..10usize, 1..5), // Indices to trigger
        // A simple way to determine outcome: a set of "failing" tasks
        failing_tasks_indices in proptest::collection::vec(0..10usize, 0..5) 
    ) {
        let mut scheduler = Scheduler::from_config(&cfg);
        let task_names: Vec<String> = scheduler.task_names().map(|s| s.to_string()).collect();
        
        // Map indices back to names
        let triggers: Vec<String> = triggers.iter()
            .filter(|&&i| i < task_names.len())
            .map(|&i| task_names[i].clone())
            .collect();
            
        let failing_tasks: HashSet<String> = failing_tasks_indices.iter()
            .filter(|&&i| i < task_names.len())
            .map(|&i| task_names[i].clone())
            .collect();

        // Queue of tasks currently "executing"
        let mut executing: Vec<String> = Vec::new();

        // Initial triggers
        for t in &triggers {
            let scheduled = scheduler.handle_trigger(t);
            for st in scheduled {
                executing.push(st.name);
            }
        }

        // Simulation loop
        let mut steps = 0;
        let max_steps = 1000; // Prevent infinite loops in test logic

        while !scheduler.is_idle() && steps < max_steps {
            steps += 1;

            // If nothing is executing, we might be stuck or done.
            // But is_idle() checks if the run is finished.
            // If is_idle() is false, but executing is empty, we are stuck.
            if executing.is_empty() {
                // Verify that we are stuck due to unsatisfied dependencies.
                // Any task in the current run should be Pending.
                let tasks_in_run = scheduler.tasks_in_current_run();
                for t in tasks_in_run {
                    let state = scheduler.run_state_of(&t);
                    // It must be Pending (or Done, but if Done it shouldn't hold up the run? 
                    // Wait, maybe_finish_run checks if ALL are terminal. 
                    // If we are here, some are NOT terminal.
                    // So they must be Pending (since Running would be in `executing`).
                    
                    // If it's Pending, check if its dependencies are satisfied.
                    // If they are satisfied, it SHOULD have been scheduled.
                    // So if it's Pending here, its dependencies must NOT be satisfied.
                    
                    if let Some(watchdag::dag::TaskRunState::Pending) = state {
                        let satisfied = scheduler.deps_satisfied(&t).unwrap_or(false);
                        prop_assert!(!satisfied, "Task {} is Pending but deps are satisfied, yet it was not scheduled!", t);
                    }
                }
                // If we passed the checks, we are validly stuck.
                break;
            }

            // Pick a task to complete (FIFO for simplicity, or random?)
            // Let's just pop the first one.
            let task = executing.remove(0);
            
            let outcome = if failing_tasks.contains(&task) {
                TaskOutcome::Failed(1)
            } else {
                TaskOutcome::Success
            };

            let new_tasks = scheduler.handle_completion(&task, outcome);
            for st in new_tasks {
                executing.push(st.name);
            }
        }
        
        prop_assert!(steps < max_steps, "Simulation timed out - infinite loop?");
    }
}
