use std::any::Any;

use crate::{
    Device,
    devices::pit::{PIT_DIVISOR, PIT_FREQUENCY_HZ},
};

pub const RTC_IO_PORT_REGISTER_SELECT: u16 = 0x0070;
pub const RTC_IO_PORT_DATA: u16 = 0x0071;

// RTC register indices (CMOS)
pub const RTC_REG_SECONDS: u8 = 0x00;
pub const RTC_REG_MINUTES: u8 = 0x02;
pub const RTC_REG_HOURS: u8 = 0x04;
pub const RTC_REG_DAY: u8 = 0x07;
pub const RTC_REG_MONTH: u8 = 0x08;
pub const RTC_REG_YEAR: u8 = 0x09;
pub const RTC_REG_CENTURY: u8 = 0x32;

/// Local time components with sub-second precision.
#[derive(Clone)]
pub struct LocalTime {
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub milliseconds: u16,
}

/// Local date components.
#[derive(Clone)]
pub struct LocalDate {
    pub century: u8,
    pub year: u8,
    pub month: u8,
    pub day: u8,
}

/// Platform-independent clock trait for time and date operations.
/// Native implementations use chrono, WASM uses js_sys::Date.
pub trait Clock: Send {
    /// Returns local time with sub-second precision
    fn get_local_time(&self) -> LocalTime;
    /// Returns local date with century
    fn get_local_date(&self) -> LocalDate;
}

pub struct RTC {
    clock: Box<dyn Clock>,
    /// Currently selected CMOS register index (written via port 0x70)
    selected_register: u8,
}

impl RTC {
    pub fn new(clock: Box<dyn Clock>) -> Self {
        Self {
            clock,
            selected_register: 0,
        }
    }

    /// Returns the BDA timer counter value (ticks since midnight at ~18.2 Hz).
    ///
    /// Computed as: total_milliseconds_since_midnight * PIT_FREQUENCY_HZ / (PIT_DIVISOR * MS_PER_SECOND)
    pub fn timer_counter(&self) -> u32 {
        const MS_PER_SECOND: u64 = 1_000;

        let time = self.clock.get_local_time();
        let total_ms = (time.hours as u64 * 3_600 + time.minutes as u64 * 60 + time.seconds as u64)
            * MS_PER_SECOND
            + time.milliseconds as u64;
        ((total_ms * PIT_FREQUENCY_HZ) / (PIT_DIVISOR * MS_PER_SECOND)) as u32
    }
}

fn to_bcd(val: u8) -> u8 {
    ((val / 10) << 4) | (val % 10)
}

impl Device for RTC {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.selected_register = 0;
    }

    fn memory_read_u8(&self, _addr: usize) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8) -> bool {
        false
    }

    fn io_read_u8(&self, port: u16) -> Option<u8> {
        if port != RTC_IO_PORT_DATA {
            return None;
        }

        let val = match self.selected_register {
            0x00 => to_bcd(self.clock.get_local_time().seconds),
            0x02 => to_bcd(self.clock.get_local_time().minutes),
            0x04 => to_bcd(self.clock.get_local_time().hours),
            0x07 => to_bcd(self.clock.get_local_date().day),
            0x08 => to_bcd(self.clock.get_local_date().month),
            0x09 => to_bcd(self.clock.get_local_date().year),
            // Status Register A: bit 7 = 0 (update not in progress)
            0x0A => 0x00,
            // Status Register B: bit 1 = 24h mode, BCD format (not binary)
            0x0B => 0x02,
            0x32 => to_bcd(self.clock.get_local_date().century),
            reg => {
                log::warn!("RTC: read from unimplemented CMOS register 0x{reg:02X}");
                0xFF
            }
        };

        Some(val)
    }

    fn io_write_u8(&mut self, port: u16, val: u8) -> bool {
        match port {
            RTC_IO_PORT_REGISTER_SELECT => {
                // Bit 7 is the NMI disable bit; mask it out to get the register index
                self.selected_register = val & 0x7F;
                true
            }
            RTC_IO_PORT_DATA => {
                // Writes to CMOS data are ignored; we use the real system clock
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::devices::rtc::{Clock, LocalDate, LocalTime};

    pub struct MockClock {
        local_time: LocalTime,
        local_date: LocalDate,
    }

    impl MockClock {
        pub fn new() -> Self {
            Self {
                local_time: LocalTime {
                    hours: 11,
                    minutes: 5,
                    seconds: 30,
                    milliseconds: 745,
                },
                local_date: LocalDate {
                    century: 20,
                    year: 26,
                    month: 3,
                    day: 2,
                },
            }
        }
    }

    impl Clock for MockClock {
        fn get_local_time(&self) -> LocalTime {
            self.local_time.clone()
        }

        fn get_local_date(&self) -> LocalDate {
            self.local_date.clone()
        }
    }
}
