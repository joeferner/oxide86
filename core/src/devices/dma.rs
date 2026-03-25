//! Intel 8237A DMA Controller.
//!
//! The PC has two cascaded 8237A controllers:
//!
//! - **DMA1** (channels 0–3): I/O ports 0x0000–0x000F, page registers 0x0087,
//!   0x0083, 0x0081, 0x0082.
//! - **DMA2** (channels 4–7): I/O ports 0x00C0–0x00DF, page registers 0x008F,
//!   0x008B, 0x0089, 0x008A.
//!
//! Channel 0 on DMA1 is the memory-refresh channel; its DREQ line is permanently
//! asserted by hardware.  When channel 0 is unmasked the controller advances its
//! `current_address` and decrements its `current_count` at approximately one DMA
//! bus cycle per 4 CPU cycles (matching the real 8237A at 4.77 MHz).
//!
//! All other channels are still stub-only (registers accepted, no data movement).

use std::any::Any;

use crate::Device;

/// CPU cycles consumed per DMA bus cycle.  Real hardware runs one DMA cycle
/// every ~4 CPU clocks; this ratio keeps the timing realistic without being
/// exact.
const CPU_CYCLES_PER_DMA_CYCLE: u32 = 4;

/// A pending DMA data transfer returned by `DmaController::tick` and executed
/// by the Bus.
///
/// - `write_to_memory = true`: device supplies a byte → Bus writes it to `phys_addr`
///   (8237A "WRITE" transfer, bits 3-2 = 01).
/// - `write_to_memory = false`: Bus reads a byte from `phys_addr` → device accepts it
///   (8237A "READ" transfer, bits 3-2 = 10).
#[derive(Debug)]
pub(crate) struct DmaTransfer {
    /// Global channel number (0–3 = DMA1, 4–7 = DMA2).
    pub channel: u8,
    /// Physical address = (page << 16) | current_address (before advancement).
    pub phys_addr: u32,
    /// `true` = device → memory (DMA WRITE); `false` = memory → device (DMA READ).
    pub write_to_memory: bool,
}

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
    /// Cycle count at the last tick, used to pace DMA transfers.
    last_tick_cycle: u32,
    /// Bitmask of channels with an active DMA request (DREQ).
    /// Bit 0 is permanently set — channel 0 is the memory-refresh channel.
    dreq: u8,
}

impl Default for Dma8237 {
    fn default() -> Self {
        Self {
            channels: Default::default(),
            flip_flop: false,
            command: 0,
            status: 0,
            mask: 0x0F, // all channels masked on reset
            last_tick_cycle: 0,
            dreq: 0x01, // channel 0 DREQ permanently active (memory refresh)
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
            0x09 => {
                let ch = val & 0x03;
                if val & 0x04 != 0 {
                    self.dreq |= 1 << ch;
                } else {
                    self.dreq &= !(1 << ch);
                }
                // Channel 0 DREQ is always active regardless of software requests
                self.dreq |= 0x01;
            }
            // Single channel mask
            0x0A => {
                let ch = val & 0x03;
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
                // Channel 0 DREQ survives reset (hardware line)
                self.dreq = 0x01;
            }
            // Clear mask register (unmask all channels)
            0x0E => self.mask = 0,
            // Write all channel masks
            0x0F => self.mask = val & 0x0F,
            _ => {}
        }
    }

    /// Advance active channels based on elapsed CPU cycles.
    ///
    /// For each unmasked channel that has a pending DREQ the controller
    /// simulates `elapsed / CPU_CYCLES_PER_DMA_CYCLE` DMA bus cycles.  Each
    /// bus cycle captures the current physical address, advances
    /// `current_address`, then decrements `current_count`.  When
    /// `current_count` wraps through 0xFFFF (Terminal Count) the TC bit is
    /// set.  Auto-init channels reload from their base registers; non-auto-init
    /// channels are automatically masked.
    ///
    /// Returns one `DmaTransfer` per bus cycle on channels whose mode is WRITE
    /// (device → memory) or READ (memory → device).  Verify-mode channels only
    /// advance counters and produce no transfers.
    ///
    /// `channel_base` is 0 for DMA1 and 4 for DMA2, used to report global
    /// channel numbers in the returned ops.
    fn tick(&mut self, cycle_count: u32, channel_base: u8) -> Vec<DmaTransfer> {
        let elapsed = cycle_count.wrapping_sub(self.last_tick_cycle);
        let dma_cycles = elapsed / CPU_CYCLES_PER_DMA_CYCLE;
        if dma_cycles == 0 {
            return Vec::new();
        }
        self.last_tick_cycle = self
            .last_tick_cycle
            .wrapping_add(dma_cycles * CPU_CYCLES_PER_DMA_CYCLE);

        let mut transfers = Vec::new();

        for ch in 0..4usize {
            let ch_bit = 1u8 << ch;
            if self.mask & ch_bit != 0 {
                continue; // channel is masked
            }
            if self.dreq & ch_bit == 0 {
                continue; // no DMA request on this channel
            }

            let mode = self.channels[ch].mode;
            let auto_init = mode & 0x10 != 0;
            let decrement = mode & 0x20 != 0;
            // Bits 3-2: 00=verify, 01=write (device→mem), 10=read (mem→device)
            let transfer_type = (mode >> 2) & 0x03;
            let is_verify = transfer_type == 0x00;
            let write_to_memory = transfer_type == 0x01; // device → memory

            let mut remaining = dma_cycles;
            while remaining > 0 {
                remaining -= 1;

                // 1. Capture physical address before advancement (correct 8237A order)
                if !is_verify {
                    let phys_addr = ((self.channels[ch].page as u32) << 16)
                        | (self.channels[ch].current_address as u32);
                    transfers.push(DmaTransfer {
                        channel: channel_base + ch as u8,
                        phys_addr,
                        write_to_memory,
                    });
                }

                // 2. Advance address
                if decrement {
                    self.channels[ch].current_address =
                        self.channels[ch].current_address.wrapping_sub(1);
                } else {
                    self.channels[ch].current_address =
                        self.channels[ch].current_address.wrapping_add(1);
                }

                // 3. Decrement count; Terminal Count fires when it wraps 0 → 0xFFFF
                if self.channels[ch].current_count == 0 {
                    self.status |= ch_bit; // set TC bit
                    if auto_init {
                        self.channels[ch].current_address = self.channels[ch].base_address;
                        self.channels[ch].current_count = self.channels[ch].base_count;
                    } else {
                        self.mask |= ch_bit; // mask channel on TC (no auto-init)
                        break;
                    }
                } else {
                    self.channels[ch].current_count -= 1;
                }
            }
        }

        transfers
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

    /// Called from `Bus::increment_cycle_count` to advance DMA transfers.
    /// Returns all pending data transfers for the Bus to execute.
    pub(crate) fn tick(&mut self, cycle_count: u32) -> Vec<DmaTransfer> {
        let mut transfers = self.dma1.tick(cycle_count, 0);
        transfers.extend(self.dma2.tick(cycle_count, 4));
        transfers
    }

    /// Assert or deassert a DREQ line on the given global channel (0–7).
    /// Channel 0 DREQ is permanently active and cannot be cleared.
    pub(crate) fn set_dreq(&mut self, global_channel: u8, asserted: bool) {
        if global_channel < 4 {
            if asserted {
                self.dma1.dreq |= 1 << global_channel;
            } else {
                self.dma1.dreq &= !(1 << global_channel);
                self.dma1.dreq |= 0x01; // channel 0 always active
            }
        } else {
            let ch = global_channel - 4;
            if asserted {
                self.dma2.dreq |= 1 << ch;
            } else {
                self.dma2.dreq &= !(1 << ch);
            }
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
            0x0081 => {
                self.dma1.channels[2].page = val;
                true
            }
            0x0082 => {
                self.dma1.channels[3].page = val;
                true
            }
            0x0083 => {
                self.dma1.channels[1].page = val;
                true
            }
            0x0087 => {
                self.dma1.channels[0].page = val;
                true
            }
            0x0089 => {
                self.dma2.channels[2].page = val;
                true
            }
            0x008A => {
                self.dma2.channels[3].page = val;
                true
            }
            0x008B => {
                self.dma2.channels[1].page = val;
                true
            }
            0x008F => {
                self.dma2.channels[0].page = val;
                true
            }
            _ => false,
        }
    }
}
