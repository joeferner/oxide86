use crate::Bus;
use crate::cpu::Cpu;
use crate::cpu::bios::Bios;
use crate::memory::{
    BDA_MOUSE_BUTTONS, BDA_MOUSE_MAX_X, BDA_MOUSE_MAX_Y, BDA_MOUSE_MIN_X, BDA_MOUSE_MIN_Y,
    BDA_MOUSE_VISIBLE, BDA_MOUSE_X, BDA_MOUSE_Y, BDA_SEGMENT,
};

impl Cpu {
    /// INT 0x33 - Mouse Services
    /// AX register contains the function number
    pub(super) fn handle_int33(&mut self, bus: &mut Bus, io: &mut Bios) {
        let function = self.ax;

        match function {
            0x00 => self.int33_reset_driver(bus, io),
            0x01 => self.int33_show_cursor(bus),
            0x02 => self.int33_hide_cursor(bus),
            0x03 => self.int33_get_position_and_buttons(bus, io),
            0x04 => self.int33_set_cursor_position(bus),
            0x07 => self.int33_set_horizontal_limits(bus),
            0x08 => self.int33_set_vertical_limits(bus),
            0x0B => self.int33_read_motion_counters(io),
            _ => {
                log::warn!("Unhandled INT 0x33 function: AX=0x{:04X}", function);
            }
        }
    }

    /// INT 33h, AX=00h - Reset Driver and Read Status
    /// Input:
    ///   AX = 0000h
    /// Output:
    ///   AX = FFFFh if mouse support is available, 0000h otherwise
    ///   BX = number of buttons (typically 2 or 3)
    fn int33_reset_driver(&mut self, bus: &mut Bus, io: &mut Bios) {
        if io.mouse_is_present() {
            self.ax = 0xFFFF; // Mouse present
            self.bx = 3; // Report 3 buttons (left, right, middle)

            // Initialize mouse state in BDA
            let bda_base = (BDA_SEGMENT as usize) * 16;

            // Set default position (center of screen: 320, 100 for 640x200)
            bus.write_u16(bda_base + BDA_MOUSE_X, 320);
            bus.write_u16(bda_base + BDA_MOUSE_Y, 100);

            // Clear button state
            bus.write_u8(bda_base + BDA_MOUSE_BUTTONS, 0);

            // Set visibility counter to -1 (hidden by default)
            bus.write_u8(bda_base + BDA_MOUSE_VISIBLE, 0xFF); // -1 as signed byte

            // Set default horizontal limits (0-639 for 640x200 mode)
            bus.write_u16(bda_base + BDA_MOUSE_MIN_X, 0);
            bus.write_u16(bda_base + BDA_MOUSE_MAX_X, 639);

            // Set default vertical limits (0-199 for 640x200 mode)
            bus.write_u16(bda_base + BDA_MOUSE_MIN_Y, 0);
            bus.write_u16(bda_base + BDA_MOUSE_MAX_Y, 199);
        } else {
            self.ax = 0x0000; // Mouse not present
            self.bx = 0;
        }
    }

    /// INT 33h, AX=01h - Show Cursor
    /// Input:
    ///   AX = 0001h
    /// Output:
    ///   None
    /// Notes:
    ///   Increments visibility counter. Cursor is visible when counter >= 0
    fn int33_show_cursor(&mut self, bus: &mut Bus) {
        let bda_base = (BDA_SEGMENT as usize) * 16;
        let counter = bus.read_u8(bda_base + BDA_MOUSE_VISIBLE) as i8;
        let new_counter = counter.wrapping_add(1);
        bus.write_u8(bda_base + BDA_MOUSE_VISIBLE, new_counter as u8);
    }

    /// INT 33h, AX=02h - Hide Cursor
    /// Input:
    ///   AX = 0002h
    /// Output:
    ///   None
    /// Notes:
    ///   Decrements visibility counter. Cursor is visible when counter >= 0
    fn int33_hide_cursor(&mut self, bus: &mut Bus) {
        let bda_base = (BDA_SEGMENT as usize) * 16;
        let counter = bus.read_u8(bda_base + BDA_MOUSE_VISIBLE) as i8;
        let new_counter = counter.wrapping_sub(1);
        bus.write_u8(bda_base + BDA_MOUSE_VISIBLE, new_counter as u8);
    }

    /// INT 33h, AX=03h - Get Position and Button Status
    /// Input:
    ///   AX = 0003h
    /// Output:
    ///   BX = button status (bit 0=left, bit 1=right, bit 2=middle)
    ///   CX = horizontal position (column)
    ///   DX = vertical position (row)
    fn int33_get_position_and_buttons(&mut self, bus: &mut Bus, io: &mut Bios) {
        // Get current state from mouse device
        let state = io.mouse_get_state();

        let bda_base = (BDA_SEGMENT as usize) * 16;

        // Clamp position to boundaries
        let min_x = bus.read_u16(bda_base + BDA_MOUSE_MIN_X);
        let max_x = bus.read_u16(bda_base + BDA_MOUSE_MAX_X);
        let min_y = bus.read_u16(bda_base + BDA_MOUSE_MIN_Y);
        let max_y = bus.read_u16(bda_base + BDA_MOUSE_MAX_Y);

        let x = state.x.max(min_x).min(max_x);
        let y = state.y.max(min_y).min(max_y);

        // Update BDA with clamped position
        bus.write_u16(bda_base + BDA_MOUSE_X, x);
        bus.write_u16(bda_base + BDA_MOUSE_Y, y);

        // Build button status byte
        let mut buttons = 0u8;
        if state.left_button {
            buttons |= 0x01;
        }
        if state.right_button {
            buttons |= 0x02;
        }
        if state.middle_button {
            buttons |= 0x04;
        }

        bus.write_u8(bda_base + BDA_MOUSE_BUTTONS, buttons);

        // Return values in registers
        self.bx = buttons as u16;
        self.cx = x;
        self.dx = y;
    }

    /// INT 33h, AX=04h - Set Cursor Position
    /// Input:
    ///   AX = 0004h
    ///   CX = horizontal position (column)
    ///   DX = vertical position (row)
    /// Output:
    ///   None
    fn int33_set_cursor_position(&mut self, bus: &mut Bus) {
        let x = self.cx;
        let y = self.dx;

        let bda_base = (BDA_SEGMENT as usize) * 16;

        // Clamp to boundaries
        let min_x = bus.read_u16(bda_base + BDA_MOUSE_MIN_X);
        let max_x = bus.read_u16(bda_base + BDA_MOUSE_MAX_X);
        let min_y = bus.read_u16(bda_base + BDA_MOUSE_MIN_Y);
        let max_y = bus.read_u16(bda_base + BDA_MOUSE_MAX_Y);

        let clamped_x = x.max(min_x).min(max_x);
        let clamped_y = y.max(min_y).min(max_y);

        bus.write_u16(bda_base + BDA_MOUSE_X, clamped_x);
        bus.write_u16(bda_base + BDA_MOUSE_Y, clamped_y);
    }

    /// INT 33h, AX=07h - Set Horizontal Min/Max Position
    /// Input:
    ///   AX = 0007h
    ///   CX = minimum horizontal position
    ///   DX = maximum horizontal position
    /// Output:
    ///   None
    fn int33_set_horizontal_limits(&mut self, bus: &mut Bus) {
        let min_x = self.cx;
        let max_x = self.dx;

        let bda_base = (BDA_SEGMENT as usize) * 16;
        bus.write_u16(bda_base + BDA_MOUSE_MIN_X, min_x);
        bus.write_u16(bda_base + BDA_MOUSE_MAX_X, max_x);

        // Clamp current position to new limits
        let current_x = bus.read_u16(bda_base + BDA_MOUSE_X);
        let clamped_x = current_x.max(min_x).min(max_x);
        bus.write_u16(bda_base + BDA_MOUSE_X, clamped_x);
    }

    /// INT 33h, AX=08h - Set Vertical Min/Max Position
    /// Input:
    ///   AX = 0008h
    ///   CX = minimum vertical position
    ///   DX = maximum vertical position
    /// Output:
    ///   None
    fn int33_set_vertical_limits(&mut self, bus: &mut Bus) {
        let min_y = self.cx;
        let max_y = self.dx;

        let bda_base = (BDA_SEGMENT as usize) * 16;
        bus.write_u16(bda_base + BDA_MOUSE_MIN_Y, min_y);
        bus.write_u16(bda_base + BDA_MOUSE_MAX_Y, max_y);

        // Clamp current position to new limits
        let current_y = bus.read_u16(bda_base + BDA_MOUSE_Y);
        let clamped_y = current_y.max(min_y).min(max_y);
        bus.write_u16(bda_base + BDA_MOUSE_Y, clamped_y);
    }

    /// INT 33h, AX=0Bh - Read Motion Counters
    /// Input:
    ///   AX = 000Bh
    /// Output:
    ///   CX = horizontal mickey count (signed)
    ///   DX = vertical mickey count (signed)
    /// Notes:
    ///   Motion counters are reset to 0 after reading
    ///   Mickeys are raw mouse movement units (typically 8 mickeys per pixel)
    fn int33_read_motion_counters(&mut self, io: &mut Bios) {
        let (motion_x, motion_y) = io.mouse_get_motion();

        self.cx = motion_x as u16;
        self.dx = motion_y as u16;
    }
}
