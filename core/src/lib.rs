use std::{any::Any, cell::RefCell, rc::Rc};

use anyhow::{Context, Result};

pub mod bus;
pub mod computer;
pub mod cpu;
pub mod devices;
pub mod disk;
pub mod memory;
pub mod scan_code;
pub mod video;

#[cfg(test)]
pub mod tests;

/// Key press data
#[derive(Debug, Clone, Copy)]
pub struct KeyPress {
    /// BIOS scan code
    pub scan_code: u8,
    /// ASCII character code
    pub ascii_code: u8,
}

// Calculate physical address from segment:offset
pub(crate) fn physical_address(segment: u16, offset: u16) -> usize {
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

pub(crate) fn byte_to_printable_char(v: u8) -> char {
    if (0x20..0x7F).contains(&v) {
        v as char
    } else {
        '.'
    }
}

pub type DeviceRef = Rc<RefCell<dyn Device>>;

pub trait Device {
    fn as_any(&self) -> &dyn Any;

    fn reset(&mut self);

    fn memory_read_u8(&self, addr: usize) -> Option<u8>;
    fn memory_write_u8(&mut self, addr: usize, val: u8) -> bool;

    fn io_read_u8(&self, port: u16) -> Option<u8>;
    fn io_write_u8(&mut self, port: u16, val: u8) -> bool;
}
