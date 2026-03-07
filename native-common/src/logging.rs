use chrono::Timelike;
use std::fs::File;
use std::io::Write;

use anyhow::{Context, Result};

pub fn setup_logging() -> Result<()> {
    let log_file = File::create("oxide86.log").context("Failed to create log file")?;

    // Initialize logger from RUST_LOG env var, or use defaults if not set
    let mut builder = env_logger::Builder::from_default_env();

    // Only apply defaults if RUST_LOG is not set
    if std::env::var("RUST_LOG").is_err() {
        builder
            .filter_level(log::LevelFilter::Error)
            .filter_module("naga", log::LevelFilter::Info)
            .filter_module("wgpu_core", log::LevelFilter::Info)
            .filter_module("wgpu_hal", log::LevelFilter::Error)
            .filter_module("calloop", log::LevelFilter::Debug)
            .filter_module("oxide86_core", log::LevelFilter::Info)
            .filter_module("oxide86_native", log::LevelFilter::Info);
    }

    builder
        .format(|buf, record| {
            let now = chrono::Local::now();
            writeln!(
                buf,
                "[{:02}:{:02}:{:02}.{:03} {:5} {}] {}",
                now.hour(),
                now.minute(),
                now.second(),
                now.timestamp_subsec_millis(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    Ok(())
}
