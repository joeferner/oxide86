use crate::{Bus, cpu::Cpu};

impl Cpu {
    /// INT 0x2F - DOS Multiplex Interrupt
    /// AH register contains the multiplex number (function group)
    /// AL often contains the subfunction
    ///
    /// This interrupt is used for inter-program communication and checking
    /// if various DOS features, TSRs, and extensions are installed.
    pub(super) fn handle_int2f(&mut self, bus: &mut Bus, io: &mut super::Bios) {
        let multiplex_num = (self.ax >> 8) as u8; // Get AH
        let subfunction = (self.ax & 0xFF) as u8; // Get AL

        match multiplex_num {
            0x11 => self.int2f_network_redirector(subfunction),
            0x12 => self.int2f_dos_internal(subfunction),
            0x15 => self.int2f_mscdex(bus, io, subfunction),
            0x16 => self.int2f_windows_enhanced_mode(subfunction),
            0x43 => self.int2f_xms(subfunction),
            0x4A => self.int2f_hma_query(subfunction),
            0xB7 => self.int2f_append(subfunction),
            _ => {
                // For unknown multiplex numbers, return AL=0x00 (not installed)
                // This is the standard behavior for installation checks
                self.ax &= 0xFF00;
                log::warn!(
                    "Unhandled INT 0x2F multiplex: AH=0x{:02X}, AL=0x{:02X}",
                    multiplex_num,
                    subfunction
                );
            }
        }
    }

    /// AH=15h - MSCDEX (Microsoft CD-ROM Extension)
    /// Minimal shim so DOS programs can detect CD-ROM drives.
    ///
    /// AL=00h: Installation check → AX=0xADAD, BX=drive_count (or AX=0, BX=0)
    /// AL=0Bh: Drive check → AX=0xADAD, BX=0 if DOS drive is a CD-ROM, else AX=0
    /// AL=0Ch: Version → BX=0x0200, CX=0x0000
    /// AL=0Dh: Drive letters → write first DOS letter for each active slot to ES:BX
    fn int2f_mscdex(&mut self, bus: &mut Bus, io: &mut super::Bios, subfunction: u8) {
        let count = io.cdrom_count();
        // First CD-ROM DOS drive letter = 2 (A,B) + hard_drive_count + slot_index
        let hd_count = io.shared.drive_manager.hard_drive_count() as u8;
        let first_cdrom_dos = 2u8 + hd_count; // DOS drive index (0=A, 1=B, 2=C...)

        match subfunction {
            0x00 => {
                // Installation check
                if count > 0 {
                    self.ax = 0xADAD;
                    self.bx = count as u16;
                } else {
                    self.ax = 0x0000;
                    self.bx = 0x0000;
                }
            }
            0x0B => {
                // Drive check: BX = DOS drive index to check (0=A, 1=B, 2=C...)
                let dos_drive = self.bx as u8;
                let is_cdrom = dos_drive >= first_cdrom_dos
                    && dos_drive < first_cdrom_dos + 4
                    && io.has_cdrom(dos_drive - first_cdrom_dos);
                if is_cdrom {
                    self.ax = 0xADAD;
                    self.bx = 0x0000;
                } else {
                    self.ax = 0x0000;
                }
            }
            0x0C => {
                // Get MSCDEX version: 2.00
                self.bx = 0x0200;
                self.cx = 0x0000;
            }
            0x0D => {
                // Get drive letters: write one byte per slot to ES:BX buffer
                // Each byte is the DOS drive letter index (0=A, 1=B, 2=C...)
                let buf_addr = Self::physical_address(self.es, self.bx);
                let mut written = 0usize;
                for slot in 0u8..4 {
                    if io.has_cdrom(slot) {
                        bus.write_u8(buf_addr + written, first_cdrom_dos + slot);
                        written += 1;
                    }
                }
            }
            _ => {
                log::warn!("Unhandled INT 2Fh AH=15h (MSCDEX) AL=0x{:02X}", subfunction);
            }
        }
    }

    /// AH=11h - Network Redirector Interface
    /// Input: AL = subfunction
    /// Output: AL = 0x00 if not installed, non-zero if installed
    fn int2f_network_redirector(&mut self, subfunction: u8) {
        match subfunction {
            0x00 => {
                // Installation check
                // Return AL=0x00 (not installed)
                self.ax &= 0xFF00;
            }
            0x22 => {
                // Process termination hook
                // Called by DOS when a process terminates to allow
                // the network redirector to clean up network resources.
                // Input: DS = PSP segment of terminating process
                // Since we don't have network redirection, just return.
            }
            _ => {
                // Unknown subfunction - return not installed
                self.ax &= 0xFF00;
                log::warn!(
                    "Unhandled INT 0x2F AH=11h subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// AH=12h - DOS Internal Functions
    /// Input: AL = subfunction
    /// These are internal DOS functions used by SHARE, PRINT, etc.
    fn int2f_dos_internal(&mut self, subfunction: u8) {
        match subfunction {
            0x00 => {
                // Installation check
                // Return AL=0x00 (not installed)
                self.ax &= 0xFF00;
            }
            _ => {
                // Unknown subfunction - return not installed
                self.ax &= 0xFF00;
                log::warn!(
                    "Unhandled INT 0x2F AH=12h subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// AH=16h - Windows Enhanced Mode Installation Check
    /// Input: AL = subfunction
    /// Output: AL = 0x00 if not running under Windows, non-zero if running
    fn int2f_windows_enhanced_mode(&mut self, subfunction: u8) {
        match subfunction {
            0x00 | 0x05 | 0x06 | 0x07 => {
                // Installation/version checks
                // Return AL=0x00 (Windows not running)
                self.ax &= 0xFF00;
            }
            _ => {
                // Unknown subfunction - return not running
                self.ax &= 0xFF00;
                log::warn!(
                    "Unhandled INT 0x2F AH=16h subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// AH=43h - XMS (Extended Memory Specification) Installation Check
    /// Input: AL = 00h for installation check
    /// Output: AL = 0x80 if installed, 0x00 if not installed
    ///         If installed, ES:BX points to XMS entry point
    fn int2f_xms(&mut self, subfunction: u8) {
        match subfunction {
            0x00 => {
                // Installation check
                // Return AL=0x00 (XMS not installed)
                // 8086 doesn't support extended memory
                self.ax &= 0xFF00;
            }
            0x10 => {
                // Get XMS entry point
                // Return ES:BX = 0000:0000 (not available)
                self.ax &= 0xFF00;
                self.es = 0x0000;
                self.bx = 0x0000;
            }
            _ => {
                // Unknown subfunction - return not installed
                self.ax &= 0xFF00;
                log::warn!(
                    "Unhandled INT 0x2F AH=43h subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// AH=4Ah - HMA (High Memory Area) Query
    /// Input: AL = subfunction
    /// Output: Varies by subfunction
    fn int2f_hma_query(&mut self, subfunction: u8) {
        match subfunction {
            0x00 => {
                // Installation check for HIMEM.SYS
                // Return AL=0x00 (not installed)
                self.ax &= 0xFF00;
            }
            0x02 => {
                // Release HMA
                // Return AL=0x00 (HMA not allocated/not supported)
                // 8086 doesn't have HMA - it requires 80286+ with extended memory
                self.ax &= 0xFF00;
            }
            _ => {
                // Unknown subfunction - return not installed
                self.ax &= 0xFF00;
                log::warn!(
                    "Unhandled INT 0x2F AH=4Ah subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
    }

    /// AH=B7h - APPEND Installation Check
    /// Input: AL = 00h for installation check
    /// Output: AL = 0x00 if not installed, 0xFF if installed
    fn int2f_append(&mut self, subfunction: u8) {
        match subfunction {
            0x00 => {
                // Installation check
                // Return AL=0x00 (APPEND not installed)
                self.ax &= 0xFF00;
            }
            _ => {
                // Unknown subfunction - return not installed
                self.ax &= 0xFF00;
                log::warn!(
                    "Unhandled INT 0x2F AH=B7h subfunction: AL=0x{:02X}",
                    subfunction
                );
            }
        }
    }
}
