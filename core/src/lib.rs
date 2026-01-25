use anyhow::Result;

use crate::{cpu::Cpu, memory::Memory};
use crate::io_port::IoPort;
pub use crate::cpu::bios::{Bios, NullBios, DriveParams, KeyPress, disk_errors};
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
    /// Cycle counter for timer emulation
    cycle_count: u64,
    /// Cycles per timer tick (PIT frequency / 18.2 Hz)
    /// 8086 at 4.77 MHz: approximately 262144 cycles per tick
    cycles_per_tick: u64,
}

impl<B: Bios, I: IoDevice, V: VideoController> Computer<B, I, V> {
    pub fn new(bios: B, io_device: I, video_controller: V) -> Self {
        let mut memory = Memory::new();
        memory.initialize_ivt();
        memory.initialize_bda();

        // Initialize BDA timer counter from host system time
        let initial_ticks = bios.get_system_ticks();
        memory.write_word(memory::BDA_START + memory::BDA_TIMER_COUNTER, (initial_ticks & 0xFFFF) as u16);
        memory.write_word(memory::BDA_START + memory::BDA_TIMER_COUNTER + 2, (initial_ticks >> 16) as u16);

        Self {
            cpu: Cpu::new(),
            memory,
            bios,
            io_port: IoPort::new(io_device),
            video: Video::new(),
            video_controller,
            cycle_count: 0,
            // 8086 at 4.77 MHz with PIT at 18.2 Hz: ~262144 cycles per tick
            // This is approximate: 4770000 / 18.2 ≈ 262088
            cycles_per_tick: 262088,
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

        // Increment cycle counter and update timer
        // Approximate: assume each instruction takes 10 cycles
        // Real 8086 instructions vary from 2 to 100+ cycles
        self.increment_cycles(10);
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

    /// Increment cycle counter and update system timer if needed
    /// This simulates the PIT (Programmable Interval Timer) running at 18.2 Hz
    fn increment_cycles(&mut self, cycles: u64) {
        self.cycle_count += cycles;

        // Check if we've accumulated enough cycles for a timer tick
        if self.cycle_count >= self.cycles_per_tick {
            self.cycle_count -= self.cycles_per_tick;

            // Read current timer counter from BDA
            let counter_addr = memory::BDA_START + memory::BDA_TIMER_COUNTER;
            let low_word = self.memory.read_word(counter_addr);
            let high_word = self.memory.read_word(counter_addr + 2);
            let mut tick_count = ((high_word as u32) << 16) | (low_word as u32);

            // Increment tick count
            tick_count = tick_count.wrapping_add(1);

            // Check for midnight rollover (0x001800B0 ticks = 24 hours)
            if tick_count >= 0x001800B0 {
                tick_count = 0;
                // Set midnight overflow flag
                let overflow_addr = memory::BDA_START + memory::BDA_TIMER_OVERFLOW;
                self.memory.write_byte(overflow_addr, 1);
            }

            // Write updated tick count back to BDA
            self.memory.write_word(counter_addr, (tick_count & 0xFFFF) as u16);
            self.memory.write_word(counter_addr + 2, (tick_count >> 16) as u16);
        }
    }
}
