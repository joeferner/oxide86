use crate::{cpu::Cpu, memory::Memory};

impl Cpu {
    /// INT 0x10 - Video Services
    /// AH register contains the function number
    pub(super) fn handle_int10(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int10_set_video_mode(video),
            0x01 => self.int10_set_cursor_shape(memory),
            0x02 => self.int10_set_cursor_position(video),
            0x03 => self.int10_get_cursor_position(memory, video),
            0x05 => self.int10_select_active_page(memory, video),
            0x06 => self.int10_scroll_up(video),
            0x07 => self.int10_scroll_down(video),
            0x08 => self.int10_read_char_attr(video),
            0x09 => self.int10_write_char_attr(memory, video),
            0x0E => self.int10_teletype_output(video),
            0x0B => self.int10_set_color_palette(),
            0x0F => self.int10_get_video_mode(video),
            0x10 => self.int10_palette_registers(),
            0x12 => self.int10_alternate_function_select(),
            0x13 => self.int10_write_string(memory, video),
            0x1B => self.int10_functionality_state_info(memory, video),
            _ => {
                log::warn!("Unhandled INT 0x10 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 10h, AH=00h - Set Video Mode
    /// Input:
    ///   AL = video mode (0x00-0x03, 0x07 for text modes)
    /// Output: None
    fn int10_set_video_mode(&mut self, video: &mut crate::video::Video) {
        let mode = (self.ax & 0xFF) as u8; // AL

        // We only support text modes (0x00-0x03, 0x07)
        if mode <= 0x07 {
            video.set_mode(mode);
            // Reset cursor to top-left
            video.set_cursor(0, 0);
        } else {
            log::warn!("Unsupported video mode: 0x{:02X}", mode);
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

        if row < 25 && col < 80 {
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

        // Validate bounds
        if top > bottom || left > right || bottom >= 25 || right >= 80 {
            return;
        }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * 80 + col as usize) * 2;
                    video.write_byte(offset, b' ');
                    video.write_byte(offset + 1, attr);
                }
            }
        } else {
            // Scroll up by 'lines' rows
            for row in top..=bottom {
                for col in left..=right {
                    let dest_offset = (row as usize * 80 + col as usize) * 2;
                    let src_row = row + lines;

                    if src_row <= bottom {
                        // Copy from below - read from video buffer, not memory
                        let src_offset = (src_row as usize * 80 + col as usize) * 2;
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

        // Validate bounds
        if top > bottom || left > right || bottom >= 25 || right >= 80 {
            return;
        }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * 80 + col as usize) * 2;
                    video.write_byte(offset, b' ');
                    video.write_byte(offset + 1, attr);
                }
            }
        } else {
            // Scroll down by 'lines' rows (process bottom to top)
            for row in (top..=bottom).rev() {
                for col in left..=right {
                    let dest_offset = (row as usize * 80 + col as usize) * 2;

                    if row >= top + lines {
                        // Copy from above - read from video buffer, not memory
                        let src_row = row - lines;
                        let src_offset = (src_row as usize * 80 + col as usize) * 2;
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
        let offset = (cursor.row * 80 + cursor.col) * 2;

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

        for i in 0..count {
            let pos = cursor.row * 80 + cursor.col + (i as usize);
            if pos >= 80 * 25 {
                break; // Don't write beyond screen
            }
            let offset = pos * 2;
            video.write_byte(offset, ch);
            video.write_byte(offset + 1, attr);
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

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                video.set_cursor(cursor.row, 0);
            }
            b'\n' => {
                // Line feed - move to next line
                let new_row = if cursor.row >= 24 {
                    // Need to scroll
                    self.scroll_up_internal(video, 1);
                    24
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
                // Normal character - write and advance
                let offset = (cursor.row * 80 + cursor.col) * 2;
                video.write_byte(offset, ch);
                // Don't modify attribute byte (preserve existing color)

                // Advance cursor
                let new_col = cursor.col + 1;
                if new_col >= 80 {
                    // Wrap to next line
                    let new_row = if cursor.row >= 24 {
                        self.scroll_up_internal(video, 1);
                        24
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

        let mut addr = Self::physical_address(self.es, self.bp);

        for _ in 0..length {
            let ch = memory.read_byte(addr);
            addr += 1;

            let current_attr = if has_attributes {
                let a = memory.read_byte(addr);
                addr += 1;
                a
            } else {
                attr
            };

            let cursor = video.get_cursor();
            if cursor.row >= 25 {
                break;
            }

            let offset = (cursor.row * 80 + cursor.col) * 2;
            video.write_byte(offset, ch);
            video.write_byte(offset + 1, current_attr);

            // Advance cursor (even if not updating final position)
            let new_col = cursor.col + 1;
            if new_col >= 80 {
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
        memory.write_byte(
            crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE,
            start_line,
        );
        memory.write_byte(
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
            memory.read_byte(crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE);
        let end_line =
            memory.read_byte(crate::memory::BDA_START + crate::memory::BDA_CURSOR_END_LINE);

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
        memory.write_byte(
            crate::memory::BDA_START + crate::memory::BDA_ACTIVE_PAGE,
            page,
        );

        // Update BDA video page offset (page_number * page_size)
        let page_size =
            memory.read_word(crate::memory::BDA_START + crate::memory::BDA_VIDEO_PAGE_SIZE);
        let page_offset = (page as u16) * page_size;
        memory.write_word(
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
        let columns: u8 = 80; // Standard 80-column mode
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
                log::debug!(
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
                log::debug!("INT 10h/AH=0Bh/BH=01h: Set CGA palette={}", palette_id);
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=0Bh subfunction: BH=0x{:02X}",
                    subfunction
                );
            }
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
    fn int10_palette_registers(&mut self) {
        let subfunction = (self.ax & 0xFF) as u8; // AL

        match subfunction {
            0x00 => {
                // Set individual palette register
                // BL = palette register number, BH = value
                log::debug!("INT 10h/AH=10h/AL=00h: Set palette register");
            }
            0x01 => {
                // Set border (overscan) color
                // BH = border color value
                log::debug!("INT 10h/AH=10h/AL=01h: Set border color");
            }
            0x02 => {
                // Set all palette registers and border
                // ES:DX -> 17-byte table
                log::debug!("INT 10h/AH=10h/AL=02h: Set all palette registers");
            }
            0x03 => {
                // Toggle intensity/blinking bit
                // BL = 0 enable intensity, 1 enable blinking
                log::debug!("INT 10h/AH=10h/AL=03h: Toggle blink/intensity");
            }
            0x10 => {
                // Set individual DAC register
                // BX = register number, CH = green, CL = blue, DH = red
                log::debug!("INT 10h/AH=10h/AL=10h: Set DAC register");
            }
            0x12 => {
                // Set block of DAC registers
                // BX = first register, CX = count, ES:DX -> table
                log::debug!("INT 10h/AH=10h/AL=12h: Set DAC register block");
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
                log::debug!("INT 10h/AH=12h/BL=10h: Get EGA info");
            }
            0x30 => {
                // Select vertical resolution
                // AL = resolution (0=200, 1=350, 2=400)
                // Returns: AL = 12h if function supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::debug!("INT 10h/AH=12h/BL=30h: Select vertical resolution");
            }
            0x31 => {
                // Palette loading (enable/disable default palette loading)
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::debug!("INT 10h/AH=12h/BL=31h: Palette loading control");
            }
            0x32 => {
                // Video enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::debug!("INT 10h/AH=12h/BL=32h: Video enable/disable");
            }
            0x33 => {
                // Gray-scale summing enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::debug!("INT 10h/AH=12h/BL=33h: Gray-scale summing");
            }
            0x34 => {
                // Cursor emulation enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::debug!("INT 10h/AH=12h/BL=34h: Cursor emulation");
            }
            0x35 => {
                // Display switch interface
                // AL = 0 initial switch, 80h adapter off, FF disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::debug!("INT 10h/AH=12h/BL=35h: Display switch");
            }
            0x36 => {
                // Video refresh control
                // AL = 0 enable refresh, 1 disable refresh
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::debug!("INT 10h/AH=12h/BL=36h: Video refresh control");
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
        memory.write_word(buffer_addr, 0x0000); // Offset
        memory.write_word(buffer_addr + 2, 0x0000); // Segment

        // Offset 04h: Current video mode
        memory.write_byte(buffer_addr + 4, video.get_mode());

        // Offset 05h-06h: Number of columns
        memory.write_word(buffer_addr + 5, 80);

        // Offset 07h-08h: Length of regen buffer (page size in bytes)
        // For 80x25 text mode: 80 * 25 * 2 = 4000 bytes
        memory.write_word(buffer_addr + 7, 4000);

        // Offset 09h-0Ah: Starting address in regen buffer (current page offset)
        memory.write_word(buffer_addr + 9, 0x0000);

        // Offset 0Bh-1Ah: Cursor positions for 8 pages (row, column pairs)
        let cursor = video.get_cursor();
        for page in 0..8 {
            let offset = buffer_addr + 0x0B + (page * 2);
            if page == 0 {
                memory.write_byte(offset, cursor.col as u8); // Column
                memory.write_byte(offset + 1, cursor.row as u8); // Row
            } else {
                memory.write_byte(offset, 0);
                memory.write_byte(offset + 1, 0);
            }
        }

        // Offset 1Bh-1Ch: Cursor type (start/end scan lines)
        let cursor_start =
            memory.read_byte(crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE);
        let cursor_end =
            memory.read_byte(crate::memory::BDA_START + crate::memory::BDA_CURSOR_END_LINE);
        memory.write_byte(buffer_addr + 0x1B, cursor_end);
        memory.write_byte(buffer_addr + 0x1C, cursor_start);

        // Offset 1Dh: Active display page
        memory.write_byte(buffer_addr + 0x1D, 0);

        // Offset 1Eh-1Fh: CRTC port address (3D4h for color, 3B4h for mono)
        memory.write_word(buffer_addr + 0x1E, 0x03D4);

        // Offset 20h: Current setting of register 3x8h
        memory.write_byte(buffer_addr + 0x20, 0x00);

        // Offset 21h: Current setting of register 3x9h
        memory.write_byte(buffer_addr + 0x21, 0x00);

        // Offset 22h: Number of rows - 1
        memory.write_byte(buffer_addr + 0x22, 24); // 25 rows - 1

        // Offset 23h-24h: Character height (scan lines per character)
        memory.write_word(buffer_addr + 0x23, 16); // 16 scan lines for VGA

        // Offset 25h: Active display combination code
        memory.write_byte(buffer_addr + 0x25, 0x08); // VGA with color analog display

        // Offset 26h: Alternate display combination code
        memory.write_byte(buffer_addr + 0x26, 0x00); // No alternate display

        // Offset 27h-28h: Number of colors supported (0 = mono)
        memory.write_word(buffer_addr + 0x27, 16); // 16 colors in text mode

        // Offset 29h: Number of pages supported
        memory.write_byte(buffer_addr + 0x29, 8);

        // Offset 2Ah: Number of scan lines active
        // 0 = 200, 1 = 350, 2 = 400
        memory.write_byte(buffer_addr + 0x2A, 2); // 400 scan lines

        // Offset 2Bh: Primary character block
        memory.write_byte(buffer_addr + 0x2B, 0);

        // Offset 2Ch: Secondary character block
        memory.write_byte(buffer_addr + 0x2C, 0);

        // Offset 2Dh: Miscellaneous state flags
        // Bit 0: All modes on all displays
        // Bit 1: Gray summing enabled
        // Bit 2: Monochrome display attached
        // Bit 3: Default palette loading disabled
        // Bit 4: Cursor emulation enabled
        // Bit 5: Blinking enabled
        // Bit 6-7: Reserved
        memory.write_byte(buffer_addr + 0x2D, 0x21); // All modes + blinking

        // Offset 2Eh-2Fh: Reserved
        memory.write_byte(buffer_addr + 0x2E, 0);
        memory.write_byte(buffer_addr + 0x2F, 0);

        // Offset 30h: Video memory available
        // 0 = 64KB, 1 = 128KB, 2 = 192KB, 3 = 256KB
        memory.write_byte(buffer_addr + 0x30, 3); // 256KB

        // Offset 31h: Save pointer state flags
        memory.write_byte(buffer_addr + 0x31, 0);

        // Offset 32h-3Fh: Reserved (fill with zeros)
        for i in 0x32..0x40 {
            memory.write_byte(buffer_addr + i, 0);
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
        self.ax = (self.ax & 0xFF00) | (lines as u16); // AL = lines
        self.bx = 0x0700; // BH = 0x07 (white on black)
        self.cx = 0x0000; // CH=0, CL=0 (top-left)
        self.dx = 0x184F; // DH=24, DL=79 (bottom-right)

        self.int10_scroll_up(video);

        // Restore registers
        self.ax = saved_ax;
        self.bx = saved_bx;
        self.cx = saved_cx;
        self.dx = saved_dx;
    }
}
