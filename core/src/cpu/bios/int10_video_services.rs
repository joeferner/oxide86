use crate::{
    bus::Bus,
    cpu::{
        Cpu,
        bios::bda::{
            bda_get_columns, bda_get_crt_controller_port_address, bda_get_cursor_pos, bda_get_rows,
            bda_set_cursor_pos,
        },
    },
    video::{CGA_MEMORY_START, video_calculate_linear_offset, video_set_cursor_pos},
};

impl Cpu {
    /// INT 0x10 - Video Services
    /// AH register contains the function number
    pub(in crate::cpu) fn handle_int10_video_services(&mut self, bus: &mut Bus) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x0E => self.int10_teletype_output(bus),
            _ => {
                log::warn!("Unhandled INT 0x10 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 10h, AH=0Eh - Teletype Output
    /// Input:
    ///   AL = character to write
    ///   BL = foreground color (in graphics modes)
    ///   BH = page number (0 for text mode)
    /// Output: None
    pub(in crate::cpu) fn int10_teletype_output(&mut self, bus: &mut Bus) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let (cursor_row, cursor_col) = bda_get_cursor_pos(bus);
        let columns = bda_get_columns(bus);
        let rows = bda_get_rows(bus);

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                set_cursor_pos(bus, cursor_row, 0, columns);
            }
            b'\n' => {
                // Line feed - move to next line
                let new_row = if cursor_row >= rows - 1 {
                    // Need to scroll
                    scroll_up(1, bus);
                    rows - 1
                } else {
                    cursor_row + 1
                };
                set_cursor_pos(bus, new_row, cursor_col, columns);
            }
            b'\x08' => {
                // Backspace
                if cursor_col > 0 {
                    set_cursor_pos(bus, cursor_row, cursor_col - 1, columns);
                }
            }
            ch => {
                // Normal character - handle based on video mode
                // TODO  if bus.video().is_graphics_mode() {
                // TODO      // Graphics mode: draw character pixel-by-pixel
                // TODO      let fg_color = (self.bx & 0xFF) as u8; // BL
                // TODO      // AH=0Eh (teletype) draws transparent characters - no background, no XOR
                // TODO      self.draw_char_graphics(
                // TODO          bus,
                // TODO          ch,
                // TODO          cursor.row,
                // TODO          cursor.col,
                // TODO          fg_color,
                // TODO          GraphicsDrawMode::Transparent,
                // TODO      );
                // TODO  } else {
                // Text mode: write character byte directly
                let offset = (cursor_row as usize * columns as usize + cursor_col as usize) * 2;
                bus.memory_write_u8(CGA_MEMORY_START + offset, ch);
                // TODO     // Preserve existing color, but substitute 0x07 for 0x00 (black on black)
                // TODO     // since text with attribute 0x00 is always invisible. Many BIOS implementations
                // TODO     // do this as a compatibility measure for programs that clear the screen with
                // TODO     // attribute 0x00 before exiting (e.g., EDIT, Checkit).
                // TODO     let existing_attr = bus.video().read_byte(offset + 1);
                // TODO     if existing_attr == 0x00 {
                // TODO         bus.video_mut().write_byte(offset + 1, 0x07);
                // TODO     }
                // TODO  }

                // Advance cursor
                let new_col = cursor_col + 1;
                if new_col >= columns {
                    // Wrap to next line
                    let new_row = if cursor_row >= rows - 1 {
                        scroll_up(1, bus);
                        rows - 1
                    } else {
                        cursor_row + 1
                    };
                    set_cursor_pos(bus, new_row, 0, columns);
                } else {
                    set_cursor_pos(bus, cursor_row, new_col, columns);
                }
            }
        }
    }
}

fn set_cursor_pos(bus: &mut Bus, row: u8, col: u8, columns: u8) {
    let crt_controller_port = bda_get_crt_controller_port_address(bus);

    bda_set_cursor_pos(bus, row, col);
    video_set_cursor_pos(
        bus,
        crt_controller_port,
        video_calculate_linear_offset(row, col, columns),
    );
}

struct ScrollUp {
    /// number of lines to scroll (0 = clear entire window)
    pub lines: u8,
    /// attribute for blank lines
    pub attr: u8,
    /// row of upper-left corner of window
    pub top: u8,
    /// column of upper-left corner
    pub left: u8,
    /// row of lower-right corner
    pub bottom: u8,
    /// column of lower-right corner
    pub right: u8,
    /// total number of columns in the video
    pub cols: u8,
}

fn scroll_up_advanced(options: ScrollUp, bus: &mut Bus) {
    if options.lines == 0 {
        // Clear entire window
        for row in options.top..=options.bottom {
            for col in options.left..=options.right {
                let offset = (row as usize * options.cols as usize + col as usize) * 2;
                bus.memory_write_u8(CGA_MEMORY_START + offset, b' ');
                bus.memory_write_u8(CGA_MEMORY_START + offset + 1, options.attr);
            }
        }
    } else {
        // Scroll up by 'lines' rows
        for row in options.top..=options.bottom {
            for col in options.left..=options.right {
                let dest_offset = (row as usize * options.cols as usize + col as usize) * 2;
                let src_row = row + options.lines;

                if src_row <= options.bottom {
                    // Copy from below - read from video buffer, not memory
                    let src_offset = (src_row as usize * options.cols as usize + col as usize) * 2;
                    let ch = bus.memory_read_u8(CGA_MEMORY_START + src_offset);
                    let at = bus.memory_read_u8(CGA_MEMORY_START + src_offset + 1);
                    bus.memory_write_u8(CGA_MEMORY_START + dest_offset, ch);
                    bus.memory_write_u8(CGA_MEMORY_START + dest_offset + 1, at);
                } else {
                    // Fill with blanks
                    bus.memory_write_u8(CGA_MEMORY_START + dest_offset, b' ');
                    bus.memory_write_u8(CGA_MEMORY_START + dest_offset + 1, options.attr);
                }
            }
        }
    }
}

fn scroll_up(lines: u8, bus: &mut Bus) {
    let rows = bda_get_rows(bus);
    let cols = bda_get_columns(bus);
    scroll_up_advanced(
        ScrollUp {
            lines,
            attr: 0x07,
            top: 0,
            left: 0,
            bottom: rows,
            right: cols,
            cols,
        },
        bus,
    );
}
