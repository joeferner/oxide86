use crate::Bus;
use crate::cpu::Cpu;

mod draw_char_graphics;
mod int10_read_pixel;
mod int10_write_pixel;

/// Drawing mode for characters in graphics mode
#[derive(Debug, Clone, Copy)]
enum GraphicsDrawMode {
    /// Transparent: draw only foreground pixels, leave background unchanged
    Transparent,
    /// Opaque: draw foreground and background (bg=0)
    Opaque,
    /// XOR: XOR foreground pixels with existing content
    Xor,
    /// XOR with inverted glyph: used for inverse video effect (char & attr both have bit 7 set)
    XorInverted,
}

impl Cpu {
    pub(super) fn handle_int10(&mut self, bus: &mut Bus) {
        match function {
            
            
            0x0C => self.int10_write_pixel(bus),
            0x0D => self.int10_read_pixel(bus),
            0x10 => self.int10_palette_registers(bus),
            0x13 => self.int10_write_string(bus),
            0x4F => self.int10_vbe(),
            0xFA => self.int10_installation_checks(),
            
        }
    }

    /// INT 10h, AH=4Fh - VESA BIOS Extensions (VBE)
    /// Returns AX=0x014F (function failed) for all subfunctions,
    /// signalling to callers that VBE is not supported.
    fn int10_vbe(&mut self) {
        let subfunction = (self.ax & 0xFF) as u8;
        log::warn!(
            "INT 10h AH=4Fh (VBE): AL=0x{:02X} not supported, returning failure",
            subfunction
        );
        // AH=0x01 (failed), AL=0x4F (function recognized)
        self.ax = 0x014F;
    }

    



    /// INT 10h, AH=13h - Write String
    /// Input:
    ///   AL = write mode (bit 0: update cursor, bit 1: string has attributes)
    ///   BH = page number
    ///   BL = attribute (if mode bit 1 = 0)
    ///   CX = string length
    ///   DH = row
    ///   DL = column
    ///   ES:BP = pointer to string
    /// Output: None
    fn int10_write_string(&mut self, bus: &mut Bus) {
        let mode = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx & 0xFF) as u8; // BL
        let length = self.cx;
        let row = (self.dx >> 8) as u8; // DH
        let col = (self.dx & 0xFF) as u8; // DL

        let update_cursor = (mode & 0x01) != 0;
        let has_attributes = (mode & 0x02) != 0;

        // Set initial position
        bus.video_mut().set_cursor(row as usize, col as usize);

        let cols = bus.video().get_cols();
        let rows = bus.video().get_rows();
        let mut addr = Self::physical_address(self.es, self.bp);

        for _ in 0..length {
            let ch = bus.read_u8(addr);
            addr += 1;

            let current_attr = if has_attributes {
                let a = bus.read_u8(addr);
                addr += 1;
                a
            } else {
                attr
            };

            let cursor = bus.video().get_cursor();
            if cursor.row >= rows {
                break;
            }

            if bus.video().is_graphics_mode() {
                // Graphics mode: draw character pixel-by-pixel
                let fg_color = current_attr & 0x0F; // Use lower 4 bits as color
                // AH=13h draws opaque characters - background is color 0, no XOR
                self.draw_char_graphics(
                    bus,
                    ch,
                    cursor.row,
                    cursor.col,
                    fg_color,
                    GraphicsDrawMode::Opaque,
                );
            } else {
                // Text mode: write to video memory
                let offset = (cursor.row * cols + cursor.col) * 2;
                bus.video_mut().write_byte(offset, ch);
                bus.video_mut().write_byte(offset + 1, current_attr);
            }

            // Advance cursor (even if not updating final position)
            let new_col = cursor.col + 1;
            if new_col >= cols {
                bus.video_mut().set_cursor(cursor.row + 1, 0);
            } else {
                bus.video_mut().set_cursor(cursor.row, new_col);
            }
        }

        // Restore cursor if mode doesn't update it
        if !update_cursor {
            bus.video_mut().set_cursor(row as usize, col as usize);
        }
    }


    /// INT 10h, AH=10h - Set/Get Palette Registers
    /// Subfunction in AL:
    ///   00h = Set individual palette register
    ///   01h = Set border color
    ///   02h = Set all palette registers
    ///   03h = Toggle intensity/blinking bit
    ///   07h = Read individual palette register
    ///   08h = Read overscan register
    ///   09h = Read all palette registers
    ///   10h = Set individual DAC register
    ///   12h = Set block of DAC registers
    ///   15h = Read individual DAC register
    ///   17h = Read block of DAC registers
    ///   1Ah = Read color page state
    ///   1Bh = Perform gray-scale summing
    fn int10_palette_registers(&mut self, bus: &mut Bus) {
        use crate::video_card_type::VideoCardType;
        // CGA BIOS does not implement AH=10h (EGA/VGA function only)
        if bus.video().card_type() == VideoCardType::CGA {
            log::warn!("INT 10h AH=10h: not supported by CGA card - ignoring");
            return;
        }

        let subfunction = (self.ax & 0xFF) as u8; // AL

        // DAC register operations are VGA-only (EGA has no DAC)
        if bus.video().card_type() == VideoCardType::EGA
            && matches!(subfunction, 0x10 | 0x12 | 0x15 | 0x17 | 0x1A | 0x1B)
        {
            log::warn!(
                "INT 10h/AH=10h/AL={:02X}h: DAC function not supported by EGA card - ignoring",
                subfunction
            );
            return;
        }

        match subfunction {
            0x00 => {
                // Set individual AC palette register
                // BL = register number (0-15), BH = color value (EGA 6-bit index)
                let register = (self.bx & 0xFF) as u8; // BL
                let value = ((self.bx >> 8) & 0xFF) as u8; // BH
                bus.video_mut().set_ac_register(register, value);
                log::debug!(
                    "INT 10h/AH=10h/AL=00h: Set AC register {} = {}",
                    register,
                    value
                );
            }
            0x01 => {
                // Set border (overscan) color
                // BH = border color value
                log::warn!("INT 10h/AH=10h/AL=01h: Set border color");
            }
            0x02 => {
                // Set all AC palette registers and border
                // ES:DX -> 17-byte table (16 AC register values + 1 border color byte)
                let table_addr = Self::physical_address(self.es, self.dx);
                for i in 0..16usize {
                    let value = bus.read_u8(table_addr + i);
                    bus.video_mut().set_ac_register(i as u8, value);
                }
                let border = bus.read_u8(table_addr + 16);
                log::debug!(
                    "INT 10h/AH=10h/AL=02h: Set all AC palette registers, border={}",
                    border
                );
            }
            0x03 => {
                // Toggle intensity/blinking bit
                // BL = 0: bit 7 = high-intensity background (16 bg colors, no blink)
                // BL = 1: bit 7 = character blink (8 bg colors, blink enabled, default)
                let blink_enabled = (self.bx & 0xFF) as u8 != 0;
                bus.video_mut().set_blink_enabled(blink_enabled);
                log::debug!(
                    "INT 10h/AH=10h/AL=03h: {} mode",
                    if blink_enabled { "blink" } else { "intensity" }
                );
            }
            0x10 => {
                // Set individual DAC register
                // Input: BX = register number (0-255)
                //        DH = red value (0-63)
                //        CH = green value (0-63)
                //        CL = blue value (0-63)
                let register = (self.bx & 0xFF) as u8; // Use low byte only
                let red = ((self.dx >> 8) & 0x3F) as u8; // DH, mask to 6 bits
                let green = ((self.cx >> 8) & 0x3F) as u8; // CH, mask to 6 bits
                let blue = (self.cx & 0x3F) as u8; // CL, mask to 6 bits

                bus.video_mut()
                    .set_vga_dac_register(register, red, green, blue);

                log::debug!(
                    "INT 10h/AH=10h/AL=10h: Set DAC register {} to RGB({}, {}, {})",
                    register,
                    red,
                    green,
                    blue
                );
            }
            0x12 => {
                // Set block of DAC registers
                // Input: BX = first register number (0-255)
                //        CX = number of registers to set (0-256)
                //        ES:DX -> table of RGB triplets (3 bytes per entry)
                let first_register = (self.bx & 0xFF) as u8; // Use low byte only
                let count = self.cx;
                let mut table_addr = Self::physical_address(self.es, self.dx);

                for i in 0..count {
                    let register = first_register.wrapping_add(i as u8);
                    let red = bus.read_u8(table_addr) & 0x3F; // Mask to 6 bits
                    let green = bus.read_u8(table_addr + 1) & 0x3F;
                    let blue = bus.read_u8(table_addr + 2) & 0x3F;
                    table_addr += 3;

                    bus.video_mut()
                        .set_vga_dac_register(register, red, green, blue);
                }

                log::debug!(
                    "INT 10h/AH=10h/AL=12h: Set {} DAC registers starting at {}",
                    count,
                    first_register
                );
            }
            0x15 => {
                // Read individual DAC register
                // Input: BX = register number (0-255)
                // Output: DH = red value (0-63)
                //         CH = green value (0-63)
                //         CL = blue value (0-63)
                let register = (self.bx & 0xFF) as u8;
                let rgb = bus.video().get_vga_dac_register(register);

                self.dx = (self.dx & 0x00FF) | ((rgb[0] as u16) << 8); // DH = red
                self.cx = ((rgb[1] as u16) << 8) | (rgb[2] as u16); // CH = green, CL = blue

                log::debug!(
                    "INT 10h/AH=10h/AL=15h: Read DAC register {} = RGB({}, {}, {})",
                    register,
                    rgb[0],
                    rgb[1],
                    rgb[2]
                );
            }
            0x17 => {
                // Read block of DAC registers
                // Input: BX = first register number (0-255)
                //        CX = number of registers to read (0-256)
                //        ES:DX -> buffer for RGB triplets (3 bytes per entry)
                let first_register = (self.bx & 0xFF) as u8;
                let count = self.cx;
                let mut table_addr = Self::physical_address(self.es, self.dx);

                for i in 0..count {
                    let register = first_register.wrapping_add(i as u8);
                    let rgb = bus.video().get_vga_dac_register(register);

                    bus.write_u8(table_addr, rgb[0]); // Red
                    bus.write_u8(table_addr + 1, rgb[1]); // Green
                    bus.write_u8(table_addr + 2, rgb[2]); // Blue
                    table_addr += 3;
                }

                log::debug!(
                    "INT 10h/AH=10h/AL=17h: Read {} DAC registers starting at {}",
                    count,
                    first_register
                );
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=10h palette subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    

    

    /// Helper function for internal scrolling (used by teletype)
    fn scroll_up_internal(&mut self, bus: &mut Bus, lines: u8) {
        // Save registers
        let saved_ax = self.ax;
        let saved_bx = self.bx;
        let saved_cx = self.cx;
        let saved_dx = self.dx;

        // Set up parameters for scroll_up
        let rows = bus.video().get_rows() as u8;
        let cols = bus.video().get_cols() as u8;
        self.ax = (self.ax & 0xFF00) | (lines as u16); // AL = lines
        self.bx = 0x0700; // BH = 0x07 (white on black)
        self.cx = 0x0000; // CH=0, CL=0 (top-left)
        self.dx = (((rows - 1) as u16) << 8) | ((cols - 1) as u16); // DH=rows-1, DL=cols-1 (bottom-right)

        self.int10_scroll_up(bus);

        // Restore registers
        self.ax = saved_ax;
        self.bx = saved_bx;
        self.cx = saved_cx;
        self.dx = saved_dx;
    }

    

    

    

    

    /// INT 10h, AH=FAh - Installation Checks (EGA RIL / FASTBUFF.COM)
    /// This function has two uses depending on BX value:
    ///
    /// When BX=0000h: EGA Register Interface Library - INTERROGATE DRIVER
    /// Input:
    ///   AH = FAh, BX = 0000h
    /// Output:
    ///   BX = 0000h (if RIL driver not present)
    ///   ES:BX -> version number (if present): byte 0=major, byte 1=minor
    ///
    /// When BX!=0000h: FASTBUFF.COM - INSTALLATION CHECK
    /// Input:
    ///   AH = FAh
    /// Output:
    ///   AX = 00FAh (if installed), ES = segment of resident code
    ///
    /// This emulator returns "not installed" for both.
    fn int10_installation_checks(&mut self) {
        if self.bx == 0x0000 {
            // EGA Register Interface Library - INTERROGATE DRIVER
            // Return BX = 0000h to indicate driver not present
            self.bx = 0x0000;
            log::debug!("INT 10h/AH=FAh/BX=0000h: EGA Register Interface not present");
        } else {
            // FASTBUFF.COM - INSTALLATION CHECK
            // Don't modify AX (leave it as is to indicate not installed)
            // The check looks for AX = 00FAh, so leaving AX unchanged signals "not installed"
            log::debug!("INT 10h/AH=FAh: FASTBUFF.COM not installed");
        }
    }


}
