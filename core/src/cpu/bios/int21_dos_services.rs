use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag},
    physical_address,
};

impl Cpu {
    pub(in crate::cpu) fn handle_int21_dos_services(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(500);
        // Enable interrupts during DOS services (DOS runs with IF=1)
        self.set_flag(cpu_flag::INTERRUPT, true);
        let function = (self.ax >> 8) as u8; // Get AH directly
        match function {
            0x02 => self.int21_write_char(bus),
            0x09 => self.int21_write_string(bus),
            0x4c => self.int21_exit(bus),
            _ => log::warn!("Unhandled INT 0x21 function: AH=0x{function:02X}"),
        }
    }

    /// INT 21h, AH=02h - Write Character to STDOUT
    /// Input: DL = character to write
    fn int21_write_char(&mut self, bus: &mut Bus) {
        let ch = self.get_reg8(2); // DL register
        // Use teletype output for proper screen handling
        let saved_ax = self.ax;
        self.ax = (self.ax & 0xFF00) | (ch as u16);
        self.int10_teletype_output(bus);
        self.ax = saved_ax;
    }

    /// INT 21h, AH=09h - Write String to STDOUT
    /// Input: DS:DX = pointer to '$'-terminated string
    fn int21_write_string(&mut self, bus: &mut Bus) {
        let mut addr = physical_address(self.ds, self.dx);
        let saved_ax = self.ax;

        loop {
            let ch = bus.memory_read_u8(addr);
            if ch == b'$' {
                break;
            }
            // Use teletype output for each character
            self.ax = (self.ax & 0xFF00) | (ch as u16);
            self.int10_teletype_output(bus);
            addr += 1;
        }

        self.ax = saved_ax;
    }

    /// INT 21h, AH=4Ch - Exit Program
    /// Input: AL = return code
    fn int21_exit(&mut self, bus: &mut Bus) {
        let return_code = (self.ax & 0xff) as u8;

        // Read the terminate address (INT 22h) from the PSP at offset 0x0A
        let psp_segment = self.current_psp;
        let terminate_offset_addr = physical_address(psp_segment, 0x0A);
        let terminate_ip = bus.memory_read_u16(terminate_offset_addr);
        let terminate_cs = bus.memory_read_u16(terminate_offset_addr + 2);

        log::info!(
            "INT 0x21 AH=0x4C: Terminating from PSP {:04X}, jumping to {:04X}:{:04X}",
            psp_segment,
            terminate_cs,
            terminate_ip
        );

        // Restore parent's PSP
        let parent_psp_addr = physical_address(psp_segment, 0x16);
        let parent_psp = bus.memory_read_u16(parent_psp_addr);
        if parent_psp != 0 {
            self.current_psp = parent_psp;
        }

        // Jump to the terminate address
        if terminate_cs == 0 && terminate_ip == 0 {
            // No return address - halt the CPU (top-level program).
            // Clear IF so that pending IRQs (e.g. timer) cannot wake the CPU
            // and resume execution after the INT 21h instruction.
            self.halted = true;
            self.set_flag(cpu_flag::INTERRUPT, false);
            self.exit_code = Some(return_code);
        } else {
            // Return to parent program
            self.cs = terminate_cs;
            self.ip = terminate_ip;
        }
    }
}
