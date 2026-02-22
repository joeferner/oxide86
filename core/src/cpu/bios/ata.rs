//! ATA / ATAPI command handlers for the primary IDE channel.
//!
//! These are `impl Bios` methods called from the CPU I/O instruction dispatch
//! (`cpu/instructions/io.rs`) when the driver accesses ports 0x1F0–0x1F7 or 0x3F6.
//! Because they live on `Bios`, they have full access to the `DriveManager`.

use crate::{
    DriveNumber,
    cdrom::CD_SECTOR_SIZE,
    io::ata::{self},
};

use super::Bios;

/// Which device occupies a given position on the primary channel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AtaDeviceType {
    None,
    /// ATA hard drive. Inner value: BIOS drive number (0x80 = C:, 0x81 = D:, …).
    HardDrive(u8),
    /// ATAPI CD-ROM. Inner value: CD-ROM slot index (0–3).
    CdRom(u8),
}

// ── Public I/O entry points (called from cpu/instructions/io.rs) ─────────────

impl Bios {
    /// Read a byte from ATA primary channel register `reg` (relative to base 0x1F0).
    pub fn ata_read_u8(&mut self, reg: u8) -> u8 {
        let value = self.shared.ata_primary.read_u8(reg);
        log::trace!("ATA Read:  reg 0x1F{} -> 0x{:02X}", reg, value);
        value
    }

    /// Read a 16-bit word from the ATA data port (0x1F0).
    pub fn ata_read_u16(&mut self) -> u16 {
        let value = self.shared.ata_primary.read_u16();
        log::trace!("ATA Read16: 0x1F0 -> 0x{:04X}", value);
        value
    }

    /// Read the alternate status register (0x3F7) — no interrupt side-effects.
    pub fn ata_read_alt_status(&self) -> u8 {
        self.shared.ata_primary.read_alt_status()
    }

    /// Write a byte to ATA primary channel register `reg` (relative to base 0x1F0).
    pub fn ata_write_u8(&mut self, reg: u8, value: u8) {
        log::trace!("ATA Write: reg 0x1F{} <- 0x{:02X}", reg, value);
        let needs_exec = self.shared.ata_primary.write_u8(reg, value);

        // After device-head write, update status to reflect whether the
        // selected device actually exists (driver polls this after selection).
        // Only do this when the channel is idle — if a transfer is in progress
        // (e.g. WaitingPacket after 0xA0) the driver may re-select the device
        // while polling for DRQ, and we must not clobber the current status.
        if reg == 6 {
            let is_slave = value & 0x10 != 0;
            let device = self.ata_device_at(is_slave);
            let exists = device != AtaDeviceType::None;
            if matches!(self.shared.ata_primary.transfer, ata::TransferState::Idle) {
                self.shared.ata_primary.status = if exists { ata::status::DRDY } else { 0x00 };
                // Update cylinder registers with device signature so drivers can
                // identify ATAPI devices by reading 0x1F4/0x1F5 after selection.
                if matches!(device, AtaDeviceType::CdRom(_)) {
                    self.shared.ata_primary.lba_mid = ata::ATAPI_SIG_LBA_MID;
                    self.shared.ata_primary.lba_high = ata::ATAPI_SIG_LBA_HIGH;
                } else {
                    self.shared.ata_primary.lba_mid = 0x00;
                    self.shared.ata_primary.lba_high = 0x00;
                }
            }
        }

        if needs_exec {
            self.ata_execute();
        }
    }

    /// Write a 16-bit word to the ATA data port (0x1F0).
    ///
    /// In `WaitingPacket` state, accumulates CDB bytes; dispatches the ATAPI
    /// command when all 12 bytes have been received.  In `DataIn` state,
    /// accumulates write-sector data.
    pub fn ata_write_u16(&mut self, value: u16) {
        log::trace!("ATA Write16: 0x1F0 <- 0x{:04X}", value);
        let done = self.shared.ata_primary.write_u16(value);
        if done {
            if self.shared.ata_primary.packet_ready {
                self.atapi_dispatch();
            } else {
                // DataIn complete — write sector to disk
                self.ata_flush_data_in();
            }
            // Fire IRQ14 after CDB/sector transfer completes (if interrupts enabled)
            if (self.shared.ata_primary.control & 0x02) == 0 {
                self.shared.pending_ata_irq = true;
            }
        }
    }

    /// Write the device-control register (0x3F6).
    pub fn ata_write_control(&mut self, value: u8) {
        log::trace!("ATA Write: control 0x3F6 <- 0x{:02X}", value);
        let reset_done = self.shared.ata_primary.write_control(value);
        if reset_done {
            // Apply ATAPI signature if the selected device is a CD-ROM
            let is_slave = self.shared.ata_primary.slave_selected();
            let atapi = matches!(self.ata_device_at(is_slave), AtaDeviceType::CdRom(_));
            self.shared.ata_primary.do_reset(atapi);
        }
    }

    // ── Device lookup ────────────────────────────────────────────────────────

    /// Return which device occupies master (`is_slave = false`) or slave.
    ///
    /// Convention (matches typical PC configuration):
    /// - Primary master → first real HDD (0x80) if present, else CD-ROM slot 0.
    /// - Primary slave  → second HDD (0x81) if present; else CD-ROM slot 0 when
    ///   HDD 0 is on master; else None.
    pub(super) fn ata_device_at(&self, is_slave: bool) -> AtaDeviceType {
        let dm = &self.shared.drive_manager;
        let hd_count = dm.hard_drive_count();

        if !is_slave {
            // Master
            if hd_count >= 1 {
                AtaDeviceType::HardDrive(0x80)
            } else if dm.has_cdrom(0) {
                AtaDeviceType::CdRom(0)
            } else {
                AtaDeviceType::None
            }
        } else {
            // Slave
            if hd_count >= 2 {
                AtaDeviceType::HardDrive(0x81)
            } else if hd_count == 1 && dm.has_cdrom(0) {
                AtaDeviceType::CdRom(0)
            } else if hd_count == 0 && dm.has_cdrom(1) {
                AtaDeviceType::CdRom(1)
            } else {
                AtaDeviceType::None
            }
        }
    }

    // ── Command dispatch ─────────────────────────────────────────────────────

    fn ata_execute(&mut self) {
        let cmd = match self.shared.ata_primary.pending_cmd.take() {
            Some(c) => c,
            None => return,
        };
        let is_slave = self.shared.ata_primary.slave_selected();
        let device = self.ata_device_at(is_slave);

        log::debug!(
            "ATA Command: 0x{:02X} on {} ({:?})",
            cmd,
            if is_slave { "slave" } else { "master" },
            device
        );

        match cmd {
            // NOP — return success without doing anything
            0x00 => self.shared.ata_primary.set_ok(),
            // IDENTIFY DEVICE (ATA hard drive)
            0xEC => self.ata_cmd_identify(device),
            // IDENTIFY PACKET DEVICE (ATAPI)
            0xA1 => self.ata_cmd_identify_packet(device),
            // READ SECTORS (with and without retry)
            0x20 | 0x21 => self.ata_cmd_read_sectors(device),
            // WRITE SECTORS (with and without retry)
            0x30 | 0x31 => self.ata_cmd_write_sectors(device),
            // PACKET — prepare to receive CDB from driver
            0xA0 => match device {
                AtaDeviceType::CdRom(_) => {
                    self.shared.ata_primary.begin_wait_packet();
                }
                _ => {
                    log::warn!("ATA PACKET (0xA0) sent to non-ATAPI device {:?}", device);
                    self.shared.ata_primary.set_error_bits(ata::error::ABRT);
                }
            },
            // SET FEATURES — accept and ignore (common during driver init)
            0xEF => {
                self.shared.ata_primary.set_ok();
            }
            // INITIALIZE DEVICE PARAMETERS (CHS geometry setup)
            0x91 => {
                self.shared.ata_primary.set_ok();
            }
            // DEVICE RESET (ATAPI only)
            0x08 => {
                let atapi = matches!(device, AtaDeviceType::CdRom(_));
                self.shared.ata_primary.do_reset(atapi);
            }
            // RECALIBRATE (0x10–0x1F), SEEK (0x70–0x7F)
            0x10..=0x1F | 0x70..=0x7F => {
                self.shared.ata_primary.set_ok();
            }
            _ => {
                log::warn!("Unimplemented ATA command: 0x{:02X}", cmd);
                self.shared.ata_primary.set_error_bits(ata::error::ABRT);
            }
        }

        // Fire ATA IRQ14 after command completion unless:
        //  - PACKET (0xA0): waiting for CDB — IRQ fires later via atapi_dispatch()
        //  - DEVICE RESET (0x08): resets don't generate interrupts
        //  - nIEN=1 (bit 1 of control register): driver has disabled interrupts
        let waiting_for_cdb = matches!(
            self.shared.ata_primary.transfer,
            ata::TransferState::WaitingPacket
        );
        let nien = (self.shared.ata_primary.control & 0x02) != 0;
        if !waiting_for_cdb && cmd != 0x08 && !nien {
            self.shared.pending_ata_irq = true;
        }
    }

    // ── ATA hard-drive commands ──────────────────────────────────────────────

    fn ata_cmd_identify(&mut self, device: AtaDeviceType) {
        let drive_num = match device {
            AtaDeviceType::HardDrive(n) => n,
            _ => {
                // Not a hard drive — could be ATAPI or None.
                // ATAPI-4 spec: IDENTIFY DEVICE (0xEC) sent to an ATAPI device must
                // respond with DRQ=1 and 512 bytes of IDENTIFY PACKET DEVICE data,
                // identical to IDENTIFY PACKET DEVICE (0xA1).  Older ATAPI-1 behavior
                // (ABRT + signature) is not accepted by Windows 9x-era drivers.
                if matches!(device, AtaDeviceType::CdRom(_)) {
                    self.ata_cmd_identify_packet(device);
                } else {
                    self.shared.ata_primary.set_error_bits(ata::error::ABRT);
                }
                return;
            }
        };

        let drive = DriveNumber::from_standard(drive_num);
        let geometry = match self.shared.drive_manager.get_hard_drive_disk(drive) {
            Some(disk) => *disk.geometry(),
            None => {
                self.shared.ata_primary.set_error_bits(ata::error::ABRT);
                return;
            }
        };

        let total_sectors = geometry.total_sectors() as u32;
        let mut buf = vec![0u8; 512];

        // Word 0: General config — fixed/non-removable ATA device
        write_word(&mut buf, 0, 0x0040);
        // Word 1: Number of cylinders
        write_word(&mut buf, 1, geometry.cylinders);
        // Word 3: Number of heads
        write_word(&mut buf, 3, geometry.heads);
        // Word 6: Sectors per track
        write_word(&mut buf, 6, geometry.sectors_per_track);
        // Words 10–19: Serial number (20 bytes, byte-swapped pairs)
        write_ata_str(&mut buf, 10, "OX86HD01            ");
        // Words 23–26: Firmware revision (8 bytes)
        write_ata_str(&mut buf, 23, "1.0     ");
        // Words 27–46: Model number (40 bytes)
        write_ata_str(&mut buf, 27, "Oxide86 ATA Hard Drive          ");
        // Word 47: Maximum sectors per interrupt (bit 8 set, bits 7-0 = max)
        write_word(&mut buf, 47, 0x8001);
        // Word 49: Capabilities — LBA supported (bit 9)
        write_word(&mut buf, 49, 0x0200);
        // Word 51: PIO cycle timing
        write_word(&mut buf, 51, 0x0200);
        // Word 53: Fields 54-58 and 64-70 valid
        write_word(&mut buf, 53, 0x0007);
        // Words 54–56: Current CHS translation
        write_word(&mut buf, 54, geometry.cylinders);
        write_word(&mut buf, 55, geometry.heads);
        write_word(&mut buf, 56, geometry.sectors_per_track);
        // Words 57–58: Current capacity in sectors (CHS)
        let chs_sectors =
            geometry.cylinders as u32 * geometry.heads as u32 * geometry.sectors_per_track as u32;
        write_word(&mut buf, 57, (chs_sectors & 0xFFFF) as u16);
        write_word(&mut buf, 58, (chs_sectors >> 16) as u16);
        // Word 63: Multiword DMA modes supported/active
        write_word(&mut buf, 63, 0x0007);
        // Word 64: Advanced PIO modes supported
        write_word(&mut buf, 64, 0x0003);
        // Words 60–61: Total user addressable sectors (LBA28)
        write_word(&mut buf, 60, (total_sectors & 0xFFFF) as u16);
        write_word(&mut buf, 61, (total_sectors >> 16) as u16);
        // Word 80: ATA/ATAPI versions supported (ATA-2 through ATA-6)
        write_word(&mut buf, 80, 0x007E);
        // Word 83: command set supported — LBA48 not supported (bit 10 = 0)
        write_word(&mut buf, 83, 0x4000);
        // Word 85: enabled command sets
        write_word(&mut buf, 85, 0x0000);

        log::debug!(
            "ATA IDENTIFY: drive 0x{:02X}, {} sectors",
            drive_num,
            total_sectors
        );
        self.shared.ata_primary.load_data_out(buf);
    }

    fn ata_cmd_identify_packet(&mut self, device: AtaDeviceType) {
        let slot = match device {
            AtaDeviceType::CdRom(s) => s,
            _ => {
                self.shared.ata_primary.set_error_bits(ata::error::ABRT);
                return;
            }
        };

        // Write ATAPI signature to cylinder registers (some drivers check these)
        self.shared.ata_primary.lba_mid = ata::ATAPI_SIG_LBA_MID;
        self.shared.ata_primary.lba_high = ata::ATAPI_SIG_LBA_HIGH;

        let mut buf = vec![0u8; 512];

        // Word 0: ATAPI, CD-ROM (type 5), 12-byte packets, removable
        //   bits 15-14 = 10 (ATAPI)
        //   bits 12-8  = 00101 (CD-ROM)
        //   bit 7      = 1 (removable)
        //   bits 1-0   = 00 (12-byte packet)
        write_word(&mut buf, 0, 0x8580);
        // Words 10–19: Serial number
        write_ata_str(&mut buf, 10, "OX86CD00            ");
        // Words 23–26: Firmware revision
        write_ata_str(&mut buf, 23, "1.00    ");
        // Words 27–46: Model number
        write_ata_str(&mut buf, 27, "Oxide86 ATAPI CD-ROM            ");
        // Word 49: LBA supported
        write_word(&mut buf, 49, 0x0200);
        // Word 53: Words 64–70 and 88 are valid
        write_word(&mut buf, 53, 0x0006);
        // Word 63: Multiword DMA modes
        write_word(&mut buf, 63, 0x0007);
        // Word 64: Advanced PIO modes
        write_word(&mut buf, 64, 0x0003);
        // Word 65–68: timing (minimum cycle times, ns)
        write_word(&mut buf, 65, 120);
        write_word(&mut buf, 66, 120);
        write_word(&mut buf, 67, 120);
        write_word(&mut buf, 68, 120);
        // Word 80: ATA/ATAPI standards
        write_word(&mut buf, 80, 0x007E);
        // Word 82: Command sets supported — PACKET feature set
        write_word(&mut buf, 82, 0x0014);

        log::debug!("ATAPI IDENTIFY PACKET DEVICE: slot {}", slot);
        self.shared.ata_primary.load_data_out(buf);
    }

    fn ata_cmd_read_sectors(&mut self, device: AtaDeviceType) {
        let drive_num = match device {
            AtaDeviceType::HardDrive(n) => n,
            _ => {
                self.shared.ata_primary.set_error_bits(ata::error::ABRT);
                return;
            }
        };
        let drive = DriveNumber::from_standard(drive_num);
        let ch = &self.shared.ata_primary;
        let count = if ch.sector_count == 0 {
            256u16
        } else {
            ch.sector_count as u16
        };

        let lba = if ch.device_head & 0x40 != 0 {
            // LBA28 mode
            ((ch.device_head as u32 & 0x0F) << 24)
                | ((ch.lba_high as u32) << 16)
                | ((ch.lba_mid as u32) << 8)
                | (ch.lba_low as u32)
        } else {
            // CHS mode — use ATA register values directly
            let cylinder = ((ch.lba_high as u32) << 8) | (ch.lba_mid as u32);
            let head = (ch.device_head & 0x0F) as u32;
            let sector = ch.lba_low as u32;
            // Standard BIOS geometry: 16 heads, 63 sectors/track
            cylinder * 16 * 63 + head * 63 + sector.saturating_sub(1)
        };

        match self.disk_read_sectors_lba(drive, lba, count) {
            Ok(data) => {
                log::debug!(
                    "ATA READ: drive 0x{:02X} LBA {} count {} OK ({} bytes)",
                    drive_num,
                    lba,
                    count,
                    data.len()
                );
                self.shared.ata_primary.load_data_out(data);
            }
            Err(e) => {
                log::warn!(
                    "ATA READ: drive 0x{:02X} LBA {} error: {:?}",
                    drive_num,
                    lba,
                    e
                );
                self.shared.ata_primary.set_error_bits(ata::error::ABRT);
            }
        }
    }

    fn ata_cmd_write_sectors(&mut self, device: AtaDeviceType) {
        let drive_num = match device {
            AtaDeviceType::HardDrive(n) => n,
            _ => {
                self.shared.ata_primary.set_error_bits(ata::error::ABRT);
                return;
            }
        };
        let ch = &self.shared.ata_primary;
        let lba = if ch.device_head & 0x40 != 0 {
            ((ch.device_head as u32 & 0x0F) << 24)
                | ((ch.lba_high as u32) << 16)
                | ((ch.lba_mid as u32) << 8)
                | (ch.lba_low as u32)
        } else {
            let cylinder = ((ch.lba_high as u32) << 8) | (ch.lba_mid as u32);
            let head = (ch.device_head & 0x0F) as u32;
            let sector = ch.lba_low as u32;
            cylinder * 16 * 63 + head * 63 + sector.saturating_sub(1)
        };
        // Prepare to receive 512 bytes from the driver
        self.shared.ata_primary.begin_data_in(lba, drive_num);
    }

    /// Called when a `DataIn` transfer completes (write-sector data fully received).
    fn ata_flush_data_in(&mut self) {
        if let Some((data, lba, drive_num)) = self.shared.ata_primary.take_data_in() {
            let drive = DriveNumber::from_standard(drive_num);
            match self
                .shared
                .drive_manager
                .disk_write_sectors_lba(drive, lba, &data)
            {
                Ok(_) => {
                    log::debug!("ATA WRITE: drive 0x{:02X} LBA {} OK", drive_num, lba);
                    self.shared.ata_primary.set_ok();
                }
                Err(e) => {
                    log::warn!(
                        "ATA WRITE: drive 0x{:02X} LBA {} error: {:?}",
                        drive_num,
                        lba,
                        e
                    );
                    self.shared.ata_primary.set_error_bits(ata::error::ABRT);
                }
            }
        }
    }

    // ── ATAPI command dispatch ───────────────────────────────────────────────

    fn atapi_dispatch(&mut self) {
        self.shared.ata_primary.packet_ready = false;
        let cdb = self.shared.ata_primary.packet_buf;
        let is_slave = self.shared.ata_primary.slave_selected();
        let device = self.ata_device_at(is_slave);
        let slot = match device {
            AtaDeviceType::CdRom(s) => s,
            _ => {
                self.shared.ata_primary.set_error_bits(ata::error::ABRT);
                return;
            }
        };

        log::debug!("ATAPI CDB[0]=0x{:02X} slot={}", cdb[0], slot);

        match cdb[0] {
            0x00 => self.atapi_test_unit_ready(slot),
            0x03 => self.atapi_request_sense(slot),
            0x12 => self.atapi_inquiry(slot, &cdb),
            0x1B => self.atapi_start_stop(slot, cdb[4]),
            0x1E => self.atapi_prevent_allow_removal(),
            0x25 => self.atapi_read_capacity(slot),
            0x28 => self.atapi_read10(slot, &cdb),
            0x43 => self.atapi_read_toc(slot, &cdb),
            0x4A => self.atapi_get_event_status(&cdb),
            0x5A => self.atapi_mode_sense10(slot, &cdb),
            0x1A => self.atapi_mode_sense6(slot, &cdb),
            _ => {
                log::warn!("Unimplemented ATAPI command: 0x{:02X}", cdb[0]);
                self.shared.ata_primary.set_sense(0x05, 0x20, 0x00); // ILLEGAL REQUEST / INVALID COMMAND OPCODE
                self.shared.ata_primary.set_error_bits(0x54); // ABRT + sense key
            }
        }
    }

    fn atapi_test_unit_ready(&mut self, slot: u8) {
        if self.shared.drive_manager.has_cdrom(slot) {
            self.shared.ata_primary.set_sense(0, 0, 0);
            self.shared.ata_primary.atapi_set_ok();
        } else {
            // NOT READY / MEDIUM NOT PRESENT
            self.shared.ata_primary.set_sense(0x02, 0x3A, 0x00);
            self.shared.ata_primary.set_error_bits(0x54);
        }
    }

    fn atapi_request_sense(&mut self, _slot: u8) {
        let key = self.shared.ata_primary.sense_key;
        let asc = self.shared.ata_primary.asc;
        let ascq = self.shared.ata_primary.ascq;

        let mut buf = [0u8; 18];
        buf[0] = 0x70; // Current errors, fixed format
        buf[2] = key & 0x0F;
        buf[7] = 0x0A; // Additional sense length = 10
        buf[12] = asc;
        buf[13] = ascq;

        self.shared.ata_primary.atapi_load_data_out(buf.to_vec());
    }

    fn atapi_inquiry(&mut self, slot: u8, cdb: &[u8; 12]) {
        let alloc_len = cdb[4] as usize;
        let mut buf = vec![0u8; 36.max(alloc_len)];
        buf[0] = 0x05; // CD-ROM device type
        buf[1] = 0x80; // Removable media
        buf[2] = 0x00; // SCSI-1 compliance
        buf[3] = 0x21; // ATAPI response data format
        buf[4] = 0x1F; // Additional length (31 bytes)
        // Vendor (bytes 8–15, 8 chars)
        let vendor = b"OX86    ";
        buf[8..16].copy_from_slice(vendor);
        // Product (bytes 16–31, 16 chars)
        let product = b"CDROM           ";
        buf[16..32].copy_from_slice(product);
        // Revision (bytes 32–35, 4 chars)
        buf[32..36].copy_from_slice(b"1.00");

        let _ = slot;
        buf.truncate(alloc_len.max(36));
        self.shared.ata_primary.atapi_load_data_out(buf);
    }

    fn atapi_start_stop(&mut self, slot: u8, power_cond: u8) {
        // LoEj bit (bit 1) + Start bit (bit 0): we just acknowledge
        let _ = (slot, power_cond);
        self.shared.ata_primary.atapi_set_ok();
    }

    fn atapi_prevent_allow_removal(&mut self) {
        // Always succeed — we don't physically eject
        self.shared.ata_primary.atapi_set_ok();
    }

    fn atapi_read_capacity(&mut self, slot: u8) {
        let total_sectors = if let Some(image) = self.shared.drive_manager.cdrom_image(slot) {
            (image.size() / CD_SECTOR_SIZE) as u32
        } else {
            self.shared.ata_primary.set_sense(0x02, 0x3A, 0x00);
            self.shared.ata_primary.set_error_bits(0x54);
            return;
        };

        let last_lba = total_sectors.saturating_sub(1);
        let block_size: u32 = CD_SECTOR_SIZE as u32;

        let mut buf = [0u8; 8];
        // Last LBA (big-endian)
        buf[0..4].copy_from_slice(&last_lba.to_be_bytes());
        // Block size (big-endian)
        buf[4..8].copy_from_slice(&block_size.to_be_bytes());

        self.shared.ata_primary.atapi_load_data_out(buf.to_vec());
    }

    fn atapi_read10(&mut self, slot: u8, cdb: &[u8; 12]) {
        // LBA: CDB bytes 2–5 (big-endian)
        let lba = u32::from_be_bytes([cdb[2], cdb[3], cdb[4], cdb[5]]);
        // Transfer length: CDB bytes 7–8 (big-endian)
        let count = u16::from_be_bytes([cdb[7], cdb[8]]) as u32;

        let image = match self.shared.drive_manager.cdrom_image(slot) {
            Some(img) => img,
            None => {
                self.shared.ata_primary.set_sense(0x02, 0x3A, 0x00);
                self.shared.ata_primary.set_error_bits(0x54);
                return;
            }
        };

        let mut data = Vec::with_capacity((count as usize) * CD_SECTOR_SIZE);
        for i in 0..count {
            match image.read_sector(lba + i) {
                Ok(sector) => data.extend_from_slice(&sector),
                Err(e) => {
                    log::warn!("ATAPI READ10: sector {} error: {}", lba + i, e);
                    self.shared.ata_primary.set_sense(0x03, 0x11, 0x00); // MEDIUM ERROR
                    self.shared.ata_primary.set_error_bits(0x54);
                    return;
                }
            }
        }

        log::debug!("ATAPI READ10: slot {} LBA {} count {} OK", slot, lba, count);
        self.shared.ata_primary.atapi_load_data_out(data);
    }

    fn atapi_read_toc(&mut self, slot: u8, cdb: &[u8; 12]) {
        let msf = cdb[1] & 0x02 != 0;
        let alloc_len = u16::from_be_bytes([cdb[7], cdb[8]]) as usize;

        let total_sectors = if let Some(image) = self.shared.drive_manager.cdrom_image(slot) {
            (image.size() / CD_SECTOR_SIZE) as u32
        } else {
            self.shared.ata_primary.set_sense(0x02, 0x3A, 0x00);
            self.shared.ata_primary.set_error_bits(0x54);
            return;
        };

        // Build minimal TOC: 2 descriptors (track 1 + lead-out)
        // TOC header: data length (BE, excludes length field) + first + last track
        // Each descriptor: 8 bytes
        // Total: 4 (header) + 2×8 = 20 bytes; data length = 18 (20 - 2)
        let mut buf = vec![0u8; 20];

        // Header
        buf[0] = 0x00; // TOC data length high (18 = 0x0012)
        buf[1] = 0x12; // TOC data length low
        buf[2] = 0x01; // First track
        buf[3] = 0x01; // Last track

        // Track 1 descriptor (offset 4)
        buf[4] = 0x00; // Reserved
        buf[5] = 0x14; // ADR=1 (Q subchannel, LBA), Control=4 (data)
        buf[6] = 0x01; // Track number = 1
        buf[7] = 0x00; // Reserved
        // Track 1 start address (LBA 0)
        write_be_u32(&mut buf[8..12], 0);

        // Lead-out descriptor (offset 12)
        buf[12] = 0x00; // Reserved
        buf[13] = 0x14; // ADR/Control
        buf[14] = 0xAA; // Lead-out track number
        buf[15] = 0x00; // Reserved
        // Lead-out address
        if msf {
            let msf_addr = lba_to_msf(total_sectors);
            buf[16] = 0x00;
            buf[17] = msf_addr[0]; // M
            buf[18] = msf_addr[1]; // S
            buf[19] = msf_addr[2]; // F
        } else {
            write_be_u32(&mut buf[16..20], total_sectors);
        }

        buf.truncate(alloc_len.max(4));
        self.shared.ata_primary.atapi_load_data_out(buf);
    }

    fn atapi_get_event_status(&mut self, cdb: &[u8; 12]) {
        let alloc_len = u16::from_be_bytes([cdb[7], cdb[8]]) as usize;
        // Return "No Events" response (8 bytes)
        let mut buf = vec![0u8; 8.max(alloc_len)];
        // Event header: length = 6, notification class = No Events (0x00)
        buf[0] = 0x00; // Event data length high
        buf[1] = 0x06; // Event data length low = 6
        buf[2] = 0x80; // NEA=1 (No Event Available, polled mode)
        buf[3] = 0x00; // Supported event classes
        buf.truncate(alloc_len.max(8));
        self.shared.ata_primary.atapi_load_data_out(buf);
    }

    fn atapi_mode_sense10(&mut self, slot: u8, cdb: &[u8; 12]) {
        let page_code = cdb[2] & 0x3F;
        let alloc_len = u16::from_be_bytes([cdb[7], cdb[8]]) as usize;
        let _ = slot;
        // Return a minimal Mode Sense(10) response header + requested page
        // For unknown pages, just return an empty response with no error
        let mut buf = vec![0u8; 8.max(alloc_len)];
        // Mode Parameter Header(10): 8 bytes
        buf[0] = 0x00; // Mode Data Length MSB
        buf[1] = 0x06; // Mode Data Length LSB = 6 (length of data following this field)
        buf[2] = 0x00; // Medium Type
        buf[3] = 0x00; // Device-Specific Parameter
        buf[4] = 0x00; // Reserved
        buf[5] = 0x00; // Reserved
        buf[6] = 0x00; // Block Descriptor Length MSB
        buf[7] = 0x00; // Block Descriptor Length LSB

        let _ = page_code;
        buf.truncate(alloc_len.max(8));
        self.shared.ata_primary.atapi_load_data_out(buf);
    }

    fn atapi_mode_sense6(&mut self, slot: u8, cdb: &[u8; 12]) {
        let alloc_len = cdb[4] as usize;
        let _ = (slot, cdb);
        // Minimal Mode Sense(6) response header (4 bytes)
        let mut buf = vec![0u8; 4.max(alloc_len)];
        buf[0] = 0x03; // Mode Data Length = 3 (bytes following)
        buf[1] = 0x00; // Medium Type
        buf[2] = 0x00; // Device-Specific
        buf[3] = 0x00; // Block Descriptor Length
        buf.truncate(alloc_len.max(4));
        self.shared.ata_primary.atapi_load_data_out(buf);
    }
}

// ── Helper functions ─────────────────────────────────────────────────────────

/// Write a 16-bit word (LE) at word offset `word_idx` in a byte buffer.
fn write_word(buf: &mut [u8], word_idx: usize, value: u16) {
    let byte_idx = word_idx * 2;
    if byte_idx + 1 < buf.len() {
        let [lo, hi] = value.to_le_bytes();
        buf[byte_idx] = lo;
        buf[byte_idx + 1] = hi;
    }
}

/// Write a big-endian u32 into a byte slice.
fn write_be_u32(buf: &mut [u8], value: u32) {
    if buf.len() >= 4 {
        buf[0..4].copy_from_slice(&value.to_be_bytes());
    }
}

/// Write an ATA-style string (byte pairs swapped within each word) at `word_start`.
///
/// ATA IDENTIFY strings store characters in swapped pairs: for the string "AB",
/// byte 0 (low byte of word) = 'B', byte 1 (high byte) = 'A'.  Pad with spaces.
fn write_ata_str(buf: &mut [u8], word_start: usize, text: &str) {
    let byte_start = word_start * 2;
    let max_chars = {
        let remaining = buf.len().saturating_sub(byte_start);
        // Round down to even
        remaining & !1
    };
    // Fill with spaces first
    for b in buf[byte_start..byte_start + max_chars].iter_mut() {
        *b = b' ';
    }
    // Write swapped character pairs
    let bytes = text.as_bytes();
    let mut i = 0usize;
    while i + 1 < max_chars {
        let c0 = bytes.get(i).copied().unwrap_or(b' ');
        let c1 = bytes.get(i + 1).copied().unwrap_or(b' ');
        // Swap: low byte = second char, high byte = first char
        buf[byte_start + i] = c1;
        buf[byte_start + i + 1] = c0;
        i += 2;
    }
}

/// Convert a CD-ROM LBA sector number to MSF (Minute/Second/Frame) encoding.
/// Each second has 75 frames; each minute has 60 seconds.
fn lba_to_msf(lba: u32) -> [u8; 3] {
    // Add the 2-second pre-gap offset (150 frames)
    let frame_total = lba + 150;
    let frame = (frame_total % 75) as u8;
    let second = ((frame_total / 75) % 60) as u8;
    let minute = (frame_total / 75 / 60) as u8;
    [minute, second, frame]
}
