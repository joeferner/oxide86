use crate::{
    bus::Bus,
    cpu::{
        Cpu,
        bios::bda::{
            bda_clear_midnight_overflow, bda_clear_timer_overflow, bda_get_system_time,
            bda_set_timer_counter,
        },
        cpu_flag,
    },
    devices::rtc::{RTC_IO_PORT_DATA, RTC_IO_PORT_REGISTER_SELECT},
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

        log::debug!("INT 0x1A 0x{function:02X}");

        match function {
            0x00 => self.int1a_get_system_time(bus),
            0x01 => self.int1a_set_system_time(bus),
            0x02 => self.int1a_read_rtc_time(bus),
            0x04 => self.int1a_read_rtc_date(bus),
            _ => {
                log::warn!("Unhandled INT 0x1A function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 1Ah, AH=01h - Set System Time
    /// Sets the system timer tick counter
    /// Input:
    ///   CX:DX = number of clock ticks since midnight (CX = high word, DX = low word)
    /// Output: None
    fn int1a_set_system_time(&mut self, bus: &mut Bus) {
        let high_word = self.cx;
        let low_word = self.dx;

        bda_set_timer_counter(bus, low_word, high_word);
        bda_clear_midnight_overflow(bus);
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

    /// INT 1Ah, AH=02h - Read Real Time Clock Time
    /// Reads the current time from the RTC (AT systems only, not available on original 8086/XT)
    /// Input: None
    /// Output:
    ///   CF = 0 if successful
    ///   CF = 1 if RTC not operating or not present
    ///   CH = hours (BCD format, 0-23)
    ///   CL = minutes (BCD format, 0-59)
    ///   DH = seconds (BCD format, 0-59)
    ///   DL = daylight saving time flag (0 = standard time, 1 = daylight time)
    fn int1a_read_rtc_time(&mut self, bus: &mut Bus) {
        if !bus.has_rtc() {
            self.set_flag(cpu_flag::CARRY, true);
            return;
        }

        // RTC register indices (CMOS)
        const RTC_REG_SECONDS: u8 = 0x00;
        const RTC_REG_MINUTES: u8 = 0x02;
        const RTC_REG_HOURS: u8 = 0x04;

        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, RTC_REG_SECONDS);
        let seconds_bcd = bus.io_read_u8(RTC_IO_PORT_DATA);

        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, RTC_REG_MINUTES);
        let minutes_bcd = bus.io_read_u8(RTC_IO_PORT_DATA);

        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, RTC_REG_HOURS);
        let hours_bcd = bus.io_read_u8(RTC_IO_PORT_DATA);

        // Set output registers
        self.cx = ((hours_bcd as u16) << 8) | (minutes_bcd as u16); // CH = hours, CL = minutes
        self.dx = (seconds_bcd as u16) << 8; // DH = seconds, DL = DST flag (0 = standard time)

        // Clear CF to indicate success
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// INT 1Ah, AH=04h - Read Real Time Clock Date
    /// Reads the current date from the RTC (AT systems only, not available on original 8086/XT)
    /// Input: None
    /// Output:
    ///   CF = 0 if successful
    ///   CF = 1 if RTC not operating or not present
    ///   CH = century (BCD format, e.g., 0x19 or 0x20)
    ///   CL = year (BCD format, 0-99)
    ///   DH = month (BCD format, 1-12)
    ///   DL = day (BCD format, 1-31)
    fn int1a_read_rtc_date(&mut self, bus: &mut Bus) {
        if !bus.has_rtc() {
            self.set_flag(cpu_flag::CARRY, true);
            return;
        }

        // RTC register indices (CMOS)
        const RTC_REG_DAY: u8 = 0x07;
        const RTC_REG_MONTH: u8 = 0x08;
        const RTC_REG_YEAR: u8 = 0x09;
        const RTC_REG_CENTURY: u8 = 0x32;

        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, RTC_REG_DAY);
        let day_bcd = bus.io_read_u8(RTC_IO_PORT_DATA);

        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, RTC_REG_MONTH);
        let month_bcd = bus.io_read_u8(RTC_IO_PORT_DATA);

        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, RTC_REG_YEAR);
        let year_bcd = bus.io_read_u8(RTC_IO_PORT_DATA);

        bus.io_write_u8(RTC_IO_PORT_REGISTER_SELECT, RTC_REG_CENTURY);
        let century_bcd = bus.io_read_u8(RTC_IO_PORT_DATA);

        // Set output registers
        self.cx = ((century_bcd as u16) << 8) | (year_bcd as u16); // CH = century, CL = year
        self.dx = ((month_bcd as u16) << 8) | (day_bcd as u16); // DH = month, DL = day

        // Clear CF to indicate success
        self.set_flag(cpu_flag::CARRY, false);
    }
}
