use crate::video::{CGA_MEMORY_END, CGA_MEMORY_START};

use anyhow::{Result, anyhow};

// 1MB = 0x100000 bytes
pub const MEMORY_SIZE: usize = 0x100000;

// IBM BIOS standard 8x8 font location for chars 0x00-0x7F.
// Many programs (e.g. Sierra AGI) hardcode "mov ds, 0xF000; mov si, 0xFA6E" to read
// glyph data directly rather than going through INT 43h.  We must mirror chars 0-127
// of our 8x8 font here to match real IBM BIOS behaviour.
// Only 128 chars (1024 bytes) fit: 0xFFA6E + 0x400 = 0xFFE6E < 0x100000.
pub const FONT_8X8_IBM_ADDR: usize = 0xFFA6E; // F000:FA6E, chars 0x00-0x7F only

pub struct Memory {
    data: Vec<u8>,
    /// A20 gate state (true = enabled, addresses can go above 1MB)
    /// When false, bit 20 is masked off (addresses wrap at 1MB like 8086)
    a20_enabled: bool,
    /// Total memory size in KB (conventional + extended)
    memory_kb: u32,
}

impl Memory {
    pub fn new() -> Self {
        Self::new_with_size(1024)
    }

    /// Create memory with a specific size in KB.
    /// Conventional memory is min(memory_kb, 640) KB.
    /// Extended memory is max(0, memory_kb - 1024) KB (requires 286+ CPU to be useful).
    /// Physical allocation is at least 1 MB to cover the full 8086 address space.
    pub fn new_with_size(memory_kb: u32) -> Self {
        let physical_size = (memory_kb as usize * 1024).max(MEMORY_SIZE);
        Self {
            data: vec![0; physical_size],
            a20_enabled: true, // Enabled by default (AT-class behavior)
            memory_kb,
        }
    }

    /// Conventional memory in KB (up to 640 KB, reported via INT 12h / BDA)
    pub fn conventional_memory_kb(&self) -> u16 {
        self.memory_kb.min(640) as u16
    }

    // Load BIOS - typically at the end of the first megabyte
    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        let bios_size = bios_data.len();

        // BIOS is loaded at the top of the first megabyte (0xF0000 for 64KB BIOS)
        let bios_start = MEMORY_SIZE - bios_size;

        self.load_at(bios_start, bios_data)
    }

    /// Set A20 gate state (controlled by keyboard controller)
    pub fn set_a20_enabled(&mut self, enabled: bool) {
        self.a20_enabled = enabled;
    }

    /// Get A20 gate state
    pub fn is_a20_enabled(&self) -> bool {
        self.a20_enabled
    }

    /// Apply A20 gate logic to an address
    /// When A20 is disabled, bit 20 is masked off (wraps at 1MB like 8086)
    fn apply_a20_gate(&self, address: usize) -> usize {
        if self.a20_enabled {
            address
        } else {
            // A20 disabled: mask off bit 20 (wrap at 1MB)
            address & 0xFFFFF // Keep only lower 20 bits (0-1MB range)
        }
    }

    pub fn read_u8(&self, address: usize) -> u8 {
        let addr = self.apply_a20_gate(address);
// MIGRATED          if addr >= self.data.len() {
// MIGRATED              return 0xFF; // Reading beyond memory returns 0xFF
// MIGRATED          }
// MIGRATED          self.data[addr]
    }

    /// Read a byte from a physical address, bypassing the A20 gate.
    /// Used by INT 15h AH=87h (Move Extended Memory Block) to access
    /// memory above 1 MB regardless of A20 state.
    pub fn read_physical_u8(&self, address: usize) -> u8 {
        if address >= self.data.len() {
            return 0xFF;
        }
        self.data[address]
    }

    /// Write a byte to a physical address, bypassing the A20 gate.
    /// Used by INT 15h AH=87h (Move Extended Memory Block) to access
    /// memory above 1 MB regardless of A20 state.
    pub fn write_physical_u8(&mut self, address: usize, value: u8) {
        if address < self.data.len() {
            self.data[address] = value;
        }
    }

    pub fn write_u8(&mut self, address: usize, value: u8) {
        let addr = self.apply_a20_gate(address);
// MIGRATED          if addr >= self.data.len() {
// MIGRATED              // Writing beyond memory is silently ignored
// MIGRATED              return;
// MIGRATED          }

        // Log writes to Interrupt Vector Table (IVT)
        if (IVT_START..=IVT_END).contains(&addr) {
            // Determine which interrupt vector is being modified
            let int_num = addr / IVT_ENTRY_SIZE;
            let byte_offset = addr % IVT_ENTRY_SIZE;

            // Only log when the first byte of a vector is written to reduce noise
            if byte_offset == 0 {
                // Read the complete vector (will be partially old, partially new after this write)
                let offset_low = value as u16; // This byte being written now
                let offset_high = self.data[addr + 1] as u16;
                let segment_low = self.data[addr + 2] as u16;
                let segment_high = self.data[addr + 3] as u16;
                log::trace!(
                    "IVT Write: INT 0x{:02X} vector being modified (addr 0x{:04X}), will be {:04X}:{:04X}+",
                    int_num,
                    addr,
                    (segment_high << 8) | segment_low,
                    (offset_high << 8) | offset_low
                );
            }
        }

// MIGRATED          self.data[addr] = value;
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

    // Read a 32-bit double word (little-endian)
    pub fn read_u32(&self, address: usize) -> u32 {
        let low = self.read_u16(address) as u32;
        let high = self.read_u16(address + 2) as u32;
        (high << 16) | low
    }

    // Write a 32-bit double word (little-endian)
    pub fn write_u32(&mut self, address: usize, value: u32) {
        self.write_u16(address, (value & 0xFFFF) as u16);
        self.write_u16(address + 2, (value >> 16) as u16);
    }

    /// Clear RAM to zero on reset.
    /// Zeroes conventional memory (0-640 KB) and extended memory (above 1 MB),
    /// skipping the ROM/UMA region (0xA0000-0xFFFFF) which is re-initialised
    /// by initialize_ivt / initialize_bda / initialize_fonts.
    /// This prevents stale VDISK/HIMEM.SYS signatures or driver state from a
    /// previous boot from affecting the new boot.
    pub fn clear_conventional_memory(&mut self) {
        const CONVENTIONAL_END: usize = 0xA0000; // 640 KB
        const EXTENDED_START: usize = 0x100000; // 1 MB - extended memory starts here

        let len = self.data.len();

        // Clear conventional memory (0x00000 - 0x9FFFF)
        let conv_end = CONVENTIONAL_END.min(len);
        for byte in &mut self.data[0..conv_end] {
            *byte = 0;
        }

        // Clear extended memory (above 1 MB) - HIMEM.SYS/VDISK signatures live here
        if len > EXTENDED_START {
            for byte in &mut self.data[EXTENDED_START..len] {
                *byte = 0;
            }
        }
    }

    /// Initialize the Interrupt Vector Table (IVT)
    /// Sets up interrupt handlers for BIOS and DOS-like services
    pub fn initialize_ivt(&mut self) {
        log::debug!("BEGIN initialize_ivt");

        // IVT is at 0x0000-0x03FF (256 entries * 4 bytes each)
        // Each entry contains: [offset_low, offset_high, segment_low, segment_high]

        // Initialize each interrupt vector to a unique offset in the F000 segment
        // This allows us to identify which interrupt was called when DOS chains back to BIOS
        // Format: F000:XXYY where XX is the interrupt number high byte, YY is low byte
        // For example: INT 13h -> F000:0013, INT 21h -> F000:0021
        let default_segment = 0xF000;

        // Initialize all 256 interrupt vectors with unique offsets
        for int_num in 0..256 {
            let ivt_addr = int_num * IVT_ENTRY_SIZE;
            let offset = int_num as u16; // Use interrupt number as offset
            self.write_u16(ivt_addr + 2, default_segment);
            self.write_u16(ivt_addr, offset);

            // Write IRET instruction at each handler location
            let handler_addr = ((default_segment as usize) << 4) + (offset as usize);
            self.write_u8(handler_addr, 0xCF); // IRET instruction
        }

        log::debug!("END initialize_ivt");
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
    /// Get a slice of the raw video memory (B8000-BFFFF)
    pub fn get_video_memory(&self) -> &[u8] {
        let end = (CGA_MEMORY_END + 1).min(self.data.len());
        &self.data[CGA_MEMORY_START..end]
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

        // Keyboard buffer range (0x0040:0080-0083)
        // These are the start and end pointers of the circular keyboard buffer in BDA
        self.write_u16(BDA_START + BDA_KEYBOARD_BUFFER_START, 0x001E); // Buffer starts at BDA+0x1E
        self.write_u16(BDA_START + BDA_KEYBOARD_BUFFER_END, 0x003E); // Buffer ends at BDA+0x3E

        // EGA/VGA rows and character height (0x0040:0084-0085)
        // Programs (e.g., Turbo Pascal, dBASE) read these to determine screen dimensions
        self.write_u8(BDA_START + BDA_EGA_ROWS, 24); // 25 rows - 1 = 24
        self.write_u8(BDA_START + BDA_EGA_CHAR_HEIGHT, 16); // 8x16 VGA font

        // Mouse position (0x0040:00E0-00E3) - custom emulator area, not standard BDA
        // Initialize to center of default 640x200 resolution
        self.write_u16(BDA_START + BDA_MOUSE_X, 320); // Center X
        self.write_u16(BDA_START + BDA_MOUSE_Y, 100); // Center Y

        // Mouse button state (0x0040:00E4)
        self.write_u8(BDA_START + BDA_MOUSE_BUTTONS, 0); // No buttons pressed

        // Mouse cursor visibility counter (0x0040:00E5)
        // Counter < 0 means hidden, >= 0 means visible
        // Initialize to -1 (hidden by default)
        self.write_u8(BDA_START + BDA_MOUSE_VISIBLE, 0xFF); // -1 as unsigned byte

        // Mouse coordinate boundaries (0x0040:00E6-00ED)
        // Default to 640x200 DOS graphics resolution
        self.write_u16(BDA_START + BDA_MOUSE_MIN_X, 0); // Minimum X
        self.write_u16(BDA_START + BDA_MOUSE_MAX_X, 639); // Maximum X
        self.write_u16(BDA_START + BDA_MOUSE_MIN_Y, 0); // Minimum Y
        self.write_u16(BDA_START + BDA_MOUSE_MAX_Y, 199); // Maximum Y
    }

    /// Initialize ROM font data
    /// Loads the 8x16 and 8x8 fonts into ROM BIOS area
    pub fn initialize_fonts(&mut self) {
        use crate::font::Cp437Font;

        let font = Cp437Font::new();

        // Copy 8x16 VGA font to ROM at F000:FA6E
        // 256 characters × 16 bytes = 4096 bytes
        // Use 0..256 instead of 0..=255u8 to avoid infinite loop (u8 wraps at 255)
        for ch in 0..256 {
            let glyph = font.get_glyph_16(ch as u8);
            let dest_addr = FONT_8X16_ADDR + ch * 16;
            for (i, &byte) in glyph.iter().enumerate() {
                self.write_u8(dest_addr + i, byte);
            }
        }

        // Copy 8x8 CGA font to ROM at F000:C000
        // 256 characters × 8 bytes = 2048 bytes
        for ch in 0..256 {
            let glyph = font.get_glyph_8(ch as u8);
            let dest_addr = FONT_8X8_ADDR + ch * 8;
            for (i, &byte) in glyph.iter().enumerate() {
                self.write_u8(dest_addr + i, byte);
            }
        }

        // Mirror chars 0x00-0x7F to IBM BIOS standard address F000:FA6E.
        // Programs like Sierra AGI hardcode this address to read glyph data directly.
        // Only 128 chars fit: 0xFFA6E + 128*8 = 0xFFE6E which is still within 1 MB.
        for ch in 0..128usize {
            let glyph = font.get_glyph_8(ch as u8);
            let dest_addr = FONT_8X8_IBM_ADDR + ch * 8;
            for (i, &byte) in glyph.iter().enumerate() {
                self.write_u8(dest_addr + i, byte);
            }
        }

        // Set INT 43h → 8x8 font (used by programs in graphics mode to draw characters)
        // Real BIOS sets this on mode change; we point it at the 8x8 font for graphics modes
        self.set_interrupt_vector(0x43, FONT_8X8_SEGMENT, FONT_8X8_OFFSET);

        // Set INT 1Fh → upper half of 8x8 font (chars 128-255)
        // Each char is 8 bytes; chars 128-255 start at offset 128*8 = 0x400 bytes in
        self.set_interrupt_vector(0x1F, FONT_8X8_SEGMENT, FONT_8X8_OFFSET + 0x400);

        log::debug!(
            "Initialized ROM fonts: 8x16 at {:05X}, 8x8 at {:05X}; INT 43h=>{:04X}:{:04X}, INT 1Fh=>{:04X}:{:04X}",
            FONT_8X16_ADDR,
            FONT_8X8_ADDR,
            FONT_8X8_SEGMENT,
            FONT_8X8_OFFSET,
            FONT_8X8_SEGMENT,
            FONT_8X8_OFFSET + 0x400,
        );
    }

    /// Get a reference to the entire memory data
    pub fn data(&self) -> &[u8] {
        &self.data
    }
}
