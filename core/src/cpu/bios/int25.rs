use crate::{
    DriveNumber,
    cpu::{Cpu, cpu_flag},
    memory::Memory,
};

impl Cpu {
    /// INT 25h - DOS Absolute Disk Read
    ///
    /// Reads sectors directly from disk using logical sector addressing.
    /// This bypasses the file system and reads raw sectors.
    ///
    /// Input:
    ///   AL = drive number (0=A:, 1=B:, 2=C:, etc.)
    ///   CX = number of sectors to read (or 0xFFFF for extended call)
    ///   DX = starting logical sector number (for CX != 0xFFFF)
    ///   DS:BX = buffer address (or pointer to parameter block if CX = 0xFFFF)
    ///
    /// For extended call (CX = 0xFFFF, DOS 4.0+ for drives > 32MB):
    ///   DS:BX points to parameter block:
    ///     DWORD starting sector
    ///     WORD  sector count
    ///     DWORD buffer address
    ///
    /// Output:
    ///   CF = clear if successful
    ///   CF = set if error, AL = error code
    ///
    /// Note: INT 25h/26h leave FLAGS on the stack. The caller must POP them.
    /// This is handled by the calling code, not the interrupt handler.
    pub(super) fn handle_int25<K: crate::KeyboardInput, D: crate::DiskController>(
        &mut self,
        memory: &mut Memory,
        io: &mut super::Bios<K, D>,
    ) {
        let drive = DriveNumber::from_dos((self.ax & 0xFF) as u8); // AL = drive number
        let count = self.cx;
        let buffer_addr: usize;
        let start_sector: u32;
        let sector_count: u16;

        if count == 0xFFFF {
            // Extended call - read parameter block from DS:BX
            let param_addr = Self::physical_address(self.ds, self.bx);

            // Parameter block format:
            // DWORD starting sector (offset 0)
            // WORD sector count (offset 4)
            // DWORD buffer address (offset 6)
            let start_low = memory.read_u16(param_addr) as u32;
            let start_high = memory.read_u16(param_addr + 2) as u32;
            start_sector = start_low | (start_high << 16);
            sector_count = memory.read_u16(param_addr + 4);
            let buf_offset = memory.read_u16(param_addr + 6);
            let buf_segment = memory.read_u16(param_addr + 8);
            buffer_addr = Self::physical_address(buf_segment, buf_offset);

            log::debug!(
                "INT 25h: Extended read - drive={}, start={}, count={}, buffer={:05X}",
                drive,
                start_sector,
                sector_count,
                buffer_addr
            );
        } else {
            // Standard call
            start_sector = self.dx as u32;
            sector_count = count;
            buffer_addr = Self::physical_address(self.ds, self.bx);

            log::debug!(
                "INT 25h: Standard read - drive={}, start={}, count={}, buffer={:05X}",
                drive,
                start_sector,
                sector_count,
                buffer_addr
            );
        }

        // Perform the read
        match io.disk_read_sectors_lba(drive, start_sector, sector_count) {
            Ok(data) => {
                // Write data to buffer
                for (i, &byte) in data.iter().enumerate() {
                    memory.write_u8(buffer_addr + i, byte);
                }

                // Clear carry flag (success)
                self.set_flag(cpu_flag::CARRY, false);

                log::debug!(
                    "INT 25h: Successfully read {} bytes to {:05X}",
                    data.len(),
                    buffer_addr
                );
            }
            Err(error_code) => {
                // Set error code in AL and set carry flag
                self.ax = (self.ax & 0xFF00) | (error_code as u16);
                self.set_flag(cpu_flag::CARRY, true);

                log::warn!(
                    "INT 25h: Read failed - drive={}, error={}",
                    drive,
                    error_code
                );
            }
        }
    }
}
