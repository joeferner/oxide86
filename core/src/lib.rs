use anyhow::{Context, Result};

pub mod computer;
pub mod cpu;
pub mod memory;
pub mod memory_bus;
pub mod io_bus;
pub mod video;

#[cfg(test)]
pub mod tests;

// Calculate physical address from segment:offset
pub fn physical_address(segment: u16, offset: u16) -> usize {
    ((segment as usize) << 4) + (offset as usize)
}

pub fn parse_hex_or_dec(s: &str) -> Result<u16> {
    if let Some(hex) = s.strip_prefix("0x") {
        u16::from_str_radix(hex, 16).with_context(|| format!("Invalid hex value: {}", s))
    } else {
        s.parse::<u16>()
            .with_context(|| format!("Invalid decimal value: {}", s))
    }
}

pub trait Device {
    fn read_u8(&self, addr: usize) -> Option<u8>;
    fn write_u8(&mut self, addr: usize, val: u8) -> bool;
}
