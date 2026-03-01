use crate::{
    bus::Bus,
    cpu::{
        Cpu,
        bios::bda::{
            bda_get_active_page, bda_get_columns, bda_get_crt_controller_port_address,
            bda_get_cursor_pos, bda_get_rows, bda_get_video_mode, bda_get_video_page_size,
            bda_set_active_page, bda_set_cursor_end_line, bda_set_cursor_pos,
            bda_set_cursor_start_line, bda_set_video_page_offset,
        },
    },
    video::{
        CGA_MEMORY_START, VIDEO_MODE_03H_COLOR_TEXT_80_X_25, video_calculate_linear_offset,
        video_card::{
            VIDEO_CARD_CONTROL_ADDR, VIDEO_CARD_DATA_ADDR, VIDEO_CARD_REG_CURSOR_END_LINE,
            VIDEO_CARD_REG_CURSOR_START_LINE,
        },
        video_set_cursor_pos,
    },
};

impl Cpu {
    /// INT 0x10 - Video Services
    /// AH register contains the function number
    pub(in crate::cpu) fn handle_int10_video_services(&mut self, bus: &mut Bus) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x01 => self.int10_set_cursor_shape(bus),
            0x02 => self.int10_set_cursor_position(bus),
            0x05 => self.int10_select_active_page(bus),
            0x06 => self.int10_scroll_up(bus),
            0x09 => self.int10_write_char_attr(bus),
            0x0E => self.int10_teletype_output(bus),
            0x0F => self.int10_get_video_mode(bus),
            _ => {
                log::warn!("Unhandled INT 0x10 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 10h, AH=01h - Set Cursor Shape
    /// Input:
    ///   CH = cursor start scan line (bits 0-4), cursor options (bits 5-6)
    ///   CL = cursor end scan line (bits 0-4)
    /// Output: None
    fn int10_set_cursor_shape(&mut self, bus: &mut Bus) {
        let start_line = (self.cx >> 8) as u8; // CH
        let end_line = (self.cx & 0xFF) as u8; // CL

        // CH bit 5 = cursor disable (hidden). Standard CGA/EGA/VGA behavior.
        let visible = (start_line & 0x20) == 0;

        // Store cursor shape in BDA
        bda_set_cursor_start_line(bus, start_line);
        bda_set_cursor_end_line(bus, end_line);

        // Set Cursor Start Scanline
        bus.io_write_u8(VIDEO_CARD_CONTROL_ADDR, VIDEO_CARD_REG_CURSOR_START_LINE);
        bus.io_write_u8(VIDEO_CARD_DATA_ADDR, start_line);

        // Set Cursor End Scanline
        bus.io_write_u8(VIDEO_CARD_CONTROL_ADDR, VIDEO_CARD_REG_CURSOR_END_LINE);
        bus.io_write_u8(VIDEO_CARD_DATA_ADDR, end_line);

        log::debug!(
            "INT 10h/AH=01h: cursor shape CH={:02X}h CL={:02X}h visible={}",
            start_line,
            end_line,
            visible
        );
    }

    /// INT 10h, AH=02h - Set Cursor Position
    /// Input:
    ///   DH = row (0-24)
    ///   DL = column (0-79)
    ///   BH = page number (0 for text mode)
    /// Output: None
    fn int10_set_cursor_position(&mut self, bus: &mut Bus) {
        let row = (self.dx >> 8) as u8; // DH
        let col = (self.dx & 0xFF) as u8; // DL
        let page = (self.bx >> 8) as u8; // BH

        log::debug!(
            "INT 10h AH=02h: Set cursor to row={}, col={}, page={}",
            row,
            col,
            page
        );

        set_cursor_pos(bus, page, row, col);
    }

    /// INT 10h, AH=05h - Select Active Display Page
    /// Input:
    ///   AL = new page number (0-7 for text modes)
    /// Output: None
    fn int10_select_active_page(&mut self, bus: &mut Bus) {
        let page = bda_get_active_page(bus);
        let (old_cursor_row, old_cursor_col) = bda_get_cursor_pos(bus, page);

        let page = (self.ax & 0xFF) as u8; // AL

        // Validate page number (0-7 for standard text modes)
        if page > 7 {
            log::warn!("INT 10h/AH=05h: Invalid page number: {}", page);
            return;
        }

        bda_set_active_page(bus, page);

        // Calculate the memory offset for the start of the page
        // Each page size depends on the mode (e.g., 2KB or 4KB for text)
        let page_size = bda_get_video_page_size(bus);
        let start_offset = page as u16 * page_size;

        bda_set_video_page_offset(bus, start_offset);

        // Update the CRT Controller (CRTC) Hardware
        // The Start Address High (0x0C) and Low (0x0D) registers
        // tell the VGA hardware where to begin fetching pixel/text data.

        // Convert byte offset to word offset (CRTC uses words)
        let word_offset = start_offset / 2;

        // Update active page in Video struct
        // Write to CRTC Register 0x0C (Start Address High)
        bus.io_write_u8(VIDEO_CARD_CONTROL_ADDR, 0x0C);
        bus.io_write_u8(VIDEO_CARD_DATA_ADDR, ((word_offset >> 8) & 0xff) as u8);

        // Write to CRTC Register 0x0D (Start Address Low)
        bus.io_write_u8(VIDEO_CARD_CONTROL_ADDR, 0x0D);
        bus.io_write_u8(VIDEO_CARD_DATA_ADDR, (word_offset & 0xff) as u8);

        // The BIOS also tracks cursor X/Y for EACH page
        // refresh the hardware cursor to match the stored position for the NEW page.
        bda_set_cursor_pos(bus, page, old_cursor_row, old_cursor_col);

        log::debug!(
            "INT 10h/AH=05h: Selected active page {} (offset 0x{:04X})",
            page,
            start_offset
        );
    }

    /// INT 10h, AH=06h - Scroll Up Window
    /// Input:
    ///   AL = number of lines to scroll (0 = clear entire window)
    ///   BH = attribute for blank lines
    ///   CH = row of upper-left corner of window
    ///   CL = column of upper-left corner
    ///   DH = row of lower-right corner
    ///   DL = column of lower-right corner
    /// Output: None
    fn int10_scroll_up(&mut self, bus: &mut Bus) {
        let lines = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx >> 8) as u8; // BH
        let top = (self.cx >> 8) as u8; // CH
        let left = (self.cx & 0xFF) as u8; // CL
        let bottom = (self.dx >> 8) as u8; // DH
        let right = (self.dx & 0xFF) as u8; // DL

        if lines == 0 {
            log::debug!(
                "INT 10h AH=06h: CLEARING window with attr=0x{:02X}, ({},{}) to ({},{})",
                attr,
                top,
                left,
                bottom,
                right
            );
        } else {
            log::debug!(
                "INT 10h AH=06h: Scroll up lines={}, attr=0x{:02X}, window=({},{}) to ({},{})",
                lines,
                attr,
                top,
                left,
                bottom,
                right
            );
        }

        let rows = bda_get_rows(bus);
        let cols = bda_get_columns(bus);

        if bda_get_video_mode(bus) == VIDEO_MODE_03H_COLOR_TEXT_80_X_25 {
            scroll_up_advanced(
                bus,
                ScrollUp {
                    lines,
                    attr,
                    top,
                    left,
                    bottom,
                    right,
                    rows,
                    cols: cols as u8,
                },
            );
        } else {
            todo!("video mode scroll");
        }
    }

    /// INT 10h, AH=09h - Write Character and Attribute at Cursor
    /// Input:
    ///   AL = character to write
    ///   BL = attribute byte (foreground/background color in text, foreground color in graphics)
    ///   BH = page number (0 for text mode)
    ///   CX = number of times to write character
    /// Output: None (cursor position unchanged)
    fn int10_write_char_attr(&mut self, bus: &mut Bus) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx & 0xFF) as u8; // BL
        let count = self.cx;
        let page = bda_get_active_page(bus);
        let (cursor_row, cursor_col) = bda_get_cursor_pos(bus, page);
        let cols = bda_get_columns(bus) as usize;
        let rows = bda_get_rows(bus) as usize;

        if bda_get_video_mode(bus) == VIDEO_MODE_03H_COLOR_TEXT_80_X_25 {
            // Text mode: write to video memory
            for i in 0..count {
                let pos = cursor_row as usize * cols + cursor_col as usize + (i as usize);
                if pos >= cols * rows {
                    break; // Don't write beyond screen
                }
                let offset = pos * 2;
                bus.memory_write_u8(CGA_MEMORY_START + offset, ch);
                bus.memory_write_u8(CGA_MEMORY_START + offset + 1, attr);
            }
        } else {
            todo!("Graphics mode: Write Character and Attribute at Cursor")
            // // Graphics mode: draw character pixel-by-pixel
            // // IBM CGA BIOS behavior:
            // // - BL bit 7 = 1: XOR mode
            // // - BL bit 7 = 0: Normal mode
            // // - When BOTH char bit 7 AND attr bit 7 are set: invert glyph for inverse effect
            // let fg_color = attr & 0x0F; // Lower 4 bits = color index
            // let xor_mode = (attr & 0x80) != 0; // Bit 7 = XOR mode
            // let invert_glyph = (ch & 0x80) != 0 && xor_mode; // Invert if both bits set

            // log::debug!(
            //     "INT 10h AH=09h: char=0x{:02X} attr=0x{:02X} (xor={} invert={}) fg={} count={} at ({},{})",
            //     ch,
            //     attr,
            //     xor_mode,
            //     invert_glyph,
            //     fg_color,
            //     count,
            //     cursor.row,
            //     cursor.col
            // );

            // for i in 0..count {
            //     let col = cursor.col + (i as usize) % cols;
            //     let row = cursor.row + (i as usize) / cols;
            //     if row >= rows {
            //         break;
            //     }
            //     // Determine draw mode based on attribute and character bits
            //     let draw_mode = if invert_glyph {
            //         GraphicsDrawMode::XorInverted
            //     } else if xor_mode {
            //         GraphicsDrawMode::Xor
            //     } else {
            //         GraphicsDrawMode::Opaque
            //     };
            //     self.draw_char_graphics(bus, ch, row, col, fg_color, draw_mode);
            // }
        }
        // Cursor position is NOT updated by this function
    }

    /// INT 10h, AH=0Eh - Teletype Output
    /// Input:
    ///   AL = character to write
    ///   BL = foreground color (in graphics modes)
    ///   BH = page number (0 for text mode)
    /// Output: None
    pub(in crate::cpu) fn int10_teletype_output(&mut self, bus: &mut Bus) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let page = bda_get_active_page(bus);
        let (cursor_row, cursor_col) = bda_get_cursor_pos(bus, page);
        let columns = bda_get_columns(bus) as u8;
        let rows = bda_get_rows(bus);

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                set_cursor_pos(bus, page, cursor_row, 0);
            }
            b'\n' => {
                // Line feed - move to next line
                let new_row = if cursor_row >= rows - 1 {
                    // Need to scroll
                    scroll_up(bus, 1);
                    rows - 1
                } else {
                    cursor_row + 1
                };
                set_cursor_pos(bus, page, new_row, cursor_col);
            }
            b'\x08' => {
                // Backspace
                if cursor_col > 0 {
                    set_cursor_pos(bus, page, cursor_row, cursor_col - 1);
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
                        scroll_up(bus, 1);
                        rows - 1
                    } else {
                        cursor_row + 1
                    };
                    set_cursor_pos(bus, page, new_row, 0);
                } else {
                    set_cursor_pos(bus, page, cursor_row, new_col);
                }
            }
        }
    }

    /// INT 10h, AH=0Fh - Get Current Video Mode
    /// Input: None
    /// Output:
    ///   AH = number of screen columns
    ///   AL = video mode
    ///   BH = active display page
    fn int10_get_video_mode(&mut self, bus: &mut Bus) {
        let mode = bda_get_video_mode(bus);
        let columns = bda_get_columns(bus);
        let page = bda_get_active_page(bus);

        self.ax = (columns << 8) | (mode as u16);
        self.bx = (self.bx & 0x00FF) | ((page as u16) << 8);
    }
}

fn set_cursor_pos(bus: &mut Bus, page: u8, row: u8, col: u8) {
    bda_set_cursor_pos(bus, page, row, col);

    let current_page = bda_get_active_page(bus);
    if page == current_page {
        let max_cols = bda_get_columns(bus);
        let crt_controller_port = bda_get_crt_controller_port_address(bus);
        video_set_cursor_pos(
            bus,
            crt_controller_port,
            video_calculate_linear_offset(row, col, max_cols as u8),
        );
    }
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
    /// total number of rows in the video
    pub rows: u8,
    /// total number of columns in the video
    pub cols: u8,
}

fn scroll_up_advanced(bus: &mut Bus, options: ScrollUp) {
    // Clamp to valid range (real BIOS behavior: clip out-of-range coords)
    let right = options.right.min(options.cols - 1);
    let bottom = options.bottom.min(options.rows - 1);
    if options.top > bottom || options.left > right {
        return;
    }

    if options.lines == 0 {
        // Clear entire window
        for row in options.top..=bottom {
            for col in options.left..=right {
                let offset = (row as usize * options.cols as usize + col as usize) * 2;
                bus.memory_write_u8(CGA_MEMORY_START + offset, b' ');
                bus.memory_write_u8(CGA_MEMORY_START + offset + 1, options.attr);
            }
        }
    } else {
        // Scroll up by 'lines' rows
        for row in options.top..=bottom {
            for col in options.left..=right {
                let dest_offset = (row as usize * options.cols as usize + col as usize) * 2;
                let src_row = row + options.lines;

                if src_row <= bottom {
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

fn scroll_up(bus: &mut Bus, lines: u8) {
    let rows = bda_get_rows(bus);
    let cols = bda_get_columns(bus);
    scroll_up_advanced(
        bus,
        ScrollUp {
            lines,
            attr: 0x07,
            top: 0,
            left: 0,
            bottom: rows,
            right: cols as u8,
            rows,
            cols: cols as u8,
        },
    );
}
