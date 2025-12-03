// src/main.rs

use watchdag::{cli, logging, run};

#[tokio::main]
async fn main() {
    if let Err(err) = run_main().await {
        eprintln!("watchdag error: {err:?}");
        std::process::exit(1);
    }
}

async fn run_main() -> anyhow::Result<()> {
    let args = cli::parse();
    logging::init_logging(args.log_level)?;
    run(args).await
}
