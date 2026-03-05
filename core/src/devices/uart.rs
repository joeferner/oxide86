use std::{
    any::Any,
    cell::Cell,
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
    dll: u8,               // Divisor Latch Low  (offset 0, DLAB=1)
    dlm: u8,               // Divisor Latch High (offset 1, DLAB=1)
    ier: u8,               // Interrupt Enable   (offset 1, DLAB=0)
    lcr: u8,               // Line Control       (offset 3)
    mcr: u8,               // Modem Control      (offset 4)
    lsr: Cell<u8>, // Line Status        (offset 5) — Cell for interior mutability in io_read_u8
    msr: u8,       // Modem Status       (offset 6)
    rbr: Cell<u8>, // Receive Buffer     (offset 0, DLAB=0, read) — Cell for interior mutability
    thr: Cell<Option<u8>>, // Buffered TX byte when device wasn't ready; None means THR is empty
    device: Option<Arc<RwLock<dyn ComPortDevice>>>,
}

impl Port {
    pub(crate) fn reset(&mut self) {
        self.dll = 0x18; // divisor low  for 4800 baud (24 = 0x0018)
        self.dlm = 0x00; // divisor high for 4800 baud
        self.ier = 0x00; // all interrupts disabled
        self.lcr = 0x03; // 8-N-1, DLAB=0
        self.mcr = 0x00;
        self.lsr.set(LSR_THRE | LSR_TEMT); // transmitter ready
        self.msr = 0x00;
        self.rbr.set(0x00);
        self.thr.set(None);
    }

    /// Retry sending a buffered THR byte to the device. If the device now accepts
    /// it, clears the buffer and restores THRE + TEMT so the BIOS polling loop
    /// can proceed. Called on every LSR read so THRE tracks device readiness.
    fn flush_tx(&self) {
        let Some(byte) = self.thr.get() else { return };
        let Some(ref dev) = self.device else {
            // No device attached: discard the byte and unblock THRE.
            self.thr.set(None);
            self.lsr.set(self.lsr.get() | LSR_THRE | LSR_TEMT);
            return;
        };
        if let Ok(mut guard) = dev.write()
            && guard.write(byte)
        {
            self.thr.set(None);
            self.lsr.set(self.lsr.get() | LSR_THRE | LSR_TEMT);
        }
    }

    /// Poll the attached device for an incoming byte and update RBR + LSR.DR.
    /// Called before any read of RBR (offset 0) or LSR (offset 5) so the BIOS
    /// sees an up-to-date Data-Ready bit.
    fn poll_rx(&self) {
        // Don't overwrite an unread byte already sitting in RBR.
        if self.lsr.get() & LSR_DR != 0 {
            return;
        }
        let Some(ref dev) = self.device else { return };
        if let Ok(mut guard) = dev.write()
            && let Some(byte) = guard.read()
        {
            self.rbr.set(byte);
            self.lsr.set(self.lsr.get() | LSR_DR); // set DR (Data Ready)
        }
    }
}

impl Default for Port {
    fn default() -> Self {
        Self {
            dll: 0x18, // divisor low  for 4800 baud (24 = 0x0018)
            dlm: 0x00, // divisor high for 4800 baud
            ier: 0x00, // all interrupts disabled
            lcr: 0x03, // 8-N-1, DLAB=0
            mcr: 0x00,
            lsr: Cell::new(LSR_THRE | LSR_TEMT), // transmitter ready
            msr: 0x00,
            rbr: Cell::new(0x00),
            thr: Cell::new(None),
            device: None,
        }
    }
}

pub trait ComPortDevice {
    /// try reading a value from the device. If a value is not ready return None.
    fn read(&mut self) -> Option<u8>;

    /// try writing a value to the device. If the device is not ready to write return false.
    fn write(&mut self, value: u8) -> bool;
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

    #[cfg(test)]
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

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        let (idx, offset) = port_index(port)?;
        let p = &self.ports[idx];
        let dlab = p.lcr & 0x80 != 0;
        let val = match offset {
            0 => {
                if dlab {
                    p.dll
                } else {
                    // RBR read: poll device first, then return buffered byte and clear DR
                    p.poll_rx();
                    let byte = p.rbr.get();
                    p.lsr.set(p.lsr.get() & !LSR_DR); // clear DR — byte consumed
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
            3 => p.lcr,
            4 => p.mcr,
            5 => {
                // LSR read: flush any buffered TX byte (updates THRE) then poll for RX
                p.flush_tx();
                p.poll_rx();
                p.lsr.get()
            }
            6 => p.msr,
            _ => return None,
        };
        Some(val)
    }

    fn io_write_u8(&mut self, port: u16, val: u8) -> bool {
        let Some((idx, offset)) = port_index(port) else {
            return false;
        };
        let p = &mut self.ports[idx];
        let dlab = p.lcr & 0x80 != 0;
        match offset {
            0 => {
                if dlab {
                    p.dll = val;
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
                        p.thr.set(Some(val));
                        p.lsr.set(p.lsr.get() & !(LSR_THRE | LSR_TEMT));
                    }
                }
            }
            1 => {
                if dlab {
                    p.dlm = val;
                } else {
                    p.ier = val;
                }
            }
            3 => p.lcr = val,
            4 => p.mcr = val,
            5 => p.lsr.set(val),
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
