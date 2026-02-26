use std::fs::File;

use anyhow::{Context, Result};

pub fn setup_logging() -> Result<()> {
    let log_file = File::create("oxide86.log").context("Failed to create log file")?;

    // Initialize logger from RUST_LOG env var, or use defaults if not set
    let mut builder = env_logger::Builder::from_default_env();

    // Only apply defaults if RUST_LOG is not set
    if std::env::var("RUST_LOG").is_err() {
        builder
            .filter_level(log::LevelFilter::Error)
            .filter_module("oxide86_core", log::LevelFilter::Info)
            .filter_module("oxide86_native", log::LevelFilter::Info);
    }

    builder
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    Ok(())
}
