use std::{any::Any, cell::Cell};

use crate::{
    Device,
    disk::{Disk, DiskError, DiskGeometry, DriveNumber},
};

/// FDC I/O port addresses (primary controller, base 0x3F0)
pub const FDC_DOR: u16 = 0x3F2; // Digital Output Register (drive select + motor control)
pub const FDC_MSR: u16 = 0x3F4; // Main Status Register
pub const FDC_DIR: u16 = 0x3F7; // Digital Input Register (disk change line)

/// Bit 7 of DIR: disk change line (1 = disk has been changed, 0 = not changed)
pub const FDC_DIR_DISK_CHANGE: u8 = 0x80;

/// MSR bit 7: data register ready for transfer
const FDC_MSR_RQM: u8 = 0x80;

/// Floppy Disk Controller (Intel 8272A / NEC μPD765 compatible).
///
/// A single FDC manages both floppy drives (A: and B:). Drive selection is
/// done via the Digital Output Register (DOR, port 0x3F2). The Digital Input
/// Register (DIR, port 0x3F7) reflects the changeline of the currently
/// selected drive. Both drive slots always exist; a drive with no disk
/// inserted returns `DriveNotReady` on access.
pub struct FloppyDiskController {
    /// Disk image per drive: index 0 = A:, index 1 = B:. None = no disk inserted.
    drives: [Option<Box<dyn Disk>>; 2],
    /// Currently selected drive index (0 = A:, 1 = B:), set by DOR writes
    selected_drive: u8,
    /// Per-drive disk change line (mirrors DIR bit 7). Set to true when a disk is inserted or
    /// swapped; automatically cleared when the OS reads DIR via port 0x3F7.
    changeline: [Cell<bool>; 2],
}

impl FloppyDiskController {
    pub fn new() -> Self {
        Self {
            drives: [None, None],
            selected_drive: 0,
            changeline: [Cell::new(false), Cell::new(false)],
        }
    }

    /// Set the disk image for the given drive (A: or B:). Replaces any existing disk.
    pub fn set_drive_disk(&mut self, drive: DriveNumber, disk: Box<dyn Disk>) {
        assert!(
            drive.is_floppy(),
            "FloppyDiskController only supports floppy drives"
        );
        let idx = drive.to_floppy_index();
        assert!(idx < 2, "floppy drive index out of range");
        self.drives[idx] = Some(disk);
        self.changeline[idx].set(true);
    }

    pub fn disk_geometry(&self, drive: DriveNumber) -> Option<DiskGeometry> {
        self.drives
            .get(drive.to_floppy_index())?
            .as_ref()
            .map(|d| d.disk_geometry())
    }

    pub fn read_sectors(
        &self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        match self
            .drives
            .get(drive.to_floppy_index())
            .and_then(|d| d.as_ref())
        {
            Some(disk) => disk.read_sectors(cylinder, head, sector, count),
            None => Err(DiskError::DriveNotReady),
        }
    }
}

impl Device for FloppyDiskController {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {}

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        match port {
            FDC_MSR => Some(FDC_MSR_RQM),
            FDC_DIR => {
                let cell = self.changeline.get(self.selected_drive as usize)?;
                let changed = cell.get();
                cell.set(false);
                Some(if changed { FDC_DIR_DISK_CHANGE } else { 0x00 })
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8) -> bool {
        if port == FDC_DOR {
            // Bits 0-1 select the drive (0 = A:, 1 = B:)
            self.selected_drive = val & 0x01;
            true
        } else {
            false
        }
    }
}
