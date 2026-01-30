// INT 20h - Program Terminate
// This is the original DOS program termination interrupt.
// CS must contain the PSP segment when this interrupt is called.

use super::Cpu;

use crate::memory::Memory;

impl Cpu {
    /// INT 20h - Program Terminate
    /// Terminates the current program and returns control to the parent process.
    /// Note: CS must contain the PSP segment (this is handled automatically by
    /// COM programs since CS=PSP at start)
    pub(super) fn handle_int20<K: crate::KeyboardInput>(
        &mut self,
        memory: &Memory,
        io: &mut super::Bios<K>,
    ) {
        // INT 20h terminates by jumping to the address stored at PSP:0x0A (terminate address)
        // CS should contain the PSP segment
        let psp_segment = self.cs;
        let terminate_offset_addr = Self::physical_address(psp_segment, 0x0A);
        let terminate_ip = memory.read_u16(terminate_offset_addr);
        let terminate_cs = memory.read_u16(terminate_offset_addr + 2);

        log::info!(
            "INT 20h: Terminating from PSP {:04X}, jumping to {:04X}:{:04X}",
            psp_segment,
            terminate_cs,
            terminate_ip
        );

        // Restore parent's PSP
        let parent_psp_addr = Self::physical_address(psp_segment, 0x16);
        let parent_psp = memory.read_u16(parent_psp_addr);
        if parent_psp != 0 {
            io.set_psp(parent_psp);
        }

        // Jump to the terminate address
        if terminate_cs == 0 && terminate_ip == 0 {
            // No return address - halt the CPU (top-level program)
            self.halted = true;
        } else {
            // Return to parent program
            self.cs = terminate_cs;
            self.ip = terminate_ip;
        }
    }
}
