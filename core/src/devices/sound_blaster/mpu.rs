use std::collections::VecDeque;

/// MPU-401 MIDI controller in UART mode.
///
/// Ports:
///   0x330 — data (R: next byte from out_buf; W: MIDI output, discarded)
///   0x331 — command/status (R: bit 6=output ready 0=ready, bit 7=data available; W: command)
///
/// Supported commands:
///   0xFF — Reset: clear UART mode, enqueue 0xFE ACK
///   0x3F — Enter UART mode: set uart_mode, enqueue 0xFE ACK
pub(super) struct SoundBlasterMpu {
    uart_mode: bool,
    out_buf: VecDeque<u8>,
}

impl SoundBlasterMpu {
    pub(super) fn new() -> Self {
        Self {
            uart_mode: false,
            out_buf: VecDeque::new(),
        }
    }

    pub(super) fn reset(&mut self) {
        self.uart_mode = false;
        self.out_buf.clear();
    }

    pub(super) fn read_data(&mut self) -> u8 {
        self.out_buf.pop_front().unwrap_or(0x00)
    }

    /// Status byte: bit 7 = data available, bit 6 = 0 (output ready), bits 5-0 = 1.
    pub(super) fn read_status(&self) -> u8 {
        let mut status = 0x3Fu8;
        if !self.out_buf.is_empty() {
            status |= 0x80;
        }
        status
    }

    pub(super) fn write_command(&mut self, val: u8) {
        match val {
            0xFF => {
                self.uart_mode = false;
                self.out_buf.clear();
                self.out_buf.push_back(0xFE);
            }
            0x3F => {
                self.uart_mode = true;
                self.out_buf.push_back(0xFE);
            }
            _ => {
                log::warn!("MPU-401: unhandled command 0x{val:02X}");
            }
        }
    }

    pub(super) fn write_data(&mut self, val: u8) {
        log::debug!("MPU-401 MIDI out: 0x{val:02X}");
    }
}
