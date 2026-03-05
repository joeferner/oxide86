use std::fmt::{Debug, Display};

/// Drive numbering:
/// - 0x00 = Floppy A:
/// - 0x01 = Floppy B:
/// - 0x80 = Hard drive C:
/// - 0x81 = Hard drive D:
/// - 0xE0 = CD-ROM slot 0
/// - 0xE1 = CD-ROM slot 1
/// - 0xE2 = CD-ROM slot 2
/// - 0xE3 = CD-ROM slot 3
#[derive(PartialEq, PartialOrd, Clone, Copy)]
pub struct DriveNumber(u8);

impl DriveNumber {
    /// Drive numbering:
    /// - 0x00 = Floppy A:
    /// - 0x01 = Floppy B:
    /// - 0x80 = Hard drive C:
    /// - 0x81 = Hard drive D:
    pub fn from_standard(drive_num: u8) -> Self {
        Self(drive_num)
    }

    /// Drive numbering:
    /// - 0x00 = Hard drive C:
    /// - 0x01 = Hard drive D:
    pub fn from_hard_drive_index(hard_drive_index: usize) -> Self {
        Self(0x80 + (hard_drive_index as u8))
    }

    pub fn floppy_a() -> Self {
        Self(0x00)
    }

    pub fn floppy_b() -> Self {
        Self(0x01)
    }

    pub(crate) fn hard_drive_c() -> Self {
        Self(0x80)
    }

    /// Base drive number for CD-ROM drives
    pub const CDROM_BASE: u8 = 0xE0;

    /// Maximum number of CD-ROM slots
    pub const CDROM_MAX_SLOTS: u8 = 4;

    pub(crate) fn is_floppy(&self) -> bool {
        self.0 < 0x80
    }

    /// Returns true if this is a CD-ROM drive (0xE0-0xE3)
    pub(crate) fn is_cdrom(&self) -> bool {
        self.0 >= Self::CDROM_BASE && self.0 < Self::CDROM_BASE + Self::CDROM_MAX_SLOTS
    }

    pub(crate) fn as_hard_drive_index(&self) -> usize {
        // add range check
        (self.0 - 0x80) as usize
    }

    pub(crate) fn as_floppy_index(&self) -> usize {
        // add range check
        self.0 as usize
    }

    /// Drive numbering:
    /// - 0x00 = Floppy A:
    /// - 0x01 = Floppy B:
    /// - 0x80 = Hard drive C:
    /// - 0x81 = Hard drive D:
    pub(crate) fn as_standard(&self) -> u8 {
        self.0
    }

    // TODO
    // DOS (0=A, 1=B, 2=C, ...)
    // pub(crate) fn to_dos_drive(&self) -> u8 {
    //     if self.0 < 0x80 {
    //         self.0 // A: (0x00) -> 0, B: (0x01) -> 1
    //     } else {
    //         2 + (self.0 - 0x80) // C: (0x80) -> 2, D: (0x81) -> 3, etc.
    //     }
    // }

    pub fn to_letter(&self) -> char {
        if self.is_floppy() {
            (b'A' + self.0) as char
        } else if self.is_cdrom() {
            // CD-ROMs don't have a fixed DOS drive letter (MSCDEX assigns them)
            // Use placeholder letters Q+ for logging purposes
            (b'Q' + (self.0 - Self::CDROM_BASE)) as char
        } else {
            (b'C' + (self.0 - 0x80)) as char
        }
    }
}

impl Debug for DriveNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:02X}", self.0)
    }
}

impl Display for DriveNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "0x{:02X}", self.0)
    }
}
