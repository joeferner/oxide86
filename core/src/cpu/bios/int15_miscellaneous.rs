use crate::{
    bus::Bus,
    cpu::{
        Cpu,
        bios::{INT15_SYSTEM_CONFIG_OFFSET, INT15_SYSTEM_CONFIG_SEGMENT},
        cpu_flag,
    },
};

impl Cpu {
    /// INT 0x15 - Miscellaneous System Services
    /// AH register contains the function number
    pub(in crate::cpu) fn handle_int15_miscellaneous(&mut self, bus: &mut Bus) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x41 => self.int15_wait_external_event(),
            0x88 => self.int15_get_extended_memory(bus),
            0xC0 => self.int15_get_system_config(),
            _ => {
                log::warn!("Unhandled INT 0x15 function: AH=0x{:02X}", function);
                // Set carry flag to indicate function not supported
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 15h AH=41h - Wait for External Event (PS/2)
    ///
    /// Input:
    ///   AL = event type to wait for
    ///
    /// Output:
    ///   CF = 1 (function not supported on 8086)
    ///
    /// Note: This is a PS/2-specific function that is not available on 8086 systems.
    /// The 8086 predates PS/2, so this function returns "not supported".
    fn int15_wait_external_event(&mut self) {
        // TODO support this for newer processors
        // This is a PS/2 function not available on 8086 systems
        // Return function not supported
        self.set_flag(cpu_flag::CARRY, true);
    }

    /// INT 15h AH=88h - Get Extended Memory Size
    ///
    /// Output:
    ///   AX = number of contiguous 1KB blocks of memory above 1MB
    ///   CF = 0 if successful, 1 if error
    ///
    /// Note: 8086 can only address 1MB, so this returns 0 for an 8086 system.
    /// 286+ systems return the amount of extended memory available.
    fn int15_get_extended_memory(&mut self, bus: &Bus) {
        // Cap reported extended memory by both what the CPU supports and what is installed
        let cpu_max = self.cpu_type.max_extended_memory_kb();
        let installed = bus.extended_memory_kb();
        let extended_memory_kb = cpu_max.min(installed);

        self.ax = extended_memory_kb;
        self.set_flag(cpu_flag::CARRY, false);
        log::info!(
            "INT 15h AH=88h: Returning extended memory size = {} KB ({} CPU)",
            extended_memory_kb,
            self.cpu_type.name()
        );
    }

    /// INT 15h AH=C0h - Get System Configuration Parameters
    ///
    /// Output:
    ///   ES:BX = pointer to system descriptor table
    ///   CF = 0 if successful, 1 if not supported
    ///
    /// System Descriptor Table format:
    ///   Offset 0-1: Table length in bytes (not including these 2 bytes)
    ///   Offset 2: Model byte (0xFF for PC, 0xFE for XT, 0xFC for AT)
    ///   Offset 3: Submodel byte
    ///   Offset 4: BIOS revision level
    ///   Offset 5: Feature information byte 1
    ///   Offset 6: Feature information byte 2
    ///   Offset 7: Feature information byte 3
    ///   Offset 8: Feature information byte 4
    ///   Offset 9: Feature information byte 5
    fn int15_get_system_config(&mut self) {
        // Table was written to ROM area at reset; just return the pointer
        self.es = INT15_SYSTEM_CONFIG_SEGMENT;
        self.bx = INT15_SYSTEM_CONFIG_OFFSET;
        self.set_flag(cpu_flag::CARRY, false);
    }
}
