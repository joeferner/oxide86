/// JoystickInput trait for platform-specific joystick handling
///
/// PC joysticks connect via the IBM Game Control Adapter (port 0x201).
/// Up to two joysticks (A and B) can be connected, each with 2 analog axes (X/Y) and 2 buttons.
///
/// Both joystick slots A and B are served by the same trait implementation.
pub trait JoystickInput {
    /// Get normalized axis value: 0.0 = full left/up, 1.0 = full right/down, 0.5 = center
    ///
    /// # Parameters
    /// - `joystick`: Joystick slot (0 = A, 1 = B)
    /// - `axis`: Axis number (0 = X, 1 = Y)
    fn get_axis(&self, joystick: u8, axis: u8) -> f32;

    /// Get button state
    ///
    /// # Parameters
    /// - `joystick`: Joystick slot (0 = A, 1 = B)
    /// - `button`: Button number (0 = button 1, 1 = button 2)
    ///
    /// # Returns
    /// `true` if button is pressed, `false` otherwise
    fn get_button(&self, joystick: u8, button: u8) -> bool;

    /// Check if joystick is connected
    ///
    /// # Parameters
    /// - `joystick`: Joystick slot (0 = A, 1 = B)
    fn is_connected(&self, joystick: u8) -> bool;
}

/// State for a single joystick
#[derive(Debug, Clone, Copy)]
pub struct JoystickState {
    pub x: f32,        // 0.0–1.0
    pub y: f32,        // 0.0–1.0
    pub button1: bool, // true = pressed
    pub button2: bool, // true = pressed
    pub connected: bool,
}

impl Default for JoystickState {
    fn default() -> Self {
        Self {
            x: 0.5,         // center
            y: 0.5,         // center
            button1: false, // not pressed
            button2: false, // not pressed
            connected: false,
        }
    }
}

/// NullJoystick: no joystick connected (default implementation)
pub struct NullJoystick;

impl JoystickInput for NullJoystick {
    fn get_axis(&self, _joystick: u8, _axis: u8) -> f32 {
        0.5 // always at center
    }

    fn get_button(&self, _joystick: u8, _button: u8) -> bool {
        false // no buttons pressed
    }

    fn is_connected(&self, _joystick: u8) -> bool {
        false // no joystick connected
    }
}
