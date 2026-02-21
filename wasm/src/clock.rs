use oxide86_core::{Clock, LocalDate, LocalTime};

pub struct WasmClock;

impl Clock for WasmClock {
    fn get_local_time(&self) -> LocalTime {
        let date = js_sys::Date::new_0();
        LocalTime {
            hours: date.get_hours() as u8,
            minutes: date.get_minutes() as u8,
            seconds: date.get_seconds() as u8,
            milliseconds: date.get_milliseconds() as u16,
        }
    }

    fn get_local_date(&self) -> LocalDate {
        let date = js_sys::Date::new_0();
        let full_year = date.get_full_year() as i32;
        LocalDate {
            century: (full_year / 100) as u8,
            year: (full_year % 100) as u8,
            month: (date.get_month() + 1) as u8, // JavaScript months are 0-indexed
            day: date.get_date() as u8,
        }
    }
}
