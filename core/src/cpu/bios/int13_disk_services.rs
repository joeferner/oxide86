use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag},
    disk::{
        DiskError, DriveNumber, FDC_DATA, FDC_DIR, FDC_DIR_DISK_CHANGE, FDC_DOR, FDC_MSR,
        FDC_MSR_NDM,
    },
    physical_address,
};

impl Cpu {
    /// INT 0x13 - BIOS Disk Services
    /// AH register contains the function number
    ///
    /// Note: AT-class BIOS enables interrupts (STI) during disk operations so that
    /// timer IRQs (INT 0x08) can still fire. This is important for programs that
    /// depend on the BDA timer counter advancing during disk benchmarks.
    pub(in crate::cpu) fn handle_int13_disk_services(&mut self, bus: &mut Bus) {
        // Enable interrupts during disk operations (AT-class BIOS behavior)
        // This allows timer IRQs to fire even during extended disk operations
        self.set_flag(cpu_flag::INTERRUPT, true);

        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int13_reset_disk(bus),
            0x01 => self.int13_get_status(),
            0x02 => self.int13_read_sectors(bus),
            0x08 => self.int13_get_drive_params(bus),
            0x15 => self.int13_get_disk_type(bus),
            0x16 => self.int13_detect_disk_change(bus),
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
    fn int13_reset_disk(&mut self, _bus: &mut Bus) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        let success = if drive.is_floppy() {
            // TODO

            // // Reset the Digital Output Register (DOR)
            // Write_IO_Port(FDC_DOR, 0x00)
            // Wait_Microseconds(50)
            // Write_IO_Port(FDC_DOR, 0x0C) // Re-enable controller and DMA

            // // Force recalibration: Move head to Track 0
            // Command_FDC(RECALIBRATE_COMMAND, Drive)

            // // Check if the controller is ready
            // IF FDC_Timeout_Or_Error() THEN
            //     Return 0x05 // Reset Failed

            // // Reset Diskette Drive Data (BDA 0040h:003Eh)
            // Update_BDA_Disk_Status(Drive, RESET_FLAG)

            true
        } else {
            // TODO

            // // 2. Send the "Reset" signal to the Fixed Disk Controller
            // // This often involves toggling a bit in the Control Register
            // Write_IO_Port(FixedDisk_Control_Reg, 0x04) // Set Soft Reset bit
            // Wait_Microseconds(10)                      // Hold the reset
            // Write_IO_Port(FixedDisk_Control_Reg, 0x00) // Clear Reset bit

            // // 3. Wait for the Controller to clear the BUSY bit
            // // We set a timeout because a dead drive shouldn't hang the BIOS
            // StartTime = Get_System_Ticks()
            // WHILE (Read_IO_Port(FixedDisk_Status_Reg) & STATUS_BUSY):
            //     IF (Get_System_Ticks() - StartTime > TIMEOUT_VAL) THEN
            //         Return 0x80 // Controller Timeout
            //     END IF
            // END WHILE

            // // 4. Send "Recalibrate" (Execute Drive Diagnostics)
            // // This tells the drive to verify internal parameters
            // Command_HDD(DriveNumber, DRIVE_DIAGNOSTIC_CMD)

            // // 5. Update BIOS Data Area (BDA)
            // // 0040h:0074h stores the status of the last hard disk operation
            // Update_BDA_HardDisk_Status(0x00)

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

        if !drive.is_floppy() {
            log::warn!("INT 13h AH=02h: hard drive not yet implemented");
            self.ax = (self.ax & 0x00FF) | ((DiskError::DriveNotReady as u16) << 8);
            self.ax &= 0xFF00;
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = DiskError::DriveNotReady as u8;
            return;
        }

        let drive_index = drive.to_floppy_index() as u8;
        let buffer_addr = physical_address(self.es, self.bx);
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
                "INT 13h AH=02h: FDC read failed for drive {}, ST0=0x{:02X}",
                drive,
                st0
            );
            self.ax = (self.ax & 0x00FF) | ((error as u16) << 8); // AH = error
            self.ax &= 0xFF00; // AL = 0
            self.set_flag(cpu_flag::CARRY, true);
            self.last_disk_status = error as u8;
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
    fn int13_get_drive_params(&mut self, bus: &Bus) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        let geometry = if drive.is_floppy() {
            bus.floppy_controller().disk_geometry(drive)
        } else {
            None // TODO Hard drives not yet implemented
        };

        match geometry {
            Some(geometry) => {
                // Count drives of this type (CD-ROM placeholders excluded from hard drive count)
                // TODO verify how CD-ROM should be handled
                let drive_count = if drive.is_floppy() { 1 } else { 0 };
                let max_cylinder = (geometry.cylinders - 1).min(255) as u8;
                let max_head = (geometry.heads - 1).min(255) as u8;
                let max_sector = geometry.sectors_per_track.min(255) as u8;

                // Pack cylinder into CH and CL
                let cylinder = max_cylinder as u16;
                let cylinder_low = (cylinder & 0xFF) as u8;
                let cylinder_high = ((cylinder >> 8) & 0x03) as u8;

                // Pack sector and cylinder high bits into CL
                let cl = (max_sector & 0x3F) | (cylinder_high << 6);

                self.cx = ((cylinder_low as u16) << 8) | (cl as u16); // CH:CL
                self.dx = ((max_head as u16) << 8) | (drive_count as u16); // DH:DL
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
            }
            None => {
                self.ax = (self.ax & 0x00FF) | ((DiskError::DriveNotReady as u16) << 8); // AH = error code
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = DiskError::DriveNotReady as u8;
            }
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
    fn int13_get_disk_type(&mut self, bus: &Bus) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        // Drive type:
        // 0x00 = not present
        // 0x01 = floppy without change-line support
        // 0x02 = floppy with change-line support
        // 0x03 = fixed disk (hard drive)
        let drive_type = if drive.is_floppy() {
            0x02 // Floppy with change-line support
        } else {
            0x03 // Fixed disk
        };

        let geometry = if drive.is_floppy() {
            bus.floppy_controller().disk_geometry(drive)
        } else {
            None // TODO Hard drives not yet implemented
        };

        match geometry {
            Some(geometry) => {
                let sector_count = geometry.total_sectors() as u32;

                // Set AH = drive type
                self.ax = (self.ax & 0x00FF) | ((drive_type as u16) << 8);

                // If it's a fixed disk (type 0x03), set CX:DX to sector count
                if drive_type == 0x03 {
                    let high_word = ((sector_count >> 16) & 0xFFFF) as u16;
                    let low_word = (sector_count & 0xFFFF) as u16;
                    self.cx = high_word;
                    self.dx = low_word;
                }

                // Clear carry flag (drive exists)
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
            }
            None => {
                // Drive not present
                self.ax &= 0x00FF; // AH = 0x00 (drive not present)
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = DiskError::InvalidCommand as u8;
            }
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
}
