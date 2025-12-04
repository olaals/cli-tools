1. Split “pure core” vs “IO shell” for the runtime

Right now:

Scheduler is already pure and synchronous (nice).

Runtime is where channels, queue, scheduler and exec meet.

watch::watcher and exec::command are very IO-heavy.

Idea: define a “core runtime” that is just a deterministic state machine over domain events, and keep all Tokio/process/fs in a thin outer shell.

Core runtime

Conceptually:

Input: a RuntimeEvent plus the current core state.

Output: new core state + a list of “commands” to perform in the outside world, e.g.:

“schedule these tasks” (for executor)

“start new run with these triggers”

“send shutdown signal”

Completely synchronous, no channels, no Tokio.

Tests would then:

Construct an initial core runtime state (scheduler + queue + options).

Feed in a scripted sequence of RuntimeEvents (“file watch A”, “progress A”, “completion B …”).

Assert:

which tasks the core wants to run next,

when runs start/end,

queue contents, etc.

Your current Runtime becomes the “shell”: it just reads from events_rx, calls core.step(event) and fans out the resulting “commands” to exec_tx and friends.

This gives you:

Heavy integration tests that don’t touch Tokio/OS at all.

The ability to fuzz the core with random sequences and assert invariants (no unsatisfied deps, no double-run without rerun, etc).

2. “Manual stepping” API for DAG runs

You already call:

scheduler.start_new_run()

scheduler.handle_trigger(...)

scheduler.handle_progress(...)

scheduler.handle_completion(...)

Those are almost a manual stepping API already. To make it more test-friendly, you could:

Add read-only inspection methods:

“current run id”

“run_state of task X”

“deps_satisfied for task X”

“tasks participating in this run”

Optionally return a structured “step result” for each call:

e.g. a small struct with:

tasks newly scheduled

tasks newly failed

whether the run just finished

Then you can write tests at the scheduler level that simulate arbitrarily weird sequences (multiple triggers, overlapping runs, failures, etc.) and assert on internal state, not just the “ready” tasks.

This is also where a “test mode” for --task <NAME> semantics could be tried out independently of IO.

3. Make execution pluggable: real executor vs fake executor

Right now, the executor:

Actually spawns tokio::process::Command.

Actually reads ChildStdout.

Interacts with long_lived logic.

Introduce an abstraction like “Executor backend”:

Interface that takes ScheduledTask and returns:

eventual TaskCompleted events,

optional “logical progress” and “stdout trigger” events.

Then have two concrete backends:

Real executor (production)

What you have today: spawns processes, sets up stdout handlers, etc.

Fake executor (tests)

Fully in-memory.

You provide a script of expected behavior per task:

e.g. “when scheduled, emit one TaskProgressed then TaskCompleted(Success)”.

or “for task A, fail with exit code 1”.

You can also script “stdout lines” to drive the long-lived semantics without real OS commands.

With this in place, you can test end-to-end:

config → scheduler → runtime → executor → runtime → scheduler

without touching processes, sleep, or stdout.

And you can write property/invariant tests like “regardless of how exec behaves, dependents never start before deps are logically DoneSuccess”.

4. Capture and query stdout deterministically

setup_long_lived_handlers currently:

Reads from ChildStdout,

Dumps to println!,

Emits TaskProgressed / TaskTriggered.

For testability, conceptual changes:

Treat “where to send the line” as a pluggable sink:

in prod: real stdout,

in tests: an in-memory buffer or channel.

Treat “line observed” → “events to emit” as a pure function:

input: task metadata + line text,

output: zero or more RuntimeEvents (progress / trigger).

Then you can:

Unit test the pure “line → events” function with a lot of synthetic lines and regexes, including tricky patterns, without IO.

Integration test the async loop by feeding it lines via a fake reader and consuming events from a channel.

Expose a TestStdoutLog in tests that you can assert on: “for this scenario, A printed these 3 lines before B was scheduled”.

This also makes it easy to add test helpers like “wait until line matching X appears” without relying on real process output.

5. Deterministic time: pluggable clock / timer service

Places that care about time:

progress_on_time in long_lived.

File watcher startup / small sleeps in tests.

Anything else that calls sleep.

Instead of directly using tokio::time::sleep, route through a “timer service” / “clock” abstraction:

Has methods like “sleep(duration)” or “schedule callback after duration”.

In production: implemented with tokio::time.

In tests: implemented with a manual “virtual clock” that you can advance by N ms in one step without actually waiting.

That gives you:

progress_on_time tests that are effectively instant:

“advance clock to 50ms; expect TaskProgressed”.

Deterministic behavior under lots of timers:

multiple long-lived tasks,

overlapping progress windows, etc.

Tokio’s own test utilities (time::pause, advance) are one option; wrapping them behind your own clock trait keeps them from leaking everywhere.

6. Make the watcher backend injectable

You’ve already split:

pattern logic (patterns.rs),

watcher implementation (watcher.rs).

To test more behavior without touching the real filesystem or notify, inject a watcher backend:

A generic interface that produces “file changed (path, event kind)” events.

Production implementation uses notify.

Test implementation is just an in-memory stream where the test can push synthetic events: “change src/main.rs”, “change texts/input.txt”, etc.

Then you can write tests at the “runtime + watcher + scheduler” level, but:

no tempdirs,

no real FS,

no sleep to wait for notify.

For hashing:

also make fs traversal & file content reading injectable:

in prod: real fs,

in tests: an in-memory “virtual filesystem” mapping path → content.

This lets you test tricky patterns like **, excludes, and use_hash behavior in a single, fast in-memory test.

7. Deterministic integration tests: a test harness

Put the pieces above together into a reusable “test harness” concept:

Constructs:

a core runtime (scheduler + queue) from a ConfigFile or examples.

a fake watcher.

a fake executor (with scripted outcomes, stdout lines, timings).

an in-memory stdout sink and event log.

a deterministic clock.

Exposes simple methods:

“trigger file change at path X” (simulated watcher).

“advance time by N ms”.

“run one logical event step” or “drain until idle”.

“inspect all RuntimeEvents that have happened”.

“inspect scheduler internal state”.
