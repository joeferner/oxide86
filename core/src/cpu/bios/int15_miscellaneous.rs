use crate::{
    bus::Bus,
    cpu::{
        Cpu,
        bios::{
            INT15_SYSTEM_CONFIG_OFFSET, INT15_SYSTEM_CONFIG_SEGMENT,
            bda::{bda_clear_ps2_mouse_handler, bda_set_ps2_mouse_handler},
        },
        cpu_flag,
    },
};

impl Cpu {
    /// INT 0x15 - Miscellaneous System Services
    /// AH register contains the function number
    pub(in crate::cpu) fn handle_int15_miscellaneous(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(200);
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x10 => self.int15_top_view_multi_dos(),
            0x41 => self.int15_wait_external_event(),
            0x4F => self.int15_keyboard_intercept(),
            0x53 => self.int15_apm_not_present(),
            0x88 => self.int15_get_extended_memory(bus),
            0x91 => self.int15_device_interrupt_complete(),
            0xC0 => self.int15_get_system_config(),
            0xC1 => self.int15_get_ebda_segment(),
            0xC2 => self.int15_ps2_mouse_services(bus),
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

    /// INT 15h AH=53h - APM (Advanced Power Management) Interface
    ///
    /// Output:
    ///   CF = 1, AH = 86h (function not supported)
    ///
    /// APM is not present in this emulation; programs that check for APM
    /// should gracefully fall back to non-APM operation.
    fn int15_apm_not_present(&mut self) {
        self.ax = (self.ax & 0x00FF) | 0x8600; // AH = 0x86 (function not supported)
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

    // ── INT 15h AH=C2h — PS/2 Mouse BIOS Services ───────────────────────────

    /// INT 15h AH=C2h - PS/2 Mouse BIOS Services
    ///
    /// AL = subfunction number.  Mouse handler far-pointer is stored in the
    /// BDA extension area (0x0040:EE–F2) so any code, not just the BIOS, can
    /// locate it if needed.
    fn int15_ps2_mouse_services(&mut self, bus: &mut Bus) {
        let subfunction = self.ax as u8; // AL

        match subfunction {
            0x00 => self.int15_ps2_mouse_enable_disable(bus),
            0x01 => self.int15_ps2_mouse_reset(bus),
            0x02 => self.int15_ps2_mouse_set_sample_rate(),
            0x03 => self.int15_ps2_mouse_set_resolution(),
            0x04 => self.int15_ps2_mouse_get_type(),
            0x05 => self.int15_ps2_mouse_initialize(bus),
            0x06 => self.int15_ps2_mouse_extended_commands(),
            0x07 => self.int15_ps2_mouse_set_handler(bus),
            _ => {
                log::warn!(
                    "INT 15h AH=C2h: unhandled subfunction AL=0x{:02X}",
                    subfunction
                );
                self.ax = (self.ax & 0x00FF) | 0x0100; // AH = 0x01 (invalid function)
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 15h AH=C2h AL=00h — Enable/Disable Mouse
    ///
    /// Input:  BH = 0x00 disable, 0x01 enable
    /// Output: CF = 0, AH = 0 on success
    fn int15_ps2_mouse_enable_disable(&mut self, bus: &mut Bus) {
        let enable = (self.bx >> 8) as u8 == 0x01; // BH
        bus.keyboard_controller_mut().set_aux_enabled(enable);
        self.ax &= 0x00FF; // AH = 0
        self.set_flag(cpu_flag::CARRY, false);
        log::debug!(
            "INT 15h AH=C2h AL=00h: PS/2 mouse {}",
            if enable { "enabled" } else { "disabled" }
        );
    }

    /// INT 15h AH=C2h AL=01h — Reset Mouse
    ///
    /// Output: CF = 0, AH = 0, BX = 0x00AA (reset OK), CL = 0x00 (standard mouse)
    fn int15_ps2_mouse_reset(&mut self, bus: &mut Bus) {
        bus.keyboard_controller_mut().set_aux_enabled(false);
        bda_clear_ps2_mouse_handler(bus);
        self.ax = 0x0000; // AH = 0
        self.bx = 0x00AA; // reset-complete sentinel
        self.cx = 0x0000; // CL = 0x00: standard PS/2 mouse device ID
        self.set_flag(cpu_flag::CARRY, false);
        log::debug!("INT 15h AH=C2h AL=01h: PS/2 mouse reset");
    }

    /// INT 15h AH=C2h AL=02h — Set Sample Rate
    ///
    /// Input:  BH = rate index (0–6 → 10/20/40/60/80/100/200 samples/s)
    /// Output: CF = 0, AH = 0
    fn int15_ps2_mouse_set_sample_rate(&mut self) {
        self.ax &= 0x00FF; // AH = 0
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 15h AH=C2h AL=03h — Set Resolution
    ///
    /// Input:  BH = resolution (0 = 1/mm, 1 = 2/mm, 2 = 4/mm, 3 = 8/mm)
    /// Output: CF = 0, AH = 0
    fn int15_ps2_mouse_set_resolution(&mut self) {
        self.ax &= 0x00FF; // AH = 0
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 15h AH=C2h AL=04h — Get Device Type
    ///
    /// Output: CF = 0, AH = 0, BH = 0x00 (standard PS/2 mouse)
    fn int15_ps2_mouse_get_type(&mut self) {
        self.ax &= 0x00FF; // AH = 0
        self.bx &= 0x00FF; // BH = 0x00: standard mouse
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 15h AH=C2h AL=05h — Initialize Mouse
    ///
    /// Input:  BH = packet size (must be 3 for a standard mouse)
    /// Output: CF = 0, AH = 0 on success; CF = 1, AH = 0x02 for bad packet size
    fn int15_ps2_mouse_initialize(&mut self, bus: &mut Bus) {
        let packet_size = (self.bx >> 8) as u8; // BH
        if packet_size != 3 {
            self.ax = (self.ax & 0x00FF) | 0x0200; // AH = 0x02 (invalid input)
            self.set_flag(cpu_flag::CARRY, true);
            return;
        }
        bda_clear_ps2_mouse_handler(bus);
        self.ax &= 0x00FF; // AH = 0
        self.set_flag(cpu_flag::CARRY, false);
        log::debug!("INT 15h AH=C2h AL=05h: PS/2 mouse initialized (packet size 3)");
    }

    /// INT 15h AH=C2h AL=06h — Extended Commands
    ///
    /// BH=00h: return status → BX = status, CX = resolution, DX = sample rate
    /// BH=01h: set 1:1 scaling
    /// BH=02h: set 2:1 scaling
    fn int15_ps2_mouse_extended_commands(&mut self) {
        let sub = (self.bx >> 8) as u8; // BH
        match sub {
            0x00 => {
                self.bx = 0x0000; // status flags
                self.cx = 0x0002; // 4 counts/mm resolution
                self.dx = 0x0064; // 100 samples/sec
                self.ax &= 0x00FF;
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x01 | 0x02 => {
                // Set 1:1 or 2:1 scaling — accept silently
                self.ax &= 0x00FF;
                self.set_flag(cpu_flag::CARRY, false);
            }
            _ => {
                log::warn!(
                    "INT 15h AH=C2h AL=06h: unhandled extended command BH=0x{:02X}",
                    sub
                );
                self.ax = (self.ax & 0x00FF) | 0x0100; // AH = 0x01
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 15h AH=C2h AL=07h — Set Mouse Handler Address
    ///
    /// Input:  ES:BX = far pointer to handler, CX = event mask
    /// Output: CF = 0, AH = 0
    ///
    /// The handler is called with a virtual FAR CALL when INT 74h fires:
    ///   AL = PS/2 status byte, BL = ΔX, CL = ΔY, DL = ΔZ, AH = 0
    fn int15_ps2_mouse_set_handler(&mut self, bus: &mut Bus) {
        let seg = self.es;
        let off = self.bx;
        let mask = self.cx as u8;
        bda_set_ps2_mouse_handler(bus, seg, off, mask);
        self.ax &= 0x00FF; // AH = 0
        self.set_flag(cpu_flag::CARRY, false);
        log::debug!(
            "INT 15h AH=C2h AL=07h: PS/2 mouse handler {:04X}:{:04X} mask=0x{:02X}",
            seg,
            off,
            mask
        );
    }
}
