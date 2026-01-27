use crate::video::{VIDEO_MEMORY_END, VIDEO_MEMORY_START};
use anyhow::{Result, anyhow};

// 1MB = 0x100000 bytes
pub const MEMORY_SIZE: usize = 0x100000;

// IVT (Interrupt Vector Table) constants
pub const IVT_START: usize = 0x0000;
pub const IVT_END: usize = 0x03FF;
pub const IVT_ENTRY_SIZE: usize = 4; // Each entry is 4 bytes (offset, segment)

// BIOS data area and interrupt handler locations
pub const BIOS_INTERRUPT_HANDLERS: usize = 0xF000; // Segment where BIOS handlers are located

// BIOS Data Area (BDA) constants
pub const BDA_SEGMENT: u16 = 0x0040;
pub const BDA_START: usize = 0x0400; // Physical address (0x40 * 16)
pub const BDA_SIZE: usize = 0x100; // 256 bytes

// BDA field offsets (from 0x0040:0000)
pub const BDA_COM_PORTS: usize = 0x00; // COM1-COM4 port addresses (4 words)
pub const BDA_LPT_PORTS: usize = 0x08; // LPT1-LPT4 port addresses (4 words)
pub const BDA_EQUIPMENT_LIST: usize = 0x10; // Equipment list word
pub const BDA_MEMORY_SIZE: usize = 0x13; // Memory size in KB (word)
pub const BDA_KEYBOARD_FLAGS1: usize = 0x17; // Keyboard shift flags
pub const BDA_KEYBOARD_FLAGS2: usize = 0x18; // Keyboard shift flags
pub const BDA_KEYBOARD_BUFFER_HEAD: usize = 0x1A; // Keyboard buffer head pointer
pub const BDA_KEYBOARD_BUFFER_TAIL: usize = 0x1C; // Keyboard buffer tail pointer
pub const BDA_KEYBOARD_BUFFER: usize = 0x1E; // Keyboard buffer (32 bytes)
pub const BDA_VIDEO_MODE: usize = 0x49; // Current video mode
pub const BDA_SCREEN_COLUMNS: usize = 0x4A; // Number of screen columns
pub const BDA_VIDEO_PAGE_SIZE: usize = 0x4C; // Video page size in bytes
pub const BDA_VIDEO_PAGE_OFFSET: usize = 0x4E; // Current page start address
pub const BDA_CURSOR_POS: usize = 0x50; // Cursor positions for 8 pages (16 bytes)
pub const BDA_CURSOR_END_LINE: usize = 0x60; // Cursor end scan line
pub const BDA_CURSOR_START_LINE: usize = 0x61; // Cursor start scan line
pub const BDA_ACTIVE_PAGE: usize = 0x62; // Active display page
pub const BDA_CRTC_PORT: usize = 0x63; // CRT controller base port address
pub const BDA_CRT_MODE_CONTROL: usize = 0x65; // CRT mode control register
pub const BDA_CRT_PALETTE: usize = 0x66; // CRT palette register
pub const BDA_TIMER_COUNTER: usize = 0x6C; // Timer counter (dword) - ticks since midnight
pub const BDA_TIMER_OVERFLOW: usize = 0x70; // Timer midnight rollover flag (byte)
pub const BDA_NUM_HARD_DRIVES: usize = 0x75; // Number of hard drives installed (byte)

// Equipment list bits
pub const EQUIPMENT_FLOPPY_INSTALLED: u16 = 0x0001;
pub const EQUIPMENT_MATH_COPROCESSOR: u16 = 0x0002;
pub const EQUIPMENT_POINTING_DEVICE: u16 = 0x0004; // PS/2 mouse
pub const EQUIPMENT_VIDEO_MODE_MASK: u16 = 0x0030; // Bits 4-5: initial video mode
pub const EQUIPMENT_VIDEO_MODE_80X25_COLOR: u16 = 0x0020;
pub const EQUIPMENT_VIDEO_MODE_80X25_MONO: u16 = 0x0030;
pub const EQUIPMENT_FLOPPY_COUNT_MASK: u16 = 0x00C0; // Bits 6-7: number of floppies - 1
pub const EQUIPMENT_SERIAL_COUNT_MASK: u16 = 0x0E00; // Bits 9-11: number of serial ports
pub const EQUIPMENT_PRINTER_COUNT_MASK: u16 = 0xC000; // Bits 14-15: number of printers

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

    pub fn read_u8(&self, address: usize) -> u8 {
        self.data[address % MEMORY_SIZE]
    }

    pub fn write_u8(&mut self, address: usize, value: u8) {
        let addr = address % MEMORY_SIZE;
        self.data[addr] = value;

        // Check if write is in video memory range
        if (VIDEO_MEMORY_START..=VIDEO_MEMORY_END).contains(&addr) {
            let offset = addr - VIDEO_MEMORY_START;
            self.video_writes.push((offset, value));
        }
    }

    // Read a 16-bit word (little-endian)
    pub fn read_u16(&self, address: usize) -> u16 {
        let low = self.read_u8(address) as u16;
        let high = self.read_u8(address + 1) as u16;
        (high << 8) | low
    }

    // Write a 16-bit word (little-endian)
    pub fn write_u16(&mut self, address: usize, value: u16) {
        self.write_u8(address, (value & 0xFF) as u8);
        self.write_u8(address + 1, (value >> 8) as u8);
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
            self.write_u16(ivt_addr, default_offset);
            self.write_u16(ivt_addr + 2, default_segment);
        }

        // Set up specific interrupt handlers
        // INT 0x21: DOS-like services (at F000:0100)
        self.set_interrupt_vector(0x21, 0xF000, 0x0100);

        // Write the default IRET handler at F000:0000
        let iret_addr = ((default_segment as usize) << 4) + (default_offset as usize);
        self.write_u8(iret_addr, 0xCF); // IRET instruction
    }

    /// Set an interrupt vector in the IVT
    pub fn set_interrupt_vector(&mut self, int_num: u8, segment: u16, offset: u16) {
        let ivt_addr = (int_num as usize) * IVT_ENTRY_SIZE;
        self.write_u16(ivt_addr, offset);
        self.write_u16(ivt_addr + 2, segment);
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
        self.write_u8(handler_addr, 0xCF);
    }

    /// Drain video memory writes collected during instruction execution
    pub fn drain_video_writes(&mut self) -> std::vec::Drain<'_, (usize, u8)> {
        self.video_writes.drain(..)
    }

    /// Initialize the BIOS Data Area (BDA)
    /// Sets up system information at 0x0040:0000
    pub fn initialize_bda(&mut self) {
        // COM port addresses (0x0040:0000 - 4 words)
        // Standard COM port I/O addresses
        self.write_u16(BDA_START + BDA_COM_PORTS, 0x03F8); // COM1
        self.write_u16(BDA_START + BDA_COM_PORTS + 2, 0x02F8); // COM2
        self.write_u16(BDA_START + BDA_COM_PORTS + 4, 0x03E8); // COM3
        self.write_u16(BDA_START + BDA_COM_PORTS + 6, 0x02E8); // COM4

        // LPT port addresses (0x0040:0008 - 4 words)
        // Standard LPT (parallel) port I/O addresses
        self.write_u16(BDA_START + BDA_LPT_PORTS, 0x0378); // LPT1
        self.write_u16(BDA_START + BDA_LPT_PORTS + 2, 0x0278); // LPT2
        self.write_u16(BDA_START + BDA_LPT_PORTS + 4, 0x03BC); // LPT3
        self.write_u16(BDA_START + BDA_LPT_PORTS + 6, 0x0000); // LPT4 (not installed)

        // Equipment list word (0x0040:0010)
        // Bits indicate installed hardware
        let mut equipment = 0u16;
        equipment |= EQUIPMENT_FLOPPY_INSTALLED; // Floppy drive installed
        equipment |= EQUIPMENT_VIDEO_MODE_80X25_COLOR; // 80x25 color text mode
        equipment |= 0x0040; // 1 floppy drive (bits 6-7: count-1 = 0)
        // No math coprocessor, no serial ports configured in equipment list
        self.write_u16(BDA_START + BDA_EQUIPMENT_LIST, equipment);

        // Memory size in KB (0x0040:0013)
        // Report 640KB of conventional memory (maximum for PC/XT compatibility)
        self.write_u16(BDA_START + BDA_MEMORY_SIZE, 640);

        // Keyboard flags (0x0040:0017-0018)
        self.write_u8(BDA_START + BDA_KEYBOARD_FLAGS1, 0); // No shift/ctrl/alt pressed
        self.write_u8(BDA_START + BDA_KEYBOARD_FLAGS2, 0); // No special states

        // Keyboard buffer pointers (0x0040:001A-001D)
        // Buffer is empty, both pointers point to buffer start
        let buffer_start = 0x1E; // Offset within BDA
        self.write_u16(BDA_START + BDA_KEYBOARD_BUFFER_HEAD, buffer_start);
        self.write_u16(BDA_START + BDA_KEYBOARD_BUFFER_TAIL, buffer_start);

        // Keyboard buffer (0x0040:001E-003D) - 32 bytes (16 scan code/char pairs)
        // Initialize to zeros
        for i in 0..32 {
            self.write_u8(BDA_START + BDA_KEYBOARD_BUFFER + i, 0);
        }

        // Video mode (0x0040:0049)
        self.write_u8(BDA_START + BDA_VIDEO_MODE, 0x03); // 80x25 color text mode

        // Screen columns (0x0040:004A)
        self.write_u16(BDA_START + BDA_SCREEN_COLUMNS, 80);

        // Video page size (0x0040:004C)
        self.write_u16(BDA_START + BDA_VIDEO_PAGE_SIZE, 4000); // 80*25*2 bytes

        // Current video page offset (0x0040:004E)
        self.write_u16(BDA_START + BDA_VIDEO_PAGE_OFFSET, 0); // Page 0

        // Cursor positions for 8 pages (0x0040:0050-005F)
        // Each page gets a word: low byte = column, high byte = row
        for page in 0..8 {
            self.write_u16(BDA_START + BDA_CURSOR_POS + page * 2, 0x0000); // Row 0, Col 0
        }

        // Cursor shape (0x0040:0060-0061)
        self.write_u8(BDA_START + BDA_CURSOR_END_LINE, 0x0D); // Cursor end scan line
        self.write_u8(BDA_START + BDA_CURSOR_START_LINE, 0x0C); // Cursor start scan line

        // Active display page (0x0040:0062)
        self.write_u8(BDA_START + BDA_ACTIVE_PAGE, 0); // Page 0

        // CRT controller port address (0x0040:0063)
        self.write_u16(BDA_START + BDA_CRTC_PORT, 0x03D4); // Color adapter (monochrome = 0x03B4)

        // CRT mode control register (0x0040:0065)
        self.write_u8(BDA_START + BDA_CRT_MODE_CONTROL, 0x09); // 80x25 text, enable video

        // CRT palette register (0x0040:0066)
        self.write_u8(BDA_START + BDA_CRT_PALETTE, 0x00); // Default palette

        // Timer counter (0x0040:006C) - 4 bytes
        // Initialize to 0 ticks since midnight
        self.write_u16(BDA_START + BDA_TIMER_COUNTER, 0); // Low word
        self.write_u16(BDA_START + BDA_TIMER_COUNTER + 2, 0); // High word

        // Timer overflow flag (0x0040:0070)
        self.write_u8(BDA_START + BDA_TIMER_OVERFLOW, 0); // No midnight rollover yet
    }
}
