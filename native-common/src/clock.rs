use chrono::{Datelike, Local, Timelike};
use oxide86_core::{Clock, LocalDate, LocalTime};

pub struct NativeClock;

impl Clock for NativeClock {
    fn get_local_time(&self) -> LocalTime {
        let now = Local::now();
        LocalTime {
            hours: now.hour() as u8,
            minutes: now.minute() as u8,
            seconds: now.second() as u8,
            milliseconds: now.timestamp_subsec_millis() as u16,
        }
    }

    fn get_local_date(&self) -> LocalDate {
        let now = Local::now();
        let year = now.year();
        LocalDate {
            century: (year / 100) as u8,
            year: (year % 100) as u8,
            month: now.month() as u8,
            day: now.day() as u8,
        }
    }
}
