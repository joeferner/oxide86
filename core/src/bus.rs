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
            return self.video.read_byte_ega(offset);
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
            self.video.write_byte_ega(offset, value);
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

    /// Get raw video memory (VRAM from video card)
    pub fn get_video_memory_raw(&self) -> &[u8] {
        self.video.get_vram()
    }

    // Common Video delegation methods

    /// Check if display needs updating
    pub fn is_dirty(&self) -> bool {
        self.video.is_dirty()
    }

    /// Check if mode changed
    pub fn take_mode_changed(&mut self) -> bool {
        self.video.take_mode_changed()
    }

    /// Get VGA DAC palette
    pub fn get_vga_dac_palette(&self) -> &[[u8; 3]; 256] {
        self.video.get_vga_dac_palette()
    }

    /// Get text buffer for rendering
    pub fn get_buffer(&self) -> &crate::video::TextBuffer {
        self.video.get_buffer()
    }

    /// Rebuild rendering cache from VRAM
    pub fn rebuild_cache(&mut self) {
        self.video.rebuild_cache();
    }

    /// Get current video mode
    pub fn get_mode(&self) -> u8 {
        self.video.get_mode()
    }

    /// Get video memory (raw VRAM from video card)
    pub fn get_video_memory(&self) -> &[u8] {
        self.video.get_vram()
    }

    /// Get number of columns in current video mode
    pub fn get_cols(&self) -> usize {
        self.video.get_cols()
    }

    /// Get number of rows in current video mode
    pub fn get_rows(&self) -> usize {
        self.video.get_rows()
    }

    /// Clear dirty flag
    pub fn clear_dirty(&mut self) {
        self.video.clear_dirty();
    }

    /// Get cursor position
    pub fn get_cursor(&self) -> crate::video::CursorPosition {
        self.video.get_cursor()
    }

    /// Get video mode type
    pub fn get_mode_type(&self) -> crate::video::VideoMode {
        self.video.get_mode_type()
    }

    /// Get graphics buffer
    pub fn get_graphics_buffer(&self) -> Option<&crate::video::CgaBuffer> {
        self.video.get_graphics_buffer()
    }

    /// Get EGA buffer
    pub fn get_ega_buffer(&self) -> Option<&crate::video::EgaBuffer> {
        self.video.get_ega_buffer()
    }

    /// Get CGA palette
    pub fn get_palette(&self) -> &crate::video::CgaPalette {
        self.video.get_palette()
    }

    /// Get AC (Attribute Controller) palette registers
    pub fn get_ac_palette(&self) -> &[u8; 16] {
        self.video.get_ac_palette()
    }

    /// Check if composite mode is enabled
    pub fn is_composite_mode(&self) -> bool {
        self.video.is_composite_mode()
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
