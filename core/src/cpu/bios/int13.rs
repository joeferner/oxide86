use log::warn;

use crate::{Bios, cpu::{Cpu, cpu_flag}, memory::Memory};

/// INT 13h error codes
pub mod disk_errors {
    pub const SUCCESS: u8 = 0x00;
    pub const INVALID_COMMAND: u8 = 0x01;
    pub const ADDRESS_MARK_NOT_FOUND: u8 = 0x02;
    pub const WRITE_PROTECTED: u8 = 0x03;
    pub const SECTOR_NOT_FOUND: u8 = 0x04;
    pub const RESET_FAILED: u8 = 0x05;
    pub const DISK_CHANGED: u8 = 0x06;
    pub const DRIVE_PARAMETER_ACTIVITY_FAILED: u8 = 0x07;
    pub const DMA_OVERRUN: u8 = 0x08;
    pub const DMA_BOUNDARY_ERROR: u8 = 0x09;
    pub const BAD_SECTOR: u8 = 0x0A;
    pub const BAD_TRACK: u8 = 0x0B;
    pub const UNSUPPORTED_TRACK: u8 = 0x0C;
    pub const INVALID_NUMBER_OF_SECTORS: u8 = 0x0D;
    pub const CONTROL_DATA_ADDRESS_MARK_DETECTED: u8 = 0x0E;
    pub const DMA_ARBITRATION_LEVEL_OUT_OF_RANGE: u8 = 0x0F;
    pub const UNCORRECTABLE_CRC_ERROR: u8 = 0x10;
    pub const ECC_CORRECTED_DATA_ERROR: u8 = 0x11;
    pub const CONTROLLER_FAILURE: u8 = 0x20;
    pub const SEEK_FAILED: u8 = 0x40;
    pub const TIMEOUT: u8 = 0x80;
    pub const DRIVE_NOT_READY: u8 = 0xAA;
    pub const UNDEFINED_ERROR: u8 = 0xBB;
    pub const WRITE_FAULT: u8 = 0xCC;
    pub const STATUS_REGISTER_ERROR: u8 = 0xE0;
    pub const SENSE_OPERATION_FAILED: u8 = 0xFF;
}

impl Cpu {
    /// INT 0x13 - BIOS Disk Services
    /// AH register contains the function number
    pub(super) fn handle_int13<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int13_reset_disk(io),
            0x02 => self.int13_read_sectors(memory, io),
            0x03 => self.int13_write_sectors(memory, io),
            0x08 => self.int13_get_drive_params(io),
            0x15 => self.int13_get_disk_type(io),
            _ => {
                warn!("Unhandled INT 0x13 function: AH=0x{:02X}", function);
                // Set error: invalid command
                self.ax = (self.ax & 0x00FF) | ((disk_errors::INVALID_COMMAND as u16) << 8);
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 13h, AH=00h - Reset Disk System
    /// Input:
    ///   DL = drive number (0x00-0x7F for floppies, 0x80-0xFF for hard disks)
    /// Output:
    ///   AH = status (0 = success)
    ///   CF = clear if success, set if error
    fn int13_reset_disk<T: Bios>(&mut self, io: &mut T) {
        let drive = (self.dx & 0xFF) as u8; // Get DL

        let success = io.disk_reset(drive);

        if success {
            self.ax &= 0x00FF; // AH = 0 (success)
            self.set_flag(cpu_flag::CARRY, false);
        } else {
            self.ax = (self.ax & 0x00FF) | ((disk_errors::RESET_FAILED as u16) << 8);
            self.set_flag(cpu_flag::CARRY, true);
        }
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
    fn int13_read_sectors<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let sector_and_cyl_high = (self.cx & 0xFF) as u8; // CL
        let head = (self.dx >> 8) as u8; // DH
        let drive = (self.dx & 0xFF) as u8; // DL

        // Extract cylinder and sector from CL
        let sector = sector_and_cyl_high & 0x3F; // Bits 0-5
        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder_8bit = cylinder_low;

        match io.disk_read_sectors(drive, cylinder_8bit, head, sector, count) {
            Ok(data) => {
                // Write data to ES:BX
                let buffer_addr = Self::physical_address(self.es, self.bx);
                for (i, &byte) in data.iter().enumerate() {
                    memory.write_byte(buffer_addr + i, byte);
                }

                // Calculate actual sectors read
                let sectors_read = (data.len() / 512).min(count as usize) as u8;

                self.ax = (self.ax & 0xFF00) | (sectors_read as u16); // AL = sectors read
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.ax &= 0xFF00; // AL = 0 (no sectors read)
                self.set_flag(cpu_flag::CARRY, true);
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
    fn int13_write_sectors<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let sector_and_cyl_high = (self.cx & 0xFF) as u8; // CL
        let head = (self.dx >> 8) as u8; // DH
        let drive = (self.dx & 0xFF) as u8; // DL

        // Extract cylinder and sector from CL
        let sector = sector_and_cyl_high & 0x3F; // Bits 0-5
        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder_8bit = cylinder_low;

        // Read data from ES:BX
        let buffer_addr = Self::physical_address(self.es, self.bx);
        let data_len = count as usize * 512;
        let mut data = Vec::with_capacity(data_len);
        for i in 0..data_len {
            data.push(memory.read_byte(buffer_addr + i));
        }

        match io.disk_write_sectors(drive, cylinder_8bit, head, sector, count, &data) {
            Ok(sectors_written) => {
                self.ax = (self.ax & 0xFF00) | (sectors_written as u16); // AL = sectors written
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.ax &= 0xFF00; // AL = 0 (no sectors written)
                self.set_flag(cpu_flag::CARRY, true);
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
    fn int13_get_drive_params<T: Bios>(&mut self, io: &T) {
        let drive = (self.dx & 0xFF) as u8; // Get DL

        match io.disk_get_params(drive) {
            Ok(params) => {
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
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.set_flag(cpu_flag::CARRY, true);
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
    fn int13_get_disk_type<T: Bios>(&mut self, io: &T) {
        let drive = (self.dx & 0xFF) as u8; // Get DL

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
            }
            Err(_) => {
                // Drive not present
                self.ax &= 0x00FF; // AH = 0x00 (drive not present)
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }
}
