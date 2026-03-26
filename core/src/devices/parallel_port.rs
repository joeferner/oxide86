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
//!   - Bit 0: /Strobe (inverted, active low)
//!
//! Standard base addresses (matching BDA at 0040:0008):
//! - 0x0378 (LPT1), 0x0278 (LPT2), 0x03BC (LPT3)
//!
//! When no device is connected the port presents a "ready" printer that
//! silently discards output. When a [`LptPortDevice`] is attached it drives
//! the status register and receives data on each strobe pulse.

use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use crate::Device;

/// Standard status for a ready printer with no device: not busy, ACK,
/// selected, no error, no paper-out.
const STATUS_READY: u8 = 0xDF; // bits 7,6,4,3 + reserved bits set

/// Standard parallel port base addresses in IBM PC detection order.
/// The MDA card's built-in port (0x03BC) is detected first and becomes LPT1.
const LPT_BASES: [u16; 3] = [0x03BC, 0x0378, 0x0278];

/// A device that can be connected to a parallel port.
pub trait LptPortDevice {
    fn reset(&mut self);

    /// Called on every write to the data register (Base+0), before any strobe.
    /// Use this to track the current data bus value for status feedback.
    /// The default implementation does nothing.
    fn data_changed(&mut self, _data: u8) {}

    /// Called whenever the control register (Base+2) is written.
    /// Use this to track control line state for status feedback.
    /// The default implementation does nothing.
    fn control_changed(&mut self, _control: u8) {}

    /// Called when the host pulses Strobe (control bit 0 rising 0→1) with
    /// `data` on the data lines. Return `true` if the byte was accepted,
    /// `false` if the device is busy.
    fn write(&mut self, data: u8) -> bool;

    /// Returns the current status register bits 7:3.
    ///
    /// Bit layout (same as the hardware status register):
    /// - Bit 7: /BUSY  (1 = not busy)
    /// - Bit 6: /ACK   (1 = no acknowledge pulse)
    /// - Bit 5: Paper Out (1 = out of paper)
    /// - Bit 4: Select (1 = printer online)
    /// - Bit 3: /Error (1 = no error)
    ///
    /// Bits 2:0 are reserved and ignored by the caller.
    fn status(&mut self) -> u8;
}

/// State for one parallel port.
#[derive(Default)]
struct LptPort {
    data: u8,
    control: u8,
    device: Option<Arc<RwLock<dyn LptPortDevice>>>,
}

impl LptPort {
    fn reset(&mut self) {
        self.data = 0;
        self.control = 0;
        if let Some(ref dev) = self.device {
            let mut guard = dev.write().unwrap();
            guard.reset();
            guard.data_changed(0);
            guard.control_changed(0);
        }
    }

    /// Read the status register: use the attached device's status if present,
    /// otherwise report a permanently-ready printer.
    fn read_status(&mut self) -> u8 {
        if let Some(ref dev) = self.device {
            dev.write().map_or(STATUS_READY, |mut g| g.status())
        } else {
            STATUS_READY
        }
    }

    /// Write the control register. Notifies the device of the new control value
    /// and triggers `write()` when Strobe is asserted (bit 0 rising 0→1).
    ///
    /// Control bit polarity (hardware inverts bits 0, 1, 3 at the pin):
    /// - Bit 0: Strobe   — 1 = asserted (pin 1 LOW)
    /// - Bit 1: Auto-LF  — 1 = asserted (pin 14 LOW)
    /// - Bit 2: /Init    — 0 = asserted (pin 16 LOW, not inverted)
    /// - Bit 3: Sel-In   — 1 = asserted (pin 17 LOW)
    fn write_control(&mut self, val: u8) {
        let prev_strobe = self.control & 0x01;
        self.control = val;
        let new_strobe = val & 0x01;
        if let Some(ref dev) = self.device {
            let _ = dev.write().map(|mut g| g.control_changed(val));
            // Strobe is active when bit 0 = 1. Trigger write() on the rising
            // edge (0→1): this is when the printer latches the data register.
            if prev_strobe == 0 && new_strobe == 1 {
                let data = self.data;
                let _ = dev.write().map(|mut g| g.write(data));
            }
        }
    }
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

    /// Attach or detach a device on the given LPT port (1-based: 1=LPT1,
    /// 2=LPT2, 3=LPT3). Pass `None` to detach.
    pub fn set_lpt_device(&mut self, port: u8, device: Option<Arc<RwLock<dyn LptPortDevice>>>) {
        assert!((1..=3).contains(&port), "LPT port must be 1–3");
        self.ports[(port - 1) as usize].device = device;
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
        for port in &mut self.ports {
            port.reset();
        }
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
            1 => self.ports[idx].read_status(),
            2 => self.ports[idx].control,
            _ => unreachable!(),
        })
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        let Some((idx, offset)) = Self::match_port(port) else {
            return false;
        };
        match offset {
            0 => {
                self.ports[idx].data = val;
                if let Some(ref dev) = self.ports[idx].device {
                    let _ = dev.write().map(|mut g| g.data_changed(val));
                }
            }
            1 => { /* status register is read-only, ignore writes */ }
            2 => self.ports[idx].write_control(val),
            _ => unreachable!(),
        }
        true
    }
}
