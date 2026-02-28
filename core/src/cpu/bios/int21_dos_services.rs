use crate::{cpu::Cpu, io_bus::IoBus, memory_bus::MemoryBus, physical_address};

impl Cpu {
    pub(in crate::cpu) fn handle_int21_dos_services(
        &mut self,
        memory_bus: &mut MemoryBus,
        io_bus: &mut IoBus,
    ) {
        let function = (self.ax >> 8) as u8; // Get AH directly
        match function {
            0x09 => self.int21_write_string(memory_bus, io_bus),
            0x4c => self.int21_exit(),
            _ => log::warn!("Unhandled INT 0x21 function: AH=0x{function:02X}"),
        }
    }

    /// INT 21h, AH=09h - Write String to STDOUT
    /// Input: DS:DX = pointer to '$'-terminated string
    fn int21_write_string(&mut self, memory_bus: &mut MemoryBus, io_bus: &mut IoBus) {
        let mut addr = physical_address(self.ds, self.dx);
        let saved_ax = self.ax;

        loop {
            let ch = memory_bus.read_u8(addr);
            if ch == b'$' {
                break;
            }
            // Use teletype output for each character
            self.ax = (self.ax & 0xFF00) | (ch as u16);
            self.int10_teletype_output(memory_bus, io_bus);
            addr += 1;
        }

        self.ax = saved_ax;
    }

    /// INT 21h, AH=4Ch - Exit Program
    /// Input: AL = return code
    fn int21_exit(&mut self) {
        // TODO when running a single program from command line halt, when running dos exit properly
        self.halted = true;
    }
}
