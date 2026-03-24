use std::{any::Any, cell::RefCell, rc::Rc};

use anyhow::{Context, Result};

pub mod bus;
pub mod computer;
pub mod cpu;
pub mod debugger;
pub mod devices;
pub mod dis;
pub mod disk;
pub mod memory;
pub mod scan_code;
pub mod video;

pub use disk::{DiskGeometry, SECTOR_SIZE};

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

/// Returns `true` if `a` has reached or passed `b` in wrapping u32 arithmetic.
///
/// Uses the standard wrapping-subtraction trick: the difference `a - b` is
/// interpreted as a signed distance; if it is in `[0, 2^31)` then `a >= b`.
/// This is correct as long as `a` and `b` are always within 2^31 of each other.
pub(crate) fn wrapping_ge(a: u32, b: u32) -> bool {
    a.wrapping_sub(b) < 0x8000_0000
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

/// Abstraction over CPU registers + memory, used by the instruction decoder.
/// Implementations provide both register state and physical memory reads.
pub trait Computer {
    fn ax(&self) -> u16;
    fn bx(&self) -> u16;
    fn cx(&self) -> u16;
    fn dx(&self) -> u16;
    fn sp(&self) -> u16;
    fn bp(&self) -> u16;
    fn si(&self) -> u16;
    fn di(&self) -> u16;
    fn cs(&self) -> u16;
    fn ds(&self) -> u16;
    fn ss(&self) -> u16;
    fn es(&self) -> u16;
    /// Read one byte from a physical (20-bit) address.
    fn read_u8(&self, phys: u32) -> u8;
    /// Return the raw 10-byte 80-bit representation and f64 approximation of FPU ST(i).
    fn fpu_st(&self, _i: u8) -> ([u8; 10], f64) {
        ([0; 10], 0.0)
    }
    /// Pre-execution FPU ST(i) — used by the decoder to annotate store instructions
    /// with the value that was stored (before a pop moved the stack).
    fn fpu_st_pre(&self, i: u8) -> ([u8; 10], f64) {
        self.fpu_st(i)
    }
}

pub trait Device {
    fn as_any(&self) -> &dyn Any;

    fn reset(&mut self);

    fn memory_read_u8(&mut self, addr: usize, cycle_count: u32) -> Option<u8>;
    fn memory_write_u8(&mut self, addr: usize, val: u8, cycle_count: u32) -> bool;

    fn io_read_u8(&mut self, port: u16, cycle_count: u32) -> Option<u8>;
    fn io_write_u8(&mut self, port: u16, val: u8, cycle_count: u32) -> bool;
}

#[cfg(test)]
mod test {
    mod wrapping_ge_tests {
        use super::super::wrapping_ge;

        #[test]
        fn equal_values() {
            assert!(wrapping_ge(0, 0));
            assert!(wrapping_ge(100, 100));
            assert!(wrapping_ge(u32::MAX, u32::MAX));
        }

        #[test]
        fn a_ahead_of_b() {
            assert!(wrapping_ge(108, 0));
            assert!(wrapping_ge(200, 100));
        }

        #[test]
        fn a_behind_b() {
            assert!(!wrapping_ge(0, 108));
            assert!(!wrapping_ge(100, 200));
        }

        #[test]
        fn a_just_wrapped_b_has_not() {
            // a wrapped to near 0, b is still near u32::MAX
            let b = u32::MAX - 50;
            let a = 50u32; // a is 101 ahead of b in wrapping arithmetic
            assert!(wrapping_ge(a, b));
        }

        #[test]
        fn a_has_not_wrapped_b_just_did() {
            // b wrapped to near 0, a is still near u32::MAX
            let a = u32::MAX - 50;
            let b = 50u32;
            assert!(!wrapping_ge(a, b));
        }

        #[test]
        fn exactly_at_midpoint_boundary() {
            // distance of exactly 2^31 is treated as "behind" (not >=)
            let a = 0u32;
            let b = 0x8000_0000u32;
            assert!(!wrapping_ge(a, b));

            // one less than midpoint is still "behind"
            let b2 = 0x7FFF_FFFFu32;
            assert!(!wrapping_ge(a, b2));

            // a ahead by exactly 2^31 - 1 of b — wrapping_sub gives 0x7FFF_FFFF < 0x8000_0000
            let a3 = 0x7FFF_FFFFu32;
            let b3 = 0u32;
            assert!(wrapping_ge(a3, b3));
        }
    }
}
