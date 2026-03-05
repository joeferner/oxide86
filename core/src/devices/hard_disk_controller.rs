use std::{any::Any, cell::RefCell};

use crate::{
    Device,
    disk::{Disk, DriveNumber},
};

/// ATA Primary controller I/O ports (base 0x1F0)
pub const HDC_DATA: u16 = 0x1F0; // Data register (read/write)
pub const HDC_ERROR: u16 = 0x1F1; // Error (R) / Features (W)
pub const HDC_SECTOR_COUNT: u16 = 0x1F2; // Sector count
pub const HDC_SECTOR_NUM: u16 = 0x1F3; // Sector number (CHS) / LBA low
pub const HDC_CYLINDER_LOW: u16 = 0x1F4; // Cylinder low / LBA mid
pub const HDC_CYLINDER_HIGH: u16 = 0x1F5; // Cylinder high / LBA high
pub const HDC_DRIVE_HEAD: u16 = 0x1F6; // Drive/Head select
pub const HDC_COMMAND: u16 = 0x1F7; // Command (W) / Status (R)
pub const HDC_DEVICE_CONTROL: u16 = 0x3F6; // Device control (W) / Alt status (R)

/// Status register bits
pub const HDC_STATUS_BSY: u8 = 0x80; // Busy
pub const HDC_STATUS_DRDY: u8 = 0x40; // Drive ready
pub const HDC_STATUS_DRQ: u8 = 0x08; // Data request
pub const HDC_STATUS_ERR: u8 = 0x01; // Error

/// Error register bits
pub const HDC_ERR_ABRT: u8 = 0x04; // Aborted command

/// ATA commands
const HDC_CMD_READ_SECTORS: u8 = 0x20;
const HDC_CMD_VERIFY_SECTORS: u8 = 0x40;
const HDC_CMD_WRITE_SECTORS: u8 = 0x30;
const HDC_CMD_EXECUTE_DIAG: u8 = 0x90;
const HDC_CMD_IDENTIFY: u8 = 0xEC;

/// ATA PIO command phase.
enum HdcPhase {
    Idle,
    /// Serving sector or identify data bytes to the CPU
    ReadData {
        data: Vec<u8>,
        index: usize,
    },
    /// Receiving sector data bytes from the CPU before committing the write
    WriteData {
        cylinder: u8,
        head: u8,
        sector: u8,
        drive: usize,
        data: Vec<u8>,
        expected: usize,
    },
}

pub struct HardDiskController {
    disks: Vec<Box<dyn Disk>>,

    // Writable ATA registers (set before issuing a command)
    sector_count: u8,
    sector_num: u8,
    cylinder_low: u8,
    cylinder_high: u8,
    drive_head: u8,

    /// Error register – updated by each command
    error: u8,
    /// ATA PIO state machine
    phase: RefCell<HdcPhase>,
    /// SRST (software reset) flag – asserted via device-control register bit 2
    srst: bool,
}

impl HardDiskController {
    pub fn new(disks: Vec<Box<dyn Disk>>) -> Self {
        Self {
            disks,
            sector_count: 0,
            sector_num: 0,
            cylinder_low: 0,
            cylinder_high: 0,
            drive_head: 0,
            error: 0,
            phase: RefCell::new(HdcPhase::Idle),
            srst: false,
        }
    }

    pub fn get_disk(&self, drive: DriveNumber) -> Option<&dyn Disk> {
        self.disks
            .get(drive.to_hard_drive_index())
            .map(|d| d.as_ref())
    }

    pub fn drive_count(&self) -> usize {
        self.disks.len()
    }

    /// Compute the ATA status register value from current state.
    fn status(&self) -> u8 {
        if self.srst {
            return HDC_STATUS_BSY;
        }
        match &*self.phase.borrow() {
            HdcPhase::Idle => {
                if self.error != 0 {
                    HDC_STATUS_DRDY | HDC_STATUS_ERR
                } else {
                    HDC_STATUS_DRDY
                }
            }
            HdcPhase::ReadData { .. } => HDC_STATUS_DRDY | HDC_STATUS_DRQ,
            HdcPhase::WriteData { .. } => HDC_STATUS_DRDY | HDC_STATUS_DRQ,
        }
    }

    /// Drive index (0 = master, 1 = slave) from the drive/head register bit 4.
    fn selected_drive(&self) -> usize {
        ((self.drive_head >> 4) & 0x01) as usize
    }

    /// Build a minimal 512-byte ATA IDENTIFY DEVICE response for the given drive index.
    fn build_identify(&self, drive_index: usize) -> Vec<u8> {
        let mut buf = vec![0u8; 512];
        if let Some(disk) = self.disks.get(drive_index) {
            let geo = disk.disk_geometry();

            // Word 1 (bytes 2–3): number of logical cylinders
            let cyls = geo.cylinders;
            buf[2] = (cyls & 0xFF) as u8;
            buf[3] = (cyls >> 8) as u8;

            // Word 3 (bytes 6–7): number of logical heads
            let heads = geo.heads;
            buf[6] = (heads & 0xFF) as u8;
            buf[7] = (heads >> 8) as u8;

            // Word 6 (bytes 12–13): sectors per track
            let spt = geo.sectors_per_track;
            buf[12] = (spt & 0xFF) as u8;
            buf[13] = (spt >> 8) as u8;

            // Words 60–61 (bytes 120–123): total addressable sectors (LBA28)
            let total = geo.total_sectors() as u32;
            buf[120] = (total & 0xFF) as u8;
            buf[121] = ((total >> 8) & 0xFF) as u8;
            buf[122] = ((total >> 16) & 0xFF) as u8;
            buf[123] = ((total >> 24) & 0xFF) as u8;
        }
        buf
    }

    /// Execute an ATA command using the currently latched registers.
    fn execute_command(&mut self, cmd: u8) {
        let drive_index = self.selected_drive();
        match cmd {
            HDC_CMD_READ_SECTORS => {
                let cylinder = (self.cylinder_low as u16) | ((self.cylinder_high as u16) << 8);
                // CHS mode: head in bits 3:0 of drive_head
                let head = self.drive_head & 0x0F;
                let sector = self.sector_num;
                let count = self.sector_count;

                // Cylinder is truncated to u8 due to current Disk trait limit (255 cylinders max)
                let result = self
                    .disks
                    .get(drive_index)
                    .map(|disk| disk.read_sectors(cylinder as u8, head, sector, count));

                match result {
                    Some(Ok(data)) => {
                        *self.phase.borrow_mut() = HdcPhase::ReadData { data, index: 0 };
                        self.error = 0;
                    }
                    Some(Err(e)) => {
                        log::warn!("HDC READ_SECTORS failed: {e}");
                        self.error = HDC_ERR_ABRT;
                    }
                    None => {
                        log::warn!("HDC READ_SECTORS: no disk at index {drive_index}");
                        self.error = HDC_ERR_ABRT;
                    }
                }
            }

            HDC_CMD_VERIFY_SECTORS => {
                // Verify that sectors are readable (ECC check) without transferring data to host
                let cylinder = (self.cylinder_low as u16) | ((self.cylinder_high as u16) << 8);
                let head = self.drive_head & 0x0F;
                let sector = self.sector_num;
                let count = self.sector_count;

                let result = self
                    .disks
                    .get(drive_index)
                    .map(|disk| disk.read_sectors(cylinder as u8, head, sector, count));

                match result {
                    Some(Ok(_)) => {
                        // Sectors readable — stay in Idle, no data phase
                        self.error = 0;
                    }
                    Some(Err(e)) => {
                        log::warn!("HDC VERIFY_SECTORS failed: {e}");
                        self.error = HDC_ERR_ABRT;
                    }
                    None => {
                        log::warn!("HDC VERIFY_SECTORS: no disk at index {drive_index}");
                        self.error = HDC_ERR_ABRT;
                    }
                }
            }

            HDC_CMD_WRITE_SECTORS => {
                let cylinder = (self.cylinder_low as u16) | ((self.cylinder_high as u16) << 8);
                let head = self.drive_head & 0x0F;
                let sector = self.sector_num;
                let count = self.sector_count;
                let expected = (count as usize) * 512;

                if drive_index >= self.disks.len() {
                    log::warn!("HDC WRITE_SECTORS: no disk at index {drive_index}");
                    self.error = HDC_ERR_ABRT;
                    return;
                }

                self.error = 0;
                *self.phase.borrow_mut() = HdcPhase::WriteData {
                    cylinder: cylinder as u8,
                    head,
                    sector,
                    drive: drive_index,
                    data: Vec::with_capacity(expected),
                    expected,
                };
            }

            HDC_CMD_IDENTIFY => {
                if drive_index < self.disks.len() {
                    let data = self.build_identify(drive_index);
                    *self.phase.borrow_mut() = HdcPhase::ReadData { data, index: 0 };
                    self.error = 0;
                } else {
                    // No disk attached — ABRT so the caller can detect absent drive
                    self.error = HDC_ERR_ABRT;
                }
            }

            HDC_CMD_EXECUTE_DIAG => {
                // Diagnostics pass: error code 0x01 = no error detected
                self.error = 0x01;
            }

            _ => {
                log::warn!("HDC: unknown ATA command 0x{cmd:02X}");
                self.error = HDC_ERR_ABRT;
            }
        }
    }
}

impl Device for HardDiskController {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        *self.phase.borrow_mut() = HdcPhase::Idle;
        self.error = 0;
        self.srst = false;
    }

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        match port {
            HDC_ERROR => Some(self.error),
            HDC_SECTOR_COUNT => Some(self.sector_count),
            HDC_SECTOR_NUM => Some(self.sector_num),
            HDC_CYLINDER_LOW => Some(self.cylinder_low),
            HDC_CYLINDER_HIGH => Some(self.cylinder_high),
            HDC_DRIVE_HEAD => Some(self.drive_head),
            // Status and alt-status return the same value
            HDC_COMMAND | HDC_DEVICE_CONTROL => Some(self.status()),
            HDC_DATA => {
                let current = std::mem::replace(&mut *self.phase.borrow_mut(), HdcPhase::Idle);
                let (byte, next) = match current {
                    HdcPhase::ReadData { data, index } => {
                        let b = data.get(index).copied().unwrap_or(0xFF);
                        let next_index = index + 1;
                        if next_index >= data.len() {
                            (b, HdcPhase::Idle)
                        } else {
                            (
                                b,
                                HdcPhase::ReadData {
                                    data,
                                    index: next_index,
                                },
                            )
                        }
                    }
                    other => (0xFF, other),
                };
                *self.phase.borrow_mut() = next;
                Some(byte)
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8) -> bool {
        match port {
            HDC_ERROR => {
                // Features register (ignored in basic emulation)
                true
            }
            HDC_SECTOR_COUNT => {
                self.sector_count = val;
                true
            }
            HDC_SECTOR_NUM => {
                self.sector_num = val;
                true
            }
            HDC_CYLINDER_LOW => {
                self.cylinder_low = val;
                true
            }
            HDC_CYLINDER_HIGH => {
                self.cylinder_high = val;
                true
            }
            HDC_DRIVE_HEAD => {
                self.drive_head = val;
                true
            }
            HDC_COMMAND => {
                self.execute_command(val);
                true
            }
            HDC_DEVICE_CONTROL => {
                let srst = val & 0x04 != 0;
                if srst && !self.srst {
                    // Asserting software reset: halt controller
                    self.srst = true;
                    *self.phase.borrow_mut() = HdcPhase::Idle;
                } else if !srst && self.srst {
                    // Releasing software reset: controller returns to ready state
                    self.srst = false;
                    self.error = 0;
                }
                true
            }
            HDC_DATA => {
                let current = std::mem::replace(&mut *self.phase.borrow_mut(), HdcPhase::Idle);
                let next = match current {
                    HdcPhase::WriteData {
                        cylinder,
                        head,
                        sector,
                        drive,
                        mut data,
                        expected,
                    } => {
                        data.push(val);
                        if data.len() >= expected {
                            let result = self
                                .disks
                                .get(drive)
                                .map(|disk| disk.write_sectors(cylinder, head, sector, &data));
                            match result {
                                Some(Ok(())) => {
                                    self.error = 0;
                                }
                                Some(Err(e)) => {
                                    log::warn!("HDC WRITE_SECTORS failed: {e}");
                                    self.error = HDC_ERR_ABRT;
                                }
                                None => {
                                    self.error = HDC_ERR_ABRT;
                                }
                            }
                            HdcPhase::Idle
                        } else {
                            HdcPhase::WriteData {
                                cylinder,
                                head,
                                sector,
                                drive,
                                data,
                                expected,
                            }
                        }
                    }
                    other => other,
                };
                *self.phase.borrow_mut() = next;
                true
            }
            _ => false,
        }
    }
}
