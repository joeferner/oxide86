use std::{
    any::Any,
    sync::{Arc, RwLock},
};

use crate::Device;

pub const COM1_ADDR: u16 = 0x03F8;
pub const COM2_ADDR: u16 = 0x02F8;
pub const COM3_ADDR: u16 = 0x03E8;
pub const COM4_ADDR: u16 = 0x02E8;
const ADDRESSES: [u16; 4] = [COM1_ADDR, COM2_ADDR, COM3_ADDR, COM4_ADDR];

// UART crystal is 1.8432 MHz; divisor = 1843200 / (baud * 16)
//                          110  150  300  600  1200 2400 4800 9600 baud
pub const DIVISOR_TABLE: [u16; 8] = [1047, 768, 384, 192, 96, 48, 24, 12];

/// RBR/THR/DLL - DLAB=0: recv/transmit, DLAB=1: divisor low
pub const DLL: u16 = 0;
/// IER/DLM - DLAB=0: irq enable, DLAB=1: divisor high
pub const DLM: u16 = 1;
/// LCR - DLAB=0: line control, DLAB=1: (same, bit7=DLAB)
pub const LCR: u16 = 3;
/// MCR - modem control
pub const MCR: u16 = 4;
/// LSR - line status
pub const LSR: u16 = 5;
/// MSR - modem status
pub const MSR: u16 = 6;

// LSR bit masks
/// LSR bit 0 - Data Ready: a byte is available in RBR
pub const LSR_DR: u8 = 0x01;
/// LSR bit 5 - Transmitter Holding Register Empty: THR can accept a new byte
pub const LSR_THRE: u8 = 0x20;
/// LSR bit 6 - Transmitter Empty: both THR and TSR are empty
pub const LSR_TEMT: u8 = 0x40;
/// LSR bit 7 - used by BIOS INT 14h to signal a timeout (not a real UART bit)
pub const LSR_TIMEOUT: u8 = 0x80;

struct Port {
    dll: u8,         // Divisor Latch Low  (offset 0, DLAB=1)
    dlm: u8,         // Divisor Latch High (offset 1, DLAB=1)
    ier: u8,         // Interrupt Enable   (offset 1, DLAB=0)
    fcr: u8,         // FIFO Control       (offset 2, write-only; read returns IIR)
    lcr: u8,         // Line Control       (offset 3)
    mcr: u8,         // Modem Control      (offset 4)
    lsr: u8,         // Line Status        (offset 5)
    msr: u8,         // Modem Status       (offset 6)
    rbr: u8,         // Receive Buffer     (offset 0, DLAB=0, read)
    thr: Option<u8>, // Buffered TX byte when device wasn't ready; None means THR is empty
    device: Option<Arc<RwLock<dyn ComPortDevice>>>,
}

impl Port {
    pub(crate) fn reset(&mut self) {
        self.dll = 0x18; // divisor low  for 4800 baud (24 = 0x0018)
        self.dlm = 0x00; // divisor high for 4800 baud
        self.ier = 0x00; // all interrupts disabled
        self.fcr = 0x00;
        self.lcr = 0x03; // 8-N-1, DLAB=0
        self.mcr = 0x00;
        self.lsr = LSR_THRE | LSR_TEMT; // transmitter ready
        self.msr = 0x00;
        self.rbr = 0x00;
        self.thr = None;
        if let Some(device) = &self.device {
            device.write().unwrap().reset();
        }
    }

    /// Retry sending a buffered THR byte to the device. If the device now accepts
    /// it, clears the buffer and restores THRE + TEMT so the BIOS polling loop
    /// can proceed. Called on every LSR read so THRE tracks device readiness.
    fn flush_tx(&mut self) {
        let Some(byte) = self.thr else { return };
        let Some(ref dev) = self.device else {
            // No device attached: discard the byte and unblock THRE.
            self.thr = None;
            self.lsr |= LSR_THRE | LSR_TEMT;
            return;
        };
        if let Ok(mut guard) = dev.write()
            && guard.write(byte)
        {
            self.thr = None;
            self.lsr |= LSR_THRE | LSR_TEMT;
        }
    }

    /// Poll the attached device for an incoming byte and update RBR + LSR.DR.
    /// Called before any read of RBR (offset 0) or LSR (offset 5) so the BIOS
    /// sees an up-to-date Data-Ready bit.
    fn poll_rx(&mut self) {
        // Don't overwrite an unread byte already sitting in RBR.
        if self.lsr & LSR_DR != 0 {
            return;
        }
        let Some(ref dev) = self.device else { return };
        if let Ok(mut guard) = dev.write()
            && let Some(byte) = guard.read()
        {
            self.rbr = byte;
            self.lsr |= LSR_DR; // set DR (Data Ready)
        }
    }
}

impl Default for Port {
    fn default() -> Self {
        Self {
            dll: 0x18, // divisor low  for 4800 baud (24 = 0x0018)
            dlm: 0x00, // divisor high for 4800 baud
            ier: 0x00, // all interrupts disabled
            fcr: 0x00,
            lcr: 0x03, // 8-N-1, DLAB=0
            mcr: 0x00,
            lsr: LSR_THRE | LSR_TEMT, // transmitter ready
            msr: 0x00,
            rbr: 0x00,
            thr: None,
            device: None,
        }
    }
}

/// IIR (read offset 2) — no interrupt pending, FIFOs disabled
const IIR_NO_INT: u8 = 0x01;

/// MCR (Modem Control Register) signal lines, passed to [`ComPortDevice::modem_control_changed`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModemControlLines {
    /// DTR — Data Terminal Ready (MCR bit 0)
    pub dtr: bool,
    /// RTS — Request To Send (MCR bit 1)
    pub rts: bool,
    /// OUT1 — general-purpose output 1 (MCR bit 2)
    pub out1: bool,
    /// OUT2 — general-purpose output 2 / IRQ enable on AT hardware (MCR bit 3)
    pub out2: bool,
    /// LOOP — internal loopback mode (MCR bit 4)
    pub loopback: bool,
}

impl ModemControlLines {
    pub fn from_mcr(mcr: u8) -> Self {
        Self {
            dtr: mcr & 0x01 != 0,
            rts: mcr & 0x02 != 0,
            out1: mcr & 0x04 != 0,
            out2: mcr & 0x08 != 0,
            loopback: mcr & 0x10 != 0,
        }
    }
}

pub trait ComPortDevice {
    fn reset(&mut self);

    /// try reading a value from the device. If a value is not ready return None.
    fn read(&mut self) -> Option<u8>;

    /// try writing a value to the device. If the device is not ready to write return false.
    fn write(&mut self, value: u8) -> bool;

    /// Returns and clears a pending IRQ condition on this device (e.g. RX data available).
    fn take_irq(&mut self) -> bool;

    /// Called whenever the MCR is written and the modem control lines change.
    /// The default implementation does nothing.
    fn modem_control_changed(&mut self, lines: ModemControlLines);

    /// Returns the current modem status bits 7:4 (DCD/RI/DSR/CTS) driven by this device.
    /// These are OR'd into MSR bits 7:4 when the UART is not in internal loopback mode.
    /// The default implementation returns 0 (no modem lines asserted).
    fn modem_status(&mut self) -> u8 {
        0
    }
}

pub(crate) struct Uart {
    ports: [Port; 4],
}

impl Uart {
    pub(crate) fn new() -> Self {
        Self {
            ports: Default::default(),
        }
    }

    /// Returns and clears a pending IRQ for the given port (0-based).
    /// Gated on IER bit 0 (ERBFI — Received Data Available interrupt enable).
    pub(crate) fn take_pending_irq(&self, port_idx: usize) -> bool {
        let p = &self.ports[port_idx];
        if p.ier & 0x01 == 0 {
            return false; // ERBFI not enabled
        }
        // Data Ready set — covers loopback and direct RBR writes
        if p.lsr & LSR_DR != 0 {
            return true;
        }
        // Fall back to device-level IRQ check
        let Some(ref dev) = p.device else {
            return false;
        };
        dev.write().is_ok_and(|mut g| g.take_irq())
    }

    pub(crate) fn set_com_port_device(
        &mut self,
        port: u8,
        device: Option<Arc<RwLock<dyn ComPortDevice>>>,
    ) {
        if !(1..=4).contains(&port) {
            panic!("port out of range must be 1-4");
        }
        self.ports[(port - 1) as usize].device = device;
    }
}

impl Device for Uart {
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
        let (idx, offset) = port_index(port)?;
        let p = &mut self.ports[idx];
        let dlab = p.lcr & 0x80 != 0;
        let val = match offset {
            0 => {
                if dlab {
                    p.dll
                } else {
                    // RBR read: poll device first, then return buffered byte and clear DR
                    p.poll_rx();
                    let byte = p.rbr;
                    p.lsr &= !LSR_DR; // clear DR — byte consumed
                    byte
                }
            }
            1 => {
                if dlab {
                    p.dlm
                } else {
                    p.ier
                }
            }
            2 => IIR_NO_INT, // IIR: no interrupt pending, FIFOs disabled
            3 => p.lcr,
            4 => p.mcr,
            5 => {
                // LSR read: flush any buffered TX byte (updates THRE) then poll for RX
                p.flush_tx();
                p.poll_rx();
                p.lsr
            }
            6 => {
                // MSR read: bits 7:4 (DCD/RI/DSR/CTS) are driven either by the UART's
                // internal loopback (MCR bit 4) or by the attached device's modem status.
                // Bits 3:0 are delta bits (cleared on read — real UART behavior).
                let val = if p.mcr & 0x10 != 0 {
                    // Internal loopback: MCR output lines feed back to MSR inputs.
                    // DTR(bit0)→DSR(bit5), RTS(bit1)→CTS(bit4), OUT1(bit2)→RI(bit6), OUT2(bit3)→DCD(bit7)
                    let loopback_high = ((p.mcr & 0x01) << 5) // DTR  → DSR (bit 5)
                        | ((p.mcr & 0x02) << 3) // RTS  → CTS (bit 4)
                        | ((p.mcr & 0x04) << 4) // OUT1 → RI  (bit 6)
                        | ((p.mcr & 0x08) << 4); // OUT2 → DCD (bit 7)
                    (p.msr & 0x0F) | loopback_high
                } else if let Some(ref dev) = p.device {
                    // External device: modem_status returns bits 7:4 (modem lines) and
                    // bits 3:0 (delta bits, cleared on read inside the device).
                    let dev_msr = dev.write().map_or(0, |mut g| g.modem_status());
                    (p.msr & 0x0F) | dev_msr
                } else {
                    p.msr
                };
                p.msr &= !0x0F; // delta bits are cleared on read
                val
            }
            _ => return None,
        };
        Some(val)
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        let Some((idx, offset)) = port_index(port) else {
            return false;
        };
        let p = &mut self.ports[idx];
        let dlab = p.lcr & 0x80 != 0;
        match offset {
            0 => {
                if dlab {
                    p.dll = val;
                } else if p.mcr & 0x10 != 0 {
                    // MCR loopback mode: byte written to THR loops back directly to RBR.
                    // THRE/TEMT remain set (no physical transmission).
                    p.rbr = val;
                    p.lsr |= LSR_DR;
                } else {
                    // THR write: try to forward byte to the attached device.
                    // If the device isn't ready, buffer the byte and clear THRE/TEMT
                    // so the BIOS polling loop stalls until flush_tx succeeds.
                    let accepted = if let Some(ref dev) = p.device {
                        dev.write().is_ok_and(|mut g| g.write(val))
                    } else {
                        true // no device: silently discard
                    };
                    if !accepted {
                        p.thr = Some(val);
                        p.lsr &= !(LSR_THRE | LSR_TEMT);
                    }
                }
            }
            1 => {
                if dlab {
                    p.dlm = val;
                } else {
                    p.ier = val & 0x0F; // IER only implements bits 3:0 on real 8250/16550
                }
            }
            2 => p.fcr = val, // FCR: FIFO control (stored but not acted on)
            3 => p.lcr = val,
            4 => {
                let val = val & 0x1F; // MCR only implements bits 4:0 on real 8250/16550
                let prev = p.mcr;
                p.mcr = val;
                if val & 0x10 != 0 {
                    // Loopback mode: output modem lines feed back to MSR inputs.
                    // DTR(bit0)→DSR(bit5), RTS(bit1)→CTS(bit4), OUT1(bit2)→RI(bit6), OUT2(bit3)→DCD(bit7)
                    p.msr = ((val & 0x01) << 5) // DTR → DSR
                          | ((val & 0x02) << 3) // RTS → CTS
                          | ((val & 0x04) << 4) // OUT1 → RI
                          | ((val & 0x08) << 4); // OUT2 → DCD
                } else if val != prev
                    && let Some(ref dev) = p.device
                {
                    let lines = ModemControlLines::from_mcr(val);
                    if let Ok(mut guard) = dev.write() {
                        guard.modem_control_changed(lines);
                    }
                }
            }
            5 => p.lsr = val,
            6 => p.msr = val,
            _ => return false,
        }
        true
    }
}

/// Returns `(port_index, register_offset)` for a given I/O address, or `None`
/// if the address doesn't fall within any COM port range (base..base+8).
fn port_index(addr: u16) -> Option<(usize, u16)> {
    for (i, &base) in ADDRESSES.iter().enumerate() {
        if addr >= base && addr < base + 8 {
            return Some((i, addr - base));
        }
    }
    None
}

pub(crate) fn encode_parity(p: u8) -> u8 {
    match p {
        0b00 => 0x00, // no parity
        0b01 => 0x08, // odd:  PEN=1, EPS=0  → LCR bit3
        0b10 => 0x00, // no parity
        0b11 => 0x18, // even: PEN=1, EPS=1  → LCR bits 4:3
        _ => 0x00,
    }
}
