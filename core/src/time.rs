use crate::cpu::bios::{RtcDate, RtcTime};

// Time and RTC operations for BIOS implementations

/// Get local time components with subsecond precision - platform-independent
/// Returns (hours, minutes, seconds, milliseconds)
#[cfg(target_arch = "wasm32")]
fn get_local_time_components() -> (u8, u8, u8, u16) {
    let date = js_sys::Date::new_0();
    let hours = date.get_hours() as u8;
    let minutes = date.get_minutes() as u8;
    let seconds = date.get_seconds() as u8;
    let millis = date.get_milliseconds() as u16;
    (hours, minutes, seconds, millis)
}

#[cfg(not(target_arch = "wasm32"))]
fn get_local_time_components() -> (u8, u8, u8, u16) {
    // Use chrono for accurate local time with subsecond precision
    use chrono::{Local, Timelike};
    let now = Local::now();
    let millis = (now.timestamp_subsec_millis()) as u16;
    (now.hour() as u8, now.minute() as u8, now.second() as u8, millis)
}

/// Get local date components (century, year, month, day) - platform-independent
#[cfg(target_arch = "wasm32")]
fn get_local_date_components() -> (u8, u8, u8, u8) {
    let date = js_sys::Date::new_0();
    let full_year = date.get_full_year() as i32;
    let century = (full_year / 100) as u8;
    let year_in_century = (full_year % 100) as u8;
    let month = (date.get_month() + 1) as u8; // JavaScript months are 0-indexed
    let day = date.get_date() as u8;
    (century, year_in_century, month, day)
}

#[cfg(not(target_arch = "wasm32"))]
fn get_local_date_components() -> (u8, u8, u8, u8) {
    // Use chrono for accurate local time
    use chrono::{Datelike, Local};
    let now = Local::now();
    let year = now.year();
    let century = (year / 100) as u8;
    let year_in_century = (year % 100) as u8;
    (century, year_in_century, now.month() as u8, now.day() as u8)
}

pub fn get_system_ticks() -> u32 {
    // Get local time components with subsecond precision
    let (hours, minutes, seconds, millis) = get_local_time_components();

    // Calculate total milliseconds since midnight
    let seconds_since_midnight = (hours as u32 * 3600) + (minutes as u32 * 60) + (seconds as u32);
    let millis_since_midnight = (seconds_since_midnight as u64 * 1000) + (millis as u64);

    // Convert to BIOS ticks using exact timer frequency
    // Timer frequency: 1193182 / 65536 = 18.2065 Hz (NOT 18.2!)
    // Formula: ticks = milliseconds * 1193182 / 65536 / 1000
    let ticks = (millis_since_midnight * 1193182 / 65536 / 1000) as u32;

    // Ensure we don't exceed the maximum tick count for a day
    ticks.min(0x001800B0)
}

pub fn get_rtc_time() -> Option<RtcTime> {
    // Get local time components (ignore milliseconds for RTC)
    let (hours, minutes, seconds, _millis) = get_local_time_components();

    // Return RTC time (DST flag set to 0 for standard time)
    Some(RtcTime {
        hours,
        minutes,
        seconds,
        dst_flag: 0, // Standard time (no DST support in this simple implementation)
    })
}

pub fn get_rtc_date() -> Option<RtcDate> {
    // Get local date components
    let (century, year, month, day) = get_local_date_components();

    Some(RtcDate {
        century,
        year,
        month,
        day,
    })
}
