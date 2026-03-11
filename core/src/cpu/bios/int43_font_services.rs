use crate::bus::Bus;

/// Fetch a glyph for `ch` from the INT 43h font vector stored in the IVT.
///
/// INT 43h (IVT entry 0x43, address 0x010C/0x010E) points to a 256-entry font
/// table where character N's data is at `base + N * char_height`.  Games may
/// patch this vector to supply custom glyphs; this function always honours the
/// current value so patched fonts are rendered correctly.
///
/// Returns a `Vec<u8>` of `char_height` bytes (one per pixel row).
pub(super) fn fetch_glyph_int43h(bus: &Bus, ch: u8, char_height: usize) -> Vec<u8> {
    let ivt_offset = bus.memory_read_u16(0x43 * 4) as usize;
    let ivt_segment = bus.memory_read_u16(0x43 * 4 + 2) as usize;
    let base = (ivt_segment << 4) + ivt_offset;
    let glyph_base = base + (ch as usize) * char_height;
    (0..char_height)
        .map(|i| bus.memory_read_u8(glyph_base + i))
        .collect()
}
