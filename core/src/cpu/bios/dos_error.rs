use strum_macros::{Display, FromRepr};

/// INT 21h DOS error codes
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, FromRepr)]
pub enum DosError {
    Success = 0x00,
    InvalidFunction = 0x01,
    FileNotFound = 0x02,
    PathNotFound = 0x03,
    TooManyOpenFiles = 0x04,
    AccessDenied = 0x05,
    InvalidHandle = 0x06,
    MemoryControlBlocksDestroyed = 0x07,
    InsufficientMemory = 0x08,
    InvalidMemoryBlockAddress = 0x09,
    InvalidEnvironment = 0x0A,
    InvalidFormat = 0x0B,
    InvalidAccessCode = 0x0C,
    InvalidData = 0x0D,
    InvalidDrive = 0x0F,
    AttemptToRemoveCurrentDir = 0x10,
    NotSameDevice = 0x11,
    NoMoreFiles = 0x12,
}
