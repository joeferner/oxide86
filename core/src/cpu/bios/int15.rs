use crate::{
    cpu::{Cpu, cpu_flag},
    memory::Memory,
};

impl Cpu {
    pub(super) fn handle_int15<T: super::Bios>(&mut self, memory: &mut Memory, _io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH
        match function {
            0x10 => self.int15_topview_multidos(),
            0x41 => self.int15_wait_external_event(),
            0x86 => self.int15_wait(memory),
            0x88 => self.int15_get_extended_memory(),
            0xC0 => self.int15_get_system_config(memory),
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
    fn int15_topview_multidos(&mut self) {
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
    /// Note: In this emulator, we don't actually delay - we just return success
    fn int15_wait(&mut self, _memory: &mut Memory) {
        let _wait_time_high = self.cx;
        let _wait_time_low = self.dx;

        // In a real system, this would wait for CX:DX microseconds
        // For emulation purposes, we just return success immediately
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 15h AH=88h - Get Extended Memory Size
    ///
    /// Output:
    ///   AX = number of contiguous 1KB blocks of memory above 1MB
    ///   CF = 0 if successful, 1 if error
    ///
    /// Note: 8086 can only address 1MB, so this returns 0 for an 8086 system
    fn int15_get_extended_memory(&mut self) {
        // 8086 systems don't have extended memory (that's a 286+ feature)
        // Return 0 KB of extended memory
        self.ax = 0;
        self.set_flag(cpu_flag::CARRY, false);
        log::info!(
            "INT 15h AH=88h: Returning extended memory size = 0 KB (8086 has no extended memory)"
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
