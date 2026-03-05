use crate::video::{
    font::{CHAR_HEIGHT, CHAR_WIDTH, Cp437Font},
    text::TextAttribute,
};

/// Convert a 6-bit VGA DAC value (0-63) to 8-bit RGB (0-255).
#[inline]
pub fn dac_to_8bit(val: u8) -> u8 {
    let v = val & 0x3F;
    (v << 2) | (v >> 4)
}

pub struct RenderTextArgs<'a> {
    pub font: &'a Cp437Font,
    pub row: usize,
    pub col: usize,
    pub character: u8,
    pub text_attr: TextAttribute,
    pub vga_dac_palette: &'a [[u8; 3]; 256],
    pub stride: usize,
}

/// Render a single text cell to an RGBA buffer.
///
/// Uses VGA DAC palette for foreground/background colors. The output buffer
/// must be at least `stride * (row * CHAR_HEIGHT + CHAR_HEIGHT) * 4` bytes.
///
/// # Arguments
/// * `font` - CP437 font for glyph lookup
/// * `row` - Character row position (0-based)
/// * `col` - Character column position (0-based)
/// * `cell` - The text cell (character + attribute)
/// * `vga_dac_palette` - 256-entry VGA DAC palette (6-bit RGB per component)
/// * `stride` - Output buffer width in pixels (e.g., 640)
/// * `output` - RGBA buffer to write to
pub(crate) fn render_text(args: RenderTextArgs, output: &mut [u8]) {
    let glyph = args.font.get_glyph(args.character);
    let fg_dac = args.vga_dac_palette[args.text_attr.foreground as usize];
    let bg_dac = args.vga_dac_palette[args.text_attr.background as usize];
    let fg_color = [
        dac_to_8bit(fg_dac[0]),
        dac_to_8bit(fg_dac[1]),
        dac_to_8bit(fg_dac[2]),
    ];
    let bg_color = [
        dac_to_8bit(bg_dac[0]),
        dac_to_8bit(bg_dac[1]),
        dac_to_8bit(bg_dac[2]),
    ];

    let char_x = args.col * CHAR_WIDTH;
    let char_y = args.row * CHAR_HEIGHT;

    for (glyph_row, &glyph_byte) in glyph.iter().enumerate() {
        let pixel_y = char_y + glyph_row;

        for bit in 0..8 {
            let pixel_x = char_x + bit;
            let is_fg = (glyph_byte & (0x80 >> bit)) != 0;
            let color = if is_fg { fg_color } else { bg_color };

            let offset = (pixel_y * args.stride + pixel_x) * 4;
            output[offset] = color[0];
            output[offset + 1] = color[1];
            output[offset + 2] = color[2];
            output[offset + 3] = 0xFF;
        }
    }
}
