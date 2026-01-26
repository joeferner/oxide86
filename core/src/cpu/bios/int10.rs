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
            0x06 => self.int10_scroll_up(video),
            0x07 => self.int10_scroll_down(video),
            0x08 => self.int10_read_char_attr(video),
            0x09 => self.int10_write_char_attr(memory, video),
            0x0E => self.int10_teletype_output(video),
            0x0F => self.int10_get_video_mode(video),
            0x10 => self.int10_palette_registers(),
            0x12 => self.int10_alternate_function_select(),
            0x13 => self.int10_write_string(memory, video),
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

    /// INT 10h, AH=0Fh - Get Current Video Mode
    /// Input: None
    /// Output:
    ///   AH = number of screen columns
    ///   AL = video mode
    ///   BH = active display page
    fn int10_get_video_mode(&mut self, video: &crate::video::Video) {
        let mode = video.get_mode();
        let columns: u8 = 80; // Standard 80-column mode
        let page: u8 = 0; // Active page 0

        self.ax = ((columns as u16) << 8) | (mode as u16);
        self.bx = (self.bx & 0x00FF) | ((page as u16) << 8);
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
                log::trace!("INT 10h/AH=10h/AL=00h: Set palette register");
            }
            0x01 => {
                // Set border (overscan) color
                // BH = border color value
                log::trace!("INT 10h/AH=10h/AL=01h: Set border color");
            }
            0x02 => {
                // Set all palette registers and border
                // ES:DX -> 17-byte table
                log::trace!("INT 10h/AH=10h/AL=02h: Set all palette registers");
            }
            0x03 => {
                // Toggle intensity/blinking bit
                // BL = 0 enable intensity, 1 enable blinking
                log::trace!("INT 10h/AH=10h/AL=03h: Toggle blink/intensity");
            }
            0x10 => {
                // Set individual DAC register
                // BX = register number, CH = green, CL = blue, DH = red
                log::trace!("INT 10h/AH=10h/AL=10h: Set DAC register");
            }
            0x12 => {
                // Set block of DAC registers
                // BX = first register, CX = count, ES:DX -> table
                log::trace!("INT 10h/AH=10h/AL=12h: Set DAC register block");
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
                log::trace!("INT 10h/AH=12h/BL=10h: Get EGA info");
            }
            0x30 => {
                // Select vertical resolution
                // AL = resolution (0=200, 1=350, 2=400)
                // Returns: AL = 12h if function supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::trace!("INT 10h/AH=12h/BL=30h: Select vertical resolution");
            }
            0x31 => {
                // Palette loading (enable/disable default palette loading)
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::trace!("INT 10h/AH=12h/BL=31h: Palette loading control");
            }
            0x32 => {
                // Video enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::trace!("INT 10h/AH=12h/BL=32h: Video enable/disable");
            }
            0x33 => {
                // Gray-scale summing enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::trace!("INT 10h/AH=12h/BL=33h: Gray-scale summing");
            }
            0x34 => {
                // Cursor emulation enable/disable
                // AL = 0 enable, 1 disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::trace!("INT 10h/AH=12h/BL=34h: Cursor emulation");
            }
            0x35 => {
                // Display switch interface
                // AL = 0 initial switch, 80h adapter off, FF disable
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::trace!("INT 10h/AH=12h/BL=35h: Display switch");
            }
            0x36 => {
                // Video refresh control
                // AL = 0 enable refresh, 1 disable refresh
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::trace!("INT 10h/AH=12h/BL=36h: Video refresh control");
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=12h alternate function: BL=0x{:02X}",
                    subfunction
                );
            }
        }
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
