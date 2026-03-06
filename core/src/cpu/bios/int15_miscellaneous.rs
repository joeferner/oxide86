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
            0x10 => self.int15_top_view_multi_dos(),
            0x41 => self.int15_wait_external_event(),
            0x4F => self.int15_keyboard_intercept(),
            0x88 => self.int15_get_extended_memory(bus),
            0x91 => self.int15_device_interrupt_complete(),
            0xC0 => self.int15_get_system_config(),
            0xC1 => self.int15_get_ebda_segment(),
            _ => {
                log::warn!("Unhandled INT 0x15 function: AH=0x{:02X}", function);
                // Set carry flag to indicate function not supported
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 15h AH=10h - TopView/MultiDOS Plus Vendor-Specific Function
    ///
    /// This function has different meanings depending on the environment:
    /// - TopView: UNIMPLEMENTED in DESQview 2.x
    /// - MultiDOS Plus: TEST RESOURCE SEMAPHORE
    ///
    /// Output:
    ///   CF = 1 (function not supported on standard 8086 BIOS)
    ///
    /// Note: This is a vendor-specific function not available on standard 8086 systems.
    /// Standard 8086 BIOS does not implement this function.
    fn int15_top_view_multi_dos(&mut self) {
        // This is a vendor-specific function (TopView/MultiDOS Plus)
        // not available on standard 8086 BIOS
        // Return function not supported
        self.set_flag(cpu_flag::CARRY, true);
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

    /// INT 15h AH=4Fh - Keyboard Intercept
    ///
    /// Input:
    ///   AL = scan code, CF = 1 (calling convention)
    ///
    /// Output:
    ///   CF = 1: key NOT intercepted → caller should buffer key in BDA
    ///   CF = 0: key intercepted/consumed → caller should discard key
    ///
    /// Called by INT 09h before buffering a keystroke. Multitaskers (DESQview, etc.)
    /// install a custom INT 15h to route keystrokes to the active task.
    /// The default (no interception) is CF=1: pass the key through to the BDA buffer.
    fn int15_keyboard_intercept(&mut self) {
        // Set CF to indicate key is NOT intercepted and should proceed to BDA buffer
        self.set_flag(cpu_flag::CARRY, true);
    }

    /// INT 15h AH=91h - Device Interrupt Complete
    ///
    /// Input:
    ///   AL = device type (0x01 = keyboard, 0x02 = keyboard in some implementations)
    ///
    /// Called by device interrupt handlers (e.g. IO.SYS INT 09h) to signal that a
    /// device interrupt has been fully serviced. Used by PS/2-class BIOS for
    /// post-interrupt processing. Not supported on standard AT-class hardware.
    fn int15_device_interrupt_complete(&mut self) {
        // Not supported on this system; caller should check CF and continue regardless
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

    /// INT 15h AH=C1h - Get Extended BIOS Data Area (EBDA) Segment Address
    ///
    /// Output:
    ///   ES = segment of EBDA
    ///   CF = 0 if successful, 1 if EBDA not present
    ///
    /// Note: The EBDA is a feature of AT-class and later machines.
    /// Original PC/XT systems (8086) do not have an EBDA, so this function
    /// returns CF=1 to indicate the function is not supported.
    fn int15_get_ebda_segment(&mut self) {
        // 8086/PC/XT systems do not have an Extended BIOS Data Area
        // Return function not supported
        self.set_flag(cpu_flag::CARRY, true);
        log::info!("INT 15h AH=C1h: EBDA not present (8086/PC/XT system)");
    }
}
