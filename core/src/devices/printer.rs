//! IBM PC parallel port (LPT) device.
//!
//! Each parallel port has three I/O registers at its base address:
//!
//! - **Base+0** — Data register (read/write): the byte to print.
//! - **Base+1** — Status register (read-only):
//!   - Bit 7: /BUSY (1 = not busy, 0 = busy)
//!   - Bit 6: /ACK  (active-low acknowledge pulse)
//!   - Bit 5: Paper Out (1 = out of paper)
//!   - Bit 4: Select (1 = printer selected/online)
//!   - Bit 3: /Error (1 = no error)
//!   - Bits 2–0: reserved
//! - **Base+2** — Control register (read/write):
//!   - Bit 5: bidirectional enable (PS/2)
//!   - Bit 4: IRQ enable
//!   - Bit 3: /Select In (inverted)
//!   - Bit 2: Initialize (active low)
//!   - Bit 1: Auto Linefeed (inverted)
//!   - Bit 0: /Strobe (inverted)
//!
//! Standard base addresses (matching BDA at 0040:0008):
//! - 0x0378 (LPT1), 0x0278 (LPT2), 0x03BC (LPT3)
//!
//! This implementation presents a "ready" printer that silently discards
//! output. Status always reports: not busy, selected, no error, no paper-out.

use std::any::Any;

use crate::Device;

/// Standard status for a ready printer: not busy, ACK, selected, no error.
const STATUS_READY: u8 = 0xDF; // bits 7,6,4,3 + reserved bits set

/// Standard parallel port base addresses.
const LPT_BASES: [u16; 3] = [0x0378, 0x0278, 0x03BC];

/// State for one parallel port.
#[derive(Clone, Default)]
struct LptPort {
    data: u8,
    control: u8,
}

/// Parallel port device covering all three standard LPT ports.
pub struct ParallelPort {
    ports: [LptPort; 3],
}

impl ParallelPort {
    pub fn new() -> Self {
        Self {
            ports: Default::default(),
        }
    }

    /// Find which LPT port (if any) owns this I/O address, and return
    /// (port index, register offset 0–2).
    fn match_port(port: u16) -> Option<(usize, u16)> {
        for (i, &base) in LPT_BASES.iter().enumerate() {
            if port >= base && port <= base + 2 {
                return Some((i, port - base));
            }
        }
        None
    }
}

impl Device for ParallelPort {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.ports = Default::default();
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        let (idx, offset) = Self::match_port(port)?;
        Some(match offset {
            0 => self.ports[idx].data,
            1 => STATUS_READY,
            2 => self.ports[idx].control,
            _ => unreachable!(),
        })
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        let Some((idx, offset)) = Self::match_port(port) else {
            return false;
        };
        match offset {
            0 => self.ports[idx].data = val,
            1 => { /* status register is read-only, ignore writes */ }
            2 => self.ports[idx].control = val,
            _ => unreachable!(),
        }
        true
    }
}

