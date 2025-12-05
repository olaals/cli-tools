# Project Analysis and Recommendations

This report analyzes the `watchdag-cli` codebase and provides recommendations for improving maintainability, testability, and adherence to Rust best practices.

## Overview

`watchdag-cli` is a robustly structured CLI tool that leverages modern Rust async ecosystems (`tokio`) and observability (`tracing`). The architecture cleanly separates core logic (pure state machines) from the runtime (async I/O), which is a strong foundation for testability.

## Recommendations

### 1. Error Handling Strategy
**Score: 8/10**

Currently, the project uses `anyhow::Result` widely. While excellent for applications, using structured errors for library modules improves maintainability and API clarity.

*   **Recommendation**: Introduce `thiserror` for the library crates/modules (`src/lib.rs` and below). Keep `anyhow` for the top-level binary (`src/main.rs`) and tests.
*   **Benefit**: Callers can programmatically handle specific error cases (e.g., config validation errors vs. I/O errors) without string matching.
*   **Action**: Define a `WatchdagError` enum in `src/errors.rs` deriving `thiserror::Error`.

### 2. Dependency Injection for Filesystem Operations
**Score: 9/10**

The `Watcher` and `HashStore` implementations currently interact directly with the filesystem. This makes unit testing edge cases (like permission denied, missing files, or race conditions) difficult without touching the disk.

*   **Recommendation**: Abstract filesystem operations behind a trait (e.g., `FileSystem`).
*   **Benefit**: Allows injecting a mock filesystem for tests, enabling deterministic testing of file watching and hashing logic without flaky I/O.
*   **Action**: Create a `fs` module with a trait exposing `read`, `canonicalize`, etc., and implement it for `std::fs` and a mock.

### 3. Configuration Validation & Typing
**Score: 7/10**

The configuration loading (`src/config`) does a good job of separating the "raw" TOML model from the "validated" internal model. However, validation logic could be more centralized.

*   **Recommendation**: Use the "Parse, don't validate" pattern more strictly. Ensure that the `ConfigFile` struct used by the rest of the app cannot be constructed in an invalid state.
*   **Benefit**: Removes the need for defensive checks scattered throughout the codebase (e.g., checking if a dependency exists in the map).
*   **Action**: Make fields of the validated config private and expose them via getters, or use a "Builder" pattern that returns a `Result`.

### 4. Test Harness Improvements
**Score: 6/10**

The current integration tests are powerful but rely on a lot of boilerplate setup (`ConfigFileBuilder`, `FakeExecutor`).

*   **Recommendation**: Consolidate test helpers into a dedicated `test_utils` crate or module that is conditionally compiled.
*   **Benefit**: Reduces code duplication in `tests/` and makes writing new tests faster.
*   **Action**: Move `tests/common` into a `#[cfg(test)]` module in `src/lib.rs` or a separate crate in the workspace.

### 5. Observability & Metrics
**Score: 5/10**

`tracing` is used, which is great. However, for a long-running daemon like this, structured metrics would be valuable.

*   **Recommendation**: Add `tracing-subscriber` with a metrics layer or a separate metrics library.
*   **Benefit**: Users can monitor the health of their watchdag instance (e.g., number of triggers, task execution times).
*   **Action**: Evaluate `metrics` crate or `tracing` integration for exposing stats.

### 6. Module Visibility
**Score: 4/10**

Many modules are `pub mod`, exposing internal details to the crate's public API.

*   **Recommendation**: Restrict visibility using `pub(crate)` where possible. Only expose what is necessary for the binary `src/main.rs` or integration tests.
*   **Benefit**: Allows refactoring internals without breaking the "public" API (even if the API is just for the binary).
*   **Action**: Review `src/lib.rs` and change `pub mod` to `pub(crate) mod` or `mod` where appropriate.

### 7. Property-Based Testing
**Score: 7/10**

The DAG scheduler logic is complex and stateful. Unit tests cover specific scenarios, but edge cases might be missed.

*   **Recommendation**: Introduce property-based testing using `proptest`.
*   **Benefit**: Automatically generates thousands of random DAGs and event sequences to find crashes or logical inconsistencies (e.g., cycles, stuck tasks).
*   **Action**: Add `proptest` dev-dependency and write a test for the `Scheduler` state machine.

## Summary Table

| Recommendation | Score (0-10) | Effort | Impact |
| :--- | :---: | :---: | :--- |
| **DI for Filesystem** | **9** | High | High |
| **Structured Errors** | **8** | Medium | Medium |
| **Property-Based Testing** | **7** | Medium | High |
| **Config Validation** | **7** | Low | Medium |
| **Test Harness** | **6** | Low | Medium |
| **Observability** | **5** | Medium | Low |
| **Module Visibility** | **4** | Low | Low |

