//! Physical serial loopback device.
//!
//! Simulates a loopback plug wired with TX connected directly to RX.
//! Every byte written is immediately available to read back, in FIFO order.
//! There is no baud-rate modelling; bytes are available instantly.

use std::collections::VecDeque;

use crate::devices::uart::{ComPortDevice, ModemControlLines};

/// A physical loopback plug: TX → RX, modem outputs wired back to modem inputs.
///
/// Bytes written via `write` are queued and returned by `read` in FIFO order.
/// An IRQ is signalled after each write so the UART driver can poll the
/// received byte.
///
/// Modem line wiring (RS-232 loopback plug):
///   RTS (MCR bit 1) → CTS (MSR bit 4)
///   DTR (MCR bit 0) → DSR (MSR bit 5) + RI (MSR bit 6) + DCD (MSR bit 7)
///
/// Delta bits (MSR bits 3:0) are set whenever the corresponding modem input
/// changes state and are cleared after each call to `modem_status`.
pub struct SerialLoopback {
    buf: VecDeque<u8>,
    irq_pending: bool,
    /// MSR bits 7:4 driven by the loopback wiring from the MCR outputs.
    msr_high: u8,
    /// MSR bits 3:0: pending delta/change bits, cleared on modem_status read.
    msr_delta: u8,
}

impl SerialLoopback {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::new(),
            irq_pending: false,
            msr_high: 0,
            msr_delta: 0,
        }
    }
}

impl ComPortDevice for SerialLoopback {
    fn reset(&mut self) {
        self.buf.clear();
        self.irq_pending = false;
        self.msr_high = 0;
        self.msr_delta = 0;
    }

    fn read(&mut self) -> Option<u8> {
        let byte = self.buf.pop_front();
        self.irq_pending = !self.buf.is_empty();
        byte
    }

    fn write(&mut self, value: u8) -> bool {
        self.buf.push_back(value);
        self.irq_pending = true;
        true
    }

    fn take_irq(&mut self) -> bool {
        let pending = self.irq_pending;
        self.irq_pending = false;
        pending
    }

    fn modem_control_changed(&mut self, lines: ModemControlLines) {
        // Physical loopback wiring: RTS → CTS, DTR → DSR + RI + DCD
        let new_high = if lines.rts { 0x10 } else { 0 } // CTS (bit 4)
            | if lines.dtr { 0x20 } else { 0 } // DSR (bit 5)
            | if lines.dtr { 0x40 } else { 0 } // RI  (bit 6)
            | if lines.dtr { 0x80 } else { 0 }; // DCD (bit 7)

        // Compute delta bits from state change:
        //   bit 0: DCTS  — CTS changed (any direction)
        //   bit 1: DDSR  — DSR changed (any direction)
        //   bit 2: TERI  — RI trailing edge (1 → 0 only)
        //   bit 3: DDCD  — DCD changed (any direction)
        let changed = new_high ^ self.msr_high;
        let mut delta = 0u8;
        if changed & 0x10 != 0 {
            delta |= 0x01;
        } // DCTS
        if changed & 0x20 != 0 {
            delta |= 0x02;
        } // DDSR
        if changed & 0x40 != 0 && self.msr_high & 0x40 != 0 {
            delta |= 0x04;
        } // TERI (1→0)
        if changed & 0x80 != 0 {
            delta |= 0x08;
        } // DDCD

        self.msr_high = new_high;
        self.msr_delta |= delta;
    }

    fn modem_status(&mut self) -> u8 {
        // Return full MSR byte (bits 7:4 = modem lines, bits 3:0 = delta).
        // Delta bits are cleared on read, matching real UART behaviour.
        let val = self.msr_high | self.msr_delta;
        self.msr_delta = 0;
        val
    }
}
