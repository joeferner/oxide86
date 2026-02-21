//! ATA / ATAPI primary-channel register emulation (ports 0x1F0–0x1F7 and 0x3F6).
//!
//! This module is a pure state machine: it owns the ATA registers and the data
//! transfer buffer, but performs no disk I/O.  Actual sector reads / writes are
//! delegated to the `Bios` command handlers in `cpu/bios/ata.rs`, which have
//! access to the `DriveManager`.

// ── Status register bits ─────────────────────────────────────────────────────
pub mod status {
    /// Error flag — check the error register for details.
    pub const ERR: u8 = 0x01;
    /// Data Request — drive is ready to transfer data to/from the host.
    pub const DRQ: u8 = 0x08;
    /// Drive Ready — drive is spun up and ready for commands.
    pub const DRDY: u8 = 0x40;
    /// Busy — drive is executing a command; other bits are meaningless.
    pub const BSY: u8 = 0x80;
}

// ── Error register bits ──────────────────────────────────────────────────────
pub mod error {
    /// Aborted command.
    pub const ABRT: u8 = 0x04;
}

/// ATAPI device signature bytes placed in the cylinder registers after a reset.
pub const ATAPI_SIG_LBA_MID: u8 = 0x14;
pub const ATAPI_SIG_LBA_HIGH: u8 = 0xEB;

/// Data-port transfer direction.
pub enum TransferState {
    /// No transfer in progress.
    Idle,
    /// ATAPI PACKET command received; waiting for the 12-byte CDB from the driver.
    WaitingPacket,
    /// Buffer contains data the driver should read (DRQ asserted).
    DataOut { buf: Vec<u8>, pos: usize },
    /// Waiting for the driver to supply sector data for a write command (DRQ asserted).
    DataIn {
        received: Vec<u8>,
        target_size: usize,
        lba: u32,
        drive_num: u8,
    },
}

/// State for the primary ATA channel (base 0x1F0).
///
/// Device knowledge (which drive lives on master/slave) is NOT stored here; the
/// `Bios` command handlers query the `DriveManager` dynamically.
pub struct AtaChannel {
    // ── Shadow registers (last value written by the driver) ──────────────────
    /// Features / Error register — written by driver (ATA feature select / ATAPI features).
    pub features: u8,
    /// Sector Count register.
    pub sector_count: u8,
    /// LBA Low / Sector Number register.
    pub lba_low: u8,
    /// LBA Mid / Cylinder Low / ATAPI byte-count low.
    pub lba_mid: u8,
    /// LBA High / Cylinder High / ATAPI byte-count high.
    pub lba_high: u8,
    /// Device/Head register (bit 4 = drive select: 0=master, 1=slave).
    pub device_head: u8,

    // ── Read-back registers (reflect device state) ───────────────────────────
    /// Error register — set by the device after a failed command.
    pub error: u8,
    /// Status register — read from port 0x1F7 / alternate status 0x3F7.
    pub status: u8,

    // ── Device control register ──────────────────────────────────────────────
    /// Written to port 0x3F6 (nIEN = bit 1, SRST = bit 2).
    pub control: u8,
    /// Tracks whether SRST was high on the previous write (edge detection).
    srst_prev: bool,

    // ── Command pipeline ─────────────────────────────────────────────────────
    /// Set when the driver writes a command byte to port 0x1F7.
    /// Cleared by `Bios::ata_execute()` after the command is processed.
    pub pending_cmd: Option<u8>,

    // ── ATAPI packet accumulation ────────────────────────────────────────────
    /// 12-byte Command Descriptor Block accumulated from data-port writes.
    pub packet_buf: [u8; 12],
    /// Number of bytes written to `packet_buf` so far (0–12).
    pub packet_bytes: u8,
    /// Set by Bios after the ATAPI CDB is complete; cleared after dispatch.
    pub packet_ready: bool,

    // ── Data transfer ────────────────────────────────────────────────────────
    pub transfer: TransferState,

    // ── ATAPI sense data (REQUEST SENSE response) ────────────────────────────
    pub sense_key: u8,
    /// Additional Sense Code.
    pub asc: u8,
    /// Additional Sense Code Qualifier.
    pub ascq: u8,
}

impl Default for AtaChannel {
    fn default() -> Self {
        Self::new()
    }
}

impl AtaChannel {
    pub fn new() -> Self {
        Self {
            features: 0,
            sector_count: 0,
            lba_low: 0,
            lba_mid: 0,
            lba_high: 0,
            device_head: 0,
            error: 0,
            // Power-on default: drive ready, no errors
            status: status::DRDY,
            control: 0x08, // nIEN set at power-on (interrupts disabled)
            srst_prev: false,
            pending_cmd: None,
            packet_buf: [0u8; 12],
            packet_bytes: 0,
            packet_ready: false,
            transfer: TransferState::Idle,
            sense_key: 0,
            asc: 0,
            ascq: 0,
        }
    }

    /// `true` if the slave device is selected (bit 4 of `device_head`).
    pub fn slave_selected(&self) -> bool {
        self.device_head & 0x10 != 0
    }

    // ── Port reads ───────────────────────────────────────────────────────────

    /// Read a byte from a register at `reg` (0 = data port, …, 7 = status).
    pub fn read_u8(&mut self, reg: u8) -> u8 {
        match reg {
            0 => {
                // Byte read from data port: consume low byte of the next word.
                let word = self.read_u16();
                (word & 0xFF) as u8
            }
            1 => self.error,
            2 => self.sector_count,
            3 => self.lba_low,
            4 => self.lba_mid,
            5 => self.lba_high,
            6 => self.device_head,
            7 => self.status, // reading status clears any interrupt request (no-op here)
            _ => 0xFF,
        }
    }

    /// Read a 16-bit word from the data port (0x1F0).
    ///
    /// Pops two bytes from the `DataOut` buffer; clears DRQ automatically when the
    /// buffer is exhausted.
    pub fn read_u16(&mut self) -> u16 {
        let (lo, hi, exhausted) = match &mut self.transfer {
            TransferState::DataOut { buf, pos } => {
                let lo = buf.get(*pos).copied().unwrap_or(0xFF);
                let hi = buf.get(*pos + 1).copied().unwrap_or(0xFF);
                *pos += 2;
                let exhausted = *pos >= buf.len();
                (lo, hi, exhausted)
            }
            _ => (0xFF, 0xFF, true),
        };
        if exhausted {
            self.transfer = TransferState::Idle;
            self.status = status::DRDY; // DRQ cleared, DRDY remains
        }
        u16::from_le_bytes([lo, hi])
    }

    /// Read the alternate status register (0x3F7): same value as `status`, no side-effects.
    pub fn read_alt_status(&self) -> u8 {
        self.status
    }

    // ── Port writes ──────────────────────────────────────────────────────────

    /// Write a byte to register `reg` (0 = data port, …, 7 = command).
    ///
    /// Returns `true` when a command is ready for `Bios` to process (i.e., a
    /// command byte was written to register 7, or an ATAPI CDB completed via
    /// the data port while in `WaitingPacket` state).
    pub fn write_u8(&mut self, reg: u8, value: u8) -> bool {
        match reg {
            0 => self.write_u16(value as u16),
            1 => {
                self.features = value;
                false
            }
            2 => {
                self.sector_count = value;
                false
            }
            3 => {
                self.lba_low = value;
                false
            }
            4 => {
                self.lba_mid = value;
                false
            }
            5 => {
                self.lba_high = value;
                false
            }
            6 => {
                self.device_head = value;
                false
            }
            7 => {
                // Command register — only valid when not busy
                if self.status & status::BSY == 0 {
                    self.pending_cmd = Some(value);
                    self.status = status::BSY;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Write a 16-bit word to the data port (0x1F0).
    ///
    /// In `WaitingPacket` state, bytes accumulate into the CDB buffer; returns
    /// `true` when all 12 bytes have arrived.  In `DataIn` state, bytes accumulate
    /// for a write-sector command; returns `true` when the full sector is received.
    pub fn write_u16(&mut self, value: u16) -> bool {
        let [lo, hi] = value.to_le_bytes();
        match &mut self.transfer {
            TransferState::WaitingPacket => {
                let idx = self.packet_bytes as usize;
                if idx < 12 {
                    self.packet_buf[idx] = lo;
                }
                if idx + 1 < 12 {
                    self.packet_buf[idx + 1] = hi;
                }
                self.packet_bytes = (self.packet_bytes + 2).min(12);
                if self.packet_bytes >= 12 {
                    self.transfer = TransferState::Idle;
                    self.status = status::BSY;
                    self.packet_ready = true;
                    return true; // Bios must call atapi_dispatch()
                }
                false
            }
            TransferState::DataIn {
                received,
                target_size,
                ..
            } => {
                received.push(lo);
                let target = *target_size;
                if received.len() < target {
                    received.push(hi);
                }
                received.len() >= target
            }
            _ => false,
        }
    }

    /// Write the device-control register (port 0x3F6).
    ///
    /// Returns `true` on the SRST falling edge (reset complete, Bios should
    /// apply the post-reset state).
    pub fn write_control(&mut self, value: u8) -> bool {
        let srst_now = value & 0x04 != 0;
        let was_srst = self.srst_prev;
        self.srst_prev = srst_now;
        self.control = value;
        if was_srst && !srst_now {
            // Falling edge of SRST → reset the channel
            self.do_reset(false); // caller decides whether to apply ATAPI sig
            true
        } else {
            false
        }
    }

    // ── Bios-facing helpers ──────────────────────────────────────────────────

    /// Load data into the transfer buffer and assert DRQ.
    pub fn load_data_out(&mut self, buf: Vec<u8>) {
        self.transfer = TransferState::DataOut { buf, pos: 0 };
        self.status = status::DRDY | status::DRQ;
        self.error = 0;
    }

    /// Signal successful command completion (no data transfer).
    pub fn set_ok(&mut self) {
        self.transfer = TransferState::Idle;
        self.status = status::DRDY;
        self.error = 0;
    }

    /// Signal a command error.
    pub fn set_error_bits(&mut self, err_bits: u8) {
        self.transfer = TransferState::Idle;
        self.error = err_bits;
        self.status = status::DRDY | status::ERR;
    }

    /// Record ATAPI sense data for the next REQUEST SENSE response.
    pub fn set_sense(&mut self, key: u8, asc: u8, ascq: u8) {
        self.sense_key = key;
        self.asc = asc;
        self.ascq = ascq;
    }

    /// Begin accepting sector data from the driver (write-sector DRQ phase).
    pub fn begin_data_in(&mut self, lba: u32, drive_num: u8) {
        self.transfer = TransferState::DataIn {
            received: Vec::with_capacity(512),
            target_size: 512,
            lba,
            drive_num,
        };
        self.status = status::DRDY | status::DRQ;
        self.error = 0;
    }

    /// Take ownership of a completed `DataIn` buffer.
    pub fn take_data_in(&mut self) -> Option<(Vec<u8>, u32, u8)> {
        let old = std::mem::replace(&mut self.transfer, TransferState::Idle);
        if let TransferState::DataIn {
            received,
            lba,
            drive_num,
            ..
        } = old
        {
            Some((received, lba, drive_num))
        } else {
            None
        }
    }

    /// Reset the channel state (called on SRST falling edge or DEVICE RESET command).
    ///
    /// If `atapi_sig` is true, write the ATAPI signature to the cylinder registers.
    pub fn do_reset(&mut self, atapi_sig: bool) {
        self.pending_cmd = None;
        self.packet_bytes = 0;
        self.packet_ready = false;
        self.error = 0x01; // Diagnostic pass
        self.transfer = TransferState::Idle;
        self.sector_count = 0x01;
        self.lba_low = 0x01;
        if atapi_sig {
            self.lba_mid = ATAPI_SIG_LBA_MID;
            self.lba_high = ATAPI_SIG_LBA_HIGH;
        } else {
            self.lba_mid = 0x00;
            self.lba_high = 0x00;
        }
        self.status = status::DRDY;
    }

    /// Enter WaitingPacket state after the PACKET command (0xA0) is issued.
    pub fn begin_wait_packet(&mut self) {
        self.packet_bytes = 0;
        self.packet_ready = false;
        self.transfer = TransferState::WaitingPacket;
        self.status = status::DRDY | status::DRQ; // ready for CDB bytes
        self.error = 0;
    }
}
