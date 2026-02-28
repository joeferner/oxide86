use crate::{
    cpu::{
        Cpu,
        bios::bda::{
            bda_get_columns, bda_get_crt_controller_port_address, bda_get_cursor_pos, bda_get_rows,
            bda_set_cursor_pos,
        },
    },
    io_bus::IoBus,
    memory_bus::MemoryBus,
    video::{CGA_MEMORY_START, video_calculate_linear_offset, video_set_cursor_pos},
};

impl Cpu {
    /// INT 10h, AH=0Eh - Teletype Output
    /// Input:
    ///   AL = character to write
    ///   BL = foreground color (in graphics modes)
    ///   BH = page number (0 for text mode)
    /// Output: None
    pub(in crate::cpu) fn int10_teletype_output(
        &mut self,
        memory_bus: &mut MemoryBus,
        io_bus: &mut IoBus,
    ) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let (cursor_row, cursor_col) = bda_get_cursor_pos(memory_bus);
        let columns = bda_get_columns(memory_bus);
        let rows = bda_get_rows(memory_bus);

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                set_cursor_pos(cursor_row, 0, columns, memory_bus, io_bus);
            }
            b'\n' => {
                // Line feed - move to next line
                let new_row = if cursor_row >= rows - 1 {
                    // Need to scroll
                    scroll_up(1, memory_bus, io_bus);
                    rows - 1
                } else {
                    cursor_row + 1
                };
                set_cursor_pos(new_row, cursor_col, columns, memory_bus, io_bus);
            }
            b'\x08' => {
                // Backspace
                if cursor_col > 0 {
                    set_cursor_pos(cursor_row, cursor_col - 1, columns, memory_bus, io_bus);
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
                memory_bus.write_u8(CGA_MEMORY_START + offset, ch);
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
                        scroll_up(1, memory_bus, io_bus);
                        rows - 1
                    } else {
                        cursor_row + 1
                    };
                    set_cursor_pos(new_row, 0, columns, memory_bus, io_bus);
                } else {
                    set_cursor_pos(cursor_row, new_col, columns, memory_bus, io_bus);
                }
            }
        }
    }
}

fn set_cursor_pos(row: u8, col: u8, columns: u8, memory_bus: &mut MemoryBus, io_bus: &mut IoBus) {
    let crt_controller_port = bda_get_crt_controller_port_address(memory_bus);

    bda_set_cursor_pos(memory_bus, row, col);
    video_set_cursor_pos(
        io_bus,
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

fn scroll_up_advanced(options: ScrollUp, memory_bus: &mut MemoryBus, _io_bus: &mut IoBus) {
    if options.lines == 0 {
        // Clear entire window
        for row in options.top..=options.bottom {
            for col in options.left..=options.right {
                let offset = (row as usize * options.cols as usize + col as usize) * 2;
                memory_bus.write_u8(CGA_MEMORY_START + offset, b' ');
                memory_bus.write_u8(CGA_MEMORY_START + offset + 1, options.attr);
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
                    let ch = memory_bus.read_u8(CGA_MEMORY_START + src_offset);
                    let at = memory_bus.read_u8(CGA_MEMORY_START + src_offset + 1);
                    memory_bus.write_u8(CGA_MEMORY_START + dest_offset, ch);
                    memory_bus.write_u8(CGA_MEMORY_START + dest_offset + 1, at);
                } else {
                    // Fill with blanks
                    memory_bus.write_u8(CGA_MEMORY_START + dest_offset, b' ');
                    memory_bus.write_u8(CGA_MEMORY_START + dest_offset + 1, options.attr);
                }
            }
        }
    }
}

fn scroll_up(lines: u8, memory_bus: &mut MemoryBus, io_bus: &mut IoBus) {
    let rows = bda_get_rows(memory_bus);
    let cols = bda_get_columns(memory_bus);
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
        memory_bus,
        io_bus,
    );
}
