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
        }
    }

    pub(super) fn software_reset(&mut self) {
        self.reset_seq = 0;
        self.cmd = None;
        self.cmd_remaining = 0;
        self.cmd_params.clear();
        self.out_buf.clear();
        self.out_buf.push_back(0xAA);
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
            0xD1 => self.speaker_on = true,
            0xD3 => self.speaker_on = false,
            0xD8 => self
                .out_buf
                .push_back(if self.speaker_on { 0xFF } else { 0x00 }),
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
}
