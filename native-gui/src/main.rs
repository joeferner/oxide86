mod font;
mod video_controller;

use anyhow::{Context, Result};
use std::fs::File;

fn main() -> Result<()> {
    let log_file = File::create("/tmp/emu86.log").context("Failed to create log file")?;
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Pipe(Box::new(log_file)))
        .init();

    // implement gui

    Ok(())
}
