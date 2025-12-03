

CLI utility to run commands based on watching files.
Runs commands in parallel and in series defined by DAG in Watchdag.toml

Examples in examples (also used for tests)

Prefer a cross-platform file watching backend that is efficient


Failure of a task should fail dependents

If triggered again while dag is running, default behaviour should be to queue it, default max length queue is 1


If we have a DAG A -> B -> C 
and B´s watch list is triggered, then B -> C should run
If A´s and B´s is triggered then A -> B -> C should run (once)
Add this to tests


TODO: 

Create structured test cases for all of the examples
Modular design according to best practices rust for maintainable project
Logging is important
Create a makefile
Use clap cli package

2. CLI design (clap)

Document the CLI interface you want:

Binary name: watchdag

Flags / options (document in README):

--config <PATH> (default Watchdag.toml)

--once (run DAG once based on current state, no watching)

--task <NAME> (optional: run only a subgraph rooted at this task)

--log-level <LEVEL> (error, warn, info, debug, trace)

--dry-run (parse + validate, print DAG, but don’t execute)

--version, --help (from clap)




Logging plan:

Use a structured logging crate.

Levels:

info for high-level events (task started, completed, triggered).

debug for detailed scheduler decisions.

error for failures.

Document a WATCHDAG_LOG or --log-level configuration in the README.





Planned folder structure:

watchdag/
├── Cargo.toml
├── README.md
├── Makefile                 
├── examples/
│   ├── configs-behaviour.toml
│   ├── global-local-watch.toml
│   ├── global-watch.toml
│   ├── include-default.toml
│   ├── long-lived-trigger.toml
│   ├── long-lived.toml
│   ├── simple-parallel.toml
│   └── use-hash.toml
├── src/
│   ├── main.rs              # tiny, just parses CLI + calls into lib
│   ├── lib.rs               # exposes high-level `run` API and modules
│   ├── cli.rs               # clap definitions + args → ConfigPath/Options
│   ├── logging.rs           # tracing/tracing-subscriber setup
│   ├── errors.rs            # crate-wide error types
│   │
│   ├── config/
│   │   ├── mod.rs           # re-exports below
│   │   ├── model.rs         # structs for [default], [task.*], [config] etc.
│   │   ├── loader.rs        # load TOML from path, env defaults, --once, --task
│   │   └── validate.rs      # DAG sanity checks, long_lived rules, etc.
│   │
│   ├── dag/
│   │   ├── mod.rs
│   │   ├── graph.rs         # petgraph-based DAG representation
│   │   └── scheduler.rs     # dependency resolution, state machine per task
│   │                         # (Idle/Queued/Running/Progressed/Failed)
│   │
│   ├── watch/
│   │   ├── mod.rs
│   │   ├── patterns.rs      # globset compilation, default + append logic
│   │   ├── watcher.rs       # notify setup, fs events → internal events
│   │   └── hash.rs          # blake3 hashing, .watchdag/hashes IO, use_hash logic
│   │
│   ├── exec/
│   │   ├── mod.rs
│   │   ├── command.rs       # spawning commands with tokio::process::Command,
│   │   │                    # capturing stdout/stderr, exit status
│   │   └── long_lived.rs    # progress_on_stdout / progress_on_time,
│   │                        # rerun semantics, trigger_on_stdout
│   │
│   └── engine/
│       ├── mod.rs
│       ├── runtime.rs       # main orchestration loop:
│       │                    #  - receives watch events
│       │                    #  - pushes into queue
│       │                    #  - drives dag::scheduler + exec::*
│       └── queue.rs         # queue semantics, triggered_while_running_behaviour,
│                            # queue_length=1, coalescing triggers
│
└── tests/
    ├── examples_global_watch.rs      # uses examples/global-watch.toml
    ├── examples_long_lived.rs        # long-lived + trigger tests
    ├── examples_use_hash.rs          # hashing behaviour
    ├── examples_configs_behaviour.rs # queue vs cancel behaviour
    └── examples_chain_trigger.rs     # A->B->C trigger rules from README

