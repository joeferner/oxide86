use crate::{cpu::Cpu, memory_bus::MemoryBus};

impl Cpu {
    /// INT 10h, AH=0Eh - Teletype Output
    /// Input:
    ///   AL = character to write
    ///   BL = foreground color (in graphics modes)
    ///   BH = page number (0 for text mode)
    /// Output: None
    pub(crate) fn int10_teletype_output(&mut self, bus: &mut MemoryBus) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let cursor = bus.video().get_cursor();
        let rows = bus.video().get_rows();

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                bus.video_mut().set_cursor(cursor.row, 0);
            }
            b'\n' => {
                // Line feed - move to next line
                let new_row = if cursor.row >= rows - 1 {
                    // Need to scroll
                    self.scroll_up_internal(bus, 1);
                    rows - 1
                } else {
                    cursor.row + 1
                };
                bus.video_mut().set_cursor(new_row, cursor.col);
            }
            b'\x08' => {
                // Backspace
                if cursor.col > 0 {
                    bus.video_mut().set_cursor(cursor.row, cursor.col - 1);
                }
            }
            _ => {
                // Normal character - handle based on video mode
                if bus.video().is_graphics_mode() {
                    // Graphics mode: draw character pixel-by-pixel
                    let fg_color = (self.bx & 0xFF) as u8; // BL
                    // AH=0Eh (teletype) draws transparent characters - no background, no XOR
                    self.draw_char_graphics(
                        bus,
                        ch,
                        cursor.row,
                        cursor.col,
                        fg_color,
                        GraphicsDrawMode::Transparent,
                    );
                } else {
                    // Text mode: write character byte directly
                    let cols = bus.video().get_cols();
                    let offset = (cursor.row * cols + cursor.col) * 2;

                    // Log existing attribute for debugging
                    let existing_attr = bus.video().read_byte(offset + 1);
                    log::debug!(
                        "INT 10h AH=0Eh: Writing '{}' (0x{:02X}) at ({},{}) ({:04X}h) - existing attr=0x{:02X} (fg={}, bg={})",
                        if (32..127).contains(&ch) {
                            ch as char
                        } else {
                            '.'
                        },
                        ch,
                        cursor.row,
                        cursor.col,
                        offset,
                        existing_attr,
                        existing_attr & 0x0F,
                        (existing_attr >> 4) & 0x07
                    );

                    bus.video_mut().write_byte(offset, ch);
                    // Preserve existing color, but substitute 0x07 for 0x00 (black on black)
                    // since text with attribute 0x00 is always invisible. Many BIOS implementations
                    // do this as a compatibility measure for programs that clear the screen with
                    // attribute 0x00 before exiting (e.g., EDIT, Checkit).
                    if existing_attr == 0x00 {
                        bus.video_mut().write_byte(offset + 1, 0x07);
                    }
                }

                // Advance cursor
                let cols = bus.video().get_cols();
                let new_col = cursor.col + 1;
                if new_col >= cols {
                    // Wrap to next line
                    let new_row = if cursor.row >= rows - 1 {
                        self.scroll_up_internal(bus, 1);
                        rows - 1
                    } else {
                        cursor.row + 1
                    };
                    bus.video_mut().set_cursor(new_row, 0);
                } else {
                    bus.video_mut().set_cursor(cursor.row, new_col);
                }
            }
        }
    }
}
