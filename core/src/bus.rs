use crate::memory::Memory;
use crate::video::{CGA_MEMORY_END, CGA_MEMORY_START, EGA_MEMORY_END, EGA_MEMORY_START, Video};

/// System bus that routes memory accesses to appropriate devices.
/// Mirrors real PC hardware where the bus connects CPU, RAM, and
/// memory-mapped devices (video card, etc.)
pub struct Bus {
    memory: Memory,
    video: Video,
}

impl Bus {
    pub fn new(memory: Memory, video: Video) -> Self {
        Self { memory, video }
    }

    /// Get immutable reference to memory
    pub fn memory(&self) -> &Memory {
        &self.memory
    }

    /// Get mutable reference to memory
    pub fn memory_mut(&mut self) -> &mut Memory {
        &mut self.memory
    }

    /// Get immutable reference to video
    pub fn video(&self) -> &Video {
        &self.video
    }

    /// Get mutable reference to video
    pub fn video_mut(&mut self) -> &mut Video {
        &mut self.video
    }

    /// Read byte from memory or memory-mapped device
    pub fn read_u8(&self, address: usize) -> u8 {
        // Route to Video for memory-mapped ranges
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&address) {
            let offset = address - CGA_MEMORY_START;
            return self.video.read_byte(offset);
        }
        if (EGA_MEMORY_START..=EGA_MEMORY_END).contains(&address) {
            let offset = address - EGA_MEMORY_START;
            return match self.video.get_mode_type() {
                crate::video::VideoMode::Graphics320x200x256 => self.video.read_byte_vga(offset),
                _ => self.video.read_byte_ega(offset),
            };
        }

        // Normal memory access
        self.memory.read_u8(address)
    }

    /// Write byte to memory or memory-mapped device
    pub fn write_u8(&mut self, address: usize, value: u8) {
        // Route to Video for memory-mapped ranges
        if (CGA_MEMORY_START..=CGA_MEMORY_END).contains(&address) {
            let offset = address - CGA_MEMORY_START;
            self.video.write_byte(offset, value);
            return;
        }
        if (EGA_MEMORY_START..=EGA_MEMORY_END).contains(&address) {
            let offset = address - EGA_MEMORY_START;
            match self.video.get_mode_type() {
                crate::video::VideoMode::Graphics320x200x256 => {
                    self.video.write_byte_vga(offset, value);
                }
                _ => {
                    self.video.write_byte_ega(offset, value);
                }
            }
            return;
        }

        // Normal memory access
        self.memory.write_u8(address, value);
    }

    /// Read 16-bit word from memory or memory-mapped device
    pub fn read_u16(&self, address: usize) -> u16 {
        let low = self.read_u8(address) as u16;
        let high = self.read_u8(address + 1) as u16;
        (high << 8) | low
    }

    /// Write 16-bit word to memory or memory-mapped device
    pub fn write_u16(&mut self, address: usize, value: u16) {
        self.write_u8(address, (value & 0xFF) as u8);
        self.write_u8(address + 1, (value >> 8) as u8);
    }

    /// Read 32-bit dword from memory or memory-mapped device
    pub fn read_u32(&self, address: usize) -> u32 {
        let w1 = self.read_u16(address) as u32;
        let w2 = self.read_u16(address + 2) as u32;
        (w2 << 16) | w1
    }

    /// Write 32-bit dword to memory or memory-mapped device
    pub fn write_u32(&mut self, address: usize, value: u32) {
        self.write_u16(address, (value & 0xFFFF) as u16);
        self.write_u16(address + 2, (value >> 16) as u16);
    }

    // Common Memory delegation methods

    /// Load data at specific memory address
    pub fn load_at(&mut self, address: usize, data: &[u8]) -> anyhow::Result<()> {
        self.memory.load_at(address, data)
    }

    /// Load BIOS into memory
    pub fn load_bios(&mut self, bios_data: &[u8]) -> anyhow::Result<()> {
        self.memory.load_bios(bios_data)
    }

    /// Clear conventional memory
    pub fn clear_conventional_memory(&mut self) {
        self.memory.clear_conventional_memory();
    }

    /// Initialize Interrupt Vector Table
    pub fn initialize_ivt(&mut self) {
        self.memory.initialize_ivt();
    }

    /// Initialize BIOS Data Area
    pub fn initialize_bda(&mut self) {
        self.memory.initialize_bda();
    }

    /// Initialize font data
    pub fn initialize_fonts(&mut self) {
        self.memory.initialize_fonts();
    }

    /// Set A20 line state
    pub fn set_a20_enabled(&mut self, enabled: bool) {
        self.memory.set_a20_enabled(enabled);
    }

    /// Check if A20 line is enabled
    pub fn is_a20_enabled(&self) -> bool {
        self.memory.is_a20_enabled()
    }

    /// Get extended memory size in KB
    pub fn extended_memory_kb(&self) -> u16 {
        self.memory.extended_memory_kb()
    }
}
