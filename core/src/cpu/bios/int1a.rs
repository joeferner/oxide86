use crate::Bus;
use crate::cpu::Cpu;
use crate::cpu::cpu_flag;
use crate::memory::{BDA_START, BDA_TIMER_COUNTER, BDA_TIMER_OVERFLOW};

impl Cpu {
    /// INT 0x1A - Time Services
    /// AH register contains the function number
    ///
    /// Note: Like INT 0x13, we enable interrupts (STI) during time services so that
    /// timer IRQs (INT 0x08) can fire. This is important for programs that poll
    /// the system time in a tight loop waiting for it to change.
    pub(super) fn handle_int1a(&mut self, bus: &mut Bus, io: &mut super::Bios) {
        // Enable interrupts during time services (allows timer IRQs to fire)
        self.set_flag(cpu_flag::INTERRUPT, true);

        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int1a_get_system_time(bus),
            0x01 => self.int1a_set_system_time(bus),
            0x02 => self.int1a_read_rtc_time(io),
            0x04 => self.int1a_read_rtc_date(io),
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
        // Read timer counter from BDA (4 bytes, little-endian)
        let counter_addr = BDA_START + BDA_TIMER_COUNTER;
        let low_word = bus.read_u16(counter_addr);
        let high_word = bus.read_u16(counter_addr + 2);

        // Read and clear midnight flag
        let overflow_addr = BDA_START + BDA_TIMER_OVERFLOW;
        let midnight_flag = bus.read_u8(overflow_addr);
        bus.write_u8(overflow_addr, 0); // Clear the flag

        // Return values
        self.cx = high_word; // CX = high word of tick count
        self.dx = low_word; // DX = low word of tick count
        self.ax = (self.ax & 0xFF00) | (midnight_flag as u16); // AL = midnight flag
    }

    /// INT 1Ah, AH=01h - Set System Time
    /// Sets the system timer tick counter
    /// Input:
    ///   CX:DX = number of clock ticks since midnight (CX = high word, DX = low word)
    /// Output: None
    fn int1a_set_system_time(&mut self, bus: &mut Bus) {
        let high_word = self.cx;
        let low_word = self.dx;

        // Write timer counter to BDA (4 bytes, little-endian)
        let counter_addr = BDA_START + BDA_TIMER_COUNTER;
        bus.write_u16(counter_addr, low_word); // Low word
        bus.write_u16(counter_addr + 2, high_word); // High word

        // Clear midnight overflow flag when setting time
        let overflow_addr = BDA_START + BDA_TIMER_OVERFLOW;
        bus.write_u8(overflow_addr, 0);
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
    fn int1a_read_rtc_time(&mut self, io: &super::Bios) {
        let time = io.get_local_time();

        // Convert decimal values to BCD format
        let hours_bcd = Self::decimal_to_bcd(time.hours);
        let minutes_bcd = Self::decimal_to_bcd(time.minutes);
        let seconds_bcd = Self::decimal_to_bcd(time.seconds);

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
    fn int1a_read_rtc_date(&mut self, io: &super::Bios) {
        let date = io.get_local_date();

        // Convert decimal values to BCD format
        let century_bcd = Self::decimal_to_bcd(date.century);
        let year_bcd = Self::decimal_to_bcd(date.year);
        let month_bcd = Self::decimal_to_bcd(date.month);
        let day_bcd = Self::decimal_to_bcd(date.day);

        // Set output registers
        self.cx = ((century_bcd as u16) << 8) | (year_bcd as u16); // CH = century, CL = year
        self.dx = ((month_bcd as u16) << 8) | (day_bcd as u16); // DH = month, DL = day

        // Clear CF to indicate success
        self.set_flag(cpu_flag::CARRY, false);
    }

    /// Convert a decimal value (0-99) to BCD format
    /// BCD stores each decimal digit in 4 bits: 23 decimal = 0x23 BCD
    fn decimal_to_bcd(value: u8) -> u8 {
        ((value / 10) << 4) | (value % 10)
    }
}
