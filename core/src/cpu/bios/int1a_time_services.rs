use crate::{
    bus::Bus,
    cpu::{
        Cpu,
        bios::bda::{bda_clear_timer_overflow, bda_get_system_time},
        cpu_flag,
    },
};

impl Cpu {
    /// INT 0x1A - Time Services
    /// AH register contains the function number
    ///
    /// Note: Like INT 0x13, we enable interrupts (STI) during time services so that
    /// timer IRQs (INT 0x08) can fire. This is important for programs that poll
    /// the system time in a tight loop waiting for it to change.
    pub(in crate::cpu) fn handle_int1a_time_services(&mut self, bus: &mut Bus) {
        // Enable interrupts during time services (allows timer IRQs to fire)
        self.set_flag(cpu_flag::INTERRUPT, true);

        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int1a_get_system_time(bus),
            _ => {
                log::warn!("Unhandled INT 0x1A function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 1Ah, AH=00h - Get System Time
    /// Reads the system timer tick counter
    /// Input: None
    /// Output:
    ///   CX:DX = number of clock ticks since midnight (CX = high word, DX = low word)
    ///   AL = midnight flag (non-zero if midnight passed since last read, then flag is reset)
    fn int1a_get_system_time(&mut self, bus: &mut Bus) {
        let system_time = bda_get_system_time(bus);
        bda_clear_timer_overflow(bus);

        // Return values
        self.cx = system_time.high_word; // CX = high word of tick count
        self.dx = system_time.low_word; // DX = low word of tick count
        self.ax = (self.ax & 0xFF00) | (system_time.midnight_flag as u16); // AL = midnight flag
    }
}
