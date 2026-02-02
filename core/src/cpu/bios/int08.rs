use crate::cpu::Cpu;
use crate::memory::{BDA_START, BDA_TIMER_COUNTER, BDA_TIMER_OVERFLOW, Memory};

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
    /// 3. Chains to INT 0x1C (user timer tick hook)
    ///
    /// Programs can install custom INT 0x1C handlers for periodic tasks like
    /// music playback (QBASIC PLAY), animation, or polling.
    pub(super) fn handle_int08(
        &mut self,
        memory: &mut Memory,
        io: &mut super::Bios,
        video: &mut crate::video::Video,
    ) {
        // Read current timer counter from BDA (4 bytes, little-endian)
        let counter_addr = BDA_START + BDA_TIMER_COUNTER;
        let low_word = memory.read_u16(counter_addr);
        let high_word = memory.read_u16(counter_addr + 2);
        let mut tick_count = ((high_word as u32) << 16) | (low_word as u32);

        // Increment tick count
        tick_count = tick_count.wrapping_add(1);

        // Check for midnight rollover (0x001800B0 ticks = 24 hours)
        if tick_count >= TIMER_TICKS_PER_DAY {
            tick_count = 0;
            // Set midnight overflow flag
            let overflow_addr = BDA_START + BDA_TIMER_OVERFLOW;
            memory.write_u8(overflow_addr, 1);
        }

        // Write updated tick count back to BDA
        memory.write_u16(counter_addr, (tick_count & 0xFFFF) as u16);
        memory.write_u16(counter_addr + 2, (tick_count >> 16) as u16);

        // Chain to INT 0x1C (user timer tick)
        // Check if the IVT still points to BIOS or if a program installed a custom handler
        let ivt_addr = 0x1C * 4;
        let int1c_offset = memory.read_u16(ivt_addr);
        let int1c_segment = memory.read_u16(ivt_addr + 2);
        let is_bios = Self::is_bios_handler(memory, 0x1C);

        // Log only occasionally to avoid spam (every 100th tick)
        static mut TICK_COUNT: u32 = 0;
        unsafe {
            TICK_COUNT += 1;
            if TICK_COUNT % 100 == 1 {
                log::info!(
                    "INT 08h: INT 1C vector = {:04X}:{:04X}, is_bios={}",
                    int1c_segment,
                    int1c_offset,
                    is_bios
                );
            }
        }

        if is_bios {
            // BIOS handler - call directly (it's a no-op stub anyway)
            self.handle_int1c(memory, io, video);
        } else {
            // User-installed handler - set up CPU to execute it
            // This simulates an INT 0x1C instruction being executed
            self.chain_to_interrupt(0x1C, memory);
        }
    }

    /// Chain to another interrupt handler during BIOS interrupt processing
    ///
    /// This simulates the effect of an INT instruction within a BIOS handler.
    /// It pushes FLAGS, CS, IP onto the stack and sets CS:IP to the interrupt handler.
    /// When that handler executes IRET, control returns to the instruction after
    /// the original INT.
    fn chain_to_interrupt(&mut self, int_num: u8, memory: &mut Memory) {
        use crate::cpu::cpu_flag;

        // Push current state onto stack (simulating INT instruction)
        self.push(self.flags, memory);
        self.push(self.cs, memory);
        self.push(self.ip, memory);

        // Clear IF and TF flags (standard INT behavior)
        self.set_flag(cpu_flag::INTERRUPT, false);
        self.set_flag(cpu_flag::TRAP, false);

        // Load interrupt vector from IVT
        let ivt_addr = (int_num as usize) * 4;
        let offset = memory.read_u16(ivt_addr);
        let segment = memory.read_u16(ivt_addr + 2);

        // Set CS:IP to interrupt handler
        self.cs = segment;
        self.ip = offset;

        log::debug!(
            "INT 0x08: Chaining to INT 0x{:02X} at {:04X}:{:04X}",
            int_num,
            segment,
            offset
        );
    }
}
