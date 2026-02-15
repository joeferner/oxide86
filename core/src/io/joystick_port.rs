/// Port 0x201 - IBM Game Control Adapter (Joystick Port)
///
/// The IBM Game Control Adapter supports up to two joysticks (A and B),
/// each with 2 analog axes (X/Y) and 2 buttons.
///
/// # Hardware Protocol
///
/// **Write to 0x201**: Fires all four RC-timer one-shots simultaneously.
/// All axis bits go high and begin timing out based on joystick position.
///
/// **Read from 0x201**: Returns 8-bit value with axis timers and button states:
/// ```text
/// Bit 0 - Joystick A X-axis timer (1=not timed out, 0=timed out)
/// Bit 1 - Joystick A Y-axis timer
/// Bit 2 - Joystick B X-axis timer
/// Bit 3 - Joystick B Y-axis timer
/// Bit 4 - Joystick A Button 1 (0=pressed, 1=released)
/// Bit 5 - Joystick A Button 2
/// Bit 6 - Joystick B Button 1
/// Bit 7 - Joystick B Button 2
/// ```
///
/// # Axis Emulation
///
/// Programs write to 0x201, then loop counting reads until each axis bit drops to 0.
/// The count represents axis position. For emulation at 4.77 MHz:
///
/// - Axis range: ~115–3860 cycles (0%–100% deflection, ~24µs–816µs)
/// - Center: ~2000 cycles (~420µs)
/// - Formula: `timeout_cycles = 115 + axis_value_0_to_1 * 3745`
use crate::joystick::JoystickInput;

/// Minimum cycles for axis timeout (full left/up position)
const MIN_CYCLES: u64 = 115;

/// Maximum cycles for axis timeout (full right/down position)
const MAX_CYCLES: u64 = 3860;

/// Port 0x201 emulation
pub struct JoystickPort {
    joystick: Box<dyn JoystickInput>,
    /// Cycle count when last write fired the one-shots (None = never fired)
    fire_cycle: Option<u64>,
}

impl JoystickPort {
    pub fn new(joystick: Box<dyn JoystickInput>) -> Self {
        Self {
            joystick,
            fire_cycle: None,
        }
    }

    /// Fire all four RC-timer one-shots (write to port 0x201)
    ///
    /// # Parameters
    /// - `current_cycle`: Current CPU cycle count
    pub fn fire(&mut self, current_cycle: u64) {
        self.fire_cycle = Some(current_cycle);
    }

    /// Read joystick port (read from port 0x201)
    ///
    /// # Parameters
    /// - `current_cycle`: Current CPU cycle count
    ///
    /// # Returns
    /// 8-bit value with axis timers (bits 0-3) and button states (bits 4-7)
    pub fn read(&self, current_cycle: u64) -> u8 {
        let mut result = 0u8;

        // Axis bits 0-3 (1 = timer still running, 0 = timed out)
        if let Some(fire) = self.fire_cycle {
            let elapsed = current_cycle.saturating_sub(fire);

            for j in 0..2u8 {
                if !self.joystick.is_connected(j) {
                    // If joystick not connected, axes time out immediately (bits stay 0)
                    continue;
                }

                for a in 0..2u8 {
                    let axis_val = self.joystick.get_axis(j, a);
                    let timeout = MIN_CYCLES + (axis_val * (MAX_CYCLES - MIN_CYCLES) as f32) as u64;
                    let bit = j * 2 + a; // bits 0-3

                    if elapsed < timeout {
                        result |= 1 << bit; // timer still running
                    }
                }
            }
        }

        // Button bits 4-7 (0 = pressed, 1 = released)
        for j in 0..2u8 {
            for b in 0..2u8 {
                let bit = 4 + j * 2 + b;
                let pressed = self.joystick.is_connected(j) && self.joystick.get_button(j, b);

                if !pressed {
                    result |= 1 << bit; // button not pressed (bit = 1)
                }
            }
        }

        result
    }
}
