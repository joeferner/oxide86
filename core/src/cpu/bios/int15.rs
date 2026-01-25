use log::warn;
use crate::{cpu::{Cpu, FLAG_CARRY}, memory::Memory};

impl Cpu {
    pub(super) fn handle_int15<T: super::Bios>(
        &mut self,
        memory: &mut Memory,
        _io: &mut T,
    ) {
        let function = (self.ax >> 8) as u8; // Get AH
        match function {
            0x86 => self.int15_wait(memory),
            0x88 => self.int15_get_extended_memory(),
            0xC0 => self.int15_get_system_config(memory),
            _ => {
                warn!("Unhandled INT 0x15 function: AH=0x{:02X}", function);
                // Set carry flag to indicate function not supported
                self.set_flag(FLAG_CARRY, true);
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
    /// Note: In this emulator, we don't actually delay - we just return success
    fn int15_wait(&mut self, _memory: &mut Memory) {
        let _wait_time_high = self.cx;
        let _wait_time_low = self.dx;

        // In a real system, this would wait for CX:DX microseconds
        // For emulation purposes, we just return success immediately
        self.set_flag(FLAG_CARRY, false);
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
        self.set_flag(FLAG_CARRY, false);
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
            0x08, 0x00,  // Length: 8 bytes (not including length field)
            0xFF,        // Model byte: 0xFF = PC
            0x00,        // Submodel: 0 = PC
            0x01,        // BIOS revision: 1
            0x00,        // Feature byte 1: no special features
            0x00,        // Feature byte 2
            0x00,        // Feature byte 3
            0x00,        // Feature byte 4
            0x00,        // Feature byte 5
        ];

        // Write table to memory
        let physical_addr = ((table_segment as usize) << 4) + table_offset as usize;
        for (i, &byte) in table.iter().enumerate() {
            memory.write_byte(physical_addr + i, byte);
        }

        // Return pointer in ES:BX
        self.es = table_segment;
        self.bx = table_offset;
        self.set_flag(FLAG_CARRY, false);
    }
}
