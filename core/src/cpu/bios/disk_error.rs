use strum_macros::{Display, FromRepr};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, FromRepr)]
pub enum DiskError {
    Success = 0x00,
    InvalidCommand = 0x01,
    AddressMarkNotFound = 0x02,
    WriteProtected = 0x03,
    SectorNotFound = 0x04,
    ResetFailed = 0x05,
    DiskChanged = 0x06,
    DriveParameterActivityFailed = 0x07,
    DmaOverrun = 0x08,
    DmaBoundaryError = 0x09,
    BadSector = 0x0A,
    BadTrack = 0x0B,
    UnsupportedTrack = 0x0C,
    InvalidNumberOfSectors = 0x0D,
    ControlDataAddressMarkDetected = 0x0E,
    DmaArbitrationLevelOutOfRange = 0x0F,
    UncorrectableCrcError = 0x10,
    EccCorrectedDataError = 0x11,
    ControllerFailure = 0x20,
    SeekFailed = 0x40,
    Timeout = 0x80,
    DriveNotReady = 0xAA,
    UndefinedError = 0xBB,
    WriteFault = 0xCC,
    StatusRegisterError = 0xE0,
    SenseOperationFailed = 0xFF,
}
