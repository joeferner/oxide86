use std::{cell::RefCell, sync::Arc};

use anyhow::{Context, Result};
use oxide86_core::{
    Device,
    computer::Computer,
    cpu::Cpu,
    memory::Memory,
    memory_bus::MemoryBus,
    parse_hex_or_dec,
    video::{VideoBuffer, VideoCard},
};

use crate::cli::CommonCli;

pub mod cli;
pub mod logging;

pub fn create_computer(cli: &CommonCli, buffer: Arc<VideoBuffer>) -> Result<Computer> {
    let cpu = Cpu::new();
    let memory = Memory::new(2048 * 1024); // TODO fill from cli args
    let devices: Vec<RefCell<Box<dyn Device>>> =
        vec![RefCell::new(Box::new(VideoCard::new(buffer)))];
    let memory_bus = MemoryBus::new(memory, devices);
    let mut computer = Computer::new(cpu, memory_bus);

    if let Some(program_path) = &cli.program {
        // Load program from file
        let program_data = std::fs::read(program_path)
            .with_context(|| format!("Failed to read program file: {}", program_path))?;

        let segment = parse_hex_or_dec(&cli.segment)?;
        let offset = parse_hex_or_dec(&cli.offset)?;

        computer
            .load_program(&program_data, segment, offset)
            .context("Failed to load program")?;

        log::info!(
            "Loaded {} bytes at {:04X}:{:04X}",
            program_data.len(),
            segment,
            offset
        );
    } else {
        todo!("if not program what?");
    }

    Ok(computer)
}
