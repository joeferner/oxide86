//! Intel 8237A DMA Controller stub.
//!
//! The PC has two cascaded 8237A controllers:
//!
//! - **DMA1** (channels 0–3): I/O ports 0x0000–0x000F, page registers 0x0087,
//!   0x0083, 0x0081, 0x0082.
//! - **DMA2** (channels 4–7): I/O ports 0x00C0–0x00DF, page registers 0x008F,
//!   0x008B, 0x0089, 0x008A.
//!
//! This is a minimal stub that accepts all register reads/writes so that BIOS
//! and DOS DMA programming does not trigger "no device responded" warnings.
//! No actual DMA transfers are performed — the floppy controller handles data
//! transfer via PIO in the BIOS int 13h implementation.

use std::any::Any;

use crate::Device;

/// Per-channel state.
#[derive(Clone, Default)]
struct DmaChannel {
    base_address: u16,
    base_count: u16,
    current_address: u16,
    current_count: u16,
    mode: u8,
    page: u8,
}

/// One 8237A controller (4 channels).
#[derive(Clone)]
struct Dma8237 {
    channels: [DmaChannel; 4],
    /// High/low byte flip-flop (false = low byte next).
    flip_flop: bool,
    command: u8,
    status: u8,
    mask: u8,
}

impl Default for Dma8237 {
    fn default() -> Self {
        Self {
            channels: Default::default(),
            flip_flop: false,
            command: 0,
            status: 0,
            mask: 0x0F, // all channels masked on reset
        }
    }
}

impl Dma8237 {
    fn read(&mut self, offset: u16) -> u8 {
        match offset {
            // Channel base/current address (even) and count (odd) registers
            0x00 | 0x02 | 0x04 | 0x06 => {
                let ch = (offset / 2) as usize;
                let val = if self.flip_flop {
                    (self.channels[ch].current_address >> 8) as u8
                } else {
                    self.channels[ch].current_address as u8
                };
                self.flip_flop = !self.flip_flop;
                val
            }
            0x01 | 0x03 | 0x05 | 0x07 => {
                let ch = ((offset - 1) / 2) as usize;
                let val = if self.flip_flop {
                    (self.channels[ch].current_count >> 8) as u8
                } else {
                    self.channels[ch].current_count as u8
                };
                self.flip_flop = !self.flip_flop;
                val
            }
            // Status register
            0x08 => {
                let s = self.status;
                self.status &= 0xF0; // TC bits cleared on read
                s
            }
            // Temporary register (not used, return 0)
            0x0D => 0,
            // Multi-channel mask register (readable on 82C37 / later chips)
            0x0F => self.mask,
            _ => 0xFF,
        }
    }

    fn write(&mut self, offset: u16, val: u8) {
        match offset {
            // Channel base/current address
            0x00 | 0x02 | 0x04 | 0x06 => {
                let ch = (offset / 2) as usize;
                if self.flip_flop {
                    self.channels[ch].base_address =
                        (self.channels[ch].base_address & 0x00FF) | ((val as u16) << 8);
                } else {
                    self.channels[ch].base_address =
                        (self.channels[ch].base_address & 0xFF00) | val as u16;
                }
                self.channels[ch].current_address = self.channels[ch].base_address;
                self.flip_flop = !self.flip_flop;
            }
            // Channel base/current count
            0x01 | 0x03 | 0x05 | 0x07 => {
                let ch = ((offset - 1) / 2) as usize;
                if self.flip_flop {
                    self.channels[ch].base_count =
                        (self.channels[ch].base_count & 0x00FF) | ((val as u16) << 8);
                } else {
                    self.channels[ch].base_count =
                        (self.channels[ch].base_count & 0xFF00) | val as u16;
                }
                self.channels[ch].current_count = self.channels[ch].base_count;
                self.flip_flop = !self.flip_flop;
            }
            // Command register
            0x08 => self.command = val,
            // Single channel request
            0x09 => { /* software request — ignored in stub */ }
            // Single channel mask
            0x0A => {
                let ch = (val & 0x03) as u8;
                if val & 0x04 != 0 {
                    self.mask |= 1 << ch;
                } else {
                    self.mask &= !(1 << ch);
                }
            }
            // Mode register
            0x0B => {
                let ch = (val & 0x03) as usize;
                self.channels[ch].mode = val;
            }
            // Clear byte pointer flip-flop
            0x0C => self.flip_flop = false,
            // Master clear (software reset)
            0x0D => {
                self.flip_flop = false;
                self.command = 0;
                self.status = 0;
                self.mask = 0x0F;
            }
            // Clear mask register (unmask all channels)
            0x0E => self.mask = 0,
            // Write all channel masks
            0x0F => self.mask = val & 0x0F,
            _ => {}
        }
    }
}

/// DMA controller device covering both DMA1 and DMA2 plus page registers.
pub struct DmaController {
    dma1: Dma8237,
    dma2: Dma8237,
}

impl DmaController {
    pub fn new() -> Self {
        Self {
            dma1: Dma8237::default(),
            dma2: Dma8237::default(),
        }
    }
}

impl Device for DmaController {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.dma1 = Dma8237::default();
        self.dma2 = Dma8237::default();
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        match port {
            // DMA1: ports 0x0000–0x000F
            0x0000..=0x000F => Some(self.dma1.read(port)),
            // DMA2: ports 0x00C0–0x00DF (word-aligned, shift to 0–F)
            0x00C0..=0x00DF => Some(self.dma2.read((port - 0x00C0) / 2)),
            // Page registers
            0x0081 => Some(self.dma1.channels[2].page),
            0x0082 => Some(self.dma1.channels[3].page),
            0x0083 => Some(self.dma1.channels[1].page),
            0x0087 => Some(self.dma1.channels[0].page),
            0x0089 => Some(self.dma2.channels[2].page),
            0x008A => Some(self.dma2.channels[3].page),
            0x008B => Some(self.dma2.channels[1].page),
            0x008F => Some(self.dma2.channels[0].page),
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        match port {
            // DMA1: ports 0x0000–0x000F
            0x0000..=0x000F => {
                self.dma1.write(port, val);
                true
            }
            // DMA2: ports 0x00C0–0x00DF
            0x00C0..=0x00DF => {
                self.dma2.write((port - 0x00C0) / 2, val);
                true
            }
            // Page registers
            0x0081 => { self.dma1.channels[2].page = val; true }
            0x0082 => { self.dma1.channels[3].page = val; true }
            0x0083 => { self.dma1.channels[1].page = val; true }
            0x0087 => { self.dma1.channels[0].page = val; true }
            0x0089 => { self.dma2.channels[2].page = val; true }
            0x008A => { self.dma2.channels[3].page = val; true }
            0x008B => { self.dma2.channels[1].page = val; true }
            0x008F => { self.dma2.channels[0].page = val; true }
            _ => false,
        }
    }
}
