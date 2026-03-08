use crate::{
    bus::Bus,
    byte_to_printable_char,
    cpu::{
        Cpu,
        bios::bda::{
            bda_get_active_page, bda_get_columns, bda_get_crt_controller_port_address,
            bda_get_crt_palette, bda_get_cursor_end_line, bda_get_cursor_pos,
            bda_get_cursor_start_line, bda_get_display_combination_code, bda_get_rows,
            bda_get_video_mode, bda_get_video_page_size, bda_set_active_page, bda_set_columns,
            bda_set_crt_palette, bda_set_cursor_end_line, bda_set_cursor_pos,
            bda_set_cursor_start_line, bda_set_display_combination_code, bda_set_rows,
            bda_set_video_mode, bda_set_video_page_offset, bda_set_video_page_size,
        },
    },
    physical_address,
    video::{
        CGA_MEMORY_START, EGA_MEMORY_START, EGA_PLANE_SIZE, TEXT_MODE_SIZE,
        VIDEO_MODE_0DH_EGA_320_X_200_16, VIDEO_MODE_02H_COLOR_TEXT_80_X_25,
        VIDEO_MODE_03H_COLOR_TEXT_80_X_25, VIDEO_MODE_04H_CGA_320_X_200_4,
        VIDEO_MODE_06H_CGA_640_X_200_2, VideoCardType,
        font::{CHAR_HEIGHT_8, Cp437Font},
        video_calculate_linear_offset,
        video_card::{
            AC_ADDR_DATA_PORT, AC_DATA_READ_PORT, AC_REG_COLOR_SELECT, AC_REG_MODE_CONTROL,
            DAC_DATA_PORT, DAC_READ_INDEX_PORT, DAC_WRITE_INDEX_PORT, INPUT_STATUS_1_PORT,
            VIDEO_CARD_CONTROL_ADDR, VIDEO_CARD_DATA_ADDR, VIDEO_CARD_REG_CURSOR_END_LINE,
            VIDEO_CARD_REG_CURSOR_START_LINE,
        },
        video_set_cursor_pos,
    },
};

// ROM Font Data locations
// These addresses are in the ROM BIOS area (F000 segment)
// Must fit within 1MB memory (0x100000)
// 8x16 font needs 4096 bytes (0x1000), 8x8 font needs 2048 bytes (0x800)
const FONT_8X16_SEGMENT: u16 = 0xF000;
const FONT_8X16_OFFSET: u16 = 0xB000; // F000:B000
#[allow(dead_code)]
const FONT_8X16_ADDR: usize = 0xFB000; // Physical address, ends at 0xFC000

const FONT_8X8_SEGMENT: u16 = 0xF000;
const FONT_8X8_OFFSET: u16 = 0xC000; // F000:C000
#[allow(dead_code)]
const FONT_8X8_ADDR: usize = 0xFC000; // Physical address, ends at 0xFC800

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
            0x07 => self.int10_scroll_down(bus),
            0x08 => self.int10_read_char_attr(bus),
            0x09 => self.int10_write_char_attr(bus),
            0x0A => self.int10_write_char(bus),
            0x0B => self.int10_set_color_palette(bus),
            0x0E => self.int10_teletype_output(bus),
            0x0F => self.int10_get_video_mode(bus),
            0x10 => self.int10_palette_registers(bus),
            0x11 => self.int10_character_generator(bus),
            0x12 => self.int10_alternate_function_select(bus),
            0x15 => self.int10_return_physical_display_params(bus),
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

        // Clear the "Clear Memory" bit (Bit 7 of AL)
        // If Bit 7 is set, BIOS doesn't clear the screen memory
        let clear_screen_flag = (mode & 0x80) == 0;
        let mode = mode & 0x7f;

        if clear_screen_flag {
            match mode {
                VIDEO_MODE_04H_CGA_320_X_200_4 | VIDEO_MODE_06H_CGA_640_X_200_2 => {
                    // Clear both CGA interleaved banks (even rows at 0x0000, odd at 0x2000)
                    for i in 0..0x4000usize {
                        bus.memory_write_u8(CGA_MEMORY_START + i, 0);
                    }
                }
                VIDEO_MODE_0DH_EGA_320_X_200_16 => {
                    // Clear all 4 EGA planes (sequencer map mask defaults to 0x0F = all planes)
                    for i in 0..EGA_PLANE_SIZE {
                        bus.memory_write_u8(EGA_MEMORY_START + i, 0);
                    }
                }
                _ => {
                    // Text mode: fill with space + light gray on black
                    for i in (0..TEXT_MODE_SIZE).step_by(2) {
                        bus.memory_write_u8(CGA_MEMORY_START + i, 0x20);
                        bus.memory_write_u8(CGA_MEMORY_START + i + 1, 0x07);
                    }
                }
            }
        }

        let mode_info = bus.video_card_mut().set_mode(mode);

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

        let rows = bda_get_rows(bus);
        let cols = bda_get_columns(bus);

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
        let bottom = bottom.min(rows); // rows is already last valid row index from BDA
        if top > bottom || left > right {
            return;
        }

        // TODO
        // if bus.video().is_graphics_mode() {
        //     bus.video_mut()
        //         .scroll_down_window(lines, top, left, bottom, right, attr);
        //     return;
        // }

        if lines == 0 {
            // Clear entire window
            for row in top..=bottom {
                for col in left..=right {
                    let offset = (row as usize * cols as usize + col as usize) * 2;
                    bus.memory_write_u8(CGA_MEMORY_START + offset, b' ');
                    bus.memory_write_u8(CGA_MEMORY_START + offset + 1, attr);
                }
            }
        } else {
            // Scroll down by 'lines' rows (process bottom to top)
            for row in (top..=bottom).rev() {
                for col in left..=right {
                    let dest_offset = (row as usize * cols as usize + col as usize) * 2;

                    if row >= top + lines {
                        // Copy from above - read from video buffer, not memory
                        let src_row = row - lines;
                        let src_offset = (src_row as usize * cols as usize + col as usize) * 2;
                        let ch = bus.memory_read_u8(CGA_MEMORY_START + src_offset);
                        let at = bus.memory_read_u8(CGA_MEMORY_START + src_offset + 1);
                        bus.memory_write_u8(CGA_MEMORY_START + dest_offset, ch);
                        bus.memory_write_u8(CGA_MEMORY_START + dest_offset + 1, at);
                    } else {
                        // Fill with blanks
                        bus.memory_write_u8(CGA_MEMORY_START + dest_offset, b' ');
                        bus.memory_write_u8(CGA_MEMORY_START + dest_offset + 1, attr);
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
                if pos >= cols * (rows + 1) {
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
            if pos >= cols as usize * (rows as usize + 1) {
                break; // Don't write beyond screen
            }
            let offset = pos * 2;
            bus.memory_write_u8(CGA_MEMORY_START + offset, ch);
            // Don't modify attribute byte (offset + 1) - preserve existing color
        }
        // }

        // Cursor position is NOT updated by this function
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

        // CGA Color Select Register is at CRTC base + 5 (0x3D9 for color, 0x3B9 for mono).
        // Bit layout:
        //   Bits 0-3: border/background color
        //   Bit 4:    intensity of background
        //   Bit 5:    palette select (0 = green/red/brown, 1 = cyan/magenta/white)
        let color_select_port = bda_get_crt_controller_port_address(bus) + 5;
        let current = bda_get_crt_palette(bus);

        match subfunction {
            0x00 => {
                // Set background/border color: update bits 0-4, preserve bit 5
                let new_value = (current & 0xE0) | (value & 0x1F);
                bda_set_crt_palette(bus, new_value);
                bus.io_write_u8(color_select_port, new_value);
                log::debug!(
                    "INT 10h/AH=0Bh/BH=00h: Set border/background color=0x{:02X}",
                    new_value
                );
            }
            0x01 => {
                // Select CGA palette: update bit 5, preserve bits 0-4
                let new_value = (current & 0xDF) | ((value & 0x01) << 5);
                bda_set_crt_palette(bus, new_value);
                bus.io_write_u8(color_select_port, new_value);
                log::debug!(
                    "INT 10h/AH=0Bh/BH=01h: Set CGA palette={}, register=0x{:02X}",
                    value & 0x01,
                    new_value
                );
            }
            _ => {
                log::warn!(
                    "Unhandled INT 10h/AH=0Bh subfunction: BH=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// Draw a character pixel-by-pixel into CGA graphics VRAM.
    ///
    /// Uses the 8x8 CGA font. Draws in transparent mode: foreground pixels are
    /// OR'd into the framebuffer; background pixels are left unchanged.
    ///
    /// CGA memory is interleaved: even scan lines at offset 0x0000, odd at 0x2000.
    /// Mode 06h: 1bpp, 80 bytes/row, glyph byte maps directly to pixel bits.
    /// Mode 04h: 2bpp, 80 bytes/row, each glyph bit expands to a 2-bit color index.
    fn draw_char_cga_graphics(
        &self,
        bus: &mut Bus,
        mode: u8,
        ch: u8,
        row: u8,
        col: u8,
        fg_color: u8,
    ) {
        let font = Cp437Font::new();
        let glyph = font.get_glyph_8(ch);

        match mode {
            VIDEO_MODE_06H_CGA_640_X_200_2 => {
                for (r, &row_byte) in glyph.iter().enumerate().take(CHAR_HEIGHT_8) {
                    let pixel_y = row as usize * CHAR_HEIGHT_8 + r;
                    let bank_offset = if pixel_y % 2 == 1 { 0x2000 } else { 0 };
                    let vram_offset = bank_offset + (pixel_y / 2) * 80 + col as usize;
                    let existing = bus.memory_read_u8(CGA_MEMORY_START + vram_offset);
                    bus.memory_write_u8(CGA_MEMORY_START + vram_offset, existing | row_byte);
                }
            }
            VIDEO_MODE_04H_CGA_320_X_200_4 => {
                let fg_2bit = fg_color & 0x03;
                for (r, &row_byte) in glyph.iter().enumerate().take(CHAR_HEIGHT_8) {
                    let pixel_y = row as usize * CHAR_HEIGHT_8 + r;
                    let bank_offset = if pixel_y % 2 == 1 { 0x2000 } else { 0 };
                    // Each char is 8 pixels wide = 2 bytes at 2bpp
                    let vram_base = bank_offset + (pixel_y / 2) * 80 + col as usize * 2;

                    // Expand glyph bits to 2bpp: bits 7-4 → byte0, bits 3-0 → byte1
                    let mut byte0 = 0u8;
                    let mut byte1 = 0u8;
                    for bit in 0..4usize {
                        if (row_byte & (0x80 >> bit)) != 0 {
                            byte0 |= fg_2bit << ((3 - bit) * 2);
                        }
                    }
                    for bit in 4..8usize {
                        if (row_byte & (0x80 >> bit)) != 0 {
                            byte1 |= fg_2bit << ((7 - bit) * 2);
                        }
                    }

                    let existing0 = bus.memory_read_u8(CGA_MEMORY_START + vram_base);
                    let existing1 = bus.memory_read_u8(CGA_MEMORY_START + vram_base + 1);
                    bus.memory_write_u8(CGA_MEMORY_START + vram_base, existing0 | byte0);
                    bus.memory_write_u8(CGA_MEMORY_START + vram_base + 1, existing1 | byte1);
                }
            }
            _ => {}
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
                let mode = bda_get_video_mode(bus);
                if mode == VIDEO_MODE_04H_CGA_320_X_200_4 || mode == VIDEO_MODE_06H_CGA_640_X_200_2
                {
                    // Graphics mode: draw character pixel-by-pixel into CGA framebuffer
                    let fg_color = (self.bx & 0xFF) as u8; // BL
                    self.draw_char_cga_graphics(bus, mode, ch, cursor.row, cursor.col, fg_color);
                } else if mode == VIDEO_MODE_0DH_EGA_320_X_200_16 {
                    // EGA planar graphics: draw character transparently into EGA planes
                    let fg_color = (self.bx & 0xFF) as u8; // BL
                    bus.video_card_mut()
                        .ega_draw_char_transparent(ch, cursor.row, cursor.col, fg_color);
                } else {
                    // Text mode: write character byte directly
                    let offset = (cursor.row as usize * columns as usize + cursor.col as usize) * 2;
                    bus.memory_write_u8(CGA_MEMORY_START + offset, ch);
                }

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
        // CGA BIOS does not implement AH=10h (EGA/VGA function only)
        if bus.video_card().card_type() == VideoCardType::CGA {
            log::warn!("INT 10h AH=10h: not supported by CGA card - ignoring");
            return;
        }

        let subfunction = (self.ax & 0xFF) as u8; // AL

        // DAC register operations are VGA-only (EGA has no DAC)
        if bus.video_card().card_type() == VideoCardType::EGA
            && matches!(subfunction, 0x10 | 0x12 | 0x15 | 0x17 | 0x1A | 0x1B)
        {
            log::warn!(
                "INT 10h/AH=10h/AL={:02X}h: DAC function not supported by EGA card - ignoring",
                subfunction
            );
            return;
        }

        match subfunction {
            0x00 => {
                // Set individual AC palette register
                // BL = register number (0-15), BH = color value (EGA 6-bit index)
                let register = (self.bx & 0xFF) as u8; // BL
                let value = ((self.bx >> 8) & 0xFF) as u8; // BH
                let _ = bus.io_read_u8(INPUT_STATUS_1_PORT); // reset AC flip-flop to address mode
                bus.io_write_u8(AC_ADDR_DATA_PORT, register);
                bus.io_write_u8(AC_ADDR_DATA_PORT, value);
                log::debug!(
                    "INT 10h/AH=10h/AL=00h: Set AC register {} = {}",
                    register,
                    value
                );
            }
            0x01 => {
                // Set border (overscan) color
                // BH = border color value
                log::warn!("INT 10h/AH=10h/AL=01h: Set border color");
            }
            0x02 => {
                // Set all AC palette registers and border
                // ES:DX -> 17-byte table (16 AC register values + 1 border color byte)
                let table_addr = physical_address(self.es, self.dx);
                let _ = bus.io_read_u8(INPUT_STATUS_1_PORT); // reset AC flip-flop to address mode
                for i in 0..16usize {
                    let value = bus.memory_read_u8(table_addr + i);
                    bus.io_write_u8(AC_ADDR_DATA_PORT, i as u8);
                    bus.io_write_u8(AC_ADDR_DATA_PORT, value);
                }
                let border = bus.memory_read_u8(table_addr + 16);
                log::debug!(
                    "INT 10h/AH=10h/AL=02h: Set all AC palette registers, border={}",
                    border
                );
            }
            0x03 => {
                // Toggle intensity/blinking bit
                // BL = 0: bit 7 = high-intensity background (16 bg colors, no blink)
                // BL = 1: bit 7 = character blink (8 bg colors, blink enabled, default)
                let blink_enabled = (self.bx & 0xFF) as u8 != 0;
                let _ = bus.io_read_u8(INPUT_STATUS_1_PORT); // reset AC flip-flop to address mode
                bus.io_write_u8(AC_ADDR_DATA_PORT, AC_REG_MODE_CONTROL);
                let mode_ctrl = bus.io_read_u8(AC_DATA_READ_PORT);
                let new_mode = if blink_enabled {
                    mode_ctrl | 0x08
                } else {
                    mode_ctrl & !0x08
                };
                bus.io_write_u8(AC_ADDR_DATA_PORT, new_mode);
                log::debug!(
                    "INT 10h/AH=10h/AL=03h: {} mode",
                    if blink_enabled { "blink" } else { "intensity" }
                );
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

                bus.io_write_u8(DAC_WRITE_INDEX_PORT, register);
                bus.io_write_u8(DAC_DATA_PORT, red);
                bus.io_write_u8(DAC_DATA_PORT, green);
                bus.io_write_u8(DAC_DATA_PORT, blue);

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
                let mut table_addr = physical_address(self.es, self.dx);

                for i in 0..count {
                    let register = first_register.wrapping_add(i as u8);
                    let red = bus.memory_read_u8(table_addr) & 0x3F; // Mask to 6 bits
                    let green = bus.memory_read_u8(table_addr + 1) & 0x3F;
                    let blue = bus.memory_read_u8(table_addr + 2) & 0x3F;
                    table_addr += 3;

                    bus.io_write_u8(0x3C8, register);
                    bus.io_write_u8(0x3C9, red);
                    bus.io_write_u8(0x3C9, green);
                    bus.io_write_u8(0x3C9, blue);
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
                bus.io_write_u8(DAC_READ_INDEX_PORT, register);
                let red = bus.io_read_u8(DAC_DATA_PORT);
                let green = bus.io_read_u8(DAC_DATA_PORT);
                let blue = bus.io_read_u8(DAC_DATA_PORT);

                self.dx = (self.dx & 0x00FF) | ((red as u16) << 8); // DH = red
                self.cx = ((green as u16) << 8) | (blue as u16); // CH = green, CL = blue

                log::debug!(
                    "INT 10h/AH=10h/AL=15h: Read DAC register {} = RGB({}, {}, {})",
                    register,
                    red,
                    green,
                    blue
                );
            }
            0x17 => {
                // Read block of DAC registers
                // Input: BX = first register number (0-255)
                //        CX = number of registers to read (0-256)
                //        ES:DX -> buffer for RGB triplets (3 bytes per entry)
                let first_register = (self.bx & 0xFF) as u8;
                let count = self.cx;
                let mut table_addr = physical_address(self.es, self.dx);

                bus.io_write_u8(DAC_READ_INDEX_PORT, first_register);
                for _ in 0..count {
                    let red = bus.io_read_u8(DAC_DATA_PORT);
                    let green = bus.io_read_u8(DAC_DATA_PORT);
                    let blue = bus.io_read_u8(DAC_DATA_PORT);

                    bus.memory_write_u8(table_addr, red); // Red
                    bus.memory_write_u8(table_addr + 1, green); // Green
                    bus.memory_write_u8(table_addr + 2, blue); // Blue
                    table_addr += 3;
                }

                log::debug!(
                    "INT 10h/AH=10h/AL=17h: Read {} DAC registers starting at {}",
                    count,
                    first_register
                );
            }
            0x1A => {
                // Read color page state
                // Output: BH = color paging mode (0 = 4 pages of 64 regs, 1 = 16 pages of 16 regs)
                //         BL = current page number
                //
                // Paging mode is bit 7 of AC Mode Control register (0x10).
                // Current page comes from AC Color Select register (0x14):
                //   mode 0: page = bits [3:2]
                //   mode 1: page = bits [3:0]
                let _ = bus.io_read_u8(INPUT_STATUS_1_PORT); // reset AC flip-flop
                bus.io_write_u8(AC_ADDR_DATA_PORT, AC_REG_MODE_CONTROL);
                let mode_ctrl = bus.io_read_u8(AC_DATA_READ_PORT);

                let _ = bus.io_read_u8(INPUT_STATUS_1_PORT); // reset AC flip-flop
                bus.io_write_u8(AC_ADDR_DATA_PORT, AC_REG_COLOR_SELECT);
                let color_select = bus.io_read_u8(AC_DATA_READ_PORT);

                let paging_mode = (mode_ctrl >> 7) & 0x01; // bit 7: P54S
                let page = if paging_mode == 0 {
                    (color_select >> 2) & 0x03 // bits [3:2] select 1-of-4 pages
                } else {
                    color_select & 0x0F // bits [3:0] select 1-of-16 pages
                };

                self.bx = ((paging_mode as u16) << 8) | (page as u16); // BH=mode, BL=page

                log::debug!(
                    "INT 10h/AH=10h/AL=1Ah: Read color page state: mode={}, page={}",
                    paging_mode,
                    page
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
        // CGA BIOS does not implement AH=11h (EGA/VGA function only)
        if bus.video_card().card_type() == VideoCardType::CGA {
            log::warn!("INT 10h AH=11h: not supported by CGA card - ignoring");
            return;
        }

        let subfunction = (self.ax & 0xFF) as u8; // AL

        match subfunction {
            0x00..=0x04 => {
                // Load character set functions
                log::warn!(
                    "Unhandled INT 10h/AH=11h/AL={:02X}h: Load character set",
                    subfunction
                );
            }
            0x10..=0x14 => {
                // Load character set and program mode
                log::warn!(
                    "Unhandled INT 10h/AH=11h/AL={:02X}h: Load character set and program mode",
                    subfunction
                );
            }
            0x20..=0x24 => {
                // Set graphics character table
                log::warn!(
                    "Unhandled INT 10h/AH=11h/AL={:02X}h: Set graphics character table",
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
                let int_1f_offset = bus.memory_read_u16(0x1F * 4);
                let int_1f_segment = bus.memory_read_u16(0x1F * 4 + 2);
                // If not set, default to our ROM 8x8 font
                if int_1f_segment == 0xF000 && int_1f_offset < 0x100 {
                    // Not initialized, use ROM font
                    (FONT_8X8_SEGMENT, FONT_8X8_OFFSET, 8, 25)
                } else {
                    (int_1f_segment, int_1f_offset, 8, 25)
                }
            }
            0x01 => {
                // INT 43h pointer (8x14/8x16 graphics characters)
                // Read INT 43h vector from IVT
                let int_43_offset = bus.memory_read_u16(0x43 * 4);
                let int_43_segment = bus.memory_read_u16(0x43 * 4 + 2);
                // If not set, default to our ROM 8x16 font
                if int_43_segment == 0xF000 && int_43_offset < 0x100 {
                    // Not initialized, use ROM font
                    (FONT_8X16_SEGMENT, FONT_8X16_OFFSET, 16, 25)
                } else {
                    (int_43_segment, int_43_offset, 16, 25)
                }
            }
            0x02 => {
                // ROM 8x14 character font pointer
                // We don't have a real 8x14 font, return 8x16 instead
                (FONT_8X16_SEGMENT, FONT_8X16_OFFSET, 16, 25)
            }
            0x03 | 0x04 => {
                // ROM 8x8 double-dot font pointer (both regular and top half)
                (FONT_8X8_SEGMENT, FONT_8X8_OFFSET, 8, 25)
            }
            0x05 => {
                // ROM 9x14 alphanumeric alternate
                // We don't have a 9x14 font, return 8x16 instead
                (FONT_8X16_SEGMENT, FONT_8X16_OFFSET, 16, 25)
            }
            0x06 => {
                // ROM 8x16 font
                (FONT_8X16_SEGMENT, FONT_8X16_OFFSET, 16, 25)
            }
            0x07 => {
                // ROM 9x16 alternate
                // We don't have a 9x16 font, return 8x16 instead
                (FONT_8X16_SEGMENT, FONT_8X16_OFFSET, 16, 25)
            }
            _ => {
                // Unknown pointer type, default to 8x16
                log::warn!(
                    "INT 10h/AH=11h/AL=30h: Unknown pointer type BH=0x{:02X}, defaulting to 8x16",
                    pointer_type
                );
                (FONT_8X16_SEGMENT, FONT_8X16_OFFSET, 16, 25)
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
            0x20 => {
                // Set alternate print screen handler
                // Returns: AL = 12h if supported
                self.ax = (self.ax & 0xFF00) | 0x12;
                log::debug!("INT 10h/AH=12h/BL=20h: Set alternate print screen handler");
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

    /// INT 10h, AH=15h - Return Physical Display Parameters (VGA)
    /// Input: None
    /// Output:
    ///   AL = 15h if function supported
    ///   BH = active display code
    ///   BL = alternate display code
    fn int10_return_physical_display_params(&mut self, bus: &Bus) {
        // AH=15h is a VGA-only function; CGA and EGA leave registers unchanged
        if bus.video_card().card_type() != VideoCardType::VGA {
            log::warn!(
                "INT 10h AH=15h: not supported by {} card - ignoring",
                bus.video_card().card_type()
            );
            return;
        }

        // Active display code (08h = VGA with color display)
        let active_display = 0x08u8;
        // Alternate display code (00h = no alternate display)
        let alternate_display = 0x00u8;

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
        // AH=1Ah was introduced with PS/2 VGA BIOS (1987); CGA and EGA BIOSes do not support it
        if bus.video_card().card_type() != VideoCardType::VGA {
            log::warn!(
                "INT 10h AH=1Ah: not supported by {} card - ignoring",
                bus.video_card().card_type()
            );
            return;
        }

        let subfunction = (self.ax & 0xFF) as u8; // AL

        match subfunction {
            0x00 => {
                // Read display combination code
                let (active, alternate) = bda_get_display_combination_code(bus);

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
                bda_set_display_combination_code(bus, active_display, alternate_display);

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
    let bottom = options.bottom.min(options.rows); // rows is already last valid row index from BDA
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
