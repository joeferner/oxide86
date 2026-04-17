/// Converts a BCD-encoded byte to its decimal value.
pub(crate) fn bcd_to_dec(v: u8) -> u8 {
    (v >> 4) * 10 + (v & 0x0F)
}
