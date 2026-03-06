use crate::{
    Bus, DriveNumber,
    cpu::{Cpu, bios::disk_error::DiskError, cpu_flag},
};

impl Cpu {
    pub(super) fn handle_int13(&mut self, bus: &mut Bus, io: &mut super::Bios) {

        // Route CD-ROM drives (0xE0-0xE3) to separate handler
        let dl = (self.dx & 0xFF) as u8;
        let drive = DriveNumber::from_standard(dl);
        if drive.is_cdrom() {
            self.handle_int13_cdrom(bus, io, drive);
            return;
        }


        match function {
            0x03 => self.int13_write_sectors(bus, io),
            0x05 => self.int13_format_track(io),
            0x41 => self.int13_check_extensions_present(io),
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
    fn int13_write_sectors(&mut self, bus: &mut Bus, io: &mut super::Bios) {
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
            data.push(bus.read_u8(buffer_addr + i));
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


    /// INT 13h, AH=41h - Check Extensions Present
    /// Input:
    ///   BX = 0x55AA (magic)
    ///   DL = drive number
    /// Output:
    ///   CF = set if extensions not supported (we report no LBA support)
    ///   CF = clear + BX=0xAA55 + AH=version + CX=feature bits if supported
    fn int13_check_extensions_present(&mut self, io: &mut super::Bios) {
        // We don't implement LBA extensions; tell callers to use CHS mode
        self.ax = (self.ax & 0x00FF) | ((DiskError::InvalidCommand as u16) << 8);
        self.set_flag(cpu_flag::CARRY, true);
        io.set_last_disk_status(DiskError::InvalidCommand as u8);
    }

    /// INT 13h CD-ROM handler for drives 0xE0-0xE3.
    ///
    /// AH=00h: reset → success
    /// AH=01h: get status
    /// AH=02h: read sectors (CHS mapped to CD 512-byte sub-sectors)
    /// AH=15h: get disk type → AH=0x03 (CD-ROM), CX:DX=0
    fn handle_int13_cdrom(&mut self, bus: &mut Bus, io: &mut super::Bios, drive: DriveNumber) {
        let function = (self.ax >> 8) as u8; // AH

        match function {
            0x00 => {
                // Reset — always success for CD-ROM
                self.ax &= 0x00FF; // AH = 0
                self.set_flag(cpu_flag::CARRY, false);
                io.set_last_disk_status(DiskError::Success as u8);
            }
            0x01 => {
                // Get status of last operation
                let status = io.shared.last_disk_status;
                self.ax = (self.ax & 0x00FF) | ((status as u16) << 8);
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x02 => {
                // Read sectors
                // CHS → flat 512-byte LBA using CD-ROM convention (75 sectors per track)
                let count = (self.ax & 0xFF) as u8; // AL
                let cylinder = self.cx >> 8; // CH
                let sector = (self.cx & 0xFF) as u8; // Full CL byte (CD-ROM sectors aren't limited to 6-bit CHS encoding)
                // lba_512 = cylinder * 75 + (sector - 1)
                let lba_512 = cylinder as usize * 75 + sector.saturating_sub(1) as usize;

                let buffer_addr = Self::physical_address(self.es, self.bx);
                let mut sectors_read = 0u8;
                let mut error: Option<DiskError> = None;

                for i in 0..count as usize {
                    match io
                        .shared
                        .drive_manager
                        .cdrom_read_sector_as_512(drive, lba_512 + i)
                    {
                        Ok(sector_data) => {
                            let dest = buffer_addr + i * 512;
                            for (j, &b) in sector_data.iter().enumerate() {
                                bus.write_u8(dest + j, b);
                            }
                            sectors_read += 1;
                        }
                        Err(e) => {
                            error = Some(e);
                            break;
                        }
                    }
                }

                if let Some(e) = error {
                    self.ax = (sectors_read as u16) | ((e as u16) << 8);
                    self.set_flag(cpu_flag::CARRY, true);
                    io.set_last_disk_status(e as u8);
                } else {
                    self.ax = sectors_read as u16; // AH=0, AL=count
                    self.set_flag(cpu_flag::CARRY, false);
                    io.set_last_disk_status(DiskError::Success as u8);
                }
            }
            0x15 => {
                // Get disk type
                // AH=0x03 = CD-ROM, CX:DX=0 (sector count not meaningful)
                if io.has_cdrom(drive.cdrom_slot()) {
                    self.ax = (self.ax & 0x00FF) | (0x03u16 << 8); // AH = 3 (CD-ROM)
                    self.cx = 0;
                    self.dx = 0;
                    self.set_flag(cpu_flag::CARRY, false);
                    io.set_last_disk_status(DiskError::Success as u8);
                } else {
                    self.ax &= 0x00FF; // AH = 0 (not present)
                    self.set_flag(cpu_flag::CARRY, true);
                    io.set_last_disk_status(DiskError::InvalidCommand as u8);
                }
            }
            _ => {
                log::warn!(
                    "Unhandled INT 13h CD-ROM function: AH=0x{:02X} drive={}",
                    function,
                    drive
                );
                self.ax = (self.ax & 0x00FF) | ((DiskError::InvalidCommand as u16) << 8);
                self.set_flag(cpu_flag::CARRY, true);
                io.set_last_disk_status(DiskError::InvalidCommand as u8);
            }
        }
    }
}
