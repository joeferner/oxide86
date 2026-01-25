use anyhow::{Context, Result};
use clap::Parser;
use emu86_core::Computer;

#[derive(Parser)]
#[command(name = "emu86")]
#[command(about = "Intel 8086 CPU Emulator", long_about = None)]
struct Cli {
    /// Path to the BIOS file to load
    #[arg(long)]
    bios: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let mut computer = Computer::new();

    // Load a BIOS file (e.g., SeaBIOS, or a custom BIOS)
    let bios_data = std::fs::read(&cli.bios)
        .with_context(|| format!("Failed to read BIOS file: {}", cli.bios))?;
    computer.load_bios(&bios_data).context("Failed to load BIOS")?;
    computer.reset();

    // Now the CPU is at 0xFFFF0 and ready to execute
    computer.run();
    Ok(())
}
