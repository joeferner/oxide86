use crate::{cpu::Cpu, memory::Memory};

impl Cpu {
    /// INT 0x10 - Video Services
    /// AH register contains the function number
    pub(super) fn handle_int10(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int10_set_video_mode(memory, video),
            0x01 => self.int10_set_cursor_shape(memory),
            0x02 => self.int10_set_cursor_position(video),
            0x03 => self.int10_get_cursor_position(memory, video),
            0x05 => self.int10_select_active_page(memory, video),
            0x06 => self.int10_scroll_up(video),
            0x07 => self.int10_scroll_down(video),
            0x08 => self.int10_read_char_attr(video),
            0x09 => self.int10_write_char_attr(memory, video),
            0x0A => self.int10_write_char(video),
            0x0B => self.int10_set_color_palette(),
            0x0C => self.int10_write_pixel(video),
            0x0D => self.int10_read_pixel(video),
            0x0E => self.int10_teletype_output(video),
            0x0F => self.int10_get_video_mode(video),
            0x10 => self.int10_palette_registers(),
            0x11 => self.int10_character_generator(memory),
            0x12 => self.int10_alternate_function_select(),
            0x13 => self.int10_write_string(memory, video),
            0x15 => self.int10_return_physical_display_params(),
            0x1A => self.int10_display_combination_code(memory),
            0x1B => self.int10_functionality_state_info(memory, video),
            0xFA => self.int10_installation_checks(),
            0xFE => self.int10_get_video_buffer(memory),
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
    fn int10_set_video_mode(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let mode = (self.ax & 0xFF) as u8; // AL

        // Support text modes (0x00-0x03, 0x07) and CGA graphics modes (0x04-0x06)
        match mode {
            0x00..=0x07 => {
                video.set_mode(mode);
                // Reset cursor to top-left (only relevant for text modes)
                video.set_cursor(0, 0);

                // Update BDA with new video mode settings
                memory.write_u8(
                    crate::memory::BDA_START + crate::memory::BDA_VIDEO_MODE,
                    mode,
                );

                let cols = video.get_cols();
                let rows = video.get_rows();
                memory.write_u16(
                    crate::memory::BDA_START + crate::memory::BDA_SCREEN_COLUMNS,
                    cols as u16,
                );

                // Page size = cols * rows * 2 (char + attr)
                let page_size = cols * rows * 2;
                memory.write_u16(
                    crate::memory::BDA_START + crate::memory::BDA_VIDEO_PAGE_SIZE,
                    page_size as u16,
                );

                log::info!(
                    "INT 10h AH=00h: Updated BDA for mode 0x{:02X} - cols={}, rows={}, page_size={}",
                    mode,
                    cols,
                    rows,
                    page_size
                );
            }
            _ => {
                log::warn!("Unsupported video mode: 0x{:02X}", mode);
            }
        }
    }

    /// INT 10h, AH=02h - Set Cursor Position
    /// Input:
    ///   DH = row (0-24)
    ///   DL = column (0-79)
    ///   BH = page number (0 for text mode)
    /// Output: None
    fn int10_set_cursor_position(&mut self, video: &mut crate::video::Video) {
        let row = (self.dx >> 8) as u8; // DH
        let col = (self.dx & 0xFF) as u8; // DL

        let cols = video.get_cols();
        let rows = video.get_rows();

        if (row as usize) < rows && (col as usize) < cols {
            video.set_cursor(row as usize, col as usize);
        }
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
    fn int10_scroll_up(&mut self, video: &mut crate::video::Video) {
        let lines = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx >> 8) as u8; // BH
        let top = (self.cx >> 8) as u8; // CH
        let left = (self.cx & 0xFF) as u8; // CL
        let bottom = (self.dx >> 8) as u8; // DH
        let right = (self.dx & 0xFF) as u8; // DL

        let cols = video.get_cols();
        let rows = video.get_rows();

        // Validate bounds
        if top > bottom || left > right || (bottom as usize) >= rows || (right as usize) >= cols {
            return;
        }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * cols + col as usize) * 2;
                    video.write_byte(offset, b' ');
                    video.write_byte(offset + 1, attr);
                }
            }
        } else {
            // Scroll up by 'lines' rows
            for row in top..=bottom {
                for col in left..=right {
                    let dest_offset = (row as usize * cols + col as usize) * 2;
                    let src_row = row + lines;

                    if src_row <= bottom {
                        // Copy from below - read from video buffer, not memory
                        let src_offset = (src_row as usize * cols + col as usize) * 2;
                        let ch = video.read_byte(src_offset);
                        let at = video.read_byte(src_offset + 1);
                        video.write_byte(dest_offset, ch);
                        video.write_byte(dest_offset + 1, at);
                    } else {
                        // Fill with blanks
                        video.write_byte(dest_offset, b' ');
                        video.write_byte(dest_offset + 1, attr);
                    }
                }
            }
        }
    }

    /// INT 10h, AH=07h - Scroll Down Window
    /// Input:
    ///   AL = number of lines to scroll (0 = clear entire window)
    ///   BH = attribute for blank lines
    ///   CH = row of upper-left corner of window
    ///   CL = column of upper-left corner
    ///   DH = row of lower-right corner
    ///   DL = column of lower-right corner
    /// Output: None
    fn int10_scroll_down(&mut self, video: &mut crate::video::Video) {
        let lines = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx >> 8) as u8; // BH
        let top = (self.cx >> 8) as u8; // CH
        let left = (self.cx & 0xFF) as u8; // CL
        let bottom = (self.dx >> 8) as u8; // DH
        let right = (self.dx & 0xFF) as u8; // DL

        let cols = video.get_cols();
        let rows = video.get_rows();

        // Validate bounds
        if top > bottom || left > right || (bottom as usize) >= rows || (right as usize) >= cols {
            return;
        }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * cols + col as usize) * 2;
                    video.write_byte(offset, b' ');
                    video.write_byte(offset + 1, attr);
                }
            }
        } else {
            // Scroll down by 'lines' rows (process bottom to top)
            for row in (top..=bottom).rev() {
                for col in left..=right {
                    let dest_offset = (row as usize * cols + col as usize) * 2;

                    if row >= top + lines {
                        // Copy from above - read from video buffer, not memory
                        let src_row = row - lines;
                        let src_offset = (src_row as usize * cols + col as usize) * 2;
                        let ch = video.read_byte(src_offset);
                        let at = video.read_byte(src_offset + 1);
                        video.write_byte(dest_offset, ch);
                        video.write_byte(dest_offset + 1, at);
                    } else {
                        // Fill with blanks
                        video.write_byte(dest_offset, b' ');
                        video.write_byte(dest_offset + 1, attr);
                    }
                }
            }
        }
    }

    /// INT 10h, AH=08h - Read Character and Attribute at Cursor Position
    /// Input:
    ///   BH = page number (0 for text mode)
    /// Output:
    ///   AH = attribute byte
    ///   AL = character
    fn int10_read_char_attr(&mut self, video: &crate::video::Video) {
        let cursor = video.get_cursor();
        let cols = video.get_cols();
        let offset = (cursor.row * cols + cursor.col) * 2;

        let ch = video.read_byte(offset);
        let attr = video.read_byte(offset + 1);

        self.ax = ((attr as u16) << 8) | (ch as u16);
    }

    /// INT 10h, AH=09h - Write Character and Attribute at Cursor
    /// Input:
    ///   AL = character to write
    ///   BL = attribute byte (foreground/background color)
    ///   BH = page number (0 for text mode)
    ///   CX = number of times to write character
    /// Output: None (cursor position unchanged)
    fn int10_write_char_attr(&mut self, _memory: &mut Memory, video: &mut crate::video::Video) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx & 0xFF) as u8; // BL
        let count = self.cx;
        let cursor = video.get_cursor();
        let cols = video.get_cols();
        let rows = video.get_rows();

        for i in 0..count {
            let pos = cursor.row * cols + cursor.col + (i as usize);
            if pos >= cols * rows {
                break; // Don't write beyond screen
            }
            let offset = pos * 2;
            video.write_byte(offset, ch);
            video.write_byte(offset + 1, attr);
        }
        // Cursor position is NOT updated by this function
    }

    /// INT 10h, AH=0Ah - Write Character at Cursor
    /// Input:
    ///   AL = character to write
    ///   BH = page number (0 for text mode)
    ///   CX = number of times to write character
    /// Output: None (cursor position unchanged, attribute preserved)
    fn int10_write_char(&mut self, video: &mut crate::video::Video) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let count = self.cx;
        let cursor = video.get_cursor();
        let cols = video.get_cols();
        let rows = video.get_rows();

        for i in 0..count {
            let pos = cursor.row * cols + cursor.col + (i as usize);
            if pos >= cols * rows {
                break; // Don't write beyond screen
            }
            let offset = pos * 2;
            video.write_byte(offset, ch);
            // Don't modify attribute byte (offset + 1) - preserve existing color
        }
        // Cursor position is NOT updated by this function
    }

    /// INT 10h, AH=0Eh - Teletype Output
    /// Input:
    ///   AL = character to write
    ///   BL = foreground color (in graphics modes)
    ///   BH = page number (0 for text mode)
    /// Output: None
    pub(crate) fn int10_teletype_output(&mut self, video: &mut crate::video::Video) {
        let ch = (self.ax & 0xFF) as u8; // AL
        let cursor = video.get_cursor();
        let rows = video.get_rows();

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                video.set_cursor(cursor.row, 0);
            }
            b'\n' => {
                // Line feed - move to next line
                let new_row = if cursor.row >= rows - 1 {
                    // Need to scroll
                    self.scroll_up_internal(video, 1);
                    rows - 1
                } else {
                    cursor.row + 1
                };
                video.set_cursor(new_row, cursor.col);
            }
            b'\x08' => {
                // Backspace
                if cursor.col > 0 {
                    video.set_cursor(cursor.row, cursor.col - 1);
                }
            }
            _ => {
                // Normal character - handle based on video mode
                if video.is_graphics_mode() {
                    // Graphics mode: draw character pixel-by-pixel
                    let fg_color = (self.bx & 0xFF) as u8; // BL
                    self.draw_char_graphics(video, ch, cursor.row, cursor.col, fg_color);
                } else {
                    // Text mode: write character byte directly
                    let cols = video.get_cols();
                    let offset = (cursor.row * cols + cursor.col) * 2;
                    video.write_byte(offset, ch);
                    // Don't modify attribute byte (preserve existing color)
                }

                // Advance cursor
                let cols = video.get_cols();
                let new_col = cursor.col + 1;
                if new_col >= cols {
                    // Wrap to next line
                    let new_row = if cursor.row >= rows - 1 {
                        self.scroll_up_internal(video, 1);
                        rows - 1
                    } else {
                        cursor.row + 1
                    };
                    video.set_cursor(new_row, 0);
                } else {
                    video.set_cursor(cursor.row, new_col);
                }
            }
        }
    }

    /// Draw a character in graphics mode using font data
    fn draw_char_graphics(
        &self,
        video: &mut crate::video::Video,
        character: u8,
        row: usize,
        col: usize,
        color: u8,
    ) {
        use crate::font::Cp437Font;

        let font = Cp437Font::new();

        match video.get_mode_type() {
            crate::video::VideoMode::Graphics320x200 | crate::video::VideoMode::Graphics640x200 => {
                // CGA graphics mode: use native 8x8 CGA font
                let glyph = font.get_glyph_8(character);
                let char_height = 8;
                let char_width = 8;

                // Calculate pixel position
                let start_x = col * char_width;
                let start_y = row * char_height;

                // Draw each row of the character
                for (py, &glyph_byte) in glyph.iter().enumerate() {
                    if start_y + py >= 200 {
                        break; // Don't draw past bottom of screen
                    }

                    // Draw each pixel in the row
                    for px in 0..char_width {
                        if start_x + px >= 320 {
                            break; // Don't draw past right edge
                        }

                        let bit = (glyph_byte >> (7 - px)) & 1;
                        if bit != 0 {
                            // Set pixel to foreground color
                            self.set_pixel_cga(video, start_x + px, start_y + py, color);
                        }
                        // Background pixels are left as-is (transparent)
                    }
                }
            }
            _ => {
                // Other modes not supported yet
            }
        }
    }

    /// Set a pixel in CGA graphics mode
    /// Calculates CGA interlaced address: even lines at 0x0000+, odd lines at 0x2000+
    fn set_pixel_cga(&self, video: &mut crate::video::Video, x: usize, y: usize, color: u8) {
        match video.get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                // 320x200, 4 colors, 2 bits per pixel
                // Calculate CGA interlaced offset
                let byte_offset = if y.is_multiple_of(2) {
                    // Even line: bank 0
                    (y / 2) * 80 + x / 4
                } else {
                    // Odd line: bank 1 (offset by 0x2000)
                    0x2000 + ((y - 1) / 2) * 80 + x / 4
                };

                let pixel_in_byte = x % 4;
                let shift = 6 - (pixel_in_byte * 2);

                // Read-modify-write
                let mut byte_val = video.read_byte(byte_offset);
                byte_val &= !(0x03 << shift); // Clear the 2-bit pixel
                byte_val |= (color & 0x03) << shift; // Set new color
                video.write_byte(byte_offset, byte_val);
            }
            crate::video::VideoMode::Graphics640x200 => {
                // 640x200, 2 colors, 1 bit per pixel
                // Calculate CGA interlaced offset
                let byte_offset = if y.is_multiple_of(2) {
                    // Even line: bank 0
                    (y / 2) * 80 + x / 8
                } else {
                    // Odd line: bank 1 (offset by 0x2000)
                    0x2000 + ((y - 1) / 2) * 80 + x / 8
                };

                let pixel_in_byte = x % 8;
                let bit_mask = 0x80 >> pixel_in_byte;

                let mut byte_val = video.read_byte(byte_offset);
                if color & 1 != 0 {
                    byte_val |= bit_mask; // Set bit
                } else {
                    byte_val &= !bit_mask; // Clear bit
                }
                video.write_byte(byte_offset, byte_val);
            }
            _ => {
                // Not a graphics mode
            }
        }
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
    fn int10_write_string(&mut self, memory: &Memory, video: &mut crate::video::Video) {
        let mode = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx & 0xFF) as u8; // BL
        let length = self.cx;
        let row = (self.dx >> 8) as u8; // DH
        let col = (self.dx & 0xFF) as u8; // DL

        let update_cursor = (mode & 0x01) != 0;
        let has_attributes = (mode & 0x02) != 0;

        // Set initial position
        video.set_cursor(row as usize, col as usize);

        let cols = video.get_cols();
        let rows = video.get_rows();
        let mut addr = Self::physical_address(self.es, self.bp);

        for _ in 0..length {
            let ch = memory.read_u8(addr);
            addr += 1;

            let current_attr = if has_attributes {
                let a = memory.read_u8(addr);
                addr += 1;
                a
            } else {
                attr
            };

            let cursor = video.get_cursor();
            if cursor.row >= rows {
                break;
            }

            let offset = (cursor.row * cols + cursor.col) * 2;
            video.write_byte(offset, ch);
            video.write_byte(offset + 1, current_attr);

            // Advance cursor (even if not updating final position)
            let new_col = cursor.col + 1;
            if new_col >= cols {
                video.set_cursor(cursor.row + 1, 0);
            } else {
                video.set_cursor(cursor.row, new_col);
            }
        }

        // Restore cursor if mode doesn't update it
        if !update_cursor {
            video.set_cursor(row as usize, col as usize);
        }
    }

    /// INT 10h, AH=01h - Set Cursor Shape
    /// Input:
    ///   CH = cursor start scan line (bits 0-4), cursor options (bits 5-6)
    ///   CL = cursor end scan line (bits 0-4)
    /// Output: None
    fn int10_set_cursor_shape(&mut self, memory: &mut Memory) {
        let start_line = (self.cx >> 8) as u8; // CH
        let end_line = (self.cx & 0xFF) as u8; // CL

        // Store cursor shape in BDA
        memory.write_u8(
            crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE,
            start_line,
        );
        memory.write_u8(
            crate::memory::BDA_START + crate::memory::BDA_CURSOR_END_LINE,
            end_line,
        );
    }

    /// INT 10h, AH=03h - Get Cursor Position and Shape
    /// Input:
    ///   BH = page number
    /// Output:
    ///   CH = cursor start scan line
    ///   CL = cursor end scan line
    ///   DH = row
    ///   DL = column
    fn int10_get_cursor_position(&mut self, memory: &Memory, video: &crate::video::Video) {
        let cursor = video.get_cursor();

        // Get cursor shape from BDA
        let start_line =
            memory.read_u8(crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE);
        let end_line =
            memory.read_u8(crate::memory::BDA_START + crate::memory::BDA_CURSOR_END_LINE);

        // Return cursor shape in CX
        self.cx = ((start_line as u16) << 8) | (end_line as u16);

        // Return cursor position in DX
        self.dx = ((cursor.row as u16) << 8) | (cursor.col as u16);
    }

    /// INT 10h, AH=05h - Select Active Display Page
    /// Input:
    ///   AL = new page number (0-7 for text modes)
    /// Output: None
    fn int10_select_active_page(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let page = (self.ax & 0xFF) as u8; // AL

        // Validate page number (0-7 for standard text modes)
        if page > 7 {
            log::warn!("INT 10h/AH=05h: Invalid page number: {}", page);
            return;
        }

        // Update active page in Video struct
        video.set_active_page(page);

        // Update BDA active page
        memory.write_u8(
            crate::memory::BDA_START + crate::memory::BDA_ACTIVE_PAGE,
            page,
        );

        // Update BDA video page offset (page_number * page_size)
        let page_size =
            memory.read_u16(crate::memory::BDA_START + crate::memory::BDA_VIDEO_PAGE_SIZE);
        let page_offset = (page as u16) * page_size;
        memory.write_u16(
            crate::memory::BDA_START + crate::memory::BDA_VIDEO_PAGE_OFFSET,
            page_offset,
        );

        log::trace!(
            "INT 10h/AH=05h: Selected active page {} (offset 0x{:04X})",
            page,
            page_offset
        );
    }

    /// INT 10h, AH=0Fh - Get Current Video Mode
    /// Input: None
    /// Output:
    ///   AH = number of screen columns
    ///   AL = video mode
    ///   BH = active display page
    fn int10_get_video_mode(&mut self, video: &crate::video::Video) {
        let mode = video.get_mode();
        let columns: u8 = video.get_cols() as u8;
        let page = video.get_active_page();

        self.ax = ((columns as u16) << 8) | (mode as u16);
        self.bx = (self.bx & 0x00FF) | ((page as u16) << 8);
    }

    /// INT 10h, AH=0Bh - Set Color Palette
    /// Subfunction in BH:
    ///   00h = Set background/border color
    ///        BL = color value (bits 0-3 = border color, bit 4 = background intensity)
    ///   01h = Set palette
    ///        BL = palette ID (0 = green/red/brown, 1 = cyan/magenta/white)
    fn int10_set_color_palette(&mut self) {
        let subfunction = (self.bx >> 8) as u8; // BH
        let value = (self.bx & 0xFF) as u8; // BL

        match subfunction {
            0x00 => {
                // Set background/border color
                // In text modes: bits 0-3 set border color
                // Bit 4 enables high-intensity background colors (instead of blinking)
                let border_color = value & 0x0F;
                let intensity = (value & 0x10) != 0;
                log::warn!(
                    "INT 10h/AH=0Bh/BH=00h: Set border color={}, intensity={}",
                    border_color,
                    intensity
                );
            }
            0x01 => {
                // Set palette for 320x200 4-color CGA graphics mode
                // BL = 0: palette 0 (green/red/brown)
                // BL = 1: palette 1 (cyan/magenta/white)
                let palette_id = value & 0x01;
                log::warn!("INT 10h/AH=0Bh/BH=01h: Set CGA palette={}", palette_id);
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=0Bh subfunction: BH=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// INT 10h, AH=0Ch - Write Graphics Pixel
    /// Input:
    ///   AL = pixel color value (0-3 for 320x200, 0-1 for 640x200)
    ///   BH = page number (0 for graphics modes, ignored)
    ///   CX = column (0-319 or 0-639)
    ///   DX = row (0-199)
    /// Output: None
    fn int10_write_pixel(&mut self, video: &mut crate::video::Video) {
        let color = (self.ax & 0xFF) as u8; // AL
        let col = self.cx as usize;
        let row = self.dx as usize;

        match video.get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                if col >= 320 || row >= 200 {
                    return;
                }
                // Calculate byte offset (4 pixels per byte)
                let pixels_per_byte = 4;
                let bytes_per_line = 320 / pixels_per_byte; // 80
                let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                let pixel_in_byte = col % pixels_per_byte;

                // Read-modify-write
                let mut byte_val = video.read_byte(byte_offset);
                let shift = 6 - (pixel_in_byte * 2); // MSB first
                byte_val &= !(0x03 << shift); // Clear 2 bits
                byte_val |= (color & 0x03) << shift; // Set 2 bits
                video.write_byte(byte_offset, byte_val);
            }
            crate::video::VideoMode::Graphics640x200 => {
                if col >= 640 || row >= 200 {
                    return;
                }
                // Calculate byte offset (8 pixels per byte)
                let pixels_per_byte = 8;
                let bytes_per_line = 640 / pixels_per_byte; // 80
                let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                let pixel_in_byte = col % pixels_per_byte;

                // Read-modify-write
                let mut byte_val = video.read_byte(byte_offset);
                let bit_mask = 0x80 >> pixel_in_byte; // MSB first
                if (color & 0x01) != 0 {
                    byte_val |= bit_mask; // Set bit
                } else {
                    byte_val &= !bit_mask; // Clear bit
                }
                video.write_byte(byte_offset, byte_val);
            }
            crate::video::VideoMode::Text { .. } => {
                // Ignore write pixel in text mode
            }
        }
    }

    /// INT 10h, AH=0Dh - Read Graphics Pixel
    /// Input:
    ///   BH = page number (0 for graphics modes, ignored)
    ///   CX = column (0-319 or 0-639)
    ///   DX = row (0-199)
    /// Output:
    ///   AL = pixel color value
    fn int10_read_pixel(&mut self, video: &crate::video::Video) {
        let col = self.cx as usize;
        let row = self.dx as usize;

        let color = match video.get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                if col >= 320 || row >= 200 {
                    0
                } else {
                    let pixels_per_byte = 4;
                    let bytes_per_line = 80;
                    let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                    let pixel_in_byte = col % pixels_per_byte;

                    let byte_val = video.read_byte(byte_offset);
                    let shift = 6 - (pixel_in_byte * 2);
                    (byte_val >> shift) & 0x03
                }
            }
            crate::video::VideoMode::Graphics640x200 => {
                if col >= 640 || row >= 200 {
                    0
                } else {
                    let pixels_per_byte = 8;
                    let bytes_per_line = 80;
                    let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                    let pixel_in_byte = col % pixels_per_byte;

                    let byte_val = video.read_byte(byte_offset);
                    let bit_mask = 0x80 >> pixel_in_byte;
                    if (byte_val & bit_mask) != 0 { 1 } else { 0 }
                }
            }
            crate::video::VideoMode::Text { .. } => 0,
        };

        self.ax = (self.ax & 0xFF00) | (color as u16);
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
    fn int10_palette_registers(&mut self) {
        let subfunction = (self.ax & 0xFF) as u8; // AL

        match subfunction {
            0x00 => {
                // Set individual palette register
                // BL = palette register number, BH = value
                log::warn!("INT 10h/AH=10h/AL=00h: Set palette register");
            }
            0x01 => {
                // Set border (overscan) color
                // BH = border color value
                log::warn!("INT 10h/AH=10h/AL=01h: Set border color");
            }
            0x02 => {
                // Set all palette registers and border
                // ES:DX -> 17-byte table
                log::warn!("INT 10h/AH=10h/AL=02h: Set all palette registers");
            }
            0x03 => {
                // Toggle intensity/blinking bit
                // BL = 0 enable intensity, 1 enable blinking
                log::warn!("INT 10h/AH=10h/AL=03h: Toggle blink/intensity");
            }
            0x10 => {
                // Set individual DAC register
                // BX = register number, CH = green, CL = blue, DH = red
                log::warn!("INT 10h/AH=10h/AL=10h: Set DAC register");
            }
            0x12 => {
                // Set block of DAC registers
                // BX = first register, CX = count, ES:DX -> table
                log::warn!("INT 10h/AH=10h/AL=12h: Set DAC register block");
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=10h palette subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
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
    fn int10_alternate_function_select(&mut self) {
        let subfunction = (self.bx & 0xFF) as u8; // BL

        match subfunction {
            0x10 => {
                // Get EGA info
                // Returns: BH = color/mono mode, BL = memory size, CH = feature bits, CL = switch setting
                self.bx = 0x0003; // BH=0 (color mode), BL=3 (256KB video memory)
                self.cx = 0x0000; // CH=0, CL=0
                log::warn!("INT 10h/AH=12h/BL=10h: Get EGA info");
            }
            0x30 => {
                // Select vertical resolution
                // AL = resolution (0=200, 1=350, 2=400)
                // Returns: AL = 12h if function supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::warn!("INT 10h/AH=12h/BL=30h: Select vertical resolution");
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
    fn int10_functionality_state_info(&mut self, memory: &mut Memory, video: &crate::video::Video) {
        let impl_type = self.bx;

        if impl_type != 0x0000 {
            // Only implementation type 0 is supported
            log::warn!(
                "INT 10h/AH=1Bh: Unsupported implementation type: 0x{:04X}",
                impl_type
            );
            return;
        }

        let buffer_addr = Self::physical_address(self.es, self.di);

        // Build the 64-byte state information structure
        // Offset 00h-03h: Pointer to static functionality table (we'll point to a dummy location)
        // For simplicity, we set this to 0 (null pointer)
        memory.write_u16(buffer_addr, 0x0000); // Offset
        memory.write_u16(buffer_addr + 2, 0x0000); // Segment

        // Offset 04h: Current video mode
        memory.write_u8(buffer_addr + 4, video.get_mode());

        // Offset 05h-06h: Number of columns
        let cols = video.get_cols();
        memory.write_u16(buffer_addr + 5, cols as u16);

        // Offset 07h-08h: Length of regen buffer (page size in bytes)
        // cols * rows * 2 bytes per cell (char + attr)
        let rows = video.get_rows();
        let buffer_size = cols * rows * 2;
        memory.write_u16(buffer_addr + 7, buffer_size as u16);

        // Offset 09h-0Ah: Starting address in regen buffer (current page offset)
        memory.write_u16(buffer_addr + 9, 0x0000);

        // Offset 0Bh-1Ah: Cursor positions for 8 pages (row, column pairs)
        let cursor = video.get_cursor();
        for page in 0..8 {
            let offset = buffer_addr + 0x0B + (page * 2);
            if page == 0 {
                memory.write_u8(offset, cursor.col as u8); // Column
                memory.write_u8(offset + 1, cursor.row as u8); // Row
            } else {
                memory.write_u8(offset, 0);
                memory.write_u8(offset + 1, 0);
            }
        }

        // Offset 1Bh-1Ch: Cursor type (start/end scan lines)
        let cursor_start =
            memory.read_u8(crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE);
        let cursor_end =
            memory.read_u8(crate::memory::BDA_START + crate::memory::BDA_CURSOR_END_LINE);
        memory.write_u8(buffer_addr + 0x1B, cursor_end);
        memory.write_u8(buffer_addr + 0x1C, cursor_start);

        // Offset 1Dh: Active display page
        memory.write_u8(buffer_addr + 0x1D, 0);

        // Offset 1Eh-1Fh: CRTC port address (3D4h for color, 3B4h for mono)
        memory.write_u16(buffer_addr + 0x1E, 0x03D4);

        // Offset 20h: Current setting of register 3x8h
        memory.write_u8(buffer_addr + 0x20, 0x00);

        // Offset 21h: Current setting of register 3x9h
        memory.write_u8(buffer_addr + 0x21, 0x00);

        // Offset 22h: Number of rows - 1
        memory.write_u8(buffer_addr + 0x22, (rows - 1) as u8);

        // Offset 23h-24h: Character height (scan lines per character)
        memory.write_u16(buffer_addr + 0x23, 16); // 16 scan lines for VGA

        // Offset 25h: Active display combination code
        memory.write_u8(buffer_addr + 0x25, 0x08); // VGA with color analog display

        // Offset 26h: Alternate display combination code
        memory.write_u8(buffer_addr + 0x26, 0x00); // No alternate display

        // Offset 27h-28h: Number of colors supported (0 = mono)
        memory.write_u16(buffer_addr + 0x27, 16); // 16 colors in text mode

        // Offset 29h: Number of pages supported
        memory.write_u8(buffer_addr + 0x29, 8);

        // Offset 2Ah: Number of scan lines active
        // 0 = 200, 1 = 350, 2 = 400
        memory.write_u8(buffer_addr + 0x2A, 2); // 400 scan lines

        // Offset 2Bh: Primary character block
        memory.write_u8(buffer_addr + 0x2B, 0);

        // Offset 2Ch: Secondary character block
        memory.write_u8(buffer_addr + 0x2C, 0);

        // Offset 2Dh: Miscellaneous state flags
        // Bit 0: All modes on all displays
        // Bit 1: Gray summing enabled
        // Bit 2: Monochrome display attached
        // Bit 3: Default palette loading disabled
        // Bit 4: Cursor emulation enabled
        // Bit 5: Blinking enabled
        // Bit 6-7: Reserved
        memory.write_u8(buffer_addr + 0x2D, 0x21); // All modes + blinking

        // Offset 2Eh-2Fh: Reserved
        memory.write_u8(buffer_addr + 0x2E, 0);
        memory.write_u8(buffer_addr + 0x2F, 0);

        // Offset 30h: Video memory available
        // 0 = 64KB, 1 = 128KB, 2 = 192KB, 3 = 256KB
        memory.write_u8(buffer_addr + 0x30, 3); // 256KB

        // Offset 31h: Save pointer state flags
        memory.write_u8(buffer_addr + 0x31, 0);

        // Offset 32h-3Fh: Reserved (fill with zeros)
        for i in 0x32..0x40 {
            memory.write_u8(buffer_addr + i, 0);
        }

        // Return AL = 1Bh to indicate function is supported
        self.ax = (self.ax & 0xFF00) | 0x1B;

        log::trace!(
            "INT 10h/AH=1Bh: Returned functionality/state info at {:05X}",
            buffer_addr
        );
    }

    /// Helper function for internal scrolling (used by teletype)
    fn scroll_up_internal(&mut self, video: &mut crate::video::Video, lines: u8) {
        // Save registers
        let saved_ax = self.ax;
        let saved_bx = self.bx;
        let saved_cx = self.cx;
        let saved_dx = self.dx;

        // Set up parameters for scroll_up
        let rows = video.get_rows() as u8;
        let cols = video.get_cols() as u8;
        self.ax = (self.ax & 0xFF00) | (lines as u16); // AL = lines
        self.bx = 0x0700; // BH = 0x07 (white on black)
        self.cx = 0x0000; // CH=0, CL=0 (top-left)
        self.dx = (((rows - 1) as u16) << 8) | ((cols - 1) as u16); // DH=rows-1, DL=cols-1 (bottom-right)

        self.int10_scroll_up(video);

        // Restore registers
        self.ax = saved_ax;
        self.bx = saved_bx;
        self.cx = saved_cx;
        self.dx = saved_dx;
    }

    /// INT 10h, AH=11h - Character Generator
    /// Subfunction in AL:
    ///   00h = Load user-specified character set
    ///   01h = Load ROM monochrome patterns (8x14)
    ///   02h = Load ROM 8x8 double-dot patterns
    ///   03h = Set block specifier
    ///   04h = Load ROM 8x16 character set
    ///   10h = Load user-specified character set and program mode
    ///   11h = Load ROM monochrome patterns (8x14) and program mode
    ///   12h = Load ROM 8x8 double-dot patterns and program mode
    ///   14h = Load ROM 8x16 character set and program mode
    ///   20h = Set user 8x8 graphics character table (INT 1Fh)
    ///   21h = Set user graphics character table
    ///   22h = Set ROM 8x14 graphics character table
    ///   23h = Set ROM 8x8 graphics character table
    ///   24h = Set ROM 8x16 graphics character table
    ///   30h = Get font information
    /// Input (for subfunction 30h):
    ///   BH = pointer type
    ///     00h = INT 1Fh pointer (8x8 graphics characters)
    ///     01h = INT 43h pointer (8x14/8x16 graphics characters)
    ///     02h = ROM 8x14 character font pointer
    ///     03h = ROM 8x8 double-dot font pointer
    ///     04h = ROM 8x8 double-dot font (top half)
    ///     05h = ROM 9x14 alphanumeric alternate
    ///     06h = ROM 8x16 font
    ///     07h = ROM 9x16 alternate
    /// Output (for subfunction 30h):
    ///   ES:BP = pointer to font
    ///   CX = bytes per character
    ///   DL = rows on screen - 1
    fn int10_character_generator(&mut self, memory: &mut Memory) {
        let subfunction = (self.ax & 0xFF) as u8; // AL

        match subfunction {
            0x00..=0x04 => {
                // Load character set functions
                log::warn!("INT 10h/AH=11h/AL={:02X}h: Load character set", subfunction);
            }
            0x10..=0x14 => {
                // Load character set and program mode
                log::warn!(
                    "INT 10h/AH=11h/AL={:02X}h: Load character set and program mode",
                    subfunction
                );
            }
            0x20..=0x24 => {
                // Set graphics character table
                log::warn!(
                    "INT 10h/AH=11h/AL={:02X}h: Set graphics character table",
                    subfunction
                );
            }
            0x30 => {
                // Get font information
                self.int10_get_font_info(memory);
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=11h subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// INT 10h, AH=11h, AL=30h - Get Font Information
    /// Input:
    ///   BH = pointer type
    /// Output:
    ///   ES:BP = pointer to font
    ///   CX = bytes per character
    ///   DL = rows on screen - 1
    fn int10_get_font_info(&mut self, memory: &Memory) {
        let pointer_type = (self.bx >> 8) as u8; // BH

        // Determine which font to return based on pointer type
        let (segment, offset, bytes_per_char, rows) = match pointer_type {
            0x00 => {
                // INT 1Fh pointer (8x8 graphics characters)
                // Read INT 1Fh vector from IVT
                let int_1f_offset = memory.read_u16(0x1F * 4);
                let int_1f_segment = memory.read_u16(0x1F * 4 + 2);
                // If not set, default to our ROM 8x8 font
                if int_1f_segment == 0xF000 && int_1f_offset < 0x100 {
                    // Not initialized, use ROM font
                    (crate::memory::FONT_8X8_SEGMENT, crate::memory::FONT_8X8_OFFSET, 8, 25)
                } else {
                    (int_1f_segment, int_1f_offset, 8, 25)
                }
            }
            0x01 => {
                // INT 43h pointer (8x14/8x16 graphics characters)
                // Read INT 43h vector from IVT
                let int_43_offset = memory.read_u16(0x43 * 4);
                let int_43_segment = memory.read_u16(0x43 * 4 + 2);
                // If not set, default to our ROM 8x16 font
                if int_43_segment == 0xF000 && int_43_offset < 0x100 {
                    // Not initialized, use ROM font
                    (crate::memory::FONT_8X16_SEGMENT, crate::memory::FONT_8X16_OFFSET, 16, 25)
                } else {
                    (int_43_segment, int_43_offset, 16, 25)
                }
            }
            0x02 => {
                // ROM 8x14 character font pointer
                // We don't have a real 8x14 font, return 8x16 instead
                (crate::memory::FONT_8X16_SEGMENT, crate::memory::FONT_8X16_OFFSET, 16, 25)
            }
            0x03 | 0x04 => {
                // ROM 8x8 double-dot font pointer (both regular and top half)
                (crate::memory::FONT_8X8_SEGMENT, crate::memory::FONT_8X8_OFFSET, 8, 25)
            }
            0x05 => {
                // ROM 9x14 alphanumeric alternate
                // We don't have a 9x14 font, return 8x16 instead
                (crate::memory::FONT_8X16_SEGMENT, crate::memory::FONT_8X16_OFFSET, 16, 25)
            }
            0x06 => {
                // ROM 8x16 font
                (crate::memory::FONT_8X16_SEGMENT, crate::memory::FONT_8X16_OFFSET, 16, 25)
            }
            0x07 => {
                // ROM 9x16 alternate
                // We don't have a 9x16 font, return 8x16 instead
                (crate::memory::FONT_8X16_SEGMENT, crate::memory::FONT_8X16_OFFSET, 16, 25)
            }
            _ => {
                // Unknown pointer type, default to 8x16
                log::warn!(
                    "INT 10h/AH=11h/AL=30h: Unknown pointer type BH=0x{:02X}, defaulting to 8x16",
                    pointer_type
                );
                (crate::memory::FONT_8X16_SEGMENT, crate::memory::FONT_8X16_OFFSET, 16, 25)
            }
        };

        // Return font pointer in ES:BP
        self.es = segment;
        self.bp = offset;

        // Return bytes per character in CX
        self.cx = bytes_per_char;

        // Return rows - 1 in DL
        self.dx = (self.dx & 0xFF00) | ((rows - 1) as u16);

        log::debug!(
            "INT 10h/AH=11h/AL=30h: Get font info BH={:02X}h -> ES:BP={:04X}:{:04X}, CX={}, DL={}",
            pointer_type,
            self.es,
            self.bp,
            self.cx,
            rows - 1
        );
    }

    /// INT 10h, AH=15h - Return Physical Display Parameters (VGA)
    /// Input: None
    /// Output:
    ///   AL = 15h if function supported
    ///   BH = active display code
    ///   BL = alternate display code
    fn int10_return_physical_display_params(&mut self) {
        // Active display code (08h = VGA with color display)
        let active_display = 0x08;
        // Alternate display code (00h = no alternate display)
        let alternate_display = 0x00;

        // Return AL = 15h to indicate function is supported
        self.ax = (self.ax & 0xFF00) | 0x15;

        // Return display codes in BH/BL
        self.bx = ((active_display as u16) << 8) | (alternate_display as u16);

        log::debug!(
            "INT 10h/AH=15h: Return physical display params (active={:02X}h, alt={:02X}h)",
            active_display,
            alternate_display
        );
    }

    /// INT 10h, AH=1Ah - Display Combination Code (VGA/MCGA)
    /// Subfunction in AL:
    ///   00h = Read display combination code
    ///   01h = Write display combination code
    /// Input (for AL=01h):
    ///   BL = active display code
    ///   BH = alternate display code
    /// Output (for AL=00h):
    ///   AL = 1Ah if function supported
    ///   BL = active display code
    ///   BH = alternate display code
    ///
    /// Display codes:
    ///   00h = no display
    ///   01h = MDA with monochrome display
    ///   02h = CGA with color display
    ///   04h = EGA with color display
    ///   05h = EGA with monochrome display
    ///   07h = VGA with monochrome analog display
    ///   08h = VGA with color analog display
    ///   0Bh = MCGA with color digital display
    ///   0Ch = MCGA with monochrome analog display
    fn int10_display_combination_code(&mut self, memory: &mut Memory) {
        let subfunction = (self.ax & 0xFF) as u8; // AL

        // Use BDA location to store display combination (not standard, but convenient)
        const DISPLAY_CODE_ADDR: usize = crate::memory::BDA_START + 0x8A;

        match subfunction {
            0x00 => {
                // Read display combination code
                let code = memory.read_u16(DISPLAY_CODE_ADDR);
                let active_display = (code & 0xFF) as u8;
                let alternate_display = (code >> 8) as u8;

                // If not initialized, return default (VGA with color)
                let (active, alternate) = if code == 0 {
                    (0x08, 0x00)
                } else {
                    (active_display, alternate_display)
                };

                // Return AL = 1Ah to indicate function is supported
                self.ax = (self.ax & 0xFF00) | 0x1A;

                // Return display codes in BL/BH
                self.bx = (alternate as u16) << 8 | active as u16;

                log::debug!(
                    "INT 10h/AH=1Ah/AL=00h: Read display combination (active={:02X}h, alt={:02X}h)",
                    active,
                    alternate
                );
            }
            0x01 => {
                // Write display combination code
                let active_display = (self.bx & 0xFF) as u8; // BL
                let alternate_display = (self.bx >> 8) as u8; // BH

                // Store in BDA
                let code = (alternate_display as u16) << 8 | active_display as u16;
                memory.write_u16(DISPLAY_CODE_ADDR, code);

                // Return AL = 1Ah to indicate function is supported
                self.ax = (self.ax & 0xFF00) | 0x1A;

                log::debug!(
                    "INT 10h/AH=1Ah/AL=01h: Write display combination (active={:02X}h, alt={:02X}h)",
                    active_display,
                    alternate_display
                );
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=1Ah subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
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

    /// INT 10h, AH=FEh - Get Video Buffer (TopView/DESQview/DOSSHELL)
    /// Input: None
    /// Output:
    ///   ES:DI = segment:offset of video buffer for current page
    ///
    /// This function returns a pointer to the video buffer that applications
    /// can write to directly for better performance. For standard text mode,
    /// this is typically B800:0000 for color displays or B000:0000 for mono.
    fn int10_get_video_buffer(&mut self, _memory: &mut Memory) {
        // For color text mode, video buffer is at B800:0000
        // For monochrome text mode, it's at B000:0000
        // We'll assume color mode (most common)
        let video_segment = 0xB800;
        let video_offset = 0x0000;

        // Return pointer in ES:DI
        self.es = video_segment;
        self.di = video_offset;

        log::debug!(
            "INT 10h/AH=FEh: Get video buffer -> ES:DI={:04X}:{:04X}",
            self.es,
            self.di
        );
    }
}
