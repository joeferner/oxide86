/// Local time components with subsecond precision.
pub struct LocalTime {
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub milliseconds: u16,
}

/// Local date components.
pub struct LocalDate {
    pub century: u8,
    pub year: u8,
    pub month: u8,
    pub day: u8,
}

/// Platform-independent clock trait for time and date operations.
/// Native implementations use chrono, WASM uses js_sys::Date.
pub trait Clock: Send {
    /// Returns local time with subsecond precision
    fn get_local_time(&self) -> LocalTime;
    /// Returns local date with century
    fn get_local_date(&self) -> LocalDate;
}
