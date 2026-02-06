use crate::{
    DriveNumber,
    cpu::{Cpu, bios::disk_error::DiskError, cpu_flag},
    memory::Memory,
};

impl Cpu {
    /// INT 0x13 - BIOS Disk Services
    /// AH register contains the function number
    ///
    /// Note: AT-class BIOS enables interrupts (STI) during disk operations so that
    /// timer IRQs (INT 0x08) can still fire. This is important for programs that
    /// depend on the BDA timer counter advancing during disk benchmarks.
    pub(super) fn handle_int13(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        // Enable interrupts during disk operations (AT-class BIOS behavior)
        // This allows timer IRQs to fire even during extended disk operations
        self.set_flag(cpu_flag::INTERRUPT, true);

        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int13_reset_disk(io),
            0x01 => self.int13_get_status(io),
            0x02 => self.int13_read_sectors(memory, io),
            0x03 => self.int13_write_sectors(memory, io),
            0x04 => self.int13_verify_sectors(io),
            0x05 => self.int13_format_track(io),
            0x08 => self.int13_get_drive_params(io),
            0x15 => self.int13_get_disk_type(io),
            0x16 => self.int13_detect_disk_change(io),
            0x18 => self.int13_set_dasd_type(memory, io),
            _ => {
                log::warn!("Unhandled INT 0x13 function: AH=0x{:02X}", function);
                // Set error: invalid command
                self.ax = (self.ax & 0x00FF) | ((DiskError::InvalidCommand as u16) << 8);
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(DiskError::InvalidCommand as u8);
            }
        }
    }

    /// INT 13h, AH=00h - Reset Disk System
    /// Input:
    ///   DL = drive number (0x00-0x7F for floppies, 0x80-0xFF for hard disks)
    /// Output:
    ///   AH = status (0 = success)
    ///   CF = clear if success, set if error
    fn int13_reset_disk(&mut self, io: &mut super::Bios) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        let success = io.disk_reset(drive);

        if success {
            self.ax &= 0x00FF; // AH = 0 (success)
            self.set_flag(cpu_flag::CARRY, false);
            io.set_last_disk_status(DiskError::Success as u8);
        } else {
            self.ax = (self.ax & 0x00FF) | ((DiskError::ResetFailed as u16) << 8);
            self.set_flag(cpu_flag::CARRY, true);
            io.set_last_disk_status(DiskError::ResetFailed as u8);
        }
    }

    /// INT 13h, AH=01h - Get Status of Last Disk Operation
    /// Input:
    ///   AH = 0x01
    ///   DL = drive number (optional, some implementations ignore it)
    /// Output:
    ///   AH = status code of last disk operation (0x00 = success, or error code)
    ///   CF = cleared (always - the status retrieval itself succeeds)
    fn int13_get_status(&mut self, io: &super::Bios) {
        // Return the last disk status
        let status = io.shared.last_disk_status;

        if self.log_interrupts_enabled {
            log::info!("INT 13h AH=01h: Get Status - returning 0x{:02X}", status);
        }

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
    fn int13_read_sectors(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let sector_and_cyl_high = (self.cx & 0xFF) as u8; // CL
        let head = (self.dx >> 8) as u8; // DH
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // DL

        // Extract cylinder and sector from CL
        let sector = sector_and_cyl_high & 0x3F; // Bits 0-5
        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder_8bit = cylinder_low;

        if self.log_interrupts_enabled {
            log::info!(
                "INT 13h AH=02h: Read {} sectors from drive {}, C/H/S={}/{}/{}",
                count,
                drive,
                cylinder_8bit,
                head,
                sector
            );
        }

        match io.disk_read_sectors(drive, cylinder_8bit, head, sector, count) {
            Ok(data) => {
                // Write data to ES:BX
                let buffer_addr = Self::physical_address(self.es, self.bx);
                for (i, &byte) in data.iter().enumerate() {
                    memory.write_u8(buffer_addr + i, byte);
                }

                // Calculate actual sectors read
                let sectors_read = (data.len() / 512).min(count as usize) as u8;

                if self.log_interrupts_enabled {
                    log::info!(
                        "INT 13h AH=02h: Successfully read {} sectors from drive {} to {:04X}:{:04X}",
                        sectors_read,
                        drive,
                        self.es,
                        self.bx
                    );
                }

                self.ax = (self.ax & 0xFF00) | (sectors_read as u16); // AL = sectors read
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                io.set_last_disk_status(DiskError::Success as u8);
            }
            Err(error_code) => {
                log::warn!(
                    "INT 13h AH=02h: Read failed for drive {}, error {}",
                    drive,
                    error_code
                );
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.ax &= 0xFF00; // AL = 0 (no sectors read)
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(error_code as u8);
            }
        }
    }

    /// INT 13h, AH=03h - Write Sectors from Memory
    /// Input:
    ///   AL = number of sectors to write (1-128)
    ///   CH = cylinder number (0-1023, low 8 bits)
    ///   CL = sector number (1-63, bits 0-5) + high 2 bits of cylinder (bits 6-7)
    ///   DH = head number (0-255)
    ///   DL = drive number
    ///   ES:BX = buffer address
    /// Output:
    ///   AH = status (0 = success)
    ///   AL = number of sectors written
    ///   CF = clear if success, set if error
    fn int13_write_sectors(&mut self, memory: &Memory, io: &mut super::Bios) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let sector_and_cyl_high = (self.cx & 0xFF) as u8; // CL
        let head = (self.dx >> 8) as u8; // DH
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // DL

        // Extract cylinder and sector from CL
        let sector = sector_and_cyl_high & 0x3F; // Bits 0-5
        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder_8bit = cylinder_low;

        // Read data from ES:BX
        let buffer_addr = Self::physical_address(self.es, self.bx);
        let data_len = count as usize * 512;
        let mut data = Vec::with_capacity(data_len);
        for i in 0..data_len {
            data.push(memory.read_u8(buffer_addr + i));
        }

        match io.disk_write_sectors(drive, cylinder_8bit, head, sector, count, &data) {
            Ok(sectors_written) => {
                self.ax = (self.ax & 0xFF00) | (sectors_written as u16); // AL = sectors written
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                io.set_last_disk_status(DiskError::Success as u8);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.ax &= 0xFF00; // AL = 0 (no sectors written)
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(error_code as u8);
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
    fn int13_verify_sectors(&mut self, io: &mut super::Bios) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let sector_and_cyl_high = (self.cx & 0xFF) as u8; // CL
        let head = (self.dx >> 8) as u8; // DH
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // DL

        // Extract cylinder and sector from CL
        let sector = sector_and_cyl_high & 0x3F; // Bits 0-5
        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder_8bit = cylinder_low;

        // Verify sectors by attempting to read them (data is discarded)
        match io.disk_read_sectors(drive, cylinder_8bit, head, sector, count) {
            Ok(data) => {
                // Calculate actual sectors verified
                let sectors_verified = (data.len() / 512).min(count as usize) as u8;

                self.ax = (self.ax & 0xFF00) | (sectors_verified as u16); // AL = sectors verified
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                io.set_last_disk_status(DiskError::Success as u8);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.ax &= 0xFF00; // AL = 0 (no sectors verified)
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(error_code as u8);
            }
        }
    }

    /// INT 13h, AH=05h - Format Track
    /// Input:
    ///   AL = number of sectors per track
    ///   CH = cylinder number (low 8 bits)
    ///   CL = sector number (bits 0-5) + high 2 bits of cylinder (bits 6-7)
    ///   DH = head number
    ///   DL = drive number
    ///   ES:BX = pointer to address field list (not used in emulation)
    /// Output:
    ///   AH = status (0 = success)
    ///   CF = clear if success, set if error
    fn int13_format_track(&mut self, io: &mut super::Bios) {
        let sectors_per_track = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let head = (self.dx >> 8) as u8; // DH
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // DL

        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder = cylinder_low;

        match io.disk_format_track(drive, cylinder, head, sectors_per_track) {
            Ok(()) => {
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                io.set_last_disk_status(DiskError::Success as u8);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(error_code as u8);
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
    fn int13_get_drive_params(&mut self, io: &mut super::Bios) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        if self.log_interrupts_enabled {
            log::info!("INT 13h AH=08h: Get Drive Parameters for drive {}", drive);
        }

        match io.disk_get_params(drive) {
            Ok(params) => {
                if self.log_interrupts_enabled {
                    log::info!(
                        "INT 13h AH=08h: Drive {} params: cyl={}, head={}, sec={}, drives={}",
                        drive,
                        params.max_cylinder,
                        params.max_head,
                        params.max_sector,
                        params.drive_count
                    );
                }
                // Pack cylinder into CH and CL
                let cylinder = params.max_cylinder as u16;
                let cylinder_low = (cylinder & 0xFF) as u8;
                let cylinder_high = ((cylinder >> 8) & 0x03) as u8;

                // Pack sector and cylinder high bits into CL
                let cl = (params.max_sector & 0x3F) | (cylinder_high << 6);

                self.cx = ((cylinder_low as u16) << 8) | (cl as u16); // CH:CL
                self.dx = ((params.max_head as u16) << 8) | (params.drive_count as u16); // DH:DL
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                io.set_last_disk_status(DiskError::Success as u8);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(error_code as u8);
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
    fn int13_get_disk_type(&mut self, io: &mut super::Bios) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        match io.disk_get_type(drive) {
            Ok((drive_type, sector_count)) => {
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
                io.set_last_disk_status(DiskError::Success as u8);
            }
            Err(_) => {
                // Drive not present
                self.ax &= 0x00FF; // AH = 0x00 (drive not present)
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(DiskError::InvalidCommand as u8);
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
    fn int13_detect_disk_change(&mut self, io: &mut super::Bios) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        match io.disk_detect_change(drive) {
            Ok(changed) => {
                if changed {
                    // Disk was changed
                    self.ax = (self.ax & 0x00FF) | ((DiskError::DiskChanged as u16) << 8);
                    self.set_flag(cpu_flag::CARRY, true);
                    io.set_last_disk_status(DiskError::DiskChanged as u8);
                } else {
                    // Disk not changed
                    self.ax &= 0x00FF; // AH = 0 (not changed)
                    self.set_flag(cpu_flag::CARRY, false);
                    io.set_last_disk_status(DiskError::Success as u8);
                }
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8);
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(error_code as u8);
            }
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
    fn int13_set_dasd_type(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL
        let tracks_low = (self.cx >> 8) as u8; // CH
        let sectors_and_tracks_high = (self.cx & 0xFF) as u8; // CL

        // Extract sectors per track and high bits of tracks
        let sectors_per_track = sectors_and_tracks_high & 0x3F; // Bits 0-5
        let tracks_high = (sectors_and_tracks_high >> 6) & 0x03; // Bits 6-7
        let tracks = ((tracks_high as u16) << 8) | (tracks_low as u16);

        if self.log_interrupts_enabled {
            log::info!(
                "INT 13h AH=18h: Set DASD Type for drive {}, tracks={}, sectors_per_track={}",
                drive,
                tracks,
                sectors_per_track
            );
        }

        // Verify drive exists and get its parameters
        match io.disk_get_params(drive) {
            Ok(_params) => {
                // Validate the requested parameters are reasonable
                if sectors_per_track == 0 || sectors_per_track > 63 || tracks == 0 || tracks > 1024
                {
                    // Invalid parameters
                    self.ax = (self.ax & 0x00FF) | (0x01_u16 << 8); // AH = 0x01 (invalid)
                    self.set_flag(cpu_flag::CARRY, true);
                    io.set_last_disk_status(DiskError::InvalidCommand as u8);
                    return;
                }

                // Build Disk Base Table (DBT) in BIOS ROM area at F000:E000
                const DBT_SEGMENT: u16 = 0xF000;
                const DBT_OFFSET: u16 = 0xE000;
                let dbt_addr = Self::physical_address(DBT_SEGMENT, DBT_OFFSET);

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

                // Write DBT to memory
                for (i, &byte) in dbt.iter().enumerate() {
                    memory.write_u8(dbt_addr + i, byte);
                }

                // Return success with ES:DI pointing to DBT
                self.es = DBT_SEGMENT;
                self.di = DBT_OFFSET;
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                io.set_last_disk_status(DiskError::Success as u8);
                if self.log_interrupts_enabled {
                    log::info!(
                        "INT 13h AH=18h: Success, DBT at {:04X}:{:04X}",
                        DBT_SEGMENT,
                        DBT_OFFSET
                    );
                }
            }
            Err(_) => {
                // Drive not present
                if self.log_interrupts_enabled {
                    log::info!("INT 13h AH=18h: Drive {} not present", drive);
                }
                self.ax = (self.ax & 0x00FF) | (0x01_u16 << 8); // AH = 0x01 (invalid)
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(DiskError::InvalidCommand as u8);
            }
        }
    }
}
