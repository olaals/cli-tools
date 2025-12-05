pub mod builders;
pub mod fake_executor;

use std::sync::Once;
use tracing_subscriber::{fmt, EnvFilter};

static INIT: Once = Once::new();

/// Initialise tracing for tests.
///
/// - Uses `with_test_writer()`, so logs are captured per-test.
/// - The Rust test harness only prints captured output for **failing** tests
///   (unless you run with `-- --nocapture`).
///
/// Enable levels with e.g.:
/// `RUST_LOG=debug cargo test`
pub fn init_tracing() {
    INIT.call_once(|| {
        let filter =
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

        fmt()
            .with_env_filter(filter)
            .with_test_writer() // print only for failing tests unless --nocapture
            .with_target(true)
            .init();
    });
}

/// Run a future with a 5-second timeout.
#[allow(dead_code)]
pub async fn with_timeout<F, T>(f: F) -> T
where
    F: std::future::Future<Output = T>,
{
    tokio::time::timeout(std::time::Duration::from_secs(5), f)
        .await
        .expect("Test timed out after 5 seconds")
}
