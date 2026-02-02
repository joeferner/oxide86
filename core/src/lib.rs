use std::fmt::{Debug, Display};

pub use crate::cpu::bios::{Bios, DosDevice, DriveParams, KeyPress, SharedBiosState};
pub use crate::decoder::{DecodedInstruction, decode_instruction, decode_instruction_with_regs};
pub use crate::disk::{
    BackedDisk, DiskBackend, DiskController, DiskGeometry, DiskImage, MemoryDiskBackend,
    PartitionedDisk, SECTOR_SIZE, parse_mbr,
};
#[cfg(not(target_arch = "wasm32"))]
pub use crate::disk_backend::FileDiskBackend;
pub use crate::drive_manager::{DiskAdapter, DriveManager};
pub use crate::io::Pit;
pub use crate::keyboard::KeyboardInput;
pub use crate::memory_allocator::MemoryAllocator;
pub use crate::mouse::{MouseInput, MouseState, NullMouse};
#[cfg(feature = "audio-rodio")]
pub use crate::rodio_speaker::RodioSpeaker;
pub use crate::serial_mouse::SerialMouse;
pub use crate::serial_port::{SerialDevice, SerialParams, SerialPortController, SerialStatus};
pub use crate::speaker::{NullSpeaker, SpeakerOutput};
pub use crate::video::{
    CursorPosition, NullVideoController, TextAttribute, TextCell, Video, VideoController, colors,
};
pub use computer::Computer;

pub mod computer;
pub mod cpu;
pub mod decoder;
pub mod disk;
#[cfg(not(target_arch = "wasm32"))]
pub mod disk_backend;
pub mod drive_manager;
pub mod io;
pub mod keyboard;
pub mod memory;
pub mod memory_allocator;
pub mod mouse;
pub mod peripheral;
#[cfg(feature = "audio-rodio")]
pub mod rodio_speaker;
pub mod serial_mouse;
pub mod serial_port;
pub mod speaker;
pub mod time;
pub mod utils;
pub mod video;

/// Drive numbering:
/// - 0x00 = Floppy A:
/// - 0x01 = Floppy B:
/// - 0x80 = Hard drive C:
/// - 0x81 = Hard drive D:
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

    pub fn is_floppy(&self) -> bool {
        self.0 < 0x80
    }

    pub fn is_hard_drive(&self) -> bool {
        self.0 >= 0x80
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
