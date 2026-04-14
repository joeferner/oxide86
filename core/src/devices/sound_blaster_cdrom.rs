use std::{any::Any, collections::VecDeque};

use crate::{Device, devices::CdromController, disk::cdrom::CdromBackend};

// Status byte bits (read from base+0)
const STATUS_RESULT_AVAIL: u8 = 0x01; // result data available at base+0
const STATUS_BUSY: u8 = 0x02;
const STATUS_ERROR: u8 = 0x04;
// bit 4: audio playing (always 0 — no audio)
const STATUS_DISC_PRESENT: u8 = 0x20;
const STATUS_DOOR_OPEN: u8 = 0x40;

// Error result byte returned when no disc or command fails
const RESULT_OK: u8 = 0x00;
const RESULT_ERROR: u8 = 0x05;

/// Converts MSF (minutes, seconds, frames) to LBA.
/// LBA = (M * 60 + S) * 75 + F - 150
fn msf_to_lba(m: u8, s: u8, f: u8) -> u32 {
    let lba = (m as u32 * 60 + s as u32) * 75 + f as u32;
    lba.saturating_sub(150)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CdromState {
    Idle,
    RecvParams(u8), // params remaining
    Execute,
    SendResult,
    // Sector streaming: after result byte, serve raw sector data via base+1
    StreamSector,
}

/// Emulates the Panasonic/Matsushita CD interface used by SBPCD.SYS.
///
/// IO port layout (base configurable, default 0x230):
/// - base+0  R: result data byte (SendResult) / sector data byte (StreamSector, result consumed) / status byte (otherwise)   W: command / param byte
/// - base+1  R: busy flag — bit 2=1 busy, bit 2=0 ready (SendResult/Idle); sector data bytes during StreamSector (after result consumed)   W: param byte (variant)
/// - base+2  R: extended status  W: reset (0xFF)
/// - base+3  R: drive select read-back  W: drive select (bits 0–1)
pub struct SoundBlasterCdrom {
    base_port: u16,
    drive_selected: u8,
    disc: Option<Box<dyn CdromBackend>>,
    state: CdromState,
    command: u8,
    params: Vec<u8>,
    result_buf: VecDeque<u8>,
    // Sector streaming state
    read_lba: u32,
    read_remaining: u32,
    read_sector_buf: [u8; 2048],
    read_sector_pos: usize,
    // Flags
    door_open: bool,
    error: bool,
    pending_irq: bool,
    irq_enabled: bool,
    irq_line: u8,
    // Panasonic ATN (attention) signal: bit 0 of base+1 asserted by drive after power-on
    // or disc change to notify host of new status. Consumed on first base+1 read in Idle.
    attention_pending: bool,
}

impl SoundBlasterCdrom {
    pub fn new(base_port: u16, disc: Option<Box<dyn CdromBackend>>, irq_line: u8) -> Self {
        // Door starts closed regardless of disc presence. A real drive's tray is closed at
        // power-on; door_open is only set true when the tray is explicitly ejected.
        // Leaving it true when no disc was given caused SBPCD.SYS to spin in a
        // "wait for tray to close" loop and report "NOT READY".
        let door_open = false;
        let attention_pending = disc.is_some();
        Self {
            base_port,
            drive_selected: 0,
            disc,
            state: CdromState::Idle,
            command: 0,
            params: Vec::new(),
            result_buf: VecDeque::new(),
            read_lba: 0,
            read_remaining: 0,
            read_sector_buf: [0u8; 2048],
            read_sector_pos: 0,
            door_open,
            error: false,
            pending_irq: false,
            irq_enabled: true,
            irq_line,
            attention_pending,
        }
    }

    fn disc_present(&self) -> bool {
        self.disc.is_some()
    }

    fn status_byte(&self) -> u8 {
        let mut s = 0u8;
        if matches!(
            self.state,
            CdromState::SendResult | CdromState::StreamSector
        ) {
            s |= STATUS_RESULT_AVAIL;
        }
        if matches!(self.state, CdromState::RecvParams(_) | CdromState::Execute) {
            s |= STATUS_BUSY;
        }
        if self.error {
            s |= STATUS_ERROR;
        }
        if self.disc_present() {
            s |= STATUS_DISC_PRESENT;
        }
        if self.door_open {
            s |= STATUS_DOOR_OPEN;
        }
        s
    }

    /// Number of parameter bytes expected for each command.
    fn params_for_command(cmd: u8) -> u8 {
        match cmd {
            0x00 => 0, // NOP / presence check
            0x01 => 0, // Stop
            0x81 => 0, // Get drive attention / ready status
            0x83 => 0, // OEM identification inquiry
            0x05 => 0, // Read status
            0x09 => 1, // Set mode
            0x0A => 3, // Seek (MSF)
            0x0B => 7, // Read sectors (MSF start 3 + count 3 + mode 1)
            0x0C => 0, // Pause
            0x0D => 0, // Resume
            0x10 => 0, // Reset
            0x11 => 2, // Read TOC (first/last track)
            0x12 => 0, // Read disc info
            _ => 0,
        }
    }

    fn execute_command(&mut self) {
        self.error = false;
        self.result_buf.clear();

        match self.command {
            // OEM identification inquiry. SBPCD.SYS reads 12 bytes via base+0 and checks
            // the first 8 against the expected manufacturer string. Matsushita/MKE drives
            // return "MATSHITA" as bytes 0–7; the remaining 4 bytes are version/padding.
            0x83 => {
                for &b in b"MATSHITA" {
                    self.result_buf.push_back(b);
                }
                self.result_buf.push_back(0x00); // bytes 8–11: version/padding
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.state = CdromState::SendResult;
            }

            // NOP — drive presence check. Returns the 2-byte Panasonic presence signature
            // [0xAA, 0x55]; SBPCD.SYS reads these via base+0 and checks the word = 0x55AA.
            0x00 => {
                self.result_buf.push_back(0xAA);
                self.result_buf.push_back(0x55);
                self.state = CdromState::SendResult;
            }

            // Stop
            0x01 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            // Get drive attention / ready status (Panasonic attention poll).
            // SBPCD polls this waiting for bit 3 (0x08 = drive ready / data disc) or bit 4
            // (0x10 = audio playing). Bit 6 (0x40) means "disc stable / not changed" — SBPCD
            // checks this in the IOCTL 0x09 "Return Media Changed" handler: bit 6 set → "not
            // changed" (0x01), bit 6 clear → "changed" (0xFF). With disc: return 0x48 so the
            // init loop exits (bit 3) and IOCTL 0x09 reports "not changed" (bit 6). Without
            // disc: return 0x08 (ready, but disc absent/changed).
            0x81 => {
                let result = if self.disc_present() {
                    0x08 | 0x40
                } else {
                    0x08
                };
                self.result_buf.push_back(result);
                self.state = CdromState::SendResult;
            }

            // Read status — 5 bytes of extended status
            0x05 => {
                self.result_buf.push_back(RESULT_OK);
                // Status bytes 2–5: disc present, no error, mode 1 data
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.state = CdromState::SendResult;
            }

            // Set mode
            0x09 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            // Seek (MSF) — fails without disc
            0x0A => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                self.read_lba = msf_to_lba(self.params[0], self.params[1], self.params[2]);
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            // Read sectors: MSF start (3 bytes) + count (3 bytes) + mode (1 byte)
            // Count is stored in params[3..6] as big-endian 24-bit.
            0x0B => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                let lba = msf_to_lba(self.params[0], self.params[1], self.params[2]);
                let count = (self.params[3] as u32) << 16
                    | (self.params[4] as u32) << 8
                    | (self.params[5] as u32);
                self.read_lba = lba;
                self.read_remaining = count;
                self.read_sector_pos = 2048; // force load on first byte read

                // Emit the status byte, then switch to streaming
                self.result_buf.push_back(RESULT_OK);
                if count > 0 {
                    self.state = CdromState::StreamSector;
                } else {
                    self.state = CdromState::SendResult;
                }
            }

            // Pause
            0x0C => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            // Resume
            0x0D => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            // Reset
            0x10 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            // Read TOC (params[0] = first track, params[1] = last track) — fails without disc
            0x11 => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                let first = self.params[0];
                let last = self.params[1];
                self.result_buf.push_back(RESULT_OK);
                self.result_buf.push_back(first);
                self.result_buf.push_back(last);
                // For each requested track, emit a minimal TOC entry (MSF = 00:02:00)
                for _track in first..=last {
                    self.result_buf.push_back(0x00); // control/ADR
                    self.result_buf.push_back(0x00); // M
                    self.result_buf.push_back(0x02); // S
                    self.result_buf.push_back(0x00); // F
                }
                self.state = CdromState::SendResult;
            }

            // Read disc info — fails without disc
            0x12 => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                let total = self.disc.as_ref().map(|d| d.total_sectors()).unwrap_or(0);
                // Convert total sectors to MSF (add 150 pregap)
                let total_frames = total + 150;
                let f = (total_frames % 75) as u8;
                let total_seconds = total_frames / 75;
                let s = (total_seconds % 60) as u8;
                let m = (total_seconds / 60) as u8;

                self.result_buf.push_back(RESULT_OK);
                self.result_buf.push_back(0x01); // first track
                self.result_buf.push_back(0x01); // last track
                self.result_buf.push_back(m);
                self.result_buf.push_back(s);
                self.result_buf.push_back(f);
                self.state = CdromState::SendResult;
            }

            // Audio play / stop — not implemented, return error
            0x0E | 0x0F => {
                log::warn!(
                    "SoundBlasterCdrom: unimplemented audio command {:#04x}",
                    self.command
                );
                self.error = true;
                self.result_buf.push_back(RESULT_ERROR);
                self.state = CdromState::SendResult;
            }

            _ => {
                log::warn!("SoundBlasterCdrom: unknown command {:#04x}", self.command);
                self.error = true;
                self.result_buf.push_back(RESULT_ERROR);
                self.state = CdromState::SendResult;
            }
        }

        if matches!(
            self.state,
            CdromState::SendResult | CdromState::StreamSector
        ) && self.irq_enabled
        {
            self.pending_irq = true;
        }
    }

    /// Read one sector data byte from base+1 during streaming.
    /// Only called after the initial result byte has been consumed via base+0.
    /// Loads a new sector when the current one is exhausted.
    fn stream_read_byte(&mut self) -> u8 {
        if self.read_sector_pos >= 2048 {
            if self.read_remaining == 0 {
                self.state = CdromState::Idle;
                return 0x00;
            }
            // Load the next sector
            match self.disc.as_mut() {
                Some(disc) => {
                    let mut buf = [0u8; 2048];
                    if disc.read_sector(self.read_lba, &mut buf).is_ok() {
                        self.read_sector_buf = buf;
                    } else {
                        log::warn!(
                            "SoundBlasterCdrom: read_sector failed at LBA {}",
                            self.read_lba
                        );
                        self.error = true;
                        self.state = CdromState::Idle;
                        return RESULT_ERROR;
                    }
                }
                None => {
                    self.error = true;
                    self.state = CdromState::Idle;
                    return RESULT_ERROR;
                }
            }
            self.read_lba += 1;
            self.read_remaining -= 1;
            self.read_sector_pos = 0;

            if self.read_remaining == 0 {
                self.state = CdromState::Idle;
            }
        }

        let byte = self.read_sector_buf[self.read_sector_pos];
        self.read_sector_pos += 1;
        byte
    }

    fn handle_write_base0(&mut self, val: u8) {
        match self.state {
            CdromState::Idle => {
                self.command = val;
                let n = Self::params_for_command(val);
                self.params.clear();
                if n == 0 {
                    self.state = CdromState::Execute;
                    self.execute_command();
                } else {
                    self.state = CdromState::RecvParams(n);
                }
            }
            CdromState::RecvParams(remaining) => {
                self.params.push(val);
                let new_remaining = remaining - 1;
                if new_remaining == 0 {
                    self.state = CdromState::Execute;
                    self.execute_command();
                } else {
                    self.state = CdromState::RecvParams(new_remaining);
                }
            }
            _ => {
                // Ignore writes in other states
            }
        }
    }
}

impl Device for SoundBlasterCdrom {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.state = CdromState::Idle;
        self.command = 0;
        self.params.clear();
        self.result_buf.clear();
        self.read_remaining = 0;
        self.read_sector_pos = 2048;
        self.error = false;
        self.pending_irq = false;
        self.attention_pending = false;
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        let offset = port.wrapping_sub(self.base_port);
        match offset {
            // base+0: result data when available, otherwise status byte.
            // SBPCD.SYS polls base+1 for busy=0, then reads result bytes here.
            0 => {
                let byte = match self.state {
                    CdromState::SendResult => {
                        let b = self.result_buf.pop_front().unwrap_or(0x00);
                        if self.result_buf.is_empty() {
                            self.state = CdromState::Idle;
                        }
                        b
                    }
                    // Drain the initial result byte before sector data is served via base+1
                    CdromState::StreamSector if !self.result_buf.is_empty() => {
                        self.result_buf.pop_front().unwrap()
                    }
                    _ => self.status_byte(),
                };
                log::debug!("SbCd R base+0 val=0x{byte:02X} state={:?}", self.state);
                Some(byte)
            }
            // base+1: busy flag (bit 2) while processing; sector data bytes once streaming.
            // SBPCD.SYS polls this register: bit 2=1 → busy, bit 2=0 → result ready at base+0.
            // Bit 0 = Panasonic ATN (attention): drive has unsolicited status to report.
            // SBPCD checks this on every command dispatch; if set it takes the attention path
            // (gap 0C45:0C56) which reads drive status and updates internal flags like [0x002e].
            1 => {
                let byte = match self.state {
                    CdromState::RecvParams(_) | CdromState::Execute => 0x04, // bit 2: busy
                    CdromState::StreamSector if self.result_buf.is_empty() => {
                        self.stream_read_byte()
                    }
                    CdromState::Idle if self.attention_pending => {
                        // Assert ATN once; cleared after host reads it so it fires only once
                        // per disc insertion / power-on cycle.
                        self.attention_pending = false;
                        0x01
                    }
                    _ => 0x00, // bit 2 clear: result ready / idle
                };
                log::debug!("SbCd R base+1 val=0x{byte:02X} state={:?}", self.state);
                Some(byte)
            }
            2 => {
                let s = self.status_byte();
                log::debug!("SbCd R base+2 ext_status=0x{s:02X}");
                Some(s)
            }
            3 => {
                log::debug!("SbCd R base+3 drive_selected={}", self.drive_selected);
                Some(self.drive_selected)
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        let offset = port.wrapping_sub(self.base_port);
        match offset {
            0 => {
                log::debug!("SbCd W base+0 val=0x{val:02X} state={:?}", self.state);
                self.handle_write_base0(val);
                true
            }
            1 => {
                log::debug!("SbCd W base+1 val=0x{val:02X} state={:?}", self.state);
                // Alternate param byte write (variant) — treat same as base+0 param
                if let CdromState::RecvParams(remaining) = self.state {
                    self.params.push(val);
                    let new_remaining = remaining - 1;
                    if new_remaining == 0 {
                        self.state = CdromState::Execute;
                        self.execute_command();
                    } else {
                        self.state = CdromState::RecvParams(new_remaining);
                    }
                }
                true
            }
            2 => {
                log::debug!("SbCd W base+2 val=0x{val:02X}");
                // Reset on write of 0xFF
                if val == 0xFF {
                    self.reset();
                }
                true
            }
            3 => {
                log::debug!("SbCd W base+3 drive_select=0x{val:02X}");
                self.drive_selected = val & 0x03;
                true
            }
            _ => false,
        }
    }
}

impl CdromController for SoundBlasterCdrom {
    fn load_disc(&mut self, disc: Box<dyn CdromBackend>) {
        self.disc = Some(disc);
        self.door_open = false;
        self.error = false;
        // Reset command state so driver can detect the new disc
        self.state = CdromState::Idle;
        self.result_buf.clear();
        // Assert ATN so SBPCD picks up new disc status on next dispatch
        self.attention_pending = true;
    }

    fn eject_disc(&mut self) {
        self.disc = None;
        self.door_open = true;
        self.state = CdromState::Idle;
        self.result_buf.clear();
        self.read_remaining = 0;
        self.attention_pending = false;
    }

    fn take_pending_irq(&mut self) -> bool {
        if self.pending_irq {
            self.pending_irq = false;
            true
        } else {
            false
        }
    }

    fn irq_line(&self) -> u8 {
        self.irq_line
    }
}
