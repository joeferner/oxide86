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
            0x13 => self.int10_write_string(bus),
            0x4F => self.int10_vbe(),
            
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

    

    

    

    




}
