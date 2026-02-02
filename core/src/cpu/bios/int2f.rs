use crate::{cpu::Cpu, memory::Memory};

impl Cpu {
    /// INT 0x2F - DOS Multiplex Interrupt
    /// AH register contains the multiplex number (function group)
    /// AL often contains the subfunction
    ///
    /// This interrupt is used for inter-program communication and checking
    /// if various DOS features, TSRs, and extensions are installed.
    pub(super) fn handle_int2f(&mut self, _memory: &mut Memory, _io: &mut super::Bios) {
        let multiplex_num = (self.ax >> 8) as u8; // Get AH
        let subfunction = (self.ax & 0xFF) as u8; // Get AL

        match multiplex_num {
            0x11 => self.int2f_network_redirector(subfunction),
            0x12 => self.int2f_dos_internal(subfunction),
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
