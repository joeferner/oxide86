use std::{any::Any, cell::RefCell, rc::Rc};

use anyhow::{Context, Result};

pub mod bus;
pub mod computer;
pub mod cpu;
pub mod devices;
pub mod dis;
pub mod disk;
pub mod memory;
pub mod scan_code;
pub mod video;

#[cfg(test)]
pub mod tests;

/// Key press data
#[derive(Debug, Clone, Copy)]
pub(crate) struct KeyPress {
    /// BIOS scan code
    pub scan_code: u8,
    /// ASCII character code
    pub ascii_code: u8,
}

// Calculate physical address from segment:offset
// 8086 has 20 address lines so addresses wrap at 1MB (0x100000)
pub(crate) fn physical_address(segment: u16, offset: u16) -> usize {
    (((segment as usize) << 4) + (offset as usize)) & 0xFFFFF
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

/// Abstraction over memory reads, used by the instruction decoder.
/// Implemented by both `Bus` (emulator) and slice-based readers (disassembler).
pub trait ByteReader {
    fn read_u8(&self, addr: usize) -> u8;

    fn read_u16(&self, addr: usize) -> u16 {
        let lo = self.read_u8(addr) as u16;
        let hi = self.read_u8(addr + 1) as u16;
        (hi << 8) | lo
    }
}

pub trait Device {
    fn as_any(&self) -> &dyn Any;

    fn reset(&mut self);

    fn memory_read_u8(&self, addr: usize, cycle_count: u32) -> Option<u8>;
    fn memory_write_u8(&mut self, addr: usize, val: u8, cycle_count: u32) -> bool;

    fn io_read_u8(&self, port: u16, cycle_count: u32) -> Option<u8>;
    fn io_write_u8(&mut self, port: u16, val: u8, cycle_count: u32) -> bool;
}
