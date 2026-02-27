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
        let crt_controller_port = bda_get_crt_controller_port_address(memory_bus);

        match ch {
            b'\r' => {
                // Carriage return - move to column 0
                bda_set_cursor_pos(memory_bus, cursor_row, 0);
                video_set_cursor_pos(
                    io_bus,
                    crt_controller_port,
                    video_calculate_linear_offset(cursor_row, 0, columns),
                );
            }
            b'\n' => {
                // Line feed - move to next line
                todo!("ch: \\n, cursor_row: {cursor_row}, cursor_col: {cursor_col}, rows: {rows}");
                //  let new_row = if cursor.row >= rows - 1 {
                //      // Need to scroll
                //      self.scroll_up_internal(bus, 1);
                //      rows - 1
                //  } else {
                //      cursor.row + 1
                //  };
                //  bus.video_mut().set_cursor(new_row, cursor.col);
            }
            b'\x08' => {
                // Backspace
                todo!(
                    "ch: backspace, cursor_row: {cursor_row}, cursor_col: {cursor_col}, rows: {rows}"
                );
                //  if cursor.col > 0 {
                //      bus.video_mut().set_cursor(cursor.row, cursor.col - 1);
                //  }
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
                // TODO     let existing_attr = bus.video().read_byte(offset + 1);
                memory_bus.write_u8(CGA_MEMORY_START + offset, ch);
                // TODO     // Preserve existing color, but substitute 0x07 for 0x00 (black on black)
                // TODO     // since text with attribute 0x00 is always invisible. Many BIOS implementations
                // TODO     // do this as a compatibility measure for programs that clear the screen with
                // TODO     // attribute 0x00 before exiting (e.g., EDIT, Checkit).
                // TODO     if existing_attr == 0x00 {
                // TODO         bus.video_mut().write_byte(offset + 1, 0x07);
                // TODO     }
                // TODO  }

                // Advance cursor
                // TODO  let cols = bus.video().get_cols();
                // TODO  let new_col = cursor.col + 1;
                // TODO  if new_col >= cols {
                // TODO      // Wrap to next line
                // TODO      let new_row = if cursor.row >= rows - 1 {
                // TODO          self.scroll_up_internal(bus, 1);
                // TODO          rows - 1
                // TODO      } else {
                // TODO          cursor.row + 1
                // TODO      };
                // TODO      bus.video_mut().set_cursor(new_row, 0);
                // TODO  } else {
                // TODO      bus.video_mut().set_cursor(cursor.row, new_col);
                // TODO  }
            }
        }
    }
}
