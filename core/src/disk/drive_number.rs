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
    /// - 0x00 = Current
    /// - 0x01 = Floppy A:
    /// - 0x02 = Floppy B:
    /// - 0x81 = Hard drive C:
    /// - 0x82 = Hard drive D:
    pub fn from_standard_with_current(drive: u8) -> Option<Self> {
        if drive == 0x00 {
            None
        } else {
            Some(Self::from_standard(drive - 1))
        }
    }

    /// Drive numbering:
    /// - 0x00 = Floppy A:
    /// - 0x01 = Floppy B:
    /// - 0x02 = Hard drive C:
    /// - 0x03 = Hard drive D:
    pub fn from_dos(drive_num: u8) -> Self {
        if drive_num < 2 {
            Self(drive_num)
        } else {
            Self((drive_num - 2) + 0x80)
        }
    }

    /// Drive numbering:
    /// - 0x00 = Current
    /// - 0x01 = Floppy A:
    /// - 0x02 = Floppy B:
    /// - 0x03 = Hard drive C:
    /// - 0x04 = Hard drive D:
    pub fn from_dos_with_current(drive: u8) -> Option<Self> {
        if drive == 0x00 {
            None
        } else {
            Some(Self::from_dos(drive - 1))
        }
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

    pub fn hard_drive_c() -> Self {
        Self(0x80)
    }

    pub fn from_letter(drive_letter: char) -> Option<DriveNumber> {
        match drive_letter {
            'A' => Some(Self::floppy_a()),
            'B' => Some(Self::floppy_b()),
            'C'..='Z' => Some(Self::from_standard(0x80 + (drive_letter as u8 - b'C'))),
            _ => None,
        }
    }

    /// Base drive number for CD-ROM drives
    pub const CDROM_BASE: u8 = 0xE0;

    /// Maximum number of CD-ROM slots
    pub const CDROM_MAX_SLOTS: u8 = 4;

    /// Create a CD-ROM drive number for the given slot (0-3)
    pub fn cdrom(slot: u8) -> Self {
        debug_assert!(slot < Self::CDROM_MAX_SLOTS, "CD-ROM slot must be 0-3");
        Self(Self::CDROM_BASE + slot)
    }

    pub fn is_floppy(&self) -> bool {
        self.0 < 0x80
    }

    pub fn is_hard_drive(&self) -> bool {
        self.0 >= 0x80 && self.0 < Self::CDROM_BASE
    }

    /// Returns true if this is a CD-ROM drive (0xE0-0xE3)
    pub fn is_cdrom(&self) -> bool {
        self.0 >= Self::CDROM_BASE && self.0 < Self::CDROM_BASE + Self::CDROM_MAX_SLOTS
    }

    /// Returns the CD-ROM slot index (0-3). Panics if not a CD-ROM drive.
    pub fn cdrom_slot(&self) -> u8 {
        debug_assert!(self.is_cdrom(), "cdrom_slot() called on non-CD-ROM drive");
        self.0 - Self::CDROM_BASE
    }

    pub fn to_hard_drive_index(&self) -> usize {
        // add range check
        (self.0 - 0x80) as usize
    }

    pub fn to_floppy_index(&self) -> usize {
        // add range check
        self.0 as usize
    }

    /// Drive numbering:
    /// - 0x00 = Floppy A:
    /// - 0x01 = Floppy B:
    /// - 0x80 = Hard drive C:
    /// - 0x81 = Hard drive D:
    pub fn to_standard(&self) -> u8 {
        self.0
    }

    /// DOS (0=A, 1=B, 2=C, ...)
    pub fn to_dos_drive(&self) -> u8 {
        if self.0 < 0x80 {
            self.0 // A: (0x00) -> 0, B: (0x01) -> 1
        } else {
            2 + (self.0 - 0x80) // C: (0x80) -> 2, D: (0x81) -> 3, etc.
        }
    }

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
