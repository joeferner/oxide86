use crate::{
    CpuType,
    cpu::{Cpu, cpu_flag},
    memory::Memory,
};

impl Cpu {
    pub(super) fn handle_int15(
        &mut self,
        memory: &mut Memory,
        _io: &mut super::Bios,
        cpu_type: CpuType,
    ) {
        let function = (self.ax >> 8) as u8; // Get AH
        match function {
            0x24 => self.int15_a20_gate(memory),
            0x10 => self.int15_top_view_multi_dos(),
            0x41 => self.int15_wait_external_event(),
            0x4F => self.int15_keyboard_intercept(),
            0x86 => self.int15_wait(memory),
            0x88 => {
                // Cap reported extended memory by both what the CPU supports and what is installed
                let cpu_max = cpu_type.max_extended_memory_kb();
                let installed = memory.extended_memory_kb();
                let extended_kb = cpu_max.min(installed);
                self.int15_get_extended_memory(cpu_type, extended_kb);
            }
            0xC0 => self.int15_get_system_config(memory),
            0xC1 => self.int15_get_ebda_segment(),
            0xD8 => self.int15_eisa_not_supported(),
            _ => {
                log::warn!("Unhandled INT 0x15 function: AH=0x{:02X}", function);
                // Set carry flag to indicate function not supported
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 15h AH=24h - A20 Gate Control (AT-class BIOS)
    ///
    /// Input:
    ///   AL = function:
    ///     00h = Disable A20
    ///     01h = Enable A20
    ///     02h = Query A20 status
    ///     03h = Query A20 support
    ///
    /// Output (AL=00/01):
    ///   AH = 0, CF = 0 on success
    ///
    /// Output (AL=02):
    ///   AH = 0, BL = 0 (disabled) or 1 (enabled), CF = 0
    ///
    /// Output (AL=03):
    ///   AH = 0, BX = 0xFFFF (loops supported), CF = 0
    fn int15_a20_gate(&mut self, memory: &mut Memory) {
        let sub_function = (self.ax & 0xFF) as u8;
        match sub_function {
            0x00 => {
                memory.set_a20_enabled(false);
                self.ax &= 0x00FF; // AH = 0
                self.set_flag(cpu_flag::CARRY, false);
                log::debug!("INT 15h AH=24h AL=00: A20 disabled");
            }
            0x01 => {
                memory.set_a20_enabled(true);
                self.ax &= 0x00FF; // AH = 0
                self.set_flag(cpu_flag::CARRY, false);
                log::debug!("INT 15h AH=24h AL=01: A20 enabled");
            }
            0x02 => {
                let enabled = memory.is_a20_enabled();
                self.ax &= 0x00FF; // AH = 0
                self.bx = (self.bx & 0xFF00) | u16::from(enabled); // BL = status
                self.set_flag(cpu_flag::CARRY, false);
                log::debug!("INT 15h AH=24h AL=02: A20 status = {}", enabled);
            }
            0x03 => {
                self.ax &= 0x00FF; // AH = 0
                self.bx = 0xFFFF; // max loops
                self.set_flag(cpu_flag::CARRY, false);
                log::debug!("INT 15h AH=24h AL=03: A20 support query");
            }
            _ => {
                log::warn!(
                    "INT 15h AH=24h: unknown sub-function AL=0x{:02X}",
                    sub_function
                );
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
        // This is a PS/2 function not available on 8086 systems
        // Return function not supported
        self.set_flag(cpu_flag::CARRY, true);
    }

    /// INT 15h AH=86h - Wait (microsecond delay)
    ///
    /// Input:
    ///   CX:DX = time to wait in microseconds
    ///
    /// Output:
    ///   CF = 0 if successful, 1 if not supported or interrupted
    ///
    /// Note: This emulates the wait by both accounting for CPU cycles AND
    /// sleeping in real time (on native platforms). This matches real hardware
    /// behavior where busy-waiting consumes both CPU cycles and wall clock time.
    fn int15_wait(&mut self, _memory: &mut Memory) {
        let wait_time_high = self.cx as u64;
        let wait_time_low = self.dx as u64;
        let wait_microseconds = (wait_time_high << 16) | wait_time_low;

        // Calculate cycles to simulate for the wait
        // At 4.77 MHz: exactly 4.77 cycles per microsecond
        // Formula: cycles = microseconds * 4.77 = (microseconds * 477) / 100
        let wait_cycles = (wait_microseconds * 477) / 100;

        // Request a busy-wait by setting pending_sleep_cycles
        // Computer::step() will burn these cycles without executing instructions
        // This matches real hardware behavior where busy-waiting consumes CPU time
        self.pending_sleep_cycles = wait_cycles;

        // Also account for INT instruction overhead (51 cycles)
        // This will be added separately by Computer::step()
        self.last_instruction_cycles = 51;

        log::debug!(
            "INT 15h AH=86h: Wait {} microseconds (~{} cycles)",
            wait_microseconds,
            wait_cycles
        );

        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 15h AH=88h - Get Extended Memory Size
    ///
    /// Output:
    ///   AX = number of contiguous 1KB blocks of memory above 1MB
    ///   CF = 0 if successful, 1 if error
    ///
    /// Note: 8086 can only address 1MB, so this returns 0 for an 8086 system.
    /// 286+ systems return the amount of extended memory available.
    fn int15_get_extended_memory(&mut self, cpu_type: CpuType, extended_memory_kb: u16) {
        self.ax = extended_memory_kb;
        self.set_flag(cpu_flag::CARRY, false);
        log::info!(
            "INT 15h AH=88h: Returning extended memory size = {} KB ({} CPU)",
            extended_memory_kb,
            cpu_type.name()
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
    fn int15_get_system_config(&mut self, memory: &mut Memory) {
        // System descriptor table location: we'll use a fixed location
        // Place it at 0xF000:0xE000 (in ROM BIOS area)
        let table_segment = 0xF000;
        let table_offset = 0xE000;

        // Build system descriptor table
        let table: [u8; 10] = [
            0x08, 0x00, // Length: 8 bytes (not including length field)
            0xFF, // Model byte: 0xFF = PC
            0x00, // Submodel: 0 = PC
            0x01, // BIOS revision: 1
            0x00, // Feature byte 1: no special features
            0x00, // Feature byte 2
            0x00, // Feature byte 3
            0x00, // Feature byte 4
            0x00, // Feature byte 5
        ];

        // Write table to memory
        let physical_addr = ((table_segment as usize) << 4) + table_offset as usize;
        for (i, &byte) in table.iter().enumerate() {
            memory.write_u8(physical_addr + i, byte);
        }

        // Return pointer in ES:BX
        self.es = table_segment;
        self.bx = table_offset;
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 15h AH=D8h - EISA System Functions (not supported)
    ///
    /// Output:
    ///   AH = 0x86 (function not supported)
    ///   CF = 1
    ///
    /// Note: EISA functions are only available on EISA-bus machines.
    /// ISA/AT systems return function not supported.
    fn int15_eisa_not_supported(&mut self) {
        self.ax = (self.ax & 0x00FF) | 0x8600; // AH = 0x86
        self.set_flag(cpu_flag::CARRY, true);
    }

    /// INT 15h AH=4Fh - Keyboard Intercept (IBM AT BIOS extension)
    ///
    /// Called by BIOS INT 09h handler before adding a key to the keyboard buffer.
    /// Allows TSRs and multitaskers to intercept keystrokes before BIOS buffers them.
    ///
    /// Input:
    ///   AH = 4Fh
    ///   AL = keyboard scan code
    ///   CF = 1 (set by INT 09h before calling)
    ///
    /// Output (default BIOS behavior):
    ///   AL = scan code (unchanged or modified)
    ///   CF = 0 → BIOS should buffer the key normally
    ///   CF = 1 → key has been handled; BIOS should NOT buffer it
    ///
    /// Note: This default BIOS implementation returns CF=0 (always buffer).
    /// Programs/TSRs that replace INT 15h can intercept keystrokes by returning CF=1.
    fn int15_keyboard_intercept(&mut self) {
        // Default BIOS behavior: proceed to buffer the key (CF=0)
        // AL (scan code) is unchanged
        self.set_flag(cpu_flag::CARRY, false);
        log::debug!(
            "INT 15h AH=4Fh: keyboard intercept scan=0x{:02X}, CF=0 (proceed to buffer)",
            self.ax as u8
        );
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
