use std::any::Any;

use crate::{
    Device,
    devices::{
        clock::Clock,
        pit::{PIT_DIVISOR, PIT_FREQUENCY_HZ},
    },
};

pub const RTC_IO_PORT_REGISTER_SELECT: u16 = 0x0070;
pub const RTC_IO_PORT_DATA: u16 = 0x0071;

// RTC / CMOS register indices
pub const RTC_REG_SECONDS: u8 = 0x00;
pub const RTC_REG_MINUTES: u8 = 0x02;
pub const RTC_REG_HOURS: u8 = 0x04;
pub const RTC_REG_DAY: u8 = 0x07;
pub const RTC_REG_MONTH: u8 = 0x08;
pub const RTC_REG_YEAR: u8 = 0x09;
pub const RTC_REG_CENTURY: u8 = 0x32;
/// CMOS alarm registers (seconds, minutes, hours)
pub const RTC_REG_ALARM_SECONDS: u8 = 0x01;
pub const RTC_REG_ALARM_MINUTES: u8 = 0x03;
pub const RTC_REG_ALARM_HOURS: u8 = 0x05;
/// Status Register B: bit 5 = AIE (alarm interrupt enable), bit 1 = 24h mode
pub const RTC_REG_STATUS_B: u8 = 0x0B;
/// CMOS floppy drive type register.
/// Bits 7:4 = drive A type, bits 3:0 = drive B type.
/// Values: 0=none, 1=360KB 5.25", 2=1.2MB 5.25", 3=720KB 3.5", 4=1.44MB 3.5", 5=2.88MB 3.5"
pub const CMOS_REG_FLOPPY_TYPES: u8 = 0x10;

pub(crate) struct Rtc {
    clock: Box<dyn Clock>,
    /// Currently selected CMOS register index (written via port 0x70)
    selected_register: u8,
    /// CMOS register 0x10: floppy drive types (bits 7:4 = A, bits 3:0 = B)
    floppy_types: u8,
    /// CMOS alarm registers (seconds=0x01, minutes=0x03, hours=0x05) — writable RAM
    alarm: [u8; 3],
    /// Status Register B (0x0B): bit 5 = AIE (alarm interrupt enable), bit 1 = 24h mode
    status_b: u8,
    /// Status Register C flags: bit 5 = AF (alarm flag), bit 7 = IRQF. Cleared on read.
    status_c: u8,
    /// BCD seconds value at which the alarm last fired, to prevent re-firing within the same second.
    last_alarm_second: u8,
    /// CMOS register 0x0F: shutdown status byte. Battery-backed, survives reset.
    /// Used by BIOS to determine POST behavior after a CPU reset.
    shutdown_byte: u8,
}

impl Rtc {
    pub(crate) fn new(clock: Box<dyn Clock>) -> Self {
        Self {
            clock,
            selected_register: 0,
            floppy_types: 0,
            alarm: [0u8; 3],
            status_b: 0x02, // 24h mode, BCD format
            status_c: 0,
            last_alarm_second: 0xFF, // invalid sentinel
            shutdown_byte: 0,
        }
    }

    /// Check if the RTC alarm interrupt is pending and consume it.
    /// Fires at most once per second when the current time matches the alarm
    /// registers and AIE (bit 5 of Status Register B) is set.
    pub(crate) fn take_pending_alarm(&mut self, cycle_count: u32) -> bool {
        // AIE = bit 5 of Status Register B
        if self.status_b & 0x20 == 0 {
            return false;
        }

        let time = self.clock.get_local_time(cycle_count);
        let current_sec = to_bcd(time.seconds);

        // Avoid firing multiple times in the same second
        if current_sec == self.last_alarm_second {
            return false;
        }

        // Bit 7 of an alarm byte = "don't care" (match any value)
        let sec_match = self.alarm[0] & 0x80 != 0 || self.alarm[0] == current_sec;
        let min_match = self.alarm[1] & 0x80 != 0 || self.alarm[1] == to_bcd(time.minutes);
        let hour_match = self.alarm[2] & 0x80 != 0 || self.alarm[2] == to_bcd(time.hours);

        if sec_match && min_match && hour_match {
            self.last_alarm_second = current_sec;
            // Set AF (bit 5) and IRQF (bit 7) in Status Register C
            self.status_c = 0xA0;
            true
        } else {
            false
        }
    }

    /// Returns the BDA timer counter value (ticks since midnight at ~18.2 Hz).
    ///
    /// Computed as: total_milliseconds_since_midnight * PIT_FREQUENCY_HZ / (PIT_DIVISOR * MS_PER_SECOND)
    pub(crate) fn timer_counter(&self, cycle_count: u32) -> u32 {
        const MS_PER_SECOND: u64 = 1_000;

        let time = self.clock.get_local_time(cycle_count);
        let total_ms = (time.hours as u64 * 3_600 + time.minutes as u64 * 60 + time.seconds as u64)
            * MS_PER_SECOND
            + time.milliseconds as u64;
        ((total_ms * PIT_FREQUENCY_HZ) / (PIT_DIVISOR * MS_PER_SECOND)) as u32
    }
}

fn to_bcd(val: u8) -> u8 {
    ((val / 10) << 4) | (val % 10)
}

impl Device for Rtc {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn reset(&mut self) {
        self.selected_register = 0;
        // floppy_types is battery-backed CMOS RAM — preserved across resets
    }

    fn memory_read_u8(&mut self, _addr: usize, _cycle_count: u32) -> Option<u8> {
        None
    }

    fn memory_write_u8(&mut self, _addr: usize, _val: u8, _cycle_count: u32) -> bool {
        false
    }

    fn io_read_u8(&mut self, port: u16, cycle_count: u32) -> Option<u8> {
        if port != RTC_IO_PORT_DATA {
            return None;
        }

        let val = match self.selected_register {
            0x00 => to_bcd(self.clock.get_local_time(cycle_count).seconds),
            0x02 => to_bcd(self.clock.get_local_time(cycle_count).minutes),
            0x04 => to_bcd(self.clock.get_local_time(cycle_count).hours),
            0x07 => to_bcd(self.clock.get_local_date(cycle_count).day),
            0x08 => to_bcd(self.clock.get_local_date(cycle_count).month),
            0x09 => to_bcd(self.clock.get_local_date(cycle_count).year),
            // Status Register A: bit 7 = 0 (update not in progress)
            0x0A => 0x00,
            // Status Register B: writable; bit 5 = AIE, bit 1 = 24h mode
            0x0B => self.status_b,
            // Status Register C: alarm/interrupt flags; cleared on read
            0x0C => {
                let val = self.status_c;
                self.status_c = 0;
                val
            }
            // Status Register D: bit 7 (VRT) = battery good / RTC present; bits 6-0 reserved (must be 0)
            0x0D => 0x80,
            // Seconds alarm / minutes alarm / hours alarm — readable CMOS RAM
            0x01 => self.alarm[0],
            0x03 => self.alarm[1],
            0x05 => self.alarm[2],
            // Shutdown status byte (battery-backed, survives reset)
            0x0F => self.shutdown_byte,
            CMOS_REG_FLOPPY_TYPES => self.floppy_types,
            // Hard disk drive types: bits 7:4 = HD0 type, bits 3:0 = HD1 type. 0x00 = no drives.
            0x12 => 0x00,
            // Equipment byte: bits 7:6 = num floppies - 1 (if present), bits 5:4 = video type
            // (00 = EGA/VGA), bit 1 = FPU present, bit 0 = floppy drive present.
            0x14 => {
                let drive_a = (self.floppy_types >> 4) & 0x0F;
                let drive_b = self.floppy_types & 0x0F;
                let num_drives = (drive_a != 0) as u8 + (drive_b != 0) as u8;
                if num_drives > 0 {
                    ((num_drives - 1) << 6) | 0x01
                } else {
                    0x00
                }
            }
            // Base memory size in KB: 640KB = 0x0280
            0x15 => 0x80, // low byte
            0x16 => 0x02, // high byte
            // Extended memory size in KB above 1MB: 0 (no extended memory)
            0x17 => 0x00, // low byte
            0x18 => 0x00, // high byte
            // Extended hard disk types (used when 0x12 nibble = 0x0F): 0 = not used
            0x19 => 0x00,
            0x1A => 0x00,
            0x32 => to_bcd(self.clock.get_local_date(cycle_count).century),
            reg => {
                log::warn!("RTC: read from unimplemented CMOS register 0x{reg:02X}");
                0xFF
            }
        };

        Some(val)
    }

    fn io_write_u8(&mut self, port: u16, val: u8, _cycle_count: u32) -> bool {
        match port {
            RTC_IO_PORT_REGISTER_SELECT => {
                // Bit 7 is the NMI disable bit; mask it out to get the register index
                self.selected_register = val & 0x7F;
                true
            }
            RTC_IO_PORT_DATA => {
                match self.selected_register {
                    CMOS_REG_FLOPPY_TYPES => self.floppy_types = val,
                    0x0F => self.shutdown_byte = val,
                    0x01 => self.alarm[0] = val,
                    0x03 => self.alarm[1] = val,
                    0x05 => self.alarm[2] = val,
                    0x0B => self.status_b = val,
                    _ => {} // All other CMOS data writes are ignored; we use the real system clock
                }
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
pub mod tests {
    use crate::devices::clock::{Clock, LocalDate, LocalTime};

    pub(crate) struct MockClock {
        local_time: LocalTime,
        local_date: LocalDate,
    }

    impl MockClock {
        pub(crate) fn new() -> Self {
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
        fn get_local_time(&self, _cycle_count: u32) -> LocalTime {
            self.local_time.clone()
        }

        fn get_local_date(&self, _cycle_count: u32) -> LocalDate {
            self.local_date.clone()
        }
    }
}
