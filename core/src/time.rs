use crate::cpu::bios::{RtcDate, RtcTime};

#[cfg(not(target_arch = "wasm32"))]
use std::time::SystemTime;

// Time and RTC operations for BIOS implementations

/// Get milliseconds since Unix epoch (platform-independent)
fn get_epoch_millis() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() as u64
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64
    }
}

pub fn get_system_ticks() -> u32 {
    // Get current system time
    let millis = get_epoch_millis();
    let total_seconds = millis / 1000;

    // Calculate seconds since midnight (local time approximation)
    // Note: This is simplified and doesn't account for timezones properly
    let seconds_in_day = 24 * 60 * 60;
    let seconds_since_midnight = (total_seconds % seconds_in_day) as u32;

    // Convert to BIOS ticks (18.2 ticks per second)
    // More precisely: 1193182 / 65536 = 18.2065 Hz
    // We use: ticks = seconds * 182 / 10
    let ticks = (seconds_since_midnight as u64 * 182 / 10) as u32;

    // Ensure we don't exceed the maximum tick count for a day
    ticks.min(0x001800B0)
}

pub fn get_rtc_time() -> Option<RtcTime> {
    // Get current system time
    let millis = get_epoch_millis();
    let total_seconds = millis / 1000;

    // Calculate time of day (simplified, doesn't account for timezone)
    let seconds_in_day = 24 * 60 * 60;
    let seconds_since_midnight = (total_seconds % seconds_in_day) as u32;

    // Convert to hours, minutes, seconds
    let hours = (seconds_since_midnight / 3600) as u8;
    let minutes = ((seconds_since_midnight % 3600) / 60) as u8;
    let seconds = (seconds_since_midnight % 60) as u8;

    // Return RTC time (DST flag set to 0 for standard time)
    Some(RtcTime {
        hours,
        minutes,
        seconds,
        dst_flag: 0, // Standard time (no DST support in this simple implementation)
    })
}

pub fn get_rtc_date() -> Option<RtcDate> {
    // Get current system time
    let millis = get_epoch_millis();
    let total_seconds = millis / 1000;

    // Calculate date (simplified Gregorian calendar calculation)
    // Days since Unix epoch (January 1, 1970)
    let days_since_epoch = (total_seconds / 86400) as i32;

    // Calculate year, month, day using a simplified algorithm
    // This is an approximation that works for dates between 1970-2099
    let mut days_remaining = days_since_epoch;

    // Start from 1970
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days_remaining < days_in_year {
            break;
        }
        days_remaining -= days_in_year;
        year += 1;
    }

    // Find month and day
    let days_in_months = if is_leap_year(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1u8;
    for &days_in_month in &days_in_months {
        if days_remaining < days_in_month {
            break;
        }
        days_remaining -= days_in_month;
        month += 1;
    }

    let day = (days_remaining + 1) as u8;

    // Calculate century and year within century
    let century = (year / 100) as u8;
    let year_in_century = (year % 100) as u8;

    Some(RtcDate {
        century,
        year: year_in_century,
        month,
        day,
    })
}

/// Check if a year is a leap year
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
