use anyhow::{Result, anyhow};
use crate::video::{VIDEO_MEMORY_START, VIDEO_MEMORY_END};

// 1MB = 0x100000 bytes
pub const MEMORY_SIZE: usize = 0x100000;

// IVT (Interrupt Vector Table) constants
pub const IVT_START: usize = 0x0000;
pub const IVT_END: usize = 0x03FF;
pub const IVT_ENTRY_SIZE: usize = 4; // Each entry is 4 bytes (offset, segment)

// BIOS data area and interrupt handler locations
pub const BIOS_INTERRUPT_HANDLERS: usize = 0xF000; // Segment where BIOS handlers are located

pub struct Memory {
    data: Vec<u8>,
    video_writes: Vec<(usize, u8)>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            data: vec![0; MEMORY_SIZE],
            video_writes: Vec::new(),
        }
    }

    // Load binary data at a specific address
    pub fn load_at(&mut self, address: usize, data: &[u8]) -> Result<()> {
        if address + data.len() > MEMORY_SIZE {
            return Err(anyhow!(
                "Data exceeds memory bounds: {address:#x} + {:#x} > {MEMORY_SIZE:#x}",
                data.len()
            ));
        }

        self.data[address..address + data.len()].copy_from_slice(data);
        Ok(())
    }

    // Load BIOS - typically at the end of the first megabyte
    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        let bios_size = bios_data.len();

        // BIOS is loaded at the top of memory
        // For a 64KB BIOS: 0x100000 - 0x10000 = 0xF0000
        let bios_start = MEMORY_SIZE - bios_size;

        self.load_at(bios_start, bios_data)
    }

    pub fn read_byte(&self, address: usize) -> u8 {
        self.data[address % MEMORY_SIZE]
    }

    pub fn write_byte(&mut self, address: usize, value: u8) {
        let addr = address % MEMORY_SIZE;
        self.data[addr] = value;

        // Check if write is in video memory range
        if (VIDEO_MEMORY_START..=VIDEO_MEMORY_END).contains(&addr) {
            let offset = addr - VIDEO_MEMORY_START;
            self.video_writes.push((offset, value));
        }
    }

    // Read a 16-bit word (little-endian)
    pub fn read_word(&self, address: usize) -> u16 {
        let low = self.read_byte(address) as u16;
        let high = self.read_byte(address + 1) as u16;
        (high << 8) | low
    }

    // Write a 16-bit word (little-endian)
    pub fn write_word(&mut self, address: usize, value: u16) {
        self.write_byte(address, (value & 0xFF) as u8);
        self.write_byte(address + 1, (value >> 8) as u8);
    }

    /// Initialize the Interrupt Vector Table (IVT)
    /// Sets up interrupt handlers for BIOS and DOS-like services
    pub fn initialize_ivt(&mut self) {
        // IVT is at 0x0000-0x03FF (256 entries * 4 bytes each)
        // Each entry contains: [offset_low, offset_high, segment_low, segment_high]

        // Default handler for unimplemented interrupts (points to IRET)
        // We'll place a simple IRET handler at F000:0000
        let default_offset = 0x0000;
        let default_segment = 0xF000;

        // Initialize all 256 interrupt vectors to default handler
        for int_num in 0..256 {
            let ivt_addr = int_num * IVT_ENTRY_SIZE;
            self.write_word(ivt_addr, default_offset);
            self.write_word(ivt_addr + 2, default_segment);
        }

        // Set up specific interrupt handlers
        // INT 0x21: DOS-like services (at F000:0100)
        self.set_interrupt_vector(0x21, 0xF000, 0x0100);

        // Write the default IRET handler at F000:0000
        let iret_addr = ((default_segment as usize) << 4) + (default_offset as usize);
        self.write_byte(iret_addr, 0xCF); // IRET instruction
    }

    /// Set an interrupt vector in the IVT
    pub fn set_interrupt_vector(&mut self, int_num: u8, segment: u16, offset: u16) {
        let ivt_addr = (int_num as usize) * IVT_ENTRY_SIZE;
        self.write_word(ivt_addr, offset);
        self.write_word(ivt_addr + 2, segment);
    }

    /// Install BIOS interrupt handlers
    /// This writes the actual interrupt handler code into memory
    pub fn install_bios_handlers(&mut self) {
        // Install INT 0x21 handler at F000:0100
        self.install_int21_handler();
    }

    /// Install INT 0x21 (DOS services) handler
    ///
    /// This is a STUB that only provides a valid return point (IRET).
    ///
    /// Real INT 0x21 processing is handled by the Bios trait implementation
    /// when using the Computer wrapper (see cpu::bios module). The Computer
    /// intercepts INT instructions and dispatches them to handle_bios_interrupt()
    /// which performs actual I/O operations.
    ///
    /// This memory-based stub exists to:
    /// - Provide a valid IVT entry at F000:0100
    /// - Allow basic operation if Cpu is used directly without Computer wrapper
    /// - Demonstrate proper interrupt handler structure
    ///
    /// A full memory-based implementation would:
    /// 1. Check AH register value for function number
    /// 2. Dispatch to appropriate handler (character I/O, file operations, etc.)
    /// 3. Perform the requested operation
    /// 4. Return with IRET
    fn install_int21_handler(&mut self) {
        let handler_segment = 0xF000;
        let handler_offset = 0x0100;
        let handler_addr = ((handler_segment as usize) << 4) + (handler_offset as usize);

        // Stub handler: Just return with IRET
        // IRET instruction (0xCF)
        self.write_byte(handler_addr, 0xCF);
    }

    /// Drain video memory writes collected during instruction execution
    pub fn drain_video_writes(&mut self) -> std::vec::Drain<'_, (usize, u8)> {
        self.video_writes.drain(..)
    }
}
