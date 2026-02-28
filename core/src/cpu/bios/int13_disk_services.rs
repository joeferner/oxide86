use crate::{
    cpu::{Cpu, cpu_flag},
    disk::{DiskError, DriveNumber},
    io_bus::IoBus,
    memory_bus::MemoryBus,
    physical_address,
};

/// Drive parameters returned by INT 13h, AH=08h
#[derive(Debug, Clone, Copy)]
pub struct DriveParams {
    /// Maximum cylinder number (0-based)
    pub max_cylinder: u8,
    /// Maximum head number (0-based)
    pub max_head: u8,
    /// Maximum sector number (1-based)
    pub max_sector: u8,
    /// Number of drives
    pub drive_count: u8,
}

impl Cpu {
    /// INT 0x13 - BIOS Disk Services
    /// AH register contains the function number
    ///
    /// Note: AT-class BIOS enables interrupts (STI) during disk operations so that
    /// timer IRQs (INT 0x08) can still fire. This is important for programs that
    /// depend on the BDA timer counter advancing during disk benchmarks.
    pub(in crate::cpu) fn handle_int13_disk_services(
        &mut self,
        memory_bus: &mut MemoryBus,
        io_bus: &mut IoBus,
    ) {
        // Enable interrupts during disk operations (AT-class BIOS behavior)
        // This allows timer IRQs to fire even during extended disk operations
        self.set_flag(cpu_flag::INTERRUPT, true);

        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x02 => self.int13_read_sectors(memory_bus, io_bus),
            0x08 => self.int13_get_drive_params(io_bus),
            _ => {
                log::warn!("Unhandled INT 0x13 function: AH=0x{:02X}", function);
                // Set error: invalid command
                self.ax = (self.ax & 0x00FF) | ((DiskError::InvalidCommand as u16) << 8);
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = DiskError::InvalidCommand as u8;
            }
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
    fn int13_read_sectors(&mut self, memory_bus: &mut MemoryBus, io_bus: &mut IoBus) {
        let count = (self.ax & 0xFF) as u8; // AL
        let cylinder_low = (self.cx >> 8) as u8; // CH
        let sector_and_cyl_high = (self.cx & 0xFF) as u8; // CL
        let head = (self.dx >> 8) as u8; // DH
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // DL

        // Extract cylinder and sector from CL
        let sector = sector_and_cyl_high & 0x3F; // Bits 0-5
        // For 8086, we only support 8-bit cylinders (compatibility mode)
        let cylinder_8bit = cylinder_low;

        match io_bus.disk_read_sectors(drive, cylinder_8bit, head, sector, count) {
            Ok(data) => {
                // Write data to ES:BX
                let buffer_addr = physical_address(self.es, self.bx);
                for (i, &byte) in data.iter().enumerate() {
                    memory_bus.write_u8(buffer_addr + i, byte);
                }

                // Calculate actual sectors read
                let sectors_read = (data.len() / 512).min(count as usize) as u8;

                self.ax = (self.ax & 0xFF00) | (sectors_read as u16); // AL = sectors read
                self.ax &= 0x00FF; // AH = 0 (success)
                self.set_flag(cpu_flag::CARRY, false);
                self.last_disk_status = DiskError::Success as u8;
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
                self.last_disk_status = error_code as u8;
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
    fn int13_get_drive_params(&mut self, io_bus: &mut IoBus) {
        let drive = DriveNumber::from_standard((self.dx & 0xFF) as u8); // Get DL

        match io_bus.disk_get_params(drive) {
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
                self.last_disk_status = DiskError::Success as u8;
            }
            Err(error_code) => {
                self.ax = (self.ax & 0x00FF) | ((error_code as u16) << 8); // AH = error code
                self.set_flag(cpu_flag::CARRY, true);
                self.last_disk_status = error_code as u8;
            }
        }
    }
}
