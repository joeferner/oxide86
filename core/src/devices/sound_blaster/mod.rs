use std::{any::Any, collections::VecDeque};

use crate::{
    Device,
    devices::{CdromController, SoundCard},
    disk::cdrom::CdromBackend,
    utils::bcd_to_dec,
};

mod dsp;
use dsp::SoundBlasterDsp;

// Status byte bits (read from base+0)
const STATUS_RESULT_AVAIL: u8 = 0x01;
const STATUS_BUSY: u8 = 0x02;
const STATUS_ERROR: u8 = 0x04;
const STATUS_DISC_PRESENT: u8 = 0x20;
const STATUS_DOOR_OPEN: u8 = 0x40;

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
    RecvParams(u8),
    Execute,
    SendResult,
    StreamSector,
}

/// Panasonic/Matsushita CD interface state, absorbed from `SoundBlasterCdrom`.
///
/// IO port layout (base configurable, default 0x230):
/// - base+0  R: result data byte (SendResult) / sector data (StreamSector) / status (otherwise)   W: command/param
/// - base+1  R: busy flag / sector data (StreamSector after result consumed)   W: param byte
/// - base+2  R: extended status  W: reset (0xFF)
/// - base+3  R: drive select read-back  W: drive select (bits 0–1)
struct SoundBlasterCdromInner {
    base_port: u16,
    drive_selected: u8,
    disc: Option<Box<dyn CdromBackend>>,
    state: CdromState,
    command: u8,
    params: Vec<u8>,
    result_buf: VecDeque<u8>,
    read_lba: u32,
    read_remaining: u32,
    read_sector_buf: [u8; 2048],
    read_sector_pos: usize,
    door_open: bool,
    error: bool,
    pending_irq: bool,
    irq_enabled: bool,
    irq_line: u8,
    attention_pending: bool,
    stream_via_base0: bool,
}

impl SoundBlasterCdromInner {
    fn new(base_port: u16, disc: Option<Box<dyn CdromBackend>>, irq_line: u8) -> Self {
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
            stream_via_base0: false,
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

    fn params_for_command(cmd: u8) -> u8 {
        match cmd {
            0x00 => 0,
            0x01 => 0,
            0x02 => 6,
            0x05 => 0,
            0x09 => 1,
            0x0A => 3,
            0x0B => 7,
            0x0C => 0,
            0x0D => 0,
            0x10 => 0,
            0x11 => 2,
            0x12 => 0,
            0x81 => 0,
            0x83 => 0,
            0x84 => 6,
            0x88 => 0,
            0x8B => 0,
            _ => 0,
        }
    }

    fn execute_command(&mut self) {
        self.error = false;
        self.result_buf.clear();

        match self.command {
            0x83 => {
                for &b in b"MATSHITA" {
                    self.result_buf.push_back(b);
                }
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.state = CdromState::SendResult;
            }

            0x00 => {
                self.result_buf.push_back(0xAA);
                self.result_buf.push_back(0x55);
                self.state = CdromState::SendResult;
            }

            0x01 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x81 => {
                let result = if self.disc_present() {
                    0x08 | 0x40 | 0x01
                } else {
                    0x08
                };
                self.result_buf.push_back(result);
                self.state = CdromState::SendResult;
            }

            0x05 => {
                self.result_buf.push_back(RESULT_OK);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.result_buf.push_back(0x00);
                self.state = CdromState::SendResult;
            }

            0x09 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

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
                self.read_sector_pos = 2048;
                self.result_buf.push_back(RESULT_OK);
                if count > 0 {
                    self.state = CdromState::StreamSector;
                } else {
                    self.state = CdromState::SendResult;
                }
            }

            0x0C => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x0D => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

            0x10 => {
                self.result_buf.push_back(RESULT_OK);
                self.state = CdromState::SendResult;
            }

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
                for _track in first..=last {
                    self.result_buf.push_back(0x00);
                    self.result_buf.push_back(0x00);
                    self.result_buf.push_back(0x02);
                    self.result_buf.push_back(0x00);
                }
                self.state = CdromState::SendResult;
            }

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
                let total_frames = total + 150;
                let f = (total_frames % 75) as u8;
                let total_seconds = total_frames / 75;
                let s = (total_seconds % 60) as u8;
                let m = (total_seconds / 60) as u8;
                self.result_buf.push_back(RESULT_OK);
                self.result_buf.push_back(0x01);
                self.result_buf.push_back(0x01);
                self.result_buf.push_back(m);
                self.result_buf.push_back(s);
                self.result_buf.push_back(f);
                self.state = CdromState::SendResult;
            }

            0x88 => {
                let total = self.disc.as_ref().map(|d| d.total_sectors()).unwrap_or(0);
                let total_frames = total + 150;
                let f = (total_frames % 75) as u8;
                let total_seconds = total_frames / 75;
                let s = (total_seconds % 60) as u8;
                let m = (total_seconds / 60) as u8;
                self.result_buf.extend(&[m, s, f, 0x08, 0x00]);
                self.state = CdromState::SendResult;
            }

            0x8B => {
                self.result_buf.extend(&[0x00u8; 6]);
                self.state = CdromState::SendResult;
            }

            0x02 => {
                if !self.disc_present() {
                    self.error = true;
                    self.result_buf.push_back(RESULT_ERROR);
                    self.state = CdromState::SendResult;
                    if self.irq_enabled {
                        self.pending_irq = true;
                    }
                    return;
                }
                let m = bcd_to_dec(self.params[0]);
                let s = bcd_to_dec(self.params[1]);
                let f = bcd_to_dec(self.params[2]);
                let count = (self.params[3] as u32) << 8 | self.params[4] as u32;
                self.read_lba = msf_to_lba(m, s, f);
                self.read_remaining = count;
                self.read_sector_pos = 2048;
                self.stream_via_base0 = true;
                if count > 0 {
                    self.state = CdromState::StreamSector;
                } else {
                    self.state = CdromState::Idle;
                }
            }

            0x84 => {
                self.state = CdromState::Idle;
            }

            0x0E | 0x0F => {
                log::warn!(
                    "SoundBlaster: unimplemented audio command {:#04x}",
                    self.command
                );
                self.error = true;
                self.result_buf.push_back(RESULT_ERROR);
                self.state = CdromState::SendResult;
            }

            _ => {
                log::warn!("SoundBlaster: unknown CD-ROM command {:#04x}", self.command);
                self.error = true;
                self.result_buf.push_back(RESULT_ERROR);
                self.state = CdromState::SendResult;
            }
        }

        let irq_on_stream =
            matches!(self.state, CdromState::StreamSector) && !self.stream_via_base0;
        if (matches!(self.state, CdromState::SendResult) || irq_on_stream) && self.irq_enabled {
            self.pending_irq = true;
        }
    }

    fn stream_read_byte(&mut self) -> u8 {
        if self.read_sector_pos >= 2048 {
            if self.read_remaining == 0 {
                self.state = CdromState::Idle;
                return 0x00;
            }
            match self.disc.as_mut() {
                Some(disc) => {
                    let mut buf = [0u8; 2048];
                    if disc.read_sector(self.read_lba, &mut buf).is_ok() {
                        self.read_sector_buf = buf;
                    } else {
                        log::warn!("SoundBlaster: read_sector failed at LBA {}", self.read_lba);
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
            if self.read_remaining == 0 && !self.stream_via_base0 {
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
            _ => {}
        }
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
        self.stream_via_base0 = false;
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        let offset = port.wrapping_sub(self.base_port);
        match offset {
            0 => {
                let byte = match self.state {
                    CdromState::SendResult => {
                        let b = self.result_buf.pop_front().unwrap_or(0x00);
                        if self.result_buf.is_empty() {
                            self.state = CdromState::Idle;
                        }
                        b
                    }
                    CdromState::StreamSector if !self.result_buf.is_empty() => {
                        self.result_buf.pop_front().unwrap()
                    }
                    CdromState::StreamSector if self.stream_via_base0 => self.stream_read_byte(),
                    _ => self.status_byte(),
                };
                log::debug!("SbCd R base+0 val=0x{byte:02X} state={:?}", self.state);
                Some(byte)
            }
            1 => {
                let byte = match self.state {
                    CdromState::RecvParams(_) | CdromState::Execute => 0x04,
                    CdromState::StreamSector
                        if self.result_buf.is_empty() && !self.stream_via_base0 =>
                    {
                        self.stream_read_byte()
                    }
                    CdromState::StreamSector if self.stream_via_base0 => {
                        if self.read_sector_pos >= 2048 && self.read_remaining == 0 {
                            self.state = CdromState::Idle;
                            self.stream_via_base0 = false;
                            0x01
                        } else {
                            0x00
                        }
                    }
                    CdromState::Idle if self.attention_pending => {
                        self.attention_pending = false;
                        0x01
                    }
                    _ => 0x00,
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

    fn load_disc(&mut self, disc: Box<dyn CdromBackend>) {
        self.disc = Some(disc);
        self.door_open = false;
        self.error = false;
        self.state = CdromState::Idle;
        self.result_buf.clear();
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

/// Sound Blaster 16 emulation.
///
/// Phase 2: wraps the Panasonic CD-ROM interface (absorbed from `SoundBlasterCdrom`).
/// Phase 4: adds DSP subsystem — reset handshake and basic commands.
/// All other SB ports (OPL3, mixer, MPU-401) are stubs — added in later phases.
pub struct SoundBlaster {
    base_port: u16,
    cdrom: SoundBlasterCdromInner,
    dsp: SoundBlasterDsp,
    #[allow(dead_code)]
    cpu_freq: u64,
}

impl SoundBlaster {
    /// Create with default SB base 0x220, CD-ROM port 0x230, no disc, IRQ 5. Used by tests.
    pub fn new(cpu_freq: u64) -> Self {
        Self {
            base_port: 0x220,
            cdrom: SoundBlasterCdromInner::new(0x230, None, 5),
            dsp: SoundBlasterDsp::new(),
            cpu_freq,
        }
    }

    /// Create with explicit CD-ROM configuration for native/CLI use.
    pub fn with_cdrom(
        cdrom_base_port: u16,
        disc: Option<Box<dyn CdromBackend>>,
        irq_line: u8,
        cpu_freq: u64,
    ) -> Self {
        Self {
            base_port: 0x220,
            cdrom: SoundBlasterCdromInner::new(cdrom_base_port, disc, irq_line),
            dsp: SoundBlasterDsp::new(),
            cpu_freq,
        }
    }
}

impl Device for SoundBlaster {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.cdrom.reset();
        self.dsp.hardware_reset();
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, cycle_count: u32) -> Option<u8> {
        let sb_off = port.wrapping_sub(self.base_port);
        match sb_off {
            0x0A => return Some(self.dsp.read_data()),
            0x0C => return Some(0x00), // write-buffer status: never busy
            0x0E => return Some(self.dsp.read_status()),
            0x0F => return Some(self.dsp.read_ack16()),
            _ => {}
        }
        self.cdrom.io_read_u8(port, cycle_count)
    }

    fn io_write_u8(&mut self, port: u16, val: u8, cycle_count: u32) -> bool {
        let sb_off = port.wrapping_sub(self.base_port);
        match sb_off {
            0x06 => {
                self.dsp.write_reset_port(val);
                return true;
            }
            0x0C => {
                self.dsp.write_command_port(val);
                return true;
            }
            _ => {}
        }
        self.cdrom.io_write_u8(port, val, cycle_count)
    }
}

impl SoundCard for SoundBlaster {
    fn advance_to_cycle(&mut self, _cycle_count: u32) {}

    fn next_sample_cycle(&self) -> u32 {
        u32::MAX
    }
}

impl CdromController for SoundBlaster {
    fn load_disc(&mut self, disc: Box<dyn CdromBackend>) {
        self.cdrom.load_disc(disc);
    }

    fn eject_disc(&mut self) {
        self.cdrom.eject_disc();
    }

    fn take_pending_irq(&mut self) -> bool {
        if self.dsp.take_pending_irq() {
            return true;
        }
        self.cdrom.take_pending_irq()
    }

    fn irq_line(&self) -> u8 {
        self.cdrom.irq_line()
    }
}
