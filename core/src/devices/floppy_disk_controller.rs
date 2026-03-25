use std::any::Any;

use crate::{
    Device,
    disk::{Disk, DiskError, DriveNumber},
};

/// FDC I/O port addresses (primary controller, base 0x3F0)
pub const FDC_DOR: u16 = 0x3F2; // Digital Output Register (drive select + motor control)
pub const FDC_MSR: u16 = 0x3F4; // Main Status Register
pub const FDC_DATA: u16 = 0x3F5; // Data Register (command / data / result)
pub const FDC_DIR: u16 = 0x3F7; // Digital Input Register (disk change line)

/// Bit 7 of DIR: disk change line (1 = disk has been changed, 0 = not changed)
pub const FDC_DIR_DISK_CHANGE: u8 = 0x80;

/// MSR bit 7: data register ready for transfer (Request for Master)
pub const FDC_MSR_RQM: u8 = 0x80;
/// MSR bit 6: data direction (1 = FDC→CPU, 0 = CPU→FDC)
pub const FDC_MSR_DIO: u8 = 0x40;
/// MSR bit 5: non-DMA mode — set during PIO data transfer
pub const FDC_MSR_NDM: u8 = 0x20;
/// MSR bit 4: controller busy
pub const FDC_MSR_CB: u8 = 0x10;

/// READ DATA command code (bits 4:0)
const FDC_CMD_READ_DATA: u8 = 0x06;
/// Number of parameter bytes following the READ DATA command byte
const FDC_CMD_READ_DATA_PARAMS: usize = 8;

/// VERIFY command code (bits 4:0): same parameters as READ DATA, no data transfer to host
const FDC_CMD_VERIFY: u8 = 0x16;
/// Number of parameter bytes following the VERIFY command byte
const FDC_CMD_VERIFY_PARAMS: usize = 8;

/// WRITE DATA command code (bits 4:0)
const FDC_CMD_WRITE_DATA: u8 = 0x05;
/// Number of parameter bytes following the WRITE DATA command byte
const FDC_CMD_WRITE_DATA_PARAMS: usize = 8;

/// RECALIBRATE command code: move head to track 0
const FDC_CMD_RECALIBRATE: u8 = 0x07;
/// RECALIBRATE takes 1 parameter byte (drive select DS1:DS0)
const FDC_CMD_RECALIBRATE_PARAMS: usize = 1;

/// SENSE INTERRUPT STATUS command code: acknowledge interrupt and return ST0/PCN
const FDC_CMD_SENSE_INTERRUPT: u8 = 0x08;

/// NEC 765 / Intel 8272A command state machine phases.
enum FdcPhase {
    Idle,
    /// Receiving command parameter bytes (command byte already stored in `cmd`)
    Command {
        cmd: u8,
        params: [u8; 8],
        received: usize,
        total: usize,
    },
    /// PIO data transfer: serving sector data bytes to the CPU, then result bytes.
    /// Used when the READ DATA command byte has modifier flags set (e.g. MFM=1).
    Execution {
        data: Vec<u8>,
        index: usize,
        result: [u8; 7],
    },
    /// DMA data transfer: sector data is served byte-by-byte via `dma_read_u8`.
    /// Used when the READ DATA command byte has no modifier flags (bare 0x06).
    /// MSR shows only CB (no RQM/DIO/NDM) while this phase is active.
    DmaExecution {
        data: Vec<u8>,
        index: usize,
        result: [u8; 7],
    },
    /// PIO write transfer: receiving sector data bytes from the CPU
    WriteExecution {
        params: [u8; 8],
        data: Vec<u8>,
        expected: usize,
    },
    /// Result phase: returning status bytes to the CPU.
    /// `len` is the number of valid bytes (7 for READ DATA, 2 for SENSE INTERRUPT STATUS).
    Result {
        bytes: [u8; 7],
        index: usize,
        len: usize,
    },
}

/// Floppy Disk Controller (Intel 8272A / NEC μPD765 compatible).
///
/// A single FDC manages both floppy drives (A: and B:). Drive selection is
/// done via the Digital Output Register (DOR, port 0x3F2). The Digital Input
/// Register (DIR, port 0x3F7) reflects the changeline of the currently
/// selected drive. Both drive slots always exist; a drive with no disk
/// inserted returns `DriveNotReady` on access.
pub(crate) struct FloppyDiskController {
    /// Disk image per drive: index 0 = A:, index 1 = B:. None = no disk inserted.
    drives: [Option<Box<dyn Disk>>; 2],
    /// Currently selected drive index (0 = A:, 1 = B:), set by DOR writes
    selected_drive: u8,
    /// Per-drive disk change line (mirrors DIR bit 7). Set to true when a disk is inserted or
    /// swapped; automatically cleared when the OS reads DIR via port 0x3F7.
    changeline: [bool; 2],
    /// NEC 765 command state machine
    phase: FdcPhase,
    /// True while the nRESET bit (DOR bit 2) is asserted low (controller held in reset)
    in_reset: bool,
    /// Pending interrupt result from RECALIBRATE or reset recovery: (ST0, PCN).
    /// Consumed by SENSE INTERRUPT STATUS.
    pending_interrupt: Option<(u8, u8)>,
    /// Pending DMA channel 2 request state change for the Bus to drain.
    /// `Some(true)` = assert DREQ 2; `Some(false)` = deassert; `None` = no change.
    pending_dreq: Option<bool>,
}

impl FloppyDiskController {
    pub(crate) fn new() -> Self {
        Self {
            drives: [None, None],
            selected_drive: 0,
            changeline: [false, false],
            phase: FdcPhase::Idle,
            in_reset: false,
            pending_interrupt: None,
            pending_dreq: None,
        }
    }

    /// Take and return any pending DMA channel 2 request state change.
    /// Called by the Bus after each I/O write and after each DMA transfer so it
    /// can update the DMA controller's DREQ line for channel 2.
    pub(crate) fn take_dreq_request(&mut self) -> Option<bool> {
        self.pending_dreq.take()
    }

    /// Insert or eject the disk for the given drive (A: or B:). Returns the previous disk if any.
    /// Pass `None` to eject the current disk.
    pub(crate) fn set_drive_disk(
        &mut self,
        drive: DriveNumber,
        disk: Option<Box<dyn Disk>>,
    ) -> Option<Box<dyn Disk>> {
        assert!(
            drive.is_floppy(),
            "FloppyDiskController only supports floppy drives"
        );
        let idx = drive.as_floppy_index();
        assert!(idx < 2, "floppy drive index out of range");
        let prev = self.drives[idx].take();
        self.drives[idx] = disk;
        self.changeline[idx] = true;
        prev
    }

    pub(crate) fn read_sectors(
        &self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        match self
            .drives
            .get(drive.as_floppy_index())
            .and_then(|d| d.as_ref())
        {
            Some(disk) => disk.read_sectors(cylinder, head, sector, count),
            None => Err(DiskError::DriveNotReady),
        }
    }

    /// Build the MSR value reflecting the current command phase.
    fn msr(&self) -> u8 {
        // While nRESET is asserted the controller is not ready
        if self.in_reset {
            return 0x00;
        }
        match &self.phase {
            FdcPhase::Idle => FDC_MSR_RQM,
            FdcPhase::Command { .. } => FDC_MSR_RQM | FDC_MSR_CB,
            FdcPhase::Execution { .. } => FDC_MSR_RQM | FDC_MSR_DIO | FDC_MSR_NDM | FDC_MSR_CB,
            // DMA execution: controller is busy but CPU has nothing to do (no RQM, DIO, NDM).
            // The DMA controller transfers data autonomously; MSR shows CB only.
            FdcPhase::DmaExecution { .. } => FDC_MSR_CB,
            FdcPhase::WriteExecution { .. } => FDC_MSR_RQM | FDC_MSR_NDM | FDC_MSR_CB,
            FdcPhase::Result { .. } => FDC_MSR_RQM | FDC_MSR_DIO | FDC_MSR_CB,
        }
    }

    /// Execute a READ DATA command given the full command byte and 8 parameter bytes.
    ///
    /// Transfer mode is determined by the command byte's modifier flags (bits 7:5):
    /// - `cmd & 0xE0 == 0` (bare 0x06, no MT/MF/SK): **DMA mode** → `DmaExecution`.
    ///   The DMA controller transfers data autonomously via `dma_read_u8`.
    /// - `cmd & 0xE0 != 0` (e.g. 0x46 with MFM=1, as sent by the BIOS): **PIO mode**
    ///   → `Execution`.  The CPU reads bytes one at a time from port 0x3F5.
    ///
    /// Returns the next FdcPhase (DmaExecution/Execution on success, Result on error).
    fn execute_read_data(&self, cmd: u8, params: &[u8; 8]) -> FdcPhase {
        let drive_head = params[0]; // HD<<2 | US1:US0
        let cylinder = params[1];
        let head = params[2];
        let sector = params[3];
        // params[4] = N (bytes-per-sector code; we always use 512)
        let eot = params[5]; // last sector number (end-of-track)
        // params[6] = GPL, params[7] = DTL — ignored in emulation

        let drive_index = drive_head & 0x03;
        let count = eot.saturating_sub(sector) + 1;

        let drive = DriveNumber::from_standard(drive_index);

        // cmd & 0xE0 == 0: bare command (no MT/MF/SK) → DMA mode
        // cmd & 0xE0 != 0: has modifier flags (e.g. BIOS sends 0x46 with MFM=1) → PIO mode
        let use_dma = cmd & 0xE0 == 0;

        match self
            .drives
            .get(drive.as_floppy_index())
            .and_then(|d| d.as_ref())
        {
            Some(disk) => match disk.read_sectors(cylinder, head, sector, count) {
                Ok(data) => {
                    let result = [
                        0x00,                // ST0: normal termination
                        0x00,                // ST1
                        0x00,                // ST2
                        cylinder,            // C
                        head,                // H
                        eot.wrapping_add(1), // R (next sector after last)
                        0x02,                // N (512 bytes/sector)
                    ];
                    if use_dma {
                        FdcPhase::DmaExecution {
                            data,
                            index: 0,
                            result,
                        }
                    } else {
                        FdcPhase::Execution {
                            data,
                            index: 0,
                            result,
                        }
                    }
                }
                Err(_) => FdcPhase::Result {
                    bytes: [
                        0x40 | (drive_head & 0x07), // ST0: abnormal termination
                        0x04,                       // ST1: No Data
                        0x00,                       // ST2
                        cylinder,
                        head,
                        sector,
                        0x02,
                    ],
                    index: 0,
                    len: 7,
                },
            },
            None => FdcPhase::Result {
                bytes: [
                    0x48 | (drive_head & 0x07), // ST0: abnormal + not ready
                    0x00,                       // ST1
                    0x00,                       // ST2
                    cylinder,
                    head,
                    sector,
                    0x02,
                ],
                index: 0,
                len: 7,
            },
        }
    }
    /// Execute a VERIFY command given the 8 parameter bytes.
    /// Reads sectors for CRC/ECC checking but does not expose data to the CPU.
    /// Returns the Result phase with 7 status bytes (no Execution/data phase).
    fn execute_verify(&self, params: &[u8; 8]) -> FdcPhase {
        let drive_head = params[0]; // HD<<2 | US1:US0
        let cylinder = params[1];
        let head = params[2];
        let sector = params[3];
        let eot = params[5];

        let drive_index = drive_head & 0x03;
        let count = eot.saturating_sub(sector) + 1;
        let drive = DriveNumber::from_standard(drive_index);

        match self
            .drives
            .get(drive.as_floppy_index())
            .and_then(|d| d.as_ref())
        {
            Some(disk) => match disk.read_sectors(cylinder, head, sector, count) {
                Ok(_) => FdcPhase::Result {
                    bytes: [
                        0x00,                // ST0: normal termination
                        0x00,                // ST1
                        0x00,                // ST2
                        cylinder,            // C
                        head,                // H
                        eot.wrapping_add(1), // R (next sector after last)
                        0x02,                // N (512 bytes/sector)
                    ],
                    index: 0,
                    len: 7,
                },
                Err(_) => FdcPhase::Result {
                    bytes: [
                        0x40 | (drive_head & 0x07), // ST0: abnormal termination
                        0x04,                       // ST1: No Data
                        0x00,                       // ST2
                        cylinder,
                        head,
                        sector,
                        0x02,
                    ],
                    index: 0,
                    len: 7,
                },
            },
            None => FdcPhase::Result {
                bytes: [
                    0x48 | (drive_head & 0x07), // ST0: abnormal + not ready
                    0x00,
                    0x00,
                    cylinder,
                    head,
                    sector,
                    0x02,
                ],
                index: 0,
                len: 7,
            },
        }
    }

    /// Execute a WRITE DATA command given the 8 parameter bytes and the sector data.
    /// Returns the Result phase with 7 status bytes.
    fn execute_write_data(&self, params: &[u8; 8], data: &[u8]) -> FdcPhase {
        let drive_head = params[0]; // HD<<2 | US1:US0
        let cylinder = params[1];
        let head = params[2];
        let sector = params[3];
        let eot = params[5]; // last sector number (end-of-track)

        let drive_index = drive_head & 0x03;
        let drive = DriveNumber::from_standard(drive_index);

        match self
            .drives
            .get(drive.as_floppy_index())
            .and_then(|d| d.as_ref())
        {
            Some(disk) => match disk.write_sectors(cylinder, head, sector, data) {
                Ok(()) => FdcPhase::Result {
                    bytes: [
                        0x00,                // ST0: normal termination
                        0x00,                // ST1
                        0x00,                // ST2
                        cylinder,            // C
                        head,                // H
                        eot.wrapping_add(1), // R (next sector after last)
                        0x02,                // N (512 bytes/sector)
                    ],
                    index: 0,
                    len: 7,
                },
                Err(DiskError::WriteProtected) => FdcPhase::Result {
                    bytes: [
                        0x40 | (drive_head & 0x07), // ST0: abnormal termination
                        0x02,                       // ST1: NW (Not Writable) bit
                        0x00,                       // ST2
                        cylinder,
                        head,
                        sector,
                        0x02,
                    ],
                    index: 0,
                    len: 7,
                },
                Err(_) => FdcPhase::Result {
                    bytes: [
                        0x40 | (drive_head & 0x07), // ST0: abnormal termination
                        0x04,                       // ST1: No Data
                        0x00,                       // ST2
                        cylinder,
                        head,
                        sector,
                        0x02,
                    ],
                    index: 0,
                    len: 7,
                },
            },
            None => FdcPhase::Result {
                bytes: [
                    0x48 | (drive_head & 0x07), // ST0: abnormal + not ready
                    0x00,                       // ST1
                    0x00,                       // ST2
                    cylinder,
                    head,
                    sector,
                    0x02,
                ],
                index: 0,
                len: 7,
            },
        }
    }
}

impl Device for FloppyDiskController {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.phase = FdcPhase::Idle;
        self.in_reset = false;
        self.pending_interrupt = None;
        self.pending_dreq = None;
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, _cycle_count: u32) -> Option<u8> {
        match port {
            FDC_MSR => Some(self.msr()),
            FDC_DATA => {
                let current = std::mem::replace(&mut self.phase, FdcPhase::Idle);
                let (byte, next) = match current {
                    FdcPhase::Execution {
                        data,
                        index,
                        result,
                    } => {
                        if index < data.len() {
                            let b = data[index];
                            let next_idx = index + 1;
                            if next_idx >= data.len() {
                                (
                                    b,
                                    FdcPhase::Result {
                                        bytes: result,
                                        index: 0,
                                        len: 7,
                                    },
                                )
                            } else {
                                (
                                    b,
                                    FdcPhase::Execution {
                                        data,
                                        index: next_idx,
                                        result,
                                    },
                                )
                            }
                        } else {
                            // Empty data buffer — go straight to result
                            (
                                0xFF,
                                FdcPhase::Result {
                                    bytes: result,
                                    index: 0,
                                    len: 7,
                                },
                            )
                        }
                    }
                    FdcPhase::Result { bytes, index, len } => {
                        let b = bytes[index];
                        let next_idx = index + 1;
                        if next_idx >= len {
                            (b, FdcPhase::Idle)
                        } else {
                            (
                                b,
                                FdcPhase::Result {
                                    bytes,
                                    index: next_idx,
                                    len,
                                },
                            )
                        }
                    }
                    other => (0xFF, other),
                };
                self.phase = next;
                Some(byte)
            }
            FDC_DIR => {
                let idx = self.selected_drive as usize;
                let changed = *self.changeline.get(idx)?;
                self.changeline[idx] = false;
                Some(if changed { FDC_DIR_DISK_CHANGE } else { 0x00 })
            }
            _ => None,
        }
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        match port {
            FDC_DOR => {
                // Bit 2: nRESET (active-low). 0 = controller held in reset, 1 = normal operation.
                let reset_released = val & 0x04 != 0;
                if !reset_released {
                    // Asserting reset: freeze controller, discard any in-progress state
                    self.in_reset = true;
                    self.phase = FdcPhase::Idle;
                    self.pending_interrupt = None;
                } else if self.in_reset {
                    // De-asserting reset: controller comes out of reset ready for commands
                    self.in_reset = false;
                }
                // Bits 0-1 select the drive (0 = A:, 1 = B:)
                self.selected_drive = val & 0x01;
                true
            }
            FDC_DATA => {
                let current = std::mem::replace(&mut self.phase, FdcPhase::Idle);
                let next = match current {
                    FdcPhase::Idle => {
                        let cmd_code = val & 0x1F;
                        match cmd_code {
                            FDC_CMD_READ_DATA => FdcPhase::Command {
                                cmd: val,
                                params: [0u8; 8],
                                received: 0,
                                total: FDC_CMD_READ_DATA_PARAMS,
                            },
                            FDC_CMD_VERIFY => FdcPhase::Command {
                                cmd: val,
                                params: [0u8; 8],
                                received: 0,
                                total: FDC_CMD_VERIFY_PARAMS,
                            },
                            FDC_CMD_WRITE_DATA => FdcPhase::Command {
                                cmd: val,
                                params: [0u8; 8],
                                received: 0,
                                total: FDC_CMD_WRITE_DATA_PARAMS,
                            },
                            FDC_CMD_RECALIBRATE => FdcPhase::Command {
                                cmd: val,
                                params: [0u8; 8],
                                received: 0,
                                total: FDC_CMD_RECALIBRATE_PARAMS,
                            },
                            FDC_CMD_SENSE_INTERRUPT => {
                                // No parameter bytes; immediately return ST0 + PCN (2 bytes only)
                                let (st0, pcn) = self.pending_interrupt.take().unwrap_or((0x80, 0));
                                FdcPhase::Result {
                                    bytes: [st0, pcn, 0, 0, 0, 0, 0],
                                    index: 0,
                                    len: 2,
                                }
                            }
                            _ => {
                                log::warn!("FDC: unknown command 0x{:02X}", val);
                                FdcPhase::Idle
                            }
                        }
                    }
                    FdcPhase::Command {
                        cmd,
                        mut params,
                        received,
                        total,
                    } => {
                        params[received] = val;
                        let received = received + 1;
                        if received < total {
                            FdcPhase::Command {
                                cmd,
                                params,
                                received,
                                total,
                            }
                        } else {
                            // All parameter bytes received; execute the command
                            let cmd_code = cmd & 0x1F;
                            match cmd_code {
                                FDC_CMD_READ_DATA => {
                                    let phase = self.execute_read_data(cmd, &params);
                                    if matches!(phase, FdcPhase::DmaExecution { .. }) {
                                        self.pending_dreq = Some(true);
                                    }
                                    phase
                                }
                                FDC_CMD_VERIFY => self.execute_verify(&params),
                                FDC_CMD_WRITE_DATA => {
                                    // Transition to write execution: wait for sector data from CPU
                                    let count = params[5].saturating_sub(params[3]) + 1;
                                    let expected = (count as usize) * 512;
                                    FdcPhase::WriteExecution {
                                        params,
                                        data: Vec::with_capacity(expected),
                                        expected,
                                    }
                                }
                                FDC_CMD_RECALIBRATE => {
                                    // Move head to cylinder 0; generate interrupt with SE set
                                    // ST0: IC=00b (normal), SE=1, drive number in bits 1:0
                                    let drive = params[0] & 0x03;
                                    self.pending_interrupt = Some((0x20 | drive, 0));
                                    FdcPhase::Idle
                                }
                                _ => FdcPhase::Idle,
                            }
                        }
                    }
                    FdcPhase::WriteExecution {
                        params,
                        mut data,
                        expected,
                    } => {
                        data.push(val);
                        if data.len() >= expected {
                            self.execute_write_data(&params, &data)
                        } else {
                            FdcPhase::WriteExecution {
                                params,
                                data,
                                expected,
                            }
                        }
                    }
                    // Ignore writes during read execution or result phases
                    other => other,
                };
                self.phase = next;
                true
            }
            _ => false,
        }
    }

    /// Called by the DMA controller for each byte of a channel 2 DMA transfer
    /// (device → memory direction).  Serves the next byte from the `DmaExecution`
    /// sector buffer, advancing the internal index.  When the last byte is served
    /// the FDC transitions to the Result phase and signals DREQ deassert.
    fn dma_read_u8(&mut self) -> Option<u8> {
        let current = std::mem::replace(&mut self.phase, FdcPhase::Idle);
        match current {
            FdcPhase::DmaExecution {
                data,
                index,
                result,
            } => {
                if index < data.len() {
                    let byte = data[index];
                    let next = index + 1;
                    if next >= data.len() {
                        // Last byte: move to Result phase and release DREQ.
                        self.phase = FdcPhase::Result {
                            bytes: result,
                            index: 0,
                            len: 7,
                        };
                        self.pending_dreq = Some(false);
                    } else {
                        self.phase = FdcPhase::DmaExecution {
                            data,
                            index: next,
                            result,
                        };
                    }
                    Some(byte)
                } else {
                    // Buffer exhausted without TC — shouldn't happen with correct count setup.
                    self.phase = FdcPhase::Result {
                        bytes: result,
                        index: 0,
                        len: 7,
                    };
                    None
                }
            }
            other => {
                self.phase = other;
                None
            }
        }
    }
}
