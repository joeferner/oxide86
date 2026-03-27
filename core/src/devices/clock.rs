use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

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
    /// Returns local time with sub-second precision.
    /// `cycle_count` is the current CPU cycle counter; emulated clock implementations
    /// use it to derive time from emulated cycles rather than wall-clock time.
    fn get_local_time(&self, cycle_count: u32) -> LocalTime;
    /// Returns local date with century.
    /// `cycle_count` is the current CPU cycle counter; emulated clock implementations
    /// use it to derive the date from emulated cycles rather than wall-clock time.
    fn get_local_date(&self, cycle_count: u32) -> LocalDate;
}

const NANOS_PER_MS: u64 = 1_000_000;
const NANOS_PER_SEC: u64 = 1_000_000_000;
const NANOS_PER_MIN: u64 = 60 * NANOS_PER_SEC;
const NANOS_PER_HOUR: u64 = 60 * NANOS_PER_MIN;
const NANOS_PER_DAY: u64 = 24 * NANOS_PER_HOUR;

/// A clock implementation that derives time from emulated CPU cycles rather than
/// wall-clock time. This ensures the RTC advances at the same rate as the BDA
/// timer counter, preventing timing mismatches when the emulator runs faster or
/// slower than real-time (e.g. when execution logging is enabled).
pub struct EmulatedClock {
    start_nanos_since_midnight: u64,
    start_date: LocalDate,
    clock_speed_hz: u64,
    last_cycle: AtomicU32,
    total_cycles: AtomicU64,
}

impl EmulatedClock {
    pub fn new(clock_speed_hz: u64, start_date: LocalDate, start_time: LocalTime) -> Self {
        let start_nanos = start_time.hours as u64 * NANOS_PER_HOUR
            + start_time.minutes as u64 * NANOS_PER_MIN
            + start_time.seconds as u64 * NANOS_PER_SEC
            + start_time.milliseconds as u64 * NANOS_PER_MS;
        Self {
            start_nanos_since_midnight: start_nanos,
            start_date,
            clock_speed_hz,
            last_cycle: AtomicU32::new(0),
            total_cycles: AtomicU64::new(0),
        }
    }

    fn emulated_nanos(&self, cycle_count: u32) -> u64 {
        let prev = self.last_cycle.load(Ordering::Relaxed);
        let delta = cycle_count.wrapping_sub(prev) as u64;
        let new_total = self.total_cycles.fetch_add(delta, Ordering::Relaxed) + delta;
        self.last_cycle.store(cycle_count, Ordering::Relaxed);
        let elapsed_nanos = (new_total * NANOS_PER_SEC) / self.clock_speed_hz;
        self.start_nanos_since_midnight + elapsed_nanos
    }
}

fn days_in_month(year: u16, month: u8) -> u8 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            let y = year as u32;
            if y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400)) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn advance_date(date: &LocalDate, mut days: u32) -> LocalDate {
    let mut day = date.day;
    let mut month = date.month;
    let mut year = date.century as u16 * 100 + date.year as u16;
    while days > 0 {
        let remaining = days_in_month(year, month) - day;
        if days <= remaining as u32 {
            day += days as u8;
            break;
        }
        days -= remaining as u32 + 1;
        day = 1;
        month += 1;
        if month > 12 {
            month = 1;
            year += 1;
        }
    }
    LocalDate {
        century: (year / 100) as u8,
        year: (year % 100) as u8,
        month,
        day,
    }
}

impl Clock for EmulatedClock {
    fn get_local_time(&self, cycle_count: u32) -> LocalTime {
        let nanos = self.emulated_nanos(cycle_count) % NANOS_PER_DAY;
        let hours = (nanos / NANOS_PER_HOUR) as u8;
        let rem = nanos % NANOS_PER_HOUR;
        let minutes = (rem / NANOS_PER_MIN) as u8;
        let rem = rem % NANOS_PER_MIN;
        let seconds = (rem / NANOS_PER_SEC) as u8;
        let milliseconds = ((rem % NANOS_PER_SEC) / NANOS_PER_MS) as u16;
        LocalTime {
            hours,
            minutes,
            seconds,
            milliseconds,
        }
    }

    fn get_local_date(&self, cycle_count: u32) -> LocalDate {
        let total_nanos = self.emulated_nanos(cycle_count);
        let days_elapsed = (total_nanos / NANOS_PER_DAY) as u32;
        advance_date(&self.start_date, days_elapsed)
    }
}
