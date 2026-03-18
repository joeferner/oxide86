use crate::{
    bus::Bus,
    cpu::{Cpu, CpuType, bios::bda::bda_get_num_hard_drives, cpu_flag},
    devices::{
        floppy_disk_controller::{
            FDC_DATA, FDC_DIR, FDC_DIR_DISK_CHANGE, FDC_DOR, FDC_MSR, FDC_MSR_CB, FDC_MSR_NDM,
            FDC_MSR_RQM,
        },
        hard_disk_controller::{
            HDC_COMMAND, HDC_CYLINDER_HIGH, HDC_CYLINDER_LOW, HDC_DATA, HDC_DEVICE_CONTROL,
            HDC_DRIVE_HEAD, HDC_SECTOR_COUNT, HDC_SECTOR_NUM, HDC_STATUS_BSY, HDC_STATUS_DRQ,
            HDC_STATUS_ERR,
        },
        rtc::{CMOS_REG_FLOPPY_TYPES, RTC_IO_PORT_DATA, RTC_IO_PORT_REGISTER_SELECT},
    },
    disk::{DiskError, DriveNumber},
};

/// ATA commands used by the INT 13h handlers
const HDC_CMD_READ_SECTORS: u8 = 0x20;
const HDC_CMD_VERIFY_SECTORS: u8 = 0x40;
const HDC_CMD_WRITE_SECTORS: u8 = 0x30;
const HDC_CMD_IDENTIFY: u8 = 0xEC;

impl Cpu {
    /// INT 0x13 - BIOS Disk Services
    /// AH register contains the function number
    ///
    /// Note: AT-class BIOS enables interrupts (STI) during disk operations so that
    /// timer IRQs (INT 0x08) can still fire. This is important for programs that
    /// depend on the BDA timer counter advancing during disk benchmarks.
    pub(in crate::cpu) fn handle_int13_disk_services(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(1000);
        // Enable interrupts during disk operations (AT-class BIOS behavior)
        // This allows timer IRQs to fire even during extended disk operations
        self.set_flag(cpu_flag::INTERRUPT, true);

        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int13_reset_disk(bus),
            0x01 => self.int13_get_status(),
            0x02 => self.int13_read_sectors(bus),
            0x03 => self.int13_write_sectors(bus),
            0x04 => self.int13_verify_sectors(bus),
            0x08 => self.int13_get_drive_params(bus),
            0x15 => self.int13_get_disk_type(bus),
            0x16 => self.int13_detect_disk_change(bus),
            0x18 => self.int13_set_dasd_type(bus),
            0x41 => self.int13_check_extensions_present(),
            _ => {
                log::warn!("Unhandled INT 0x13 function: AH=0x{:02X}", function);
                // Set error: invalid command
                self.ax = (self.ax & 0x00FF) | ((DiskError::InvalidCommand as u16) << 8);
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = DiskError::InvalidCommand as u8;
            }
        }
    }

    /// INT 13h, AH=00h - Reset Disk System
    /// Input:
    ///   DL = drive number (0x00-0x7F for floppies, 0x80-0xFF for hard disks)
    /// Output:
    ///   AH = status (0 = success)
    ///   CF = clear if success, set if error
    ///
    /// Floppy procedure (NEC 765 / Intel 8272A, matches original AT BIOS):
    ///   1. Assert FDC reset via DOR (bit 2 = nRESET = 0)
    ///   2. De-assert reset, re-enable DMA (DOR = 0x0C | drive)
    ///   3. Poll MSR until RQM (bit 7) — controller ready for command
    ///   4. Send RECALIBRATE (0x07) command + drive parameter
    ///   5. Poll MSR until CB (bit 4) clears — command complete
    ///   6. Send SENSE INTERRUPT STATUS (0x08) to acknowledge interrupt
    ///   7. Read ST0 + PCN; success if ST0 bits 7:6 = 0b00
    ///
    /// Hard disk procedure (ATA soft reset):
    ///   1. Write 0x04 to device control register (0x3F6) — assert SRST
    ///   2. Write 0x00 to device control register — release SRST
    ///   3. Poll status until BSY clears
    fn int13_reset_disk(&mut self, bus: &mut Bus) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        let success = if drive.is_floppy() {
            let drive_index = drive.as_floppy_index() as u8;

            // 1. Assert FDC reset: pull nRESET low (DOR bit 2 = 0), motors off
            bus.io_write_u8(FDC_DOR, 0x00);

            // 2. De-assert reset: nRESET=1, DMA enable=1 (bits 3:2 = 0b11), select drive
            bus.io_write_u8(FDC_DOR, 0x0C | drive_index);

            // 3. Poll MSR until RQM — FDC ready to accept a command
            while bus.io_read_u8(FDC_MSR) & FDC_MSR_RQM == 0 {}

            // 4a. Send RECALIBRATE command byte
            bus.io_write_u8(FDC_DATA, 0x07);

            // 4b. Poll MSR until RQM — FDC ready for the drive parameter
            while bus.io_read_u8(FDC_MSR) & FDC_MSR_RQM == 0 {}

            // 4c. Send drive number (DS1:DS0)
            bus.io_write_u8(FDC_DATA, drive_index);

            // 5. Poll MSR until CB clears — head-seek to track 0 complete
            while bus.io_read_u8(FDC_MSR) & FDC_MSR_CB != 0 {}

            // 6. Send SENSE INTERRUPT STATUS to acknowledge the recalibrate interrupt
            bus.io_write_u8(FDC_DATA, 0x08);

            // 7. Read result: ST0 then PCN (present cylinder number, should be 0)
            let st0 = bus.io_read_u8(FDC_DATA);
            let _pcn = bus.io_read_u8(FDC_DATA);

            // ST0 bits 7:6 = 0b00 (normal termination) means success
            st0 & 0xC0 == 0x00
        } else {
            // ATA soft reset via device control register (0x3F6)
            // 1. Assert SRST (bit 2)
            bus.io_write_u8(HDC_DEVICE_CONTROL, 0x04);
            // 2. Release SRST
            bus.io_write_u8(HDC_DEVICE_CONTROL, 0x00);
            // 3. Poll until BSY clears
            while bus.io_read_u8(HDC_COMMAND) & HDC_STATUS_BSY != 0 {}
            true
        };

        if success {
            self.ax &= 0x00FF; // AH = 0 (success)
            self.set_flag(cpu_flag::CARRY, false);
            self.last_disk_status = DiskError::Success as u8;
        } else {
            self.ax = (self.ax & 0x00FF) | ((DiskError::ResetFailed as u16) << 8);
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = DiskError::ResetFailed as u8;
        }
    }

    /// INT 13h, AH=01h - Get Status of Last Disk Operation
    /// Input:
    ///   AH = 0x01
    ///   DL = drive number (optional, some implementations ignore it)
    /// Output:
    ///   AH = status code of last disk operation (0x00 = success, or error code)
    ///   CF = cleared (always - the status retrieval itself succeeds)
    fn int13_get_status(&mut self) {
        // Return the last disk status
        let status = self.last_disk_status;

        // Set AH to the last status
        self.ax = (self.ax & 0x00FF) | ((status as u16) << 8);

        // Always clear carry flag - the status retrieval itself succeeds
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 13h, AH=02h - Read Sectors into Memory
    /// Input:
    ///   AL = number of sectors to read (1-128)
    ///   CH = cylinder number (0-1023, low 8 bits)
    ///   CL = sector number (1-63, bits 0-5) + high 2 bits of cylinder (bits 6-7)
    ///   DH = head number (0-255)
    ///   DL = drive number
    ///   ES:BX = buffer address
    /// Output:
    ///   AH = status (0 = success)
    ///   AL = number of sectors read
    ///   CF = clear if success, set if error
    fn int13_read_sectors(&mut self, bus: &mut Bus) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder = (self.cx >> 8) as u8; // CH (8-bit cylinder for 8086 compatibility)
        let sector = (self.cx & 0x3F) as u8; // CL bits 0-5
        let head = (self.dx >> 8) as u8; // DH
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // DL
        let buffer_addr = bus.physical_address(self.es, self.bx);

        if drive.is_floppy() {
            let drive_index = drive.as_floppy_index() as u8;
            let eot = sector + count - 1;

            // Select drive and enable motor: nRESET=1, DMA enable=1, motor on, drive select
            bus.io_write_u8(FDC_DOR, 0x1C | drive_index);

            // Send READ DATA command (9 bytes: 1 command byte + 8 parameter bytes)
            bus.io_write_u8(FDC_DATA, 0x46); // READ DATA: MFM=1, MT=0, SK=0
            bus.io_write_u8(FDC_DATA, (head << 2) | drive_index); // HD, US1, US0
            bus.io_write_u8(FDC_DATA, cylinder); // C
            bus.io_write_u8(FDC_DATA, head); // H
            bus.io_write_u8(FDC_DATA, sector); // R (starting sector, 1-based)
            bus.io_write_u8(FDC_DATA, 0x02); // N (512 bytes/sector)
            bus.io_write_u8(FDC_DATA, eot); // EOT (last sector number)
            bus.io_write_u8(FDC_DATA, 0x1B); // GPL (gap length)
            bus.io_write_u8(FDC_DATA, 0xFF); // DTL

            // NDM bit set in MSR means PIO data transfer (execution) phase is active
            let msr = bus.io_read_u8(FDC_MSR);
            if msr & FDC_MSR_NDM != 0 {
                let total_bytes = (count as usize) * 512;
                for i in 0..total_bytes {
                    let byte = bus.io_read_u8(FDC_DATA);
                    bus.memory_write_u8(buffer_addr + i, byte);
                }
            }

            // Read 7 result bytes; ST0 is first
            let st0 = bus.io_read_u8(FDC_DATA);
            for _ in 1..7 {
                let _ = bus.io_read_u8(FDC_DATA);
            }

            // ST0 bits 7:6 = interrupt code: 0x00 = normal termination
            if st0 & 0xC0 == 0 {
                self.ax = (self.ax & 0xFF00) | (count as u16); // AL = sectors read
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
            } else {
                // ST0 bit 3 = NR (not ready)
                let error = if st0 & 0x08 != 0 {
                    DiskError::DriveNotReady
                } else {
                    DiskError::SectorNotFound
                };
                log::warn!(
                    "INT 0x13 AH=0x02: FDC read failed for drive {}, ST0=0x{:02X}",
                    drive,
                    st0
                );
                self.ax = (self.ax & 0x00FF) | ((error as u16) << 8); // AH = error
                self.ax &= 0xFF00; // AL = 0
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = error as u8;
            }
        } else {
            // Hard drive: issue ATA READ SECTORS command
            let drive_head = 0xA0 | ((drive.as_hard_drive_index() as u8) << 4) | (head & 0x0F);
            bus.io_write_u8(HDC_SECTOR_COUNT, count);
            bus.io_write_u8(HDC_SECTOR_NUM, sector);
            bus.io_write_u8(HDC_CYLINDER_LOW, cylinder);
            bus.io_write_u8(HDC_CYLINDER_HIGH, 0x00); // high 2 bits of cylinder
            bus.io_write_u8(HDC_DRIVE_HEAD, drive_head);
            bus.io_write_u8(HDC_COMMAND, HDC_CMD_READ_SECTORS);

            // Wait for BSY to clear and DRQ to set (or ERR)
            loop {
                let status = bus.io_read_u8(HDC_COMMAND);
                if status & HDC_STATUS_BSY == 0 {
                    break;
                }
            }

            let status = bus.io_read_u8(HDC_COMMAND);
            if status & HDC_STATUS_ERR != 0 || status & HDC_STATUS_DRQ == 0 {
                let error = DiskError::DriveNotReady;
                log::warn!(
                    "INT 0x13 AH=0x02: HDC read failed for drive {}, status=0x{:02X}",
                    drive,
                    status
                );
                self.ax = (self.ax & 0x00FF) | ((error as u16) << 8);
                self.ax &= 0xFF00; // AL = 0
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = error as u8;
                return;
            }

            let total_bytes = (count as usize) * 512;
            for i in 0..total_bytes {
                let byte = bus.io_read_u8(HDC_DATA);
                bus.memory_write_u8(buffer_addr + i, byte);
            }

            self.ax = (self.ax & 0xFF00) | (count as u16); // AL = sectors read
            self.ax &= 0x00FF; // AH = 0 (success)
            self.set_flag(cpu_flag::CARRY, false);
            self.last_disk_status = DiskError::Success as u8;
        }
    }

    /// INT 13h, AH=03h - Write Sectors from Memory
    /// Input:
    ///   AL = number of sectors to write (1-128)
    ///   CH = cylinder number (0-1023, low 8 bits)
    ///   CL = sector number (1-63, bits 0-5) + high 2 bits of cylinder (bits 6-7)
    ///   DH = head number (0-255)
    ///   DL = drive number
    ///   ES:BX = buffer address (source data)
    /// Output:
    ///   AH = status (0 = success)
    ///   AL = number of sectors written
    ///   CF = clear if success, set if error
    fn int13_write_sectors(&mut self, bus: &mut Bus) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder = (self.cx >> 8) as u8; // CH (8-bit cylinder for 8086 compatibility)
        let sector = (self.cx & 0x3F) as u8; // CL bits 0-5
        let head = (self.dx >> 8) as u8; // DH
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // DL
        let buffer_addr = bus.physical_address(self.es, self.bx);

        if drive.is_floppy() {
            let drive_index = drive.as_floppy_index() as u8;
            let eot = sector + count - 1;

            // Select drive and enable motor: nRESET=1, DMA enable=1, motor on, drive select
            bus.io_write_u8(FDC_DOR, 0x1C | drive_index);

            // Send WRITE DATA command (9 bytes: 1 command byte + 8 parameter bytes)
            bus.io_write_u8(FDC_DATA, 0x45); // WRITE DATA: MFM=1, MT=0, SK=0
            bus.io_write_u8(FDC_DATA, (head << 2) | drive_index); // HD, US1, US0
            bus.io_write_u8(FDC_DATA, cylinder); // C
            bus.io_write_u8(FDC_DATA, head); // H
            bus.io_write_u8(FDC_DATA, sector); // R (starting sector, 1-based)
            bus.io_write_u8(FDC_DATA, 0x02); // N (512 bytes/sector)
            bus.io_write_u8(FDC_DATA, eot); // EOT (last sector number)
            bus.io_write_u8(FDC_DATA, 0x1B); // GPL (gap length)
            bus.io_write_u8(FDC_DATA, 0xFF); // DTL

            // NDM bit set in MSR means PIO data transfer (execution) phase is active
            let msr = bus.io_read_u8(FDC_MSR);
            if msr & FDC_MSR_NDM != 0 {
                let total_bytes = (count as usize) * 512;
                for i in 0..total_bytes {
                    let byte = bus.memory_read_u8(buffer_addr + i);
                    bus.io_write_u8(FDC_DATA, byte);
                }
            }

            // Read 7 result bytes: ST0, ST1, ST2, C, H, R, N
            let st0 = bus.io_read_u8(FDC_DATA);
            let st1 = bus.io_read_u8(FDC_DATA);
            for _ in 2..7 {
                let _ = bus.io_read_u8(FDC_DATA);
            }

            // ST0 bits 7:6 = interrupt code: 0x00 = normal termination
            if st0 & 0xC0 == 0 {
                self.ax = (self.ax & 0xFF00) | (count as u16); // AL = sectors written
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
            } else {
                let error = if st1 & 0x02 != 0 {
                    DiskError::WriteProtected
                } else if st0 & 0x08 != 0 {
                    DiskError::DriveNotReady
                } else {
                    DiskError::SectorNotFound
                };
                log::warn!(
                    "INT 0x13 AH=0x03: FDC write failed for drive {}, ST0=0x{:02X} ST1=0x{:02X}",
                    drive,
                    st0,
                    st1
                );
                self.ax = (self.ax & 0x00FF) | ((error as u16) << 8); // AH = error
                self.ax &= 0xFF00; // AL = 0
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = error as u8;
            }
        } else {
            // Hard drive: issue ATA WRITE SECTORS command
            let drive_head = 0xA0 | ((drive.as_hard_drive_index() as u8) << 4) | (head & 0x0F);
            bus.io_write_u8(HDC_SECTOR_COUNT, count);
            bus.io_write_u8(HDC_SECTOR_NUM, sector);
            bus.io_write_u8(HDC_CYLINDER_LOW, cylinder);
            bus.io_write_u8(HDC_CYLINDER_HIGH, 0x00);
            bus.io_write_u8(HDC_DRIVE_HEAD, drive_head);
            bus.io_write_u8(HDC_COMMAND, HDC_CMD_WRITE_SECTORS);

            // Wait for BSY to clear and DRQ to set (controller ready for data)
            loop {
                let status = bus.io_read_u8(HDC_COMMAND);
                if status & HDC_STATUS_BSY == 0 {
                    break;
                }
            }

            let status = bus.io_read_u8(HDC_COMMAND);
            if status & HDC_STATUS_ERR != 0 || status & HDC_STATUS_DRQ == 0 {
                let error = DiskError::DriveNotReady;
                log::warn!(
                    "INT 0x13 AH=0x03: HDC write setup failed for drive {}, status=0x{:02X}",
                    drive,
                    status
                );
                self.ax = (self.ax & 0x00FF) | ((error as u16) << 8);
                self.ax &= 0xFF00;
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = error as u8;
                return;
            }

            let total_bytes = (count as usize) * 512;
            for i in 0..total_bytes {
                let byte = bus.memory_read_u8(buffer_addr + i);
                bus.io_write_u8(HDC_DATA, byte);
            }

            // After last byte the HDC commits the write; check for error
            let final_status = bus.io_read_u8(HDC_COMMAND);
            if final_status & HDC_STATUS_ERR != 0 {
                let error = DiskError::WriteFault;
                log::warn!(
                    "INT 0x13 AH=0x03: HDC write failed for drive {}, status=0x{:02X}",
                    drive,
                    final_status
                );
                self.ax = (self.ax & 0x00FF) | ((error as u16) << 8);
                self.ax &= 0xFF00;
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = error as u8;
            } else {
                self.ax = (self.ax & 0xFF00) | (count as u16); // AL = sectors written
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
            }
        }
    }

    /// INT 13h, AH=04h - Verify Disk Sectors
    /// Input:
    ///   AL = number of sectors to verify (1-128)
    ///   CH = cylinder number (0-1023, low 8 bits)
    ///   CL = sector number (1-63, bits 0-5) + high 2 bits of cylinder (bits 6-7)
    ///   DH = head number (0-255)
    ///   DL = drive number
    ///   ES:BX = not used (no data transfer)
    /// Output:
    ///   AH = status (0 = success)
    ///   AL = number of sectors verified
    ///   CF = clear if success, set if error
    fn int13_verify_sectors(&mut self, bus: &mut Bus) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder = (self.cx >> 8) as u8; // CH (8-bit cylinder for 8086 compatibility)
        let sector = (self.cx & 0x3F) as u8; // CL bits 0-5
        let head = (self.dx >> 8) as u8; // DH
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // DL

        if drive.is_floppy() {
            let drive_index = drive.as_floppy_index() as u8;
            let eot = sector + count - 1;

            // Select drive and enable motor: nRESET=1, DMA enable=1, motor on, drive select
            bus.io_write_u8(FDC_DOR, 0x1C | drive_index);

            // Send VERIFY command (0x56): same parameters as READ DATA but no data transfer
            bus.io_write_u8(FDC_DATA, 0x56); // VERIFY: MFM=1, MT=0, SK=0
            bus.io_write_u8(FDC_DATA, (head << 2) | drive_index); // HD, US1, US0
            bus.io_write_u8(FDC_DATA, cylinder); // C
            bus.io_write_u8(FDC_DATA, head); // H
            bus.io_write_u8(FDC_DATA, sector); // R (starting sector, 1-based)
            bus.io_write_u8(FDC_DATA, 0x02); // N (512 bytes/sector)
            bus.io_write_u8(FDC_DATA, eot); // EOT (last sector number)
            bus.io_write_u8(FDC_DATA, 0x1B); // GPL (gap length)
            bus.io_write_u8(FDC_DATA, 0xFF); // DTL

            // No data transfer phase for VERIFY — FDC goes straight to Result phase.
            // Read 7 result bytes; ST0 is first
            let st0 = bus.io_read_u8(FDC_DATA);
            for _ in 1..7 {
                let _ = bus.io_read_u8(FDC_DATA);
            }

            if st0 & 0xC0 == 0 {
                self.ax = (self.ax & 0xFF00) | (count as u16); // AL = sectors verified
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
            } else {
                let error = if st0 & 0x08 != 0 {
                    DiskError::DriveNotReady
                } else {
                    DiskError::SectorNotFound
                };
                log::warn!(
                    "INT 0x13 AH=0x04: FDC verify failed for drive {}, ST0=0x{:02X}",
                    drive,
                    st0
                );
                self.ax = (self.ax & 0x00FF) | ((error as u16) << 8);
                self.ax &= 0xFF00; // AL = 0
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = error as u8;
            }
        } else {
            // Hard drive: issue ATA VERIFY SECTORS command (0x40), no data transfer
            let drive_head = 0xA0 | ((drive.as_hard_drive_index() as u8) << 4) | (head & 0x0F);
            bus.io_write_u8(HDC_SECTOR_COUNT, count);
            bus.io_write_u8(HDC_SECTOR_NUM, sector);
            bus.io_write_u8(HDC_CYLINDER_LOW, cylinder);
            bus.io_write_u8(HDC_CYLINDER_HIGH, 0x00);
            bus.io_write_u8(HDC_DRIVE_HEAD, drive_head);
            bus.io_write_u8(HDC_COMMAND, HDC_CMD_VERIFY_SECTORS);

            // Wait for BSY to clear
            loop {
                let status = bus.io_read_u8(HDC_COMMAND);
                if status & HDC_STATUS_BSY == 0 {
                    break;
                }
            }

            let status = bus.io_read_u8(HDC_COMMAND);
            if status & HDC_STATUS_ERR != 0 {
                let error = DiskError::SectorNotFound;
                log::warn!(
                    "INT 0x13 AH=0x04: HDC verify failed for drive {}, status=0x{:02X}",
                    drive,
                    status
                );
                self.ax = (self.ax & 0x00FF) | ((error as u16) << 8);
                self.ax &= 0xFF00; // AL = 0
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = error as u8;
            } else {
                self.ax = (self.ax & 0xFF00) | (count as u16); // AL = sectors verified
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
            }
        }
    }

    /// INT 13h, AH=08h - Get Drive Parameters
    /// Input:
    ///   DL = drive number
    /// Output:
    ///   AH = status (0 = success)
    ///   CF = clear if success, set if error
    ///   On success:
    ///     CH = maximum cylinder number (low 8 bits)
    ///     CL = maximum sector number (bits 0-5) + high 2 bits of max cylinder (bits 6-7)
    ///     DH = maximum head number
    ///     DL = number of drives
    ///
    /// Floppy geometry is obtained via CMOS register 0x10 (standard PC AT approach):
    ///   1. Write 0x10 to port 0x70 (CMOS register select)
    ///   2. Read port 0x71 (CMOS data) → bits 7:4 = drive A type, bits 3:0 = drive B type
    ///   3. Map type code to geometry using the standard PC drive parameter table
    ///
    /// Hard disk geometry is obtained via ATA IDENTIFY DEVICE command (0xEC):
    ///   Word 1 = cylinders, word 3 = heads, word 6 = sectors per track
    fn int13_get_drive_params(&mut self, bus: &mut Bus) {
        // INT 13h AH=08h was introduced with the PC AT (80286). 8086/8088 BIOSes did not
        // implement this function; return InvalidCommand to signal the call is unsupported.
        if self.cpu_type == CpuType::I8086 {
            log::warn!("INT 0x13 AH=0x08: not supported on 8086");
            self.ax = (self.ax & 0x00FF) | ((DiskError::InvalidCommand as u16) << 8);
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = DiskError::InvalidCommand as u8;
            return;
        }

        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        if drive.is_floppy() {
            let drive_index = drive.as_floppy_index() as u8;

            // Read floppy drive types from CMOS register 0x10 (PC AT standard).
            // Bits 7:4 = drive A type, bits 3:0 = drive B type.
            // Codes: 0=none, 1=360KB 5.25", 2=1.2MB 5.25", 3=720KB 3.5", 4=1.44MB 3.5", 5=2.88MB 3.5"
            bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, CMOS_REG_FLOPPY_TYPES);
            let floppy_types = bus.io_read_u8(RTC_IO_PORT_DATA);
            let drive_type = if drive_index == 0 {
                (floppy_types >> 4) & 0x0F // drive A: bits 7:4
            } else {
                floppy_types & 0x0F // drive B: bits 3:0
            };

            // Standard PC AT drive parameter table: (max_cylinder, max_head, max_sector)
            let params: Option<(u8, u8, u8)> = match drive_type {
                0x01 => Some((39, 1, 9)),  // 360 KB 5.25": 40 cyl, 2 heads, 9 spt
                0x02 => Some((79, 1, 15)), // 1.2 MB 5.25": 80 cyl, 2 heads, 15 spt
                0x03 => Some((79, 1, 9)),  // 720 KB 3.5":  80 cyl, 2 heads, 9 spt
                0x04 => Some((79, 1, 18)), // 1.44 MB 3.5": 80 cyl, 2 heads, 18 spt
                0x05 => Some((79, 1, 36)), // 2.88 MB 3.5": 80 cyl, 2 heads, 36 spt
                _ => None,
            };

            match params {
                Some((max_cylinder, max_head, max_sector)) => {
                    // CH = max cylinder (bits 7:0); CL = max sector (bits 5:0) | cyl high (bits 7:6)
                    // For 8-bit cylinders (max 255) the high 2 bits of CL are always 0
                    let cl = max_sector & 0x3F;
                    self.cx = ((max_cylinder as u16) << 8) | (cl as u16);
                    self.dx = ((max_head as u16) << 8) | 1u16; // DH = max head, DL = drive count
                    self.ax &= 0x00FF; // AH = 0 (success)
                    self.set_flag(cpu_flag::CARRY, false);
                    self.last_disk_status = DiskError::Success as u8;
                }
                None => {
                    self.ax = (self.ax & 0x00FF) | ((DiskError::DriveNotReady as u16) << 8);
                    self.set_flag(cpu_flag::CARRY, true);
                    self.last_disk_status = DiskError::DriveNotReady as u8;
                }
            }
        } else {
            // Hard drive: issue ATA IDENTIFY DEVICE and extract CHS geometry.
            // Drive count was written to BDA[0x475] during POST by IO-port probing.
            let drive_count = bda_get_num_hard_drives(bus);

            let drive_head = 0xA0 | ((drive.as_hard_drive_index() as u8) << 4);
            bus.io_write_u8(HDC_DRIVE_HEAD, drive_head);
            bus.io_write_u8(HDC_COMMAND, HDC_CMD_IDENTIFY);

            // Wait for BSY to clear
            while bus.io_read_u8(HDC_COMMAND) & HDC_STATUS_BSY != 0 {}

            let status = bus.io_read_u8(HDC_COMMAND);
            if status & HDC_STATUS_ERR != 0 || status & HDC_STATUS_DRQ == 0 {
                self.ax = (self.ax & 0x00FF) | ((DiskError::DriveNotReady as u16) << 8);
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = DiskError::DriveNotReady as u8;
                return;
            }

            // Read 512-byte IDENTIFY response
            let mut identify = [0u8; 512];
            for b in &mut identify {
                *b = bus.io_read_u8(HDC_DATA);
            }

            // Word 1 (bytes 2–3): cylinders
            let cylinders = u16::from_le_bytes([identify[2], identify[3]]);
            // Word 3 (bytes 6–7): heads
            let heads = u16::from_le_bytes([identify[6], identify[7]]);
            // Word 6 (bytes 12–13): sectors per track
            let spt = u16::from_le_bytes([identify[12], identify[13]]);

            if cylinders == 0 || heads == 0 || spt == 0 {
                self.ax = (self.ax & 0x00FF) | ((DiskError::DriveNotReady as u16) << 8);
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = DiskError::DriveNotReady as u8;
                return;
            }

            let max_cylinder = (cylinders - 1) as u8;
            let max_head = (heads - 1) as u8;
            let max_sector = spt as u8;

            // CH = max cylinder low 8 bits; CL bits 7:6 = cyl bits 9:8 (0 for ≤ 255 cyls)
            let cl = max_sector & 0x3F;
            self.cx = ((max_cylinder as u16) << 8) | (cl as u16);
            self.dx = ((max_head as u16) << 8) | (drive_count as u16); // DH = max head, DL = drive count
            self.ax &= 0x00FF; // AH = 0 (success)
            self.set_flag(cpu_flag::CARRY, false);
            self.last_disk_status = DiskError::Success as u8;
        }
    }

    /// INT 13h, AH=15h - Get Disk Type
    /// Input:
    ///   DL = drive number (0x00-0x7F for floppies, 0x80-0xFF for hard disks)
    /// Output:
    ///   AH = drive type:
    ///     0x00 = drive not present
    ///     0x01 = floppy disk drive without change-line support
    ///     0x02 = floppy disk drive with change-line support
    ///     0x03 = fixed disk (hard disk)
    ///   CF = clear if drive exists, set if drive does not exist
    ///   For type 0x03 (fixed disk):
    ///     CX:DX = number of 512-byte sectors (32-bit value, CX=high word, DX=low word)
    fn int13_get_disk_type(&mut self, bus: &mut Bus) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        if drive.is_floppy() {
            let drive_index = drive.as_floppy_index() as u8;

            // Read floppy drive types from CMOS register 0x10 (PC AT standard).
            // Bits 7:4 = drive A type, bits 3:0 = drive B type.
            // Codes: 0=none, 1=360KB 5.25", 2=1.2MB 5.25", 3=720KB 3.5", 4=1.44MB 3.5", 5=2.88MB 3.5"
            bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, CMOS_REG_FLOPPY_TYPES);
            let floppy_types = bus.io_read_u8(RTC_IO_PORT_DATA);
            let drive_type = if drive_index == 0 {
                (floppy_types >> 4) & 0x0F // drive A: bits 7:4
            } else {
                floppy_types & 0x0F // drive B: bits 3:0
            };

            if drive_type == 0 {
                // Drive not present
                self.ax &= 0x00FF; // AH = 0x00 (not present)
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = DiskError::InvalidCommand as u8;
            } else {
                // 360KB 5.25" (type 1) has no change-line support; all others do
                let ah: u8 = if drive_type == 0x01 { 0x01 } else { 0x02 };
                self.ax = (self.ax & 0x00FF) | ((ah as u16) << 8);
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
            }
        } else {
            // Hard drive: use ATA IDENTIFY DEVICE to confirm presence and get sector count
            let drive_head = 0xA0 | ((drive.as_hard_drive_index() as u8) << 4);
            bus.io_write_u8(HDC_DRIVE_HEAD, drive_head);
            bus.io_write_u8(HDC_COMMAND, HDC_CMD_IDENTIFY);

            // Wait for BSY to clear
            while bus.io_read_u8(HDC_COMMAND) & HDC_STATUS_BSY != 0 {}

            let status = bus.io_read_u8(HDC_COMMAND);
            if status & HDC_STATUS_ERR != 0 || status & HDC_STATUS_DRQ == 0 {
                // Drive not present
                self.ax &= 0x00FF; // AH = 0x00 (not present)
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = DiskError::InvalidCommand as u8;
                return;
            }

            // Read 512-byte IDENTIFY response
            let mut identify = [0u8; 512];
            for b in &mut identify {
                *b = bus.io_read_u8(HDC_DATA);
            }

            // Words 60–61 (bytes 120–123): total addressable sectors (LBA28)
            let total_sectors =
                u32::from_le_bytes([identify[120], identify[121], identify[122], identify[123]]);

            // AH = 0x03 (fixed disk), CX:DX = total 512-byte sector count
            self.ax = (self.ax & 0x00FF) | (0x03u16 << 8);
            self.cx = (total_sectors >> 16) as u16;
            self.dx = total_sectors as u16;
            self.set_flag(cpu_flag::CARRY, false);
            self.last_disk_status = DiskError::Success as u8;
        }
    }

    /// INT 13h, AH=16h - Detect Disk Change
    /// Input:
    ///   DL = drive number (0x00-0x7F for floppies)
    /// Output:
    ///   AH = status:
    ///     0x00 = disk not changed (changeline inactive)
    ///     0x01 = invalid drive number
    ///     0x06 = disk changed (changeline active)
    ///     0x80 = drive not ready (timeout)
    ///   CF = clear if disk not changed, set if changed or error
    fn int13_detect_disk_change(&mut self, bus: &Bus) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        // This function is only valid for floppy drives
        if !drive.is_floppy() {
            self.ax = (self.ax & 0x00FF) | ((DiskError::InvalidCommand as u16) << 8);
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = DiskError::InvalidCommand as u8;
            return;
        }

        // Read the FDC Digital Input Register (DIR) at port 0x3F7.
        // Bit 7 is the changeline: 1 = disk has been changed, 0 = not changed.
        let dir = bus.io_read_u8(FDC_DIR);
        if dir & FDC_DIR_DISK_CHANGE != 0 {
            self.ax = (self.ax & 0x00FF) | ((DiskError::DiskChanged as u16) << 8);
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = DiskError::DiskChanged as u8;
        } else {
            self.ax &= 0x00FF; // AH = 0 (not changed)
            self.set_flag(cpu_flag::CARRY, false);
            self.last_disk_status = DiskError::Success as u8;
        }
    }

    /// INT 13h, AH=18h - Set DASD Type for Format (PS/2)
    /// Input:
    ///   DL = drive number (0x00-0x7F for floppies)
    ///   CH = number of tracks (low 8 bits)
    ///   CL = sectors per track (bits 0-5) + high 2 bits of tracks (bits 6-7)
    /// Output:
    ///   AH = status:
    ///     0x00 = successful, disk formatted correctly
    ///     0x01 = invalid function or parameter
    ///     0x80 = disk does not support function
    ///   CF = clear if successful, set on error
    ///   On success:
    ///     ES:DI = pointer to 11-byte Disk Base Table (DBT)
    fn int13_set_dasd_type(&mut self, bus: &mut Bus) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL
        let tracks_low = (self.cx >> 8) as u8; // CH
        let sectors_and_tracks_high = (self.cx & 0xFF) as u8; // CL

        // Extract sectors per track and high bits of tracks
        let sectors_per_track = sectors_and_tracks_high & 0x3F; // Bits 0-5
        let tracks_high = (sectors_and_tracks_high >> 6) & 0x03; // Bits 6-7
        let tracks = ((tracks_high as u16) << 8) | (tracks_low as u16);

        // AH=18h is only valid for floppy drives
        if !drive.is_floppy() {
            self.ax = (self.ax & 0x00FF) | (0x01_u16 << 8); // AH = 0x01 (invalid)
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = DiskError::InvalidCommand as u8;
            return;
        }

        let drive_index = drive.as_floppy_index() as u8;

        // Check drive exists via CMOS register 0x10 (same approach as int13_get_drive_params)
        // Bits 7:4 = drive A type, bits 3:0 = drive B type; 0 = not present
        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, CMOS_REG_FLOPPY_TYPES);
        let floppy_types = bus.io_read_u8(RTC_IO_PORT_DATA);
        let drive_type = if drive_index == 0 {
            (floppy_types >> 4) & 0x0F
        } else {
            floppy_types & 0x0F
        };

        if drive_type == 0 {
            // Drive not present
            self.ax = (self.ax & 0x00FF) | (0x01_u16 << 8); // AH = 0x01 (invalid)
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = DiskError::InvalidCommand as u8;
            return;
        }

        // Validate the requested parameters are reasonable
        if sectors_per_track == 0 || sectors_per_track > 63 || tracks == 0 || tracks > 1024 {
            self.ax = (self.ax & 0x00FF) | (0x01_u16 << 8); // AH = 0x01 (invalid)
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = DiskError::InvalidCommand as u8;
            return;
        }

        // Build Disk Base Table (DBT) in BIOS ROM area at F000:E000
        const DBT_SEGMENT: u16 = 0xF000;
        const DBT_OFFSET: u16 = 0xE000;
        let dbt_addr = bus.physical_address(DBT_SEGMENT, DBT_OFFSET);

        // Disk Base Table format (11 bytes):
        // Standard values for common floppy formats
        let dbt: [u8; 11] = [
            0xDF,              // Offset 0: Step rate (D) and head unload time (F)
            0x02,              // Offset 1: Head load time (2ms) and DMA flag
            0x25,              // Offset 2: Motor off delay (37 * 55ms = ~2 seconds)
            0x02,              // Offset 3: Bytes per sector (2 = 512 bytes)
            sectors_per_track, // Offset 4: Last sector number per track
            0x1B,              // Offset 5: Gap length between sectors (27 bytes)
            0xFF,              // Offset 6: Data length (0xFF = use bytes per sector)
            0x54,              // Offset 7: Gap length for format (84 bytes)
            0xF6,              // Offset 8: Format filler byte (typically 0xF6)
            0x0F,              // Offset 9: Head settle time (15ms)
            0x08,              // Offset 10: Motor startup time (8 * 125ms = 1 second)
        ];

        for (i, &byte) in dbt.iter().enumerate() {
            bus.memory_write_u8(dbt_addr + i, byte);
        }

        // Return success with ES:DI pointing to DBT
        self.es = DBT_SEGMENT;
        self.di = DBT_OFFSET;
        self.ax &= 0x00FF; // AH = 0 (success)
        self.set_flag(cpu_flag::CARRY, false);
        self.last_disk_status = DiskError::Success as u8;
    }

    /// INT 13h, AH=41h - Check Extensions Present
    /// Input:
    ///   BX = 0x55AA (magic)
    ///   DL = drive number
    /// Output:
    ///   CF = set if extensions not supported (we report no LBA support)
    ///   CF = clear + BX=0xAA55 + AH=version + CX=feature bits if supported
    fn int13_check_extensions_present(&mut self) {
        // We don't implement LBA extensions; tell callers to use CHS mode
        self.ax = (self.ax & 0x00FF) | ((DiskError::InvalidCommand as u16) << 8);
        self.set_flag(cpu_flag::CARRY, true);
        self.last_disk_status = DiskError::InvalidCommand as u8;
    }
}
