use log::warn;

use crate::{cpu::Cpu, memory::Memory};
use crate::memory::{BDA_START, BDA_TIMER_COUNTER, BDA_TIMER_OVERFLOW};

/// Timer ticks at 18.2 Hz (PIT channel 0 frequency)
/// Ticks per day = 24 * 60 * 60 * 18.2 = 1,573,040 (0x1800B0)
/// This constant can be used to detect midnight rollovers
#[allow(dead_code)]
pub const TIMER_TICKS_PER_DAY: u32 = 0x001800B0;

impl Cpu {
    /// INT 0x1A - Time Services
    /// AH register contains the function number
    pub(super) fn handle_int1a<T: super::Bios>(&mut self, memory: &mut Memory, _io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH

        match function {
            0x00 => self.int1a_get_system_time(memory),
            0x01 => self.int1a_set_system_time(memory),
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
        self.cx = high_word;  // CX = high word of tick count
        self.dx = low_word;   // DX = low word of tick count
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
        memory.write_word(counter_addr, low_word);     // Low word
        memory.write_word(counter_addr + 2, high_word); // High word

        // Clear midnight overflow flag when setting time
        let overflow_addr = BDA_START + BDA_TIMER_OVERFLOW;
        memory.write_byte(overflow_addr, 0);
    }
}
