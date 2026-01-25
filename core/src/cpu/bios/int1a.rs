use log::warn;

use crate::Bios;
use crate::cpu::cpu_flag;
use crate::memory::{BDA_START, BDA_TIMER_COUNTER, BDA_TIMER_OVERFLOW};
use crate::{cpu::Cpu, memory::Memory};

/// Timer ticks at 18.2 Hz (PIT channel 0 frequency)
/// Ticks per day = 24 * 60 * 60 * 18.2 = 1,573,040 (0x1800B0)
/// This constant can be used to detect midnight rollovers
#[allow(dead_code)]
pub const TIMER_TICKS_PER_DAY: u32 = 0x001800B0;

impl Cpu {
    /// INT 0x1A - Time Services
    /// AH register contains the function number
    pub(super) fn handle_int1a<T: super::Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int1a_get_system_time(memory),
            0x01 => self.int1a_set_system_time(memory),
            0x02 => self.int1a_read_rtc_time(io),
            0x04 => self.int1a_read_rtc_date(io),
            _ => {
                warn!("Unhandled INT 0x1A function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 1Ah, AH=00h - Get System Time
    /// Reads the system timer tick counter
    /// Input: None
    /// Output:
    ///   CX:DX = number of clock ticks since midnight (CX = high word, DX = low word)
    ///   AL = midnight flag (non-zero if midnight passed since last read, then flag is reset)
    fn int1a_get_system_time(&mut self, memory: &mut Memory) {
        // Read timer counter from BDA (4 bytes, little-endian)
        let counter_addr = BDA_START + BDA_TIMER_COUNTER;
        let low_word = memory.read_word(counter_addr);
        let high_word = memory.read_word(counter_addr + 2);

        // Read and clear midnight flag
        let overflow_addr = BDA_START + BDA_TIMER_OVERFLOW;
        let midnight_flag = memory.read_byte(overflow_addr);
        memory.write_byte(overflow_addr, 0); // Clear the flag

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
    fn int1a_set_system_time(&mut self, memory: &mut Memory) {
        let high_word = self.cx;
        let low_word = self.dx;

        // Write timer counter to BDA (4 bytes, little-endian)
        let counter_addr = BDA_START + BDA_TIMER_COUNTER;
        memory.write_word(counter_addr, low_word); // Low word
        memory.write_word(counter_addr + 2, high_word); // High word

        // Clear midnight overflow flag when setting time
        let overflow_addr = BDA_START + BDA_TIMER_OVERFLOW;
        memory.write_byte(overflow_addr, 0);
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
    fn int1a_read_rtc_time<T: Bios>(&mut self, io: &T) {
        match io.get_rtc_time() {
            Some(time) => {
                // Convert decimal values to BCD format
                let hours_bcd = Self::decimal_to_bcd(time.hours);
                let minutes_bcd = Self::decimal_to_bcd(time.minutes);
                let seconds_bcd = Self::decimal_to_bcd(time.seconds);

                // Set output registers
                self.cx = ((hours_bcd as u16) << 8) | (minutes_bcd as u16); // CH = hours, CL = minutes
                self.dx = ((seconds_bcd as u16) << 8) | (time.dst_flag as u16); // DH = seconds, DL = DST flag

                // Clear CF to indicate success
                self.set_flag(cpu_flag::CARRY, false);
            }
            None => {
                // RTC not available - set CF to indicate failure
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
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
    fn int1a_read_rtc_date<T: Bios>(&mut self, io: &T) {
        match io.get_rtc_date() {
            Some(date) => {
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
            None => {
                // RTC not available - set CF to indicate failure
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// Convert a decimal value (0-99) to BCD format
    /// BCD stores each decimal digit in 4 bits: 23 decimal = 0x23 BCD
    fn decimal_to_bcd(value: u8) -> u8 {
        ((value / 10) << 4) | (value % 10)
    }
}
