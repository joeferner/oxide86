use crate::Bus;
use crate::cpu::Cpu;

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
    /// INT 0x10 - Video Services
    /// AH register contains the function number
    pub(super) fn handle_int10(&mut self, bus: &mut Bus) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int10_set_video_mode(bus),
            0x01 => self.int10_set_cursor_shape(bus),
            0x02 => self.int10_set_cursor_position(bus),
            0x03 => self.int10_get_cursor_position(bus),
            0x05 => self.int10_select_active_page(bus),
            0x06 => self.int10_scroll_up(bus),
            0x07 => self.int10_scroll_down(bus),
            0x08 => self.int10_read_char_attr(bus),
            0x09 => self.int10_write_char_attr(bus),
            0x0A => self.int10_write_char(bus),
            0x0B => self.int10_set_color_palette(bus),
            0x0C => self.int10_write_pixel(bus),
            0x0D => self.int10_read_pixel(bus),
            0x0E => self.int10_teletype_output(bus),
            0x0F => self.int10_get_video_mode(bus),
            0x10 => self.int10_palette_registers(bus),
            0x11 => self.int10_character_generator(bus),
            0x12 => self.int10_alternate_function_select(),
            0x13 => self.int10_write_string(bus),
            0x15 => self.int10_return_physical_display_params(),
            0x1A => self.int10_display_combination_code(bus),
            0x1B => self.int10_functionality_state_info(bus),
            0xFA => self.int10_installation_checks(),
            0xFE => self.int10_get_video_buffer(bus),
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

        // Check if the requested mode is supported by the video card type
        if !bus.video().supports_mode(mode) {
            log::warn!(
                "INT 10h AH=00h: Video mode 0x{:02X} not supported by {} card - ignoring",
                mode,
                bus.video().card_type()
            );
            return;
        }

        // Support text modes (0x00-0x03, 0x07), CGA graphics (0x04-0x06), EGA graphics (0x0D), VGA (0x13)
        match mode {
            0x00..=0x07 | 0x0D | 0x13 => {
                // INT 10h mode set = RGB rendering; composite only via port 0x3D8
                bus.video_mut().set_composite_mode(false);
                bus.video_mut().set_mode(mode, false); // INT 10h clears video memory (real BIOS behavior)
                // Reset cursor to top-left (only relevant for text modes)
                bus.video_mut().set_cursor(0, 0);

                // Update BDA with new video mode settings
                bus.write_u8(
                    crate::memory::BDA_START + crate::memory::BDA_VIDEO_MODE,
                    mode,
                );

                let cols = bus.video().get_cols();
                let rows = bus.video().get_rows();
                bus.write_u16(
                    crate::memory::BDA_START + crate::memory::BDA_SCREEN_COLUMNS,
                    cols as u16,
                );

                // Page size = cols * rows * 2 (char + attr)
                let page_size = cols * rows * 2;
                bus.write_u16(
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
    fn int10_set_cursor_position(&mut self, bus: &mut Bus) {
        let row = (self.dx >> 8) as u8; // DH
        let col = (self.dx & 0xFF) as u8; // DL
        let page = (self.bx >> 8) as u8; // BH

        let cols = bus.video().get_cols();
        let rows = bus.video().get_rows();

        log::debug!(
            "INT 10h AH=02h: Set cursor to row={}, col={}, page={}",
            row,
            col,
            page
        );

        if (row as usize) < rows && (col as usize) < cols {
            bus.video_mut().set_cursor(row as usize, col as usize);
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
    fn int10_scroll_up(&mut self, bus: &mut Bus) {
        let lines = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx >> 8) as u8; // BH
        let top = (self.cx >> 8) as u8; // CH
        let left = (self.cx & 0xFF) as u8; // CL
        let bottom = (self.dx >> 8) as u8; // DH
        let right = (self.dx & 0xFF) as u8; // DL

        let cols = bus.video().get_cols();
        let rows = bus.video().get_rows();

        if lines == 0 {
            log::info!(
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

        // Clamp to valid range (real BIOS behavior: clip out-of-range coords)
        let right = right.min((cols - 1) as u8);
        let bottom = bottom.min((rows - 1) as u8);
        if top > bottom || left > right {
            return;
        }

        if bus.video().is_graphics_mode() {
            bus.video_mut()
                .scroll_up_window(lines, top, left, bottom, right);
            return;
        }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * cols + col as usize) * 2;
                    bus.video_mut().write_byte(offset, b' ');
                    bus.video_mut().write_byte(offset + 1, attr);
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
                        let ch = bus.video().read_byte(src_offset);
                        let at = bus.video().read_byte(src_offset + 1);
                        bus.video_mut().write_byte(dest_offset, ch);
                        bus.video_mut().write_byte(dest_offset + 1, at);
                    } else {
                        // Fill with blanks
                        bus.video_mut().write_byte(dest_offset, b' ');
                        bus.video_mut().write_byte(dest_offset + 1, attr);
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
    fn int10_scroll_down(&mut self, bus: &mut Bus) {
        let lines = (self.ax & 0xFF) as u8; // AL
        let attr = (self.bx >> 8) as u8; // BH
        let top = (self.cx >> 8) as u8; // CH
        let left = (self.cx & 0xFF) as u8; // CL
        let bottom = (self.dx >> 8) as u8; // DH
        let right = (self.dx & 0xFF) as u8; // DL

        let cols = bus.video().get_cols();
        let rows = bus.video().get_rows();

        log::debug!(
            "INT 10h AH=07h: Scroll down lines={}, attr=0x{:02X}, window=({},{}) to ({},{})",
            lines,
            attr,
            top,
            left,
            bottom,
            right
        );

        // Clamp to valid range (real BIOS behavior: clip out-of-range coords)
        let right = right.min((cols - 1) as u8);
        let bottom = bottom.min((rows - 1) as u8);
        if top > bottom || left > right {
            return;
        }

        if bus.video().is_graphics_mode() {
            bus.video_mut()
                .scroll_down_window(lines, top, left, bottom, right);
            return;
        }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * cols + col as usize) * 2;
                    bus.video_mut().write_byte(offset, b' ');
                    bus.video_mut().write_byte(offset + 1, attr);
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
                        let ch = bus.video().read_byte(src_offset);
                        let at = bus.video().read_byte(src_offset + 1);
                        bus.video_mut().write_byte(dest_offset, ch);
                        bus.video_mut().write_byte(dest_offset + 1, at);
                    } else {
                        // Fill with blanks
                        bus.video_mut().write_byte(dest_offset, b' ');
                        bus.video_mut().write_byte(dest_offset + 1, attr);
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
    fn int10_read_char_attr(&mut self, bus: &mut Bus) {
        let cursor = bus.video().get_cursor();
        let cols = bus.video().get_cols();
        let offset = (cursor.row * cols + cursor.col) * 2;

        let ch = bus.video().read_byte(offset);
        let attr = bus.video().read_byte(offset + 1);

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
        let cursor = bus.video().get_cursor();
        let cols = bus.video().get_cols();
        let rows = bus.video().get_rows();

        if bus.video().is_graphics_mode() {
            // Graphics mode: draw character pixel-by-pixel
            // IBM CGA BIOS behavior:
            // - BL bit 7 = 1: XOR mode
            // - BL bit 7 = 0: Normal mode
            // - When BOTH char bit 7 AND attr bit 7 are set: invert glyph for inverse effect
            let fg_color = attr & 0x0F; // Lower 4 bits = color index
            let xor_mode = (attr & 0x80) != 0; // Bit 7 = XOR mode
            let invert_glyph = (ch & 0x80) != 0 && xor_mode; // Invert if both bits set

            log::debug!(
                "INT 10h AH=09h: char=0x{:02X} attr=0x{:02X} (xor={} invert={}) fg={} count={} at ({},{})",
                ch,
                attr,
                xor_mode,
                invert_glyph,
                fg_color,
                count,
                cursor.row,
                cursor.col
            );

            for i in 0..count {
                let col = cursor.col + (i as usize) % cols;
                let row = cursor.row + (i as usize) / cols;
                if row >= rows {
                    break;
                }
                // Determine draw mode based on attribute and character bits
                let draw_mode = if invert_glyph {
                    GraphicsDrawMode::XorInverted
                } else if xor_mode {
                    GraphicsDrawMode::Xor
                } else {
                    GraphicsDrawMode::Opaque
                };
                self.draw_char_graphics(bus, ch, row, col, fg_color, draw_mode);
            }
        } else {
            // Text mode: write to video memory
            for i in 0..count {
                let pos = cursor.row * cols + cursor.col + (i as usize);
                if pos >= cols * rows {
                    break; // Don't write beyond screen
                }
                let offset = pos * 2;
                bus.video_mut().write_byte(offset, ch);
                bus.video_mut().write_byte(offset + 1, attr);
            }
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
        let cursor = bus.video().get_cursor();
        let cols = bus.video().get_cols();
        let rows = bus.video().get_rows();

        if bus.video().is_graphics_mode() {
            // Graphics mode: draw character pixel-by-pixel
            // BL contains foreground color
            let fg_color = (self.bx & 0xFF) as u8; // BL

            for i in 0..count {
                let col = cursor.col + (i as usize) % cols;
                let row = cursor.row + (i as usize) / cols;
                if row >= rows {
                    break;
                }
                // AH=0Ah draws transparent characters - no background, no XOR
                self.draw_char_graphics(bus, ch, row, col, fg_color, GraphicsDrawMode::Transparent);
            }
        } else {
            // Text mode: write to video memory
            for i in 0..count {
                let pos = cursor.row * cols + cursor.col + (i as usize);
                if pos >= cols * rows {
                    break; // Don't write beyond screen
                }
                let offset = pos * 2;
                bus.video_mut().write_byte(offset, ch);
                // Don't modify attribute byte (offset + 1) - preserve existing color
            }
        }
        // Cursor position is NOT updated by this function
    }

    /// INT 10h, AH=0Eh - Teletype Output
    /// Input:
    ///   AL = character to write
    ///   BL = foreground color (in graphics modes)
    ///   BH = page number (0 for text mode)
    /// Output: None
    pub(crate) fn int10_teletype_output(&mut self, bus: &mut Bus) {
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
                    log::trace!(
                        "INT 10h AH=0Eh: Writing '{}' (0x{:02X}) at ({},{}) - existing attr=0x{:02X} (fg={}, bg={})",
                        if (32..127).contains(&ch) {
                            ch as char
                        } else {
                            '.'
                        },
                        ch,
                        cursor.row,
                        cursor.col,
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

    /// Draw a character in graphics mode using font data
    fn draw_char_graphics(
        &self,
        bus: &mut Bus,
        character: u8,
        row: usize,
        col: usize,
        fg_color: u8,
        mode: GraphicsDrawMode,
    ) {
        use crate::font::Cp437Font;

        let font = Cp437Font::new();

        match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 | crate::video::VideoMode::Graphics640x200 => {
                // CGA graphics mode: use native 8x8 CGA font
                // In XorInverted mode, strip bit 7 from character to get base glyph
                // (e.g., 0x80 -> 0x00, then invert to get solid block)
                let char_code = if matches!(mode, GraphicsDrawMode::XorInverted) {
                    character & 0x7F
                } else {
                    character
                };
                let glyph = font.get_glyph_8(char_code);
                let char_height = 8;
                let char_width = 8;

                // Get screen dimensions based on mode
                let screen_width = match bus.video().get_mode_type() {
                    crate::video::VideoMode::Graphics320x200 => 320,
                    crate::video::VideoMode::Graphics640x200 => 640,
                    _ => 320, // fallback
                };

                // Calculate pixel position
                let start_x = col * char_width;
                let start_y = row * char_height;

                log::trace!(
                    "Drawing char 0x{:02X}->0x{:02X} at row={} col={} (pixel {},{}) fg={} mode={:?}",
                    character,
                    char_code,
                    row,
                    col,
                    start_x,
                    start_y,
                    fg_color,
                    mode
                );

                // Draw each row of the character
                for (py, &glyph_byte) in glyph.iter().enumerate() {
                    if start_y + py >= 200 {
                        break; // Don't draw past bottom of screen
                    }

                    // In XorInverted mode, invert the glyph (swap foreground/background pixels)
                    // This makes character 0x80 (blank in CGA font) become a solid block for inverse effect
                    let final_glyph_byte = if matches!(mode, GraphicsDrawMode::XorInverted) {
                        !glyph_byte // Invert all bits
                    } else {
                        glyph_byte
                    };

                    // Draw each pixel in the row
                    for px in 0..char_width {
                        if start_x + px >= screen_width {
                            break; // Don't draw past right edge
                        }

                        let bit = (final_glyph_byte >> (7 - px)) & 1;
                        if bit != 0 {
                            // Foreground pixel
                            match mode {
                                GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                                    // XOR mode: read current pixel and XOR with fg_color
                                    let current =
                                        self.get_pixel_cga(bus, start_x + px, start_y + py);
                                    let new_color = current ^ fg_color;
                                    self.set_pixel_cga(bus, start_x + px, start_y + py, new_color);
                                }
                                GraphicsDrawMode::Transparent | GraphicsDrawMode::Opaque => {
                                    // Normal mode: use fg_color
                                    self.set_pixel_cga(bus, start_x + px, start_y + py, fg_color);
                                }
                            }
                        } else if matches!(mode, GraphicsDrawMode::Opaque) {
                            // Background pixel - only draw in opaque mode (bg=0)
                            self.set_pixel_cga(bus, start_x + px, start_y + py, 0);
                        }
                        // Otherwise leave background transparent (don't draw)
                    }
                }
            }
            crate::video::VideoMode::Graphics320x200x16 => {
                // EGA 320x200 16-color planar mode
                let char_code = if matches!(mode, GraphicsDrawMode::XorInverted) {
                    character & 0x7F
                } else {
                    character
                };
                let glyph = font.get_glyph_8(char_code);
                let char_height = 8;
                let char_width = 8;

                let start_x = col * char_width;
                let start_y = row * char_height;

                for (py, &glyph_byte) in glyph.iter().enumerate() {
                    let y = start_y + py;
                    if y >= 200 {
                        break;
                    }

                    let final_glyph_byte = if matches!(mode, GraphicsDrawMode::XorInverted) {
                        !glyph_byte
                    } else {
                        glyph_byte
                    };

                    // For EGA, work byte-at-a-time per plane for efficiency
                    // All 8 pixels of a glyph row fit in one EGA byte if char is byte-aligned
                    let byte_offset = y * 40 + start_x / 8;
                    let bit_shift = start_x % 8;

                    if bit_shift == 0 {
                        // Byte-aligned: fast path
                        for plane in 0..4usize {
                            let plane_bit = (fg_color >> plane) & 1;
                            let vram_offset = plane * 8000 + byte_offset;
                            let current = bus.video().read_byte(vram_offset);

                            let new_val = match mode {
                                GraphicsDrawMode::Opaque => {
                                    // Foreground where glyph=1, background (0) where glyph=0
                                    if plane_bit != 0 { final_glyph_byte } else { 0 }
                                }
                                GraphicsDrawMode::Transparent => {
                                    // Only set foreground pixels, leave background untouched
                                    if plane_bit != 0 {
                                        current | final_glyph_byte
                                    } else {
                                        current & !final_glyph_byte
                                    }
                                }
                                GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                                    if plane_bit != 0 {
                                        current ^ final_glyph_byte
                                    } else {
                                        current
                                    }
                                }
                            };
                            bus.video_mut().write_byte(vram_offset, new_val);
                        }
                    } else {
                        // Not byte-aligned: spans two EGA bytes
                        let left_mask = final_glyph_byte >> bit_shift;
                        let right_mask = final_glyph_byte << (8 - bit_shift);

                        for plane in 0..4usize {
                            let plane_bit = (fg_color >> plane) & 1;
                            let vram_offset = plane * 8000 + byte_offset;

                            // Left byte
                            let cur_left = bus.video().read_byte(vram_offset);
                            let new_left = match mode {
                                GraphicsDrawMode::Opaque => {
                                    if plane_bit != 0 {
                                        (cur_left & !left_mask) | left_mask
                                    } else {
                                        cur_left & !left_mask
                                    }
                                }
                                GraphicsDrawMode::Transparent => {
                                    if plane_bit != 0 {
                                        cur_left | left_mask
                                    } else {
                                        cur_left & !left_mask
                                    }
                                }
                                GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                                    if plane_bit != 0 {
                                        cur_left ^ left_mask
                                    } else {
                                        cur_left
                                    }
                                }
                            };
                            bus.video_mut().write_byte(vram_offset, new_left);

                            // Right byte (if any pixels spill over)
                            if right_mask != 0 && byte_offset + 1 < 8000 {
                                let vram_offset_r = plane * 8000 + byte_offset + 1;
                                let cur_right = bus.video().read_byte(vram_offset_r);
                                let new_right = match mode {
                                    GraphicsDrawMode::Opaque => {
                                        if plane_bit != 0 {
                                            (cur_right & !right_mask) | right_mask
                                        } else {
                                            cur_right & !right_mask
                                        }
                                    }
                                    GraphicsDrawMode::Transparent => {
                                        if plane_bit != 0 {
                                            cur_right | right_mask
                                        } else {
                                            cur_right & !right_mask
                                        }
                                    }
                                    GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                                        if plane_bit != 0 {
                                            cur_right ^ right_mask
                                        } else {
                                            cur_right
                                        }
                                    }
                                };
                                bus.video_mut().write_byte(vram_offset_r, new_right);
                            }
                        }
                    }
                }
            }
            crate::video::VideoMode::Graphics320x200x256 => {
                // VGA mode 13h: 1 byte per pixel, linear framebuffer
                let char_code = if matches!(mode, GraphicsDrawMode::XorInverted) {
                    character & 0x7F
                } else {
                    character
                };
                let glyph = font.get_glyph_8(char_code);
                let char_height = 8;
                let char_width = 8;

                let start_x = col * char_width;
                let start_y = row * char_height;

                for (py, &glyph_byte) in glyph.iter().enumerate() {
                    let y = start_y + py;
                    if y >= 200 {
                        break;
                    }

                    let final_glyph_byte = if matches!(mode, GraphicsDrawMode::XorInverted) {
                        !glyph_byte
                    } else {
                        glyph_byte
                    };

                    for px in 0..char_width {
                        let x = start_x + px;
                        if x >= 320 {
                            break;
                        }
                        let offset = y * 320 + x;
                        let bit = (final_glyph_byte >> (7 - px)) & 1;
                        if bit != 0 {
                            match mode {
                                GraphicsDrawMode::Xor | GraphicsDrawMode::XorInverted => {
                                    let current = bus.video().read_byte_vga(offset);
                                    bus.video_mut().write_byte_vga(offset, current ^ fg_color);
                                }
                                GraphicsDrawMode::Transparent | GraphicsDrawMode::Opaque => {
                                    bus.video_mut().write_byte_vga(offset, fg_color);
                                }
                            }
                        } else if matches!(mode, GraphicsDrawMode::Opaque) {
                            bus.video_mut().write_byte_vga(offset, 0);
                        }
                    }
                }
            }
            _ => {
                // Other modes not supported yet
            }
        }
    }

    /// Get a pixel value in CGA graphics mode
    /// Calculates CGA interlaced address: even lines at 0x0000+, odd lines at 0x2000+
    fn get_pixel_cga(&self, bus: &mut Bus, x: usize, y: usize) -> u8 {
        match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                // 320x200, 4 colors, 2 bits per pixel
                let byte_offset = if y.is_multiple_of(2) {
                    (y / 2) * 80 + x / 4
                } else {
                    0x2000 + ((y - 1) / 2) * 80 + x / 4
                };
                let pixel_in_byte = x % 4;
                let shift = 6 - (pixel_in_byte * 2);
                let byte_val = bus.video().read_byte(byte_offset);
                (byte_val >> shift) & 0x03
            }
            crate::video::VideoMode::Graphics640x200 => {
                // 640x200, 2 colors, 1 bit per pixel
                let byte_offset = if y.is_multiple_of(2) {
                    (y / 2) * 80 + x / 8
                } else {
                    0x2000 + ((y - 1) / 2) * 80 + x / 8
                };
                let pixel_in_byte = x % 8;
                let shift = 7 - pixel_in_byte;
                let byte_val = bus.video().read_byte(byte_offset);
                (byte_val >> shift) & 0x01
            }
            _ => 0,
        }
    }

    /// Set a pixel in CGA graphics mode
    /// Calculates CGA interlaced address: even lines at 0x0000+, odd lines at 0x2000+
    fn set_pixel_cga(&self, bus: &mut Bus, x: usize, y: usize, color: u8) {
        match bus.video().get_mode_type() {
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
                let mut byte_val = bus.video().read_byte(byte_offset);
                byte_val &= !(0x03 << shift); // Clear the 2-bit pixel
                byte_val |= (color & 0x03) << shift; // Set new color
                bus.video_mut().write_byte(byte_offset, byte_val);
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

                let mut byte_val = bus.video().read_byte(byte_offset);
                if color & 1 != 0 {
                    byte_val |= bit_mask; // Set bit
                } else {
                    byte_val &= !bit_mask; // Clear bit
                }
                bus.video_mut().write_byte(byte_offset, byte_val);
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

    /// INT 10h, AH=01h - Set Cursor Shape
    /// Input:
    ///   CH = cursor start scan line (bits 0-4), cursor options (bits 5-6)
    ///   CL = cursor end scan line (bits 0-4)
    /// Output: None
    fn int10_set_cursor_shape(&mut self, bus: &mut Bus) {
        let start_line = (self.cx >> 8) as u8; // CH
        let end_line = (self.cx & 0xFF) as u8; // CL

        // Store cursor shape in BDA
        bus.write_u8(
            crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE,
            start_line,
        );
        bus.write_u8(
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
    fn int10_get_cursor_position(&mut self, bus: &mut Bus) {
        let cursor = bus.video().get_cursor();

        // Get cursor shape from BDA
        let start_line =
            bus.read_u8(crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE);
        let end_line = bus.read_u8(crate::memory::BDA_START + crate::memory::BDA_CURSOR_END_LINE);

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
        let page = (self.ax & 0xFF) as u8; // AL

        // Validate page number (0-7 for standard text modes)
        if page > 7 {
            log::warn!("INT 10h/AH=05h: Invalid page number: {}", page);
            return;
        }

        // Update active page in Video struct
        bus.video_mut().set_active_page(page);

        // Update BDA active page
        bus.write_u8(
            crate::memory::BDA_START + crate::memory::BDA_ACTIVE_PAGE,
            page,
        );

        // Update BDA video page offset (page_number * page_size)
        let page_size = bus.read_u16(crate::memory::BDA_START + crate::memory::BDA_VIDEO_PAGE_SIZE);
        let page_offset = (page as u16) * page_size;
        bus.write_u16(
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
    fn int10_get_video_mode(&mut self, bus: &mut Bus) {
        let mode = bus.video().get_mode();
        let columns: u8 = bus.video().get_cols() as u8;
        let page = bus.video().get_active_page();

        self.ax = ((columns as u16) << 8) | (mode as u16);
        self.bx = (self.bx & 0x00FF) | ((page as u16) << 8);
    }

    /// INT 10h, AH=0Bh - Set Color Palette
    /// Subfunction in BH:
    ///   00h = Set background/border color
    ///        BL = color value (bits 0-3 = border/background color, bit 4 = intensity)
    ///   01h = Set palette
    ///        BL = palette ID (0 = green/red/brown, 1 = cyan/magenta/white)
    fn int10_set_color_palette(&mut self, bus: &mut Bus) {
        let subfunction = (self.bx >> 8) as u8; // BH
        let value = (self.bx & 0xFF) as u8; // BL

        match subfunction {
            0x00 => {
                // Set background/border color
                // Behavior depends on video mode:
                // - Text modes (0x00-0x03, 0x07): bits 0-3 set border (overscan) color
                // - Graphics modes (0x04-0x06): bits 0-3 set background color (palette entry 0)
                // - Bit 4: intensity flag (enables high-intensity backgrounds in text mode)
                let color = value & 0x0F;
                let intensity = (value & 0x10) != 0;

                if bus.video().is_graphics_mode() {
                    // Graphics mode: set background color (palette entry 0)
                    bus.video_mut().set_cga_background(color);
                    bus.video_mut().set_cga_intensity(intensity);
                    log::debug!(
                        "INT 10h/AH=0Bh/BH=00h: Set graphics background={}, intensity={}",
                        color,
                        intensity
                    );
                } else {
                    // Text mode: set border (overscan) color
                    bus.video_mut().set_border_color(color);
                    // Note: intensity bit behavior in text mode is complex and hardware-dependent
                    // Some adapters use it for high-intensity backgrounds, others ignore it
                    log::debug!(
                        "INT 10h/AH=0Bh/BH=00h: Set border color={}, intensity={}",
                        color,
                        intensity
                    );
                }
            }
            0x01 => {
                // Set palette for 320x200 4-color CGA graphics mode
                // BL = 0: palette 0 (green/red/brown)
                // BL = 1: palette 1 (cyan/magenta/white)
                let palette_id = value & 0x01;
                bus.video_mut().set_cga_palette_id(palette_id);

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

    /// INT 10h, AH=0Ch - Write Graphics Pixel
    /// Input:
    ///   AL = pixel color value (0-3 for 320x200, 0-1 for 640x200)
    ///   BH = page number (0 for graphics modes, ignored)
    ///   CX = column (0-319 or 0-639)
    ///   DX = row (0-199)
    /// Output: None
    fn int10_write_pixel(&mut self, bus: &mut Bus) {
        let color = (self.ax & 0xFF) as u8; // AL
        let col = self.cx as usize;
        let row = self.dx as usize;

        match bus.video().get_mode_type() {
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
                let mut byte_val = bus.video().read_byte(byte_offset);
                let shift = 6 - (pixel_in_byte * 2); // MSB first
                byte_val &= !(0x03 << shift); // Clear 2 bits
                byte_val |= (color & 0x03) << shift; // Set 2 bits
                bus.video_mut().write_byte(byte_offset, byte_val);
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
                let mut byte_val = bus.video().read_byte(byte_offset);
                let bit_mask = 0x80 >> pixel_in_byte; // MSB first
                if (color & 0x01) != 0 {
                    byte_val |= bit_mask; // Set bit
                } else {
                    byte_val &= !bit_mask; // Clear bit
                }
                bus.video_mut().write_byte(byte_offset, byte_val);
            }
            crate::video::VideoMode::Text { .. } => {
                // Ignore write pixel in text mode
            }
            crate::video::VideoMode::Graphics320x200x16 => {
                // EGA 320x200 16-color: write pixel via per-plane vram offsets
                if col < 320 && row < 200 {
                    let byte_offset = row * 40 + col / 8;
                    let bit = 7 - (col % 8);
                    for plane in 0..4usize {
                        let plane_bit = (color as usize >> plane) & 1;
                        let vram_offset = plane * 8000 + byte_offset;
                        let mut byte_val = bus.video().read_byte(vram_offset);
                        if plane_bit != 0 {
                            byte_val |= 1 << bit;
                        } else {
                            byte_val &= !(1 << bit);
                        }
                        bus.video_mut().write_byte(vram_offset, byte_val);
                    }
                }
            }
            crate::video::VideoMode::Graphics320x200x256 => {
                // VGA mode 13h: 1 byte per pixel, linear framebuffer
                if col < 320 && row < 200 {
                    bus.video_mut().write_byte_vga(row * 320 + col, color);
                }
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
    fn int10_read_pixel(&mut self, bus: &mut Bus) {
        let col = self.cx as usize;
        let row = self.dx as usize;

        let color = match bus.video().get_mode_type() {
            crate::video::VideoMode::Graphics320x200 => {
                if col >= 320 || row >= 200 {
                    0
                } else {
                    let pixels_per_byte = 4;
                    let bytes_per_line = 80;
                    let byte_offset = row * bytes_per_line + col / pixels_per_byte;
                    let pixel_in_byte = col % pixels_per_byte;

                    let byte_val = bus.video().read_byte(byte_offset);
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

                    let byte_val = bus.video().read_byte(byte_offset);
                    let bit_mask = 0x80 >> pixel_in_byte;
                    if (byte_val & bit_mask) != 0 { 1 } else { 0 }
                }
            }
            crate::video::VideoMode::Text { .. } => 0,
            crate::video::VideoMode::Graphics320x200x16 => {
                // EGA 320x200 16-color: read pixel from per-plane vram offsets
                if col >= 320 || row >= 200 {
                    0
                } else {
                    let byte_offset = row * 40 + col / 8;
                    let bit = 7 - (col % 8);
                    let mut color = 0u8;
                    for plane in 0..4usize {
                        let byte_val = bus.video().read_byte(plane * 8000 + byte_offset);
                        if (byte_val >> bit) & 1 != 0 {
                            color |= 1 << plane;
                        }
                    }
                    color
                }
            }
            crate::video::VideoMode::Graphics320x200x256 => {
                // VGA mode 13h: 1 byte per pixel, linear framebuffer
                if col >= 320 || row >= 200 {
                    0
                } else {
                    bus.video().read_byte_vga(row * 320 + col)
                }
            }
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
    fn int10_palette_registers(&mut self, bus: &mut Bus) {
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
        bus.write_u16(buffer_addr, 0x0000); // Offset
        bus.write_u16(buffer_addr + 2, 0x0000); // Segment

        // Offset 04h: Current video mode
        bus.write_u8(buffer_addr + 4, bus.video().get_mode());

        // Offset 05h-06h: Number of columns
        let cols = bus.video().get_cols();
        bus.write_u16(buffer_addr + 5, cols as u16);

        // Offset 07h-08h: Length of regen buffer (page size in bytes)
        // cols * rows * 2 bytes per cell (char + attr)
        let rows = bus.video().get_rows();
        let buffer_size = cols * rows * 2;
        bus.write_u16(buffer_addr + 7, buffer_size as u16);

        // Offset 09h-0Ah: Starting address in regen buffer (current page offset)
        bus.write_u16(buffer_addr + 9, 0x0000);

        // Offset 0Bh-1Ah: Cursor positions for 8 pages (row, column pairs)
        let cursor = bus.video().get_cursor();
        for page in 0..8 {
            let offset = buffer_addr + 0x0B + (page * 2);
            if page == 0 {
                bus.write_u8(offset, cursor.col as u8); // Column
                bus.write_u8(offset + 1, cursor.row as u8); // Row
            } else {
                bus.write_u8(offset, 0);
                bus.write_u8(offset + 1, 0);
            }
        }

        // Offset 1Bh-1Ch: Cursor type (start/end scan lines)
        let cursor_start =
            bus.read_u8(crate::memory::BDA_START + crate::memory::BDA_CURSOR_START_LINE);
        let cursor_end = bus.read_u8(crate::memory::BDA_START + crate::memory::BDA_CURSOR_END_LINE);
        bus.write_u8(buffer_addr + 0x1B, cursor_end);
        bus.write_u8(buffer_addr + 0x1C, cursor_start);

        // Offset 1Dh: Active display page
        bus.write_u8(buffer_addr + 0x1D, 0);

        // Offset 1Eh-1Fh: CRTC port address (3D4h for color, 3B4h for mono)
        bus.write_u16(buffer_addr + 0x1E, 0x03D4);

        // Offset 20h: Current setting of register 3x8h
        bus.write_u8(buffer_addr + 0x20, 0x00);

        // Offset 21h: Current setting of register 3x9h
        bus.write_u8(buffer_addr + 0x21, 0x00);

        // Offset 22h: Number of rows - 1
        bus.write_u8(buffer_addr + 0x22, (rows - 1) as u8);

        // Offset 23h-24h: Character height (scan lines per character)
        bus.write_u16(buffer_addr + 0x23, 16); // 16 scan lines for VGA

        // Offset 25h: Active display combination code
        bus.write_u8(buffer_addr + 0x25, 0x08); // VGA with color analog display

        // Offset 26h: Alternate display combination code
        bus.write_u8(buffer_addr + 0x26, 0x00); // No alternate display

        // Offset 27h-28h: Number of colors supported (0 = mono)
        bus.write_u16(buffer_addr + 0x27, 16); // 16 colors in text mode

        // Offset 29h: Number of pages supported
        bus.write_u8(buffer_addr + 0x29, 8);

        // Offset 2Ah: Number of scan lines active
        // 0 = 200, 1 = 350, 2 = 400
        bus.write_u8(buffer_addr + 0x2A, 2); // 400 scan lines

        // Offset 2Bh: Primary character block
        bus.write_u8(buffer_addr + 0x2B, 0);

        // Offset 2Ch: Secondary character block
        bus.write_u8(buffer_addr + 0x2C, 0);

        // Offset 2Dh: Miscellaneous state flags
        // Bit 0: All modes on all displays
        // Bit 1: Gray summing enabled
        // Bit 2: Monochrome display attached
        // Bit 3: Default palette loading disabled
        // Bit 4: Cursor emulation enabled
        // Bit 5: Blinking enabled
        // Bit 6-7: Reserved
        bus.write_u8(buffer_addr + 0x2D, 0x21); // All modes + blinking

        // Offset 2Eh-2Fh: Reserved
        bus.write_u8(buffer_addr + 0x2E, 0);
        bus.write_u8(buffer_addr + 0x2F, 0);

        // Offset 30h: Video memory available
        // 0 = 64KB, 1 = 128KB, 2 = 192KB, 3 = 256KB
        bus.write_u8(buffer_addr + 0x30, 3); // 256KB

        // Offset 31h: Save pointer state flags
        bus.write_u8(buffer_addr + 0x31, 0);

        // Offset 32h-3Fh: Reserved (fill with zeros)
        for i in 0x32..0x40 {
            bus.write_u8(buffer_addr + i, 0);
        }

        // Return AL = 1Bh to indicate function is supported
        self.ax = (self.ax & 0xFF00) | 0x1B;

        log::trace!(
            "INT 10h/AH=1Bh: Returned functionality/state info at {:05X}",
            buffer_addr
        );
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
    fn int10_character_generator(&mut self, bus: &mut Bus) {
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
                self.int10_get_font_info(bus);
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
    fn int10_get_font_info(&mut self, bus: &mut Bus) {
        let pointer_type = (self.bx >> 8) as u8; // BH

        // Determine which font to return based on pointer type
        let (segment, offset, bytes_per_char, rows) = match pointer_type {
            0x00 => {
                // INT 1Fh pointer (8x8 graphics characters)
                // Read INT 1Fh vector from IVT
                let int_1f_offset = bus.read_u16(0x1F * 4);
                let int_1f_segment = bus.read_u16(0x1F * 4 + 2);
                // If not set, default to our ROM 8x8 font
                if int_1f_segment == 0xF000 && int_1f_offset < 0x100 {
                    // Not initialized, use ROM font
                    (
                        crate::memory::FONT_8X8_SEGMENT,
                        crate::memory::FONT_8X8_OFFSET,
                        8,
                        25,
                    )
                } else {
                    (int_1f_segment, int_1f_offset, 8, 25)
                }
            }
            0x01 => {
                // INT 43h pointer (8x14/8x16 graphics characters)
                // Read INT 43h vector from IVT
                let int_43_offset = bus.read_u16(0x43 * 4);
                let int_43_segment = bus.read_u16(0x43 * 4 + 2);
                // If not set, default to our ROM 8x16 font
                if int_43_segment == 0xF000 && int_43_offset < 0x100 {
                    // Not initialized, use ROM font
                    (
                        crate::memory::FONT_8X16_SEGMENT,
                        crate::memory::FONT_8X16_OFFSET,
                        16,
                        25,
                    )
                } else {
                    (int_43_segment, int_43_offset, 16, 25)
                }
            }
            0x02 => {
                // ROM 8x14 character font pointer
                // We don't have a real 8x14 font, return 8x16 instead
                (
                    crate::memory::FONT_8X16_SEGMENT,
                    crate::memory::FONT_8X16_OFFSET,
                    16,
                    25,
                )
            }
            0x03 | 0x04 => {
                // ROM 8x8 double-dot font pointer (both regular and top half)
                (
                    crate::memory::FONT_8X8_SEGMENT,
                    crate::memory::FONT_8X8_OFFSET,
                    8,
                    25,
                )
            }
            0x05 => {
                // ROM 9x14 alphanumeric alternate
                // We don't have a 9x14 font, return 8x16 instead
                (
                    crate::memory::FONT_8X16_SEGMENT,
                    crate::memory::FONT_8X16_OFFSET,
                    16,
                    25,
                )
            }
            0x06 => {
                // ROM 8x16 font
                (
                    crate::memory::FONT_8X16_SEGMENT,
                    crate::memory::FONT_8X16_OFFSET,
                    16,
                    25,
                )
            }
            0x07 => {
                // ROM 9x16 alternate
                // We don't have a 9x16 font, return 8x16 instead
                (
                    crate::memory::FONT_8X16_SEGMENT,
                    crate::memory::FONT_8X16_OFFSET,
                    16,
                    25,
                )
            }
            _ => {
                // Unknown pointer type, default to 8x16
                log::warn!(
                    "INT 10h/AH=11h/AL=30h: Unknown pointer type BH=0x{:02X}, defaulting to 8x16",
                    pointer_type
                );
                (
                    crate::memory::FONT_8X16_SEGMENT,
                    crate::memory::FONT_8X16_OFFSET,
                    16,
                    25,
                )
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
    fn int10_display_combination_code(&mut self, bus: &mut Bus) {
        let subfunction = (self.ax & 0xFF) as u8; // AL

        // Use BDA location to store display combination (not standard, but convenient)
        const DISPLAY_CODE_ADDR: usize = crate::memory::BDA_START + 0x8A;

        match subfunction {
            0x00 => {
                // Read display combination code
                let code = bus.read_u16(DISPLAY_CODE_ADDR);
                let active_display = (code & 0xFF) as u8;
                let alternate_display = (code >> 8) as u8;

                // If not initialized, return code based on video card type
                let (active, alternate) = if code == 0 {
                    (bus.video().card_type().display_combination_code(), 0x00)
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
                bus.write_u16(DISPLAY_CODE_ADDR, code);

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
    fn int10_get_video_buffer(&mut self, _bus: &mut Bus) {
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
