use std::collections::VecDeque;

pub(super) struct SoundBlasterDsp {
    reset_seq: u8,
    cmd: Option<u8>,
    cmd_remaining: u8,
    cmd_params: Vec<u8>,
    pub(super) out_buf: VecDeque<u8>,
    pub(super) irq_pending_8: bool,
    pub(super) irq_pending_16: bool,
    speaker_on: bool,
    test_reg: u8,

    // PCM DMA state
    pub(super) time_constant: u8,
    pub(super) dma_block_len: u16,
    pub(super) dma_bytes_remaining: u16,
    pub(super) dma_active: bool,
    /// Pending DREQ assertion/deassert to be drained by the Bus after IO write.
    dreq_pending: Option<bool>,
    /// Byte from Direct DAC command (0x10) to be pushed to pcm_out by mod.rs.
    direct_dac_byte: Option<u8>,
}

impl SoundBlasterDsp {
    pub(super) fn new() -> Self {
        Self {
            reset_seq: 0,
            cmd: None,
            cmd_remaining: 0,
            cmd_params: Vec::new(),
            out_buf: VecDeque::new(),
            irq_pending_8: false,
            irq_pending_16: false,
            speaker_on: false,
            test_reg: 0,
            time_constant: 0,
            dma_block_len: 0,
            dma_bytes_remaining: 0,
            dma_active: false,
            dreq_pending: None,
            direct_dac_byte: None,
        }
    }

    pub(super) fn software_reset(&mut self) {
        self.reset_seq = 0;
        self.cmd = None;
        self.cmd_remaining = 0;
        self.cmd_params.clear();
        self.out_buf.clear();
        self.out_buf.push_back(0xAA);
        self.dma_active = false;
        self.dreq_pending = Some(false);
    }

    pub(super) fn hardware_reset(&mut self) {
        self.reset_seq = 0;
        self.cmd = None;
        self.cmd_remaining = 0;
        self.cmd_params.clear();
        self.out_buf.clear();
        self.irq_pending_8 = false;
        self.irq_pending_16 = false;
        self.speaker_on = false;
        self.test_reg = 0;
        self.time_constant = 0;
        self.dma_block_len = 0;
        self.dma_bytes_remaining = 0;
        self.dma_active = false;
        self.dreq_pending = None;
        self.direct_dac_byte = None;
    }

    pub(super) fn write_reset_port(&mut self, val: u8) {
        if val != 0 {
            self.reset_seq = 1;
        } else if self.reset_seq != 0 {
            self.software_reset();
        }
    }

    fn params_for_cmd(cmd: u8) -> u8 {
        match cmd {
            0x10 => 1, // Direct DAC
            0x14 => 2, // 8-bit single-cycle DMA (unsigned)
            0x16 => 2, // 8-bit single-cycle DMA (signed)
            0x40 => 1, // Set time constant
            0x41 => 2, // Set sample rate (SB16)
            0x48 => 2, // Set DMA block size
            0xE0 | 0xE4 => 1,
            _ => 0,
        }
    }

    pub(super) fn write_command_port(&mut self, val: u8) {
        if let Some(cmd) = self.cmd {
            self.cmd_params.push(val);
            self.cmd_remaining -= 1;
            if self.cmd_remaining == 0 {
                self.cmd = None;
                self.execute_command(cmd);
                self.cmd_params.clear();
            }
        } else {
            let n = Self::params_for_cmd(val);
            if n == 0 {
                self.execute_command(val);
            } else {
                self.cmd = Some(val);
                self.cmd_remaining = n;
                self.cmd_params.clear();
            }
        }
    }

    fn execute_command(&mut self, cmd: u8) {
        match cmd {
            0x10 => {
                // Direct DAC: output one sample byte immediately (no DMA).
                self.direct_dac_byte = Some(self.cmd_params.first().copied().unwrap_or(0x80));
            }
            0x14 => {
                // 8-bit unsigned single-cycle DMA playback.
                let lo = self.cmd_params.first().copied().unwrap_or(0);
                let hi = self.cmd_params.get(1).copied().unwrap_or(0);
                self.dma_block_len = u16::from_le_bytes([lo, hi]).wrapping_add(1);
                self.dma_bytes_remaining = self.dma_block_len;
                self.dma_active = true;
                self.dreq_pending = Some(true);
            }
            0x16 => {
                // 8-bit signed single-cycle DMA playback (same data path as 0x14 for now).
                let lo = self.cmd_params.first().copied().unwrap_or(0);
                let hi = self.cmd_params.get(1).copied().unwrap_or(0);
                self.dma_block_len = u16::from_le_bytes([lo, hi]).wrapping_add(1);
                self.dma_bytes_remaining = self.dma_block_len;
                self.dma_active = true;
                self.dreq_pending = Some(true);
            }
            0x40 => {
                self.time_constant = self.cmd_params.first().copied().unwrap_or(0);
            }
            0x41 => {
                // Set output sample rate (SB16): hi byte, lo byte.
                // Store but don't use for now; timing is software-controlled.
            }
            0x48 => {
                // Set DMA block size for auto-init mode.
                let lo = self.cmd_params.first().copied().unwrap_or(0);
                let hi = self.cmd_params.get(1).copied().unwrap_or(0);
                self.dma_block_len = u16::from_le_bytes([lo, hi]).wrapping_add(1);
            }
            0xD0 => {} // Halt 8-bit DMA (no-op: DMA re-enabled by D4)
            0xD1 => self.speaker_on = true,
            0xD3 => self.speaker_on = false,
            0xD4 => {} // Continue 8-bit DMA
            0xD8 => self
                .out_buf
                .push_back(if self.speaker_on { 0xFF } else { 0x00 }),
            0xDA => {
                // Exit 8-bit auto-init DMA.
                self.dma_active = false;
                self.dreq_pending = Some(false);
            }
            0xE0 => {
                let param = self.cmd_params.first().copied().unwrap_or(0);
                self.out_buf.push_back(!param);
            }
            0xE1 => {
                self.out_buf.push_back(0x04);
                self.out_buf.push_back(0x05);
            }
            0xE3 => {
                for &b in b"COPYRIGHT (C) CREATIVE TECHNOLOGY LTD, 1992.\0" {
                    self.out_buf.push_back(b);
                }
            }
            0xE4 => {
                self.test_reg = self.cmd_params.first().copied().unwrap_or(0);
            }
            0xE8 => self.out_buf.push_back(self.test_reg),
            0xF2 => self.irq_pending_8 = true,
            0xF3 => self.irq_pending_16 = true,
            _ => log::warn!("SoundBlaster DSP: unknown command 0x{cmd:02X}"),
        }
    }

    pub(super) fn read_data(&mut self) -> u8 {
        self.out_buf.pop_front().unwrap_or(0x00)
    }

    /// Returns bit 7 set when data is ready; also acknowledges any pending 8-bit IRQ.
    pub(super) fn read_status(&mut self) -> u8 {
        let ready = if self.out_buf.is_empty() { 0x00 } else { 0x80 };
        self.irq_pending_8 = false;
        ready
    }

    /// Acknowledges a pending 16-bit IRQ.
    pub(super) fn read_ack16(&mut self) -> u8 {
        self.irq_pending_16 = false;
        0xFF
    }

    pub(super) fn take_pending_irq(&mut self) -> bool {
        if self.irq_pending_8 {
            self.irq_pending_8 = false;
            true
        } else if self.irq_pending_16 {
            self.irq_pending_16 = false;
            true
        } else {
            false
        }
    }

    /// Called by SoundBlaster::dma_write_u8 for each byte arriving via DMA.
    /// Returns (f32 sample, block_done).
    pub(super) fn dma_receive_byte(&mut self, val: u8) -> (f32, bool) {
        if !self.dma_active {
            return (0.0, false);
        }
        let sample = (val as f32 - 128.0) / 128.0;
        self.dma_bytes_remaining -= 1;
        let block_done = self.dma_bytes_remaining == 0;
        if block_done {
            self.dma_active = false;
            self.irq_pending_8 = true;
        }
        (sample, block_done)
    }

    /// DSP output sample rate derived from the time constant (mono only).
    /// Returns 11025 Hz when no time constant has been set (TC=166 default).
    pub(super) fn sample_rate(&self) -> u32 {
        let tc = self.time_constant as u32;
        if tc >= 256 {
            return 11025;
        }
        (1_000_000 / (256 - tc)).max(1)
    }

    /// Drain a pending DREQ assertion/deassert. Called by Bus after IO writes.
    pub(super) fn take_dreq_request(&mut self) -> Option<bool> {
        self.dreq_pending.take()
    }

    /// Drain a Direct DAC byte (from command 0x10). Called by mod.rs.
    pub(super) fn take_direct_dac_byte(&mut self) -> Option<u8> {
        self.direct_dac_byte.take()
    }
}
