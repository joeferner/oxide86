use crate::{
    bus::Bus,
    byte_to_printable_char,
    cpu::{
        Cpu,
        bios::bda::{
            bda_get_active_page, bda_get_columns, bda_get_crt_controller_port_address,
            bda_get_cursor_end_line, bda_get_cursor_pos, bda_get_cursor_start_line, bda_get_rows,
            bda_get_video_mode, bda_get_video_page_size, bda_set_active_page, bda_set_columns,
            bda_set_cursor_end_line, bda_set_cursor_pos, bda_set_cursor_start_line, bda_set_rows,
            bda_set_video_mode, bda_set_video_page_offset, bda_set_video_page_size,
        },
    },
    physical_address,
    video::{
        CGA_MEMORY_START, VIDEO_MODE_02H_COLOR_TEXT_80_X_25, VIDEO_MODE_03H_COLOR_TEXT_80_X_25,
        VideoCardType, video_calculate_linear_offset,
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
            0x00 => self.int10_set_video_mode(bus),
            0x01 => self.int10_set_cursor_shape(bus),
            0x02 => self.int10_set_cursor_position(bus),
            0x03 => self.int10_get_cursor_position(bus),
            0x05 => self.int10_select_active_page(bus),
            0x06 => self.int10_scroll_up(bus),
            0x08 => self.int10_read_char_attr(bus),
            0x09 => self.int10_write_char_attr(bus),
            0x0A => self.int10_write_char(bus),
            0x0E => self.int10_teletype_output(bus),
            0x0F => self.int10_get_video_mode(bus),
            0x12 => self.int10_alternate_function_select(bus),
            0x1B => self.int10_functionality_state_info(bus),
            _ => {
                log::warn!("Unhandled INT 0x10 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 10h, AH=00h - Set Video Mode
    /// Input:
    ///   AL = video mode
    ///     0x00-0x03 = text modes (40x25 or 80x25)
    ///     0x04-0x05 = CGA 320x200, 4 colors
    ///     0x06 = CGA 640x200, 2 colors
    ///     0x07 = monochrome text 80x25
    /// Output: None
    fn int10_set_video_mode(&mut self, bus: &mut Bus) {
        let mode = (self.ax & 0xFF) as u8; // AL

        // Clear the "Clear Memory" bit (Bit 7 of AL)
        // If Bit 7 is set, BIOS doesn't clear the screen memory
        let clear_screen_flag = (mode & 0x80) == 0;
        let mode = mode & 0x7f;

        let mode_info = bus.video_card_mut().set_mode(mode, clear_screen_flag);

        let mode_info = if let Some(mode_info) = mode_info {
            mode_info
        } else {
            log::warn!("unsupported mode 0x{mode:02X}");
            return;
        };

        // Update BDA with new video mode settings
        bda_set_video_mode(bus, mode);

        bda_set_columns(bus, mode_info.cols);

        // EGA/VGA rows-1 register: programs (e.g. Turbo Pascal, dBASE) read BDA[0x84] to get row count
        bda_set_rows(bus, (mode_info.rows - 1) as u8);

        // Page size = cols * rows * 2 (char + attr)
        let page_size = mode_info.cols as u16 * mode_info.rows as u16 * 2;
        bda_set_video_page_size(bus, page_size);

        set_cursor_pos(bus, 0, 0, 0);

        log::info!(
            "INT 0x10 AH=0x00: Updated BDA for mode 0x{:02X} - cols={}, rows={}, page_size={}",
            mode,
            mode_info.cols,
            mode_info.rows,
            page_size
        );
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
            "INT 0x10/AH=0x01: cursor shape CH=0x{:02X} CL=0x{:02X} visible={}",
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
            "INT 0x10 AH=0x02: Set cursor to row={}, col={}, page={}",
            row,
            col,
            page
        );

        set_cursor_pos(bus, page, row, col);
    }

    /// INT 10h, AH=03h - Get Cursor Position and Shape
    /// Input:
    ///   BH = page number
    /// Output:
    ///   CH = cursor start scan line
    ///   CL = cursor end scan line
    ///   DH = row
    ///   DL = column
    fn int10_get_cursor_position(&mut self, bus: &mut Bus) {
        let page = bda_get_active_page(bus);
        let cursor = bda_get_cursor_pos(bus, page);

        // Get cursor shape from BDA
        let start_line = bda_get_cursor_start_line(bus);
        let end_line = bda_get_cursor_end_line(bus);

        // Return cursor shape in CX
        self.cx = ((start_line as u16) << 8) | (end_line as u16);

        // Return cursor position in DX
        self.dx = ((cursor.row as u16) << 8) | (cursor.col as u16);
    }

    /// INT 10h, AH=05h - Select Active Display Page
    /// Input:
    ///   AL = new page number (0-7 for text modes)
    /// Output: None
    fn int10_select_active_page(&mut self, bus: &mut Bus) {
        let page = bda_get_active_page(bus);
        let old_cursor = bda_get_cursor_pos(bus, page);

        let page = (self.ax & 0xFF) as u8; // AL

        // Validate page number (0-7 for standard text modes)
        if page > 7 {
            log::warn!("INT 0x10/AH=0x05: Invalid page number: {}", page);
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
        bda_set_cursor_pos(bus, page, old_cursor.row, old_cursor.col);

        log::debug!(
            "INT 0x10/AH=0x05: Selected active page {} (offset 0x{:04X})",
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
                "INT 0x10 AH=0x06: CLEARING window with attr=0x{:02X}, ({},{}) to ({},{})",
                attr,
                top,
                left,
                bottom,
                right
            );
        } else {
            log::debug!(
                "INT 0x10 AH=0x06: Scroll up lines={}, attr=0x{:02X}, window=({},{}) to ({},{})",
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
        let mode = bda_get_video_mode(bus);

        if mode == VIDEO_MODE_02H_COLOR_TEXT_80_X_25 || mode == VIDEO_MODE_03H_COLOR_TEXT_80_X_25 {
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

    /// INT 10h, AH=08h - Read Character and Attribute at Cursor Position
    /// Input:
    ///   BH = page number (0 for text mode)
    /// Output:
    ///   AH = attribute byte
    ///   AL = character
    fn int10_read_char_attr(&mut self, bus: &mut Bus) {
        let page = bda_get_active_page(bus);
        let cursor = bda_get_cursor_pos(bus, page);
        let cols = bda_get_columns(bus);
        let offset = (cursor.row as usize * cols as usize + cursor.col as usize) * 2;

        let ch = bus.memory_read_u8(CGA_MEMORY_START + offset);
        let attr = bus.memory_read_u8(CGA_MEMORY_START + offset + 1);

        self.ax = ((attr as u16) << 8) | (ch as u16);
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
        let cursor = bda_get_cursor_pos(bus, page);
        let cols = bda_get_columns(bus) as usize;
        let rows = bda_get_rows(bus) as usize;
        let mode = bda_get_video_mode(bus);

        if mode == VIDEO_MODE_02H_COLOR_TEXT_80_X_25 || mode == VIDEO_MODE_03H_COLOR_TEXT_80_X_25 {
            // Text mode: write to video memory
            for i in 0..count {
                let pos = cursor.row as usize * cols + cursor.col as usize + (i as usize);
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
            //     "INT 0x10 AH=0x09: char=0x{:02X} attr=0x{:02X} (xor={} invert={}) fg={} count={} at ({},{})",
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

    /// INT 10h, AH=0Ah - Write Character at Cursor
    /// Input:
    ///   AL = character to write
    ///   BL = color (in graphics modes only)
    ///   BH = page number (0 for text mode)
    ///   CX = number of times to write character
    /// Output: None (cursor position unchanged, attribute preserved)
    fn int10_write_char(&mut self, bus: &mut Bus) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let count = self.cx;
        let page = bda_get_active_page(bus);
        let cursor = bda_get_cursor_pos(bus, page);
        let cols = bda_get_columns(bus);
        let rows = bda_get_rows(bus);

        // TODO
        // if bus.video().is_graphics_mode() {
        //     // Graphics mode: draw character pixel-by-pixel
        //     // BL contains foreground color
        //     let fg_color = (self.bx & 0xFF) as u8; // BL

        //     for i in 0..count {
        //         let col = cursor.col + (i as usize) % cols;
        //         let row = cursor.row + (i as usize) / cols;
        //         if row >= rows {
        //             break;
        //         }
        //         // AH=0Ah draws transparent characters - no background, no XOR
        //         self.draw_char_graphics(bus, ch, row, col, fg_color, GraphicsDrawMode::Transparent);
        //     }
        // } else {
        // Text mode: write to video memory
        for i in 0..count {
            let pos = cursor.row as usize * cols as usize + cursor.col as usize + (i as usize);
            if pos >= cols as usize * rows as usize {
                break; // Don't write beyond screen
            }
            let offset = pos * 2;
            bus.memory_write_u8(CGA_MEMORY_START + offset, ch);
            // Don't modify attribute byte (offset + 1) - preserve existing color
        }
        // }

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
        let cursor = bda_get_cursor_pos(bus, page);
        let columns = bda_get_columns(bus) as u8;
        let rows = bda_get_rows(bus);

        log::debug!(
            "INT 0x10 0x0E Teletype Output: ch:'{}', page:{page}, cursor:{cursor}",
            byte_to_printable_char(ch)
        );

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                set_cursor_pos(bus, page, cursor.row, 0);
            }
            b'\n' => {
                // Line feed - move to next line
                let new_row = if cursor.row >= rows - 1 {
                    // Need to scroll
                    scroll_up(bus, 1);
                    rows - 1
                } else {
                    cursor.row + 1
                };
                set_cursor_pos(bus, page, new_row, cursor.col);
            }
            b'\x08' => {
                // Backspace
                if cursor.col > 0 {
                    set_cursor_pos(bus, page, cursor.row, cursor.col - 1);
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
                let offset = (cursor.row as usize * columns as usize + cursor.col as usize) * 2;
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
                let new_col = cursor.col + 1;
                if new_col >= columns {
                    // Wrap to next line
                    let new_row = if cursor.row >= rows - 1 {
                        scroll_up(bus, 1);
                        rows - 1
                    } else {
                        cursor.row + 1
                    };
                    set_cursor_pos(bus, page, new_row, 0);
                } else {
                    set_cursor_pos(bus, page, cursor.row, new_col);
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

    /// INT 10h, AH=12h - Video Alternate Function Select
    /// BL = subfunction:
    ///   10h = Get EGA info
    ///   30h = Select vertical resolution
    ///   31h = Palette loading
    ///   32h = Video enable/disable
    ///   33h = Summing
    ///   34h = Cursor emulation
    ///   35h = Display switch
    ///   36h = Video refresh control
    fn int10_alternate_function_select(&mut self, bus: &Bus) {
        // CGA BIOS does not implement AH=12h (EGA/VGA function only)
        if bus.video_card().card_type() == VideoCardType::CGA {
            log::warn!("INT 10h AH=12h: not supported by CGA card - ignoring");
            return;
        }

        let subfunction = (self.bx & 0xFF) as u8; // BL

        match subfunction {
            0x10 => {
                // Get EGA info
                // Returns: BH = color/mono mode, BL = memory size, CH = feature bits, CL = switch setting
                self.bx = 0x0003; // BH=0 (color mode), BL=3 (256KB video memory)
                self.cx = 0x0000; // CH=0, CL=0
                log::debug!("INT 10h/AH=12h/BL=10h: Get EGA info");
            }
            0x30 => {
                // Select vertical resolution
                // Input: AL = resolution (0=200, 1=350, 2=400 scan lines)
                // Output: AL = 12h if function supported
                let requested_resolution = (self.ax & 0xFF) as u8; // AL
                let scan_lines = match requested_resolution {
                    0x00 => 200,
                    0x01 => 350,
                    0x02 => 400,
                    _ => {
                        log::warn!(
                            "INT 10h/AH=12h/BL=30h: Invalid resolution code AL=0x{:02X}",
                            requested_resolution
                        );
                        0 // Invalid
                    }
                };

                if scan_lines > 0 {
                    log::debug!(
                        "INT 10h/AH=12h/BL=30h: Select vertical resolution {} scan lines (AL=0x{:02X})",
                        scan_lines,
                        requested_resolution
                    );
                }

                // Return AL = 12h to indicate function is supported
                self.ax = (self.ax & 0xFF00) | 0x12;
            }
            0x31 => {
                // Palette loading (enable/disable default palette loading)
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::warn!("INT 10h/AH=12h/BL=31h: Palette loading control");
            }
            0x32 => {
                // Video enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::warn!("INT 10h/AH=12h/BL=32h: Video enable/disable");
            }
            0x33 => {
                // Gray-scale summing enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::warn!("INT 10h/AH=12h/BL=33h: Gray-scale summing");
            }
            0x34 => {
                // Cursor emulation enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::warn!("INT 10h/AH=12h/BL=34h: Cursor emulation");
            }
            0x35 => {
                // Display switch interface
                // AL = 0 initial switch, 80h adapter off, FF disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::warn!("INT 10h/AH=12h/BL=35h: Display switch");
            }
            0x36 => {
                // Video refresh control
                // AL = 0 enable refresh, 1 disable refresh
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::warn!("INT 10h/AH=12h/BL=36h: Video refresh control");
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=12h alternate function: BL=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// INT 10h, AH=1Bh - Return Functionality/State Information
    /// Input:
    ///   BX = implementation type (0000h = return state info)
    ///   ES:DI = pointer to 64-byte buffer
    /// Output:
    ///   AL = 1Bh if function supported
    ///   ES:DI buffer filled with state information
    fn int10_functionality_state_info(&mut self, bus: &mut Bus) {
        // AH=1Bh is a VGA-only function; CGA and EGA leave registers unchanged
        if bus.video_card().card_type() != VideoCardType::VGA {
            log::warn!(
                "INT 10h AH=1Bh: not supported by {} card - ignoring",
                bus.video_card().card_type()
            );
            return;
        }

        let impl_type = self.bx;

        if impl_type != 0x0000 {
            // Only implementation type 0 is supported
            log::warn!(
                "INT 10h/AH=1Bh: Unsupported implementation type: 0x{:04X}",
                impl_type
            );
            return;
        }

        let buffer_addr = physical_address(self.es, self.di);

        // Build the 64-byte state information structure
        // Offset 00h-03h: Pointer to static functionality table (we'll point to a dummy location)
        // For simplicity, we set this to 0 (null pointer)
        bus.memory_write_u16(buffer_addr, 0x0000); // Offset
        bus.memory_write_u16(buffer_addr + 2, 0x0000); // Segment

        // Offset 04h: Current video mode
        let mode = bda_get_video_mode(bus);
        bus.memory_write_u8(buffer_addr + 4, mode);

        // Offset 05h-06h: Number of columns
        let cols = bda_get_columns(bus);
        bus.memory_write_u16(buffer_addr + 5, cols);

        // Offset 07h-08h: Length of regen buffer (page size in bytes)
        // cols * rows * 2 bytes per cell (char + attr)
        let rows = bda_get_rows(bus);
        let buffer_size = cols * rows as u16 * 2;
        bus.memory_write_u16(buffer_addr + 7, buffer_size);

        // Offset 09h-0Ah: Starting address in regen buffer (current page offset)
        bus.memory_write_u16(buffer_addr + 9, 0x0000);

        // Offset 0Bh-1Ah: Cursor positions for 8 pages (row, column pairs)
        for page in 0..8 {
            let cursor = bda_get_cursor_pos(bus, page);
            let offset = buffer_addr + 0x0B + (page as usize * 2);
            if page == 0 {
                bus.memory_write_u8(offset, cursor.col as u8); // Column
                bus.memory_write_u8(offset + 1, cursor.row as u8); // Row
            } else {
                bus.memory_write_u8(offset, 0);
                bus.memory_write_u8(offset + 1, 0);
            }
        }

        // Offset 1Bh-1Ch: Cursor type (start/end scan lines)
        let cursor_start = bda_get_cursor_start_line(bus);
        let cursor_end = bda_get_cursor_end_line(bus);
        bus.memory_write_u8(buffer_addr + 0x1B, cursor_end);
        bus.memory_write_u8(buffer_addr + 0x1C, cursor_start);

        // Offset 1Dh: Active display page
        bus.memory_write_u8(buffer_addr + 0x1D, 0);

        // Offset 1Eh-1Fh: CRTC port address (3D4h for color, 3B4h for mono)
        bus.memory_write_u16(buffer_addr + 0x1E, 0x03D4);

        // Offset 20h: Current setting of register 3x8h
        bus.memory_write_u8(buffer_addr + 0x20, 0x00);

        // Offset 21h: Current setting of register 3x9h
        bus.memory_write_u8(buffer_addr + 0x21, 0x00);

        // Offset 22h: Number of rows - 1
        bus.memory_write_u8(buffer_addr + 0x22, rows - 1);

        // Offset 23h-24h: Character height (scan lines per character)
        bus.memory_write_u16(buffer_addr + 0x23, 16); // 16 scan lines for VGA

        // Offset 25h: Active display combination code
        bus.memory_write_u8(buffer_addr + 0x25, 0x08); // VGA with color analog display

        // Offset 26h: Alternate display combination code
        bus.memory_write_u8(buffer_addr + 0x26, 0x00); // No alternate display

        // Offset 27h-28h: Number of colors supported (0 = mono)
        bus.memory_write_u16(buffer_addr + 0x27, 16); // 16 colors in text mode

        // Offset 29h: Number of pages supported
        bus.memory_write_u8(buffer_addr + 0x29, 8);

        // Offset 2Ah: Number of scan lines active
        // 0 = 200, 1 = 350, 2 = 400
        bus.memory_write_u8(buffer_addr + 0x2A, 2); // 400 scan lines

        // Offset 2Bh: Primary character block
        bus.memory_write_u8(buffer_addr + 0x2B, 0);

        // Offset 2Ch: Secondary character block
        bus.memory_write_u8(buffer_addr + 0x2C, 0);

        // Offset 2Dh: Miscellaneous state flags
        // Bit 0: All modes on all displays
        // Bit 1: Gray summing enabled
        // Bit 2: Monochrome display attached
        // Bit 3: Default palette loading disabled
        // Bit 4: Cursor emulation enabled
        // Bit 5: Blinking enabled
        // Bit 6-7: Reserved
        bus.memory_write_u8(buffer_addr + 0x2D, 0x21); // All modes + blinking

        // Offset 2Eh-2Fh: Reserved
        bus.memory_write_u8(buffer_addr + 0x2E, 0);
        bus.memory_write_u8(buffer_addr + 0x2F, 0);

        // Offset 30h: Video memory available
        // 0 = 64KB, 1 = 128KB, 2 = 192KB, 3 = 256KB
        bus.memory_write_u8(buffer_addr + 0x30, 3); // 256KB

        // Offset 31h: Save pointer state flags
        bus.memory_write_u8(buffer_addr + 0x31, 0);

        // Offset 32h-3Fh: Reserved (fill with zeros)
        for i in 0x32..0x40 {
            bus.memory_write_u8(buffer_addr + i, 0);
        }

        // Return AL = 1Bh to indicate function is supported
        self.ax = (self.ax & 0xFF00) | 0x1B;

        log::trace!(
            "INT 10h/AH=1Bh: Returned functionality/state info at {:05X}",
            buffer_addr
        );
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
