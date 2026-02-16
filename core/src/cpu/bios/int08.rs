use crate::Bus;
use crate::cpu::Cpu;
use crate::memory::{BDA_START, BDA_TIMER_COUNTER, BDA_TIMER_OVERFLOW};

/// Timer ticks at 18.2 Hz (PIT channel 0 frequency)
/// Ticks per day = 24 * 60 * 60 * 18.2 = 1,573,040 (0x1800B0)
pub const TIMER_TICKS_PER_DAY: u32 = 0x001800B0;

impl Cpu {
    /// INT 0x08 - Timer Hardware Interrupt (IRQ0)
    ///
    /// This is the system timer interrupt that fires 18.2 times per second.
    /// The handler:
    /// 1. Increments the BDA timer counter at 0x0040:0x006C
    /// 2. Checks for midnight rollover (sets flag at 0x0040:0x0070)
    ///
    /// Note: Chaining to INT 0x1C is now handled by Computer::process_timer_irq()
    /// which properly handles both custom and BIOS handlers using begin_irq_chain().
    pub(super) fn handle_int08(&mut self, bus: &mut Bus) {
        // Read current timer counter from BDA (4 bytes, little-endian)
        let counter_addr = BDA_START + BDA_TIMER_COUNTER;
        let low_word = bus.read_u16(counter_addr);
        let high_word = bus.read_u16(counter_addr + 2);
        let mut tick_count = ((high_word as u32) << 16) | (low_word as u32);

        // Increment tick count
        tick_count = tick_count.wrapping_add(1);

        // Check for midnight rollover (0x001800B0 ticks = 24 hours)
        if tick_count >= TIMER_TICKS_PER_DAY {
            tick_count = 0;
            // Set midnight overflow flag
            let overflow_addr = BDA_START + BDA_TIMER_OVERFLOW;
            bus.write_u8(overflow_addr, 1);
        }

        // Write updated tick count back to BDA
        bus.write_u16(counter_addr, (tick_count & 0xFFFF) as u16);
        bus.write_u16(counter_addr + 2, (tick_count >> 16) as u16);

        // Chaining to INT 0x1C is handled by Computer::process_timer_irq()
        // which has access to the full execution context needed for proper chaining
    }
}
