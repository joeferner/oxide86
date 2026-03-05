pub(crate) struct TextModePalette {}

impl TextModePalette {
    // TODO
    // Get 8-bit RGB color value (0-255) for rendering
    // pub(crate) fn get_color(color: u8) -> [u8; 3] {
    //     match color & 0x0F {
    //         0 => [0x00, 0x00, 0x00],  // Black
    //         1 => [0x00, 0x00, 0xAA],  // Blue
    //         2 => [0x00, 0xAA, 0x00],  // Green
    //         3 => [0x00, 0xAA, 0xAA],  // Cyan
    //         4 => [0xAA, 0x00, 0x00],  // Red
    //         5 => [0xAA, 0x00, 0xAA],  // Magenta
    //         6 => [0xAA, 0x55, 0x00],  // Brown
    //         7 => [0xAA, 0xAA, 0xAA],  // Light Gray
    //         8 => [0x55, 0x55, 0x55],  // Dark Gray
    //         9 => [0x55, 0x55, 0xFF],  // Light Blue
    //         10 => [0x55, 0xFF, 0x55], // Light Green
    //         11 => [0x55, 0xFF, 0xFF], // Light Cyan
    //         12 => [0xFF, 0x55, 0x55], // Light Red
    //         13 => [0xFF, 0x55, 0xFF], // Light Magenta
    //         14 => [0xFF, 0xFF, 0x55], // Yellow
    //         15 => [0xFF, 0xFF, 0xFF], // White
    //         _ => [0xFF, 0xFF, 0xFF],  // Fallback to white
    //     }
    // }

    /// Get 6-bit RGB color value (0-63) for VGA DAC registers
    pub(crate) fn get_dac_color(color: u8) -> [u8; 3] {
        match color & 0x0F {
            0 => [0x00, 0x00, 0x00],  // Black
            1 => [0x00, 0x00, 0x2A],  // Blue
            2 => [0x00, 0x2A, 0x00],  // Green
            3 => [0x00, 0x2A, 0x2A],  // Cyan
            4 => [0x2A, 0x00, 0x00],  // Red
            5 => [0x2A, 0x00, 0x2A],  // Magenta
            6 => [0x2A, 0x15, 0x00],  // Brown
            7 => [0x2A, 0x2A, 0x2A],  // Light Gray
            8 => [0x15, 0x15, 0x15],  // Dark Gray
            9 => [0x15, 0x15, 0x3F],  // Light Blue
            10 => [0x15, 0x3F, 0x15], // Light Green
            11 => [0x15, 0x3F, 0x3F], // Light Cyan
            12 => [0x3F, 0x15, 0x15], // Light Red
            13 => [0x3F, 0x15, 0x3F], // Light Magenta
            14 => [0x3F, 0x3F, 0x15], // Yellow
            15 => [0x3F, 0x3F, 0x3F], // White
            _ => [0x3F, 0x3F, 0x3F],  // Fallback to white
        }
    }
}
