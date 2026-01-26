// INT 2Ah - DOS Network Interface / Microsoft Networks
// Used for network-related operations and critical section management.
// Programs check this interrupt to determine if network software is installed.

use super::Cpu;

impl Cpu {
    /// INT 2Ah - DOS Network Interface
    /// AH register contains the function number
    ///
    /// This interrupt is used by Microsoft Networks (MS-NET, LAN Manager)
    /// for network operations and critical section management.
    pub(super) fn handle_int2a(&mut self) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int2a_installation_check(),
            0x01 => self.int2a_execute_netbios(),
            0x02 => self.int2a_set_net_printer(),
            0x03 => self.int2a_check_direct_io(),
            0x04 => self.int2a_execute_netbios_no_wait(),
            0x05 => self.int2a_get_network_resource(),
            0x06 => self.int2a_set_network_resource(),
            0x80 => self.int2a_begin_critical_section(),
            0x81 => self.int2a_end_critical_section(),
            0x82 => self.int2a_end_all_critical_sections(),
            0x84 => self.int2a_keyboard_busy_loop(),
            _ => {
                // Unknown function - silently ignore or log
                log::warn!("Unhandled INT 2Ah function: AH=0x{:02X}", function);
            }
        }
    }

    /// AH=00h - Installation Check
    /// Output: AH = 00h if not installed
    ///         AH = non-zero if installed (network software present)
    fn int2a_installation_check(&mut self) {
        // Return AH=0x00 (network not installed)
        self.ax &= 0x00FF;
    }

    /// AH=01h - Execute NetBIOS Request (wait for completion)
    /// Input: ES:BX = pointer to Network Control Block (NCB)
    /// Output: AL = return code
    fn int2a_execute_netbios(&mut self) {
        // Return AL=0x00 (success/no network)
        // Network not installed, return immediately
        self.ax &= 0xFF00;
    }

    /// AH=02h - Set Network Printer Mode
    /// Input: AL = mode
    fn int2a_set_net_printer(&mut self) {
        // No-op: network not installed
    }

    /// AH=03h - Check Direct I/O
    /// Used to check if direct disk I/O is allowed
    /// Output: CF clear if allowed, set if not
    fn int2a_check_direct_io(&mut self) {
        // Direct I/O is always allowed (no network restrictions)
        self.set_flag(super::super::cpu_flag::CARRY, false);
    }

    /// AH=04h - Execute NetBIOS Request (no wait)
    /// Input: ES:BX = pointer to Network Control Block (NCB)
    /// Output: AL = return code
    fn int2a_execute_netbios_no_wait(&mut self) {
        // Return AL=0x00 (success/no network)
        self.ax &= 0xFF00;
    }

    /// AH=05h - Get Network Resource Entry
    /// Used to enumerate network resources
    fn int2a_get_network_resource(&mut self) {
        // No resources available
        self.set_flag(super::super::cpu_flag::CARRY, true);
    }

    /// AH=06h - Set Network Resource Entry
    fn int2a_set_network_resource(&mut self) {
        // No-op: network not installed
        self.set_flag(super::super::cpu_flag::CARRY, true);
    }

    /// AH=80h - Begin DOS Critical Section
    /// Input: AL = critical section number (01h-0Fh)
    /// Used by networks to prevent DOS reentrancy during network operations
    fn int2a_begin_critical_section(&mut self) {
        // No-op: no network software to synchronize with
        // Critical sections are used by TSRs and networks to prevent
        // DOS from being reentered. Since we don't have these, just return.
    }

    /// AH=81h - End DOS Critical Section
    /// Input: AL = critical section number (01h-0Fh)
    fn int2a_end_critical_section(&mut self) {
        // No-op: no network software to synchronize with
    }

    /// AH=82h - End All DOS Critical Sections
    /// Releases all critical sections held by the caller
    fn int2a_end_all_critical_sections(&mut self) {
        // No-op: no critical sections to release
    }

    /// AH=84h - Keyboard Busy Loop
    /// Called by DOS while waiting for keyboard input
    /// Networks can use this to perform background processing
    fn int2a_keyboard_busy_loop(&mut self) {
        // No-op: no background network processing needed
    }
}
