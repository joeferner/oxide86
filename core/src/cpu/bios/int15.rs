use crate::{
    Bus, CpuType,
    cpu::{Cpu, cpu_flag},
};

impl Cpu {
    pub(super) fn handle_int15(&mut self, bus: &mut Bus, cpu_type: CpuType) {
        
        match function {
            0x24 => self.int15_a20_gate(bus),
            0x86 => self.int15_wait(),
            0x87 => self.int15_move_extended_memory(bus),
            
           
            0x53 => self.int15_apm_not_present(),
            0xD8 => self.int15_eisa_not_supported(),
         
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
    fn int15_a20_gate(&mut self, bus: &mut Bus) {
        let sub_function = (self.ax & 0xFF) as u8;
        match sub_function {
            0x00 => {
                bus.memory_mut().set_a20_enabled(false);
                self.ax &= 0x00FF; // AH = 0
                self.set_flag(cpu_flag::CARRY, false);
                log::debug!("INT 15h AH=24h AL=00: A20 disabled");
            }
            0x01 => {
                bus.memory_mut().set_a20_enabled(true);
                self.ax &= 0x00FF; // AH = 0
                self.set_flag(cpu_flag::CARRY, false);
                log::debug!("INT 15h AH=24h AL=01: A20 enabled");
            }
            0x02 => {
                let enabled = bus.memory().is_a20_enabled();
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
    fn int15_wait(&mut self) {
        let wait_time_high = self.cx as u64;
        let wait_time_low = self.dx as u64;
        let wait_microseconds = (wait_time_high << 16) | wait_time_low;

        // Calculate cycles to simulate for the wait
        // Formula: cycles = microseconds * (cpu_freq / 1_000_000)
        let wait_cycles = (wait_microseconds * self.cpu_freq) / 1_000_000;

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


    /// INT 15h AH=87h - Move Extended Memory Block
    ///
    /// Copies a block of memory between conventional and extended memory using a
    /// descriptor table. On real hardware this temporarily enters protected mode;
    /// here we simply resolve the addresses from the descriptor table and memcpy.
    ///
    /// Input:
    ///   AH = 87h
    ///   CX = number of words to copy
    ///   ES:SI = pointer to 48-byte descriptor table (6 × 8-byte entries):
    ///     Entry 0 (offset  0): Null descriptor
    ///     Entry 1 (offset  8): GDT self-descriptor
    ///     Entry 2 (offset 16): Source descriptor  (base at bytes 2-4)
    ///     Entry 3 (offset 24): Destination descriptor (base at bytes 2-4)
    ///     Entry 4 (offset 32): BIOS code descriptor
    ///     Entry 5 (offset 40): BIOS stack descriptor
    ///
    /// Descriptor base address format (286-style, 3 bytes):
    ///   byte 2 = base[7:0], byte 3 = base[15:8], byte 4 = base[23:16]
    ///
    /// Output:
    ///   AH = 0, CF = 0 on success
    fn int15_move_extended_memory(&mut self, bus: &mut Bus) {
        let word_count = self.cx as usize;
        let table_phys = ((self.es as usize) << 4) + self.si as usize;

        // Read 24-bit base from descriptor entry at given table offset.
        // The descriptor table itself is in conventional memory, so use read_u8.
        let read_base = |bus: &Bus, entry_offset: usize| -> usize {
            let lo = bus.read_u8(table_phys + entry_offset + 2) as usize;
            let mid = bus.read_u8(table_phys + entry_offset + 3) as usize;
            let hi = bus.read_u8(table_phys + entry_offset + 4) as usize;
            lo | (mid << 8) | (hi << 16)
        };

        let src_base = read_base(bus, 16); // Entry 2
        let dst_base = read_base(bus, 24); // Entry 3

        log::debug!(
            "INT 15h AH=87h: Move {} words from 0x{:06X} to 0x{:06X}",
            word_count,
            src_base,
            dst_base
        );

        // Copy word_count * 2 bytes. Addresses may be above 1 MB (extended
        // memory) so use read/write_physical_u8 which bypass the A20 gate.
        let byte_count = word_count * 2;
        // Read all source bytes first to handle overlapping moves correctly
        let buf: Vec<u8> = (0..byte_count)
            .map(|i| bus.memory().read_physical_u8(src_base + i))
            .collect();
        for (i, byte) in buf.into_iter().enumerate() {
            bus.memory_mut().write_physical_u8(dst_base + i, byte);
        }

        self.ax &= 0x00FF; // AH = 0 (success)
        self.set_flag(cpu_flag::CARRY, false);
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
}
