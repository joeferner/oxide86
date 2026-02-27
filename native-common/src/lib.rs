use std::{cell::RefCell, rc::Rc, sync::Arc};

use anyhow::{Context, Result, anyhow};
use oxide86_core::{
    DeviceRef,
    computer::Computer,
    cpu::{Cpu, CpuType},
    io_bus::IoBus,
    memory::Memory,
    memory_bus::MemoryBus,
    parse_hex_or_dec,
    video::{VideoBuffer, VideoCard},
};

use crate::cli::CommonCli;

pub mod cli;
pub mod logging;

pub fn create_computer(cli: &CommonCli, buffer: Arc<VideoBuffer>) -> Result<Computer> {
    let cpu_type = if let Some(cpu_type) = CpuType::parse(&cli.cpu_type) {
        cpu_type
    } else {
        return Err(anyhow!("Could not parse CPU type: {}", cli.cpu_type));
    };
    let cpu = Cpu::new(cpu_type);
    let memory = Memory::new(2048 * 1024); // TODO fill from cli args
    let devices: Vec<DeviceRef> = vec![Rc::new(RefCell::new(VideoCard::new(buffer)))];
    let memory_bus = MemoryBus::new(memory, devices.clone());
    let io_bus = IoBus::new(devices);
    let mut computer = Computer::new(cpu, memory_bus, io_bus);

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
