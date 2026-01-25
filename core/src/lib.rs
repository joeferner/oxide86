use anyhow::Result;

use crate::{cpu::Cpu, memory::Memory};
use crate::io_port::IoPort;
pub use crate::cpu::bios::{Bios, NullBios, DriveParams, disk_errors};
pub use crate::io_port::{IoDevice, NullIoDevice};
pub use crate::disk::{DiskController, DiskGeometry, DiskImage, SECTOR_SIZE};
pub use crate::video::{VideoController, NullVideoController, Video, TextCell, TextAttribute, CursorPosition, colors};

pub mod cpu;
pub mod memory;
pub mod io_port;
pub mod disk;
pub mod video;

pub struct Computer<B: Bios = NullBios, I: IoDevice = NullIoDevice, V: VideoController = NullVideoController> {
    cpu: Cpu,
    memory: Memory,
    bios: B,
    io_port: IoPort<I>,
    video: Video,
    video_controller: V,
}

impl<B: Bios, I: IoDevice, V: VideoController> Computer<B, I, V> {
    pub fn new(bios: B, io_device: I, video_controller: V) -> Self {
        let mut memory = Memory::new();
        memory.initialize_ivt();
        memory.initialize_bda();
        Self {
            cpu: Cpu::new(),
            memory,
            bios,
            io_port: IoPort::new(io_device),
            video: Video::new(),
            video_controller,
        }
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        self.memory.load_bios(bios_data)?;
        Ok(())
    }

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = Cpu::physical_address(segment, offset);
        self.memory.load_at(physical_addr, program_data)?;

        // Set CPU to start at this location
        self.cpu.cs = segment;
        self.cpu.ip = offset;

        // Initialize other segments to reasonable defaults
        self.cpu.ds = segment;
        self.cpu.es = segment;
        self.cpu.ss = segment;
        self.cpu.sp = 0xFFFE; // Stack grows down from top of segment

        Ok(())
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    pub fn run(&mut self) {
        while !self.cpu.is_halted() {
            self.step();
            self.update_video();
        }
    }

    /// Execute a single instruction
    pub fn step(&mut self) {
        // Get current IP to check what opcode we're about to execute
        let current_ip = self.cpu.ip;
        let current_cs = self.cpu.cs;
        let addr = Cpu::physical_address(current_cs, current_ip);
        let opcode = self.memory.read_byte(addr);

        // Check if it's an INT instruction
        match opcode {
            0xCD => {
                // INT with immediate - need to fetch the interrupt number
                let int_num = self.memory.read_byte(addr + 1);
                // Manually advance IP past the INT instruction
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                // Execute with BIOS I/O
                self.cpu.execute_int_with_io(int_num, &mut self.memory, &mut self.bios, &mut self.video);
            }
            0xCC => {
                // INT 3 - advance IP and execute INT 3
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                self.cpu.execute_int_with_io(3, &mut self.memory, &mut self.bios, &mut self.video);
            }
            _ => {
                // Normal instruction - use execute_with_io
                let opcode = self.cpu.fetch_byte(&self.memory);
                self.cpu.execute_with_io(opcode, &mut self.memory, &mut self.io_port);
            }
        }

        // Process any video memory writes that occurred during instruction execution
        for (offset, value) in self.memory.drain_video_writes() {
            self.video.write_byte(offset, value);
        }
    }

    pub fn dump_registers(&self) {
        self.cpu.dump_registers();
    }

    /// Update video display if needed (call periodically or after step)
    pub fn update_video(&mut self) {
        if self.video.is_dirty() {
            self.video_controller.update_display(self.video.get_buffer());
            self.video_controller.update_cursor(self.video.get_cursor());
            self.video.clear_dirty();
        }
    }

    /// Get video buffer for inspection
    pub fn get_video_buffer(&self) -> &[TextCell; crate::video::TEXT_MODE_COLS * crate::video::TEXT_MODE_ROWS] {
        self.video.get_buffer()
    }

    /// Check if CPU is halted
    pub fn is_halted(&self) -> bool {
        self.cpu.is_halted()
    }
}
