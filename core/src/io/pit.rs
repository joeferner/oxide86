//! Intel 8253/8254 Programmable Interval Timer (PIT) Emulation
//!
//! The PIT is a critical PC component that provides three independent timers:
//! - Channel 0: System timer (18.2 Hz) - generates INT 08h
//! - Channel 1: Memory refresh (obsolete, stubbed)
//! - Channel 2: PC speaker control
//!
//! Base frequency: 1.193182 MHz
//! Output frequency = PIT_FREQUENCY_HZ / count_register

/// PIT oscillator frequency in Hz (Intel 8253/8254 standard)
pub const PIT_FREQUENCY_HZ: u64 = 1_193_182;

/// Represents a single PIT channel (Intel 8253/8254)
pub struct PitChannel {
    /// Reload value - the initial count loaded by software
    count_register: u16,

    /// Current counter value - decrements as cycles pass
    /// u32 to accommodate count of 0 meaning 65536
    counter: u32,

    /// Output pin state - toggled based on mode
    output: bool,

    /// Operating mode (0-5)
    mode: u8,

    /// Access mode: 1=LSB only, 2=MSB only, 3=LSB then MSB
    access_mode: u8,

    /// Latched count value (for read-back operations)
    latch_value: Option<u32>,

    /// Gate input (true = enabled, false = disabled)
    /// For Channel 2: controlled by port 0x61 bit 0
    gate: bool,

    /// BCD mode (false = binary, true = BCD)
    /// We only implement binary mode, log warning for BCD
    bcd_mode: bool,

    /// Write state tracking for LSB/MSB access
    write_lsb_next: bool,

    /// Read state tracking for LSB/MSB access
    read_lsb_next: bool,

    /// Temporary storage for first byte in LSB+MSB mode
    write_buffer: u8,

    /// Half-period toggle for Mode 3 (square wave)
    /// Tracks whether we're in high or low half of the wave
    output_toggle: bool,

    /// Null count flag - true if counter is being reloaded
    null_count: bool,
}

impl PitChannel {
    fn new() -> Self {
        Self {
            count_register: 0,
            counter: 0,
            output: false,
            mode: 3,        // Mode 3: Square wave generator (typical BIOS default)
            access_mode: 3, // LSB then MSB (typical BIOS default)
            latch_value: None,
            gate: true, // Channels 0 and 1 default to enabled
            bcd_mode: false,
            write_lsb_next: true,
            read_lsb_next: true,
            write_buffer: 0,
            output_toggle: false,
            null_count: true,
        }
    }

    /// Reload counter from count_register
    fn reload_counter(&mut self) {
        // Handle special case: count of 0 means 65536
        self.counter = if self.count_register == 0 {
            65536
        } else {
            self.count_register as u32
        };

        self.null_count = false;
        self.output_toggle = false;

        // Initialize output state based on mode
        match self.mode {
            0 => self.output = false,    // Goes high when count reaches 0
            2 | 3 => self.output = true, // Starts high
            _ => {}
        }
    }

    /// Update counter based on elapsed cycles (Mode 0 - Interrupt on Terminal Count)
    fn update_mode0(&mut self, cycles: u64) {
        if !self.gate || self.null_count {
            return;
        }

        let decrement = std::cmp::min(cycles, self.counter as u64);
        self.counter = self.counter.saturating_sub(decrement as u32);

        if self.counter == 0 {
            self.output = true; // Output goes high on terminal count
        }
    }

    /// Update counter based on elapsed cycles (Mode 2 - Rate Generator)
    fn update_mode2(&mut self, cycles: u64) {
        if !self.gate || self.null_count {
            return;
        }

        // Similar to mode 3 but only pulses output low for one cycle
        let mut remaining = cycles;

        while remaining > 0 {
            let decrement = std::cmp::min(remaining, self.counter as u64);
            self.counter = self.counter.saturating_sub(decrement as u32);

            if self.counter == 1 {
                self.output = false; // Pulse low
            } else if self.counter == 0 {
                self.counter = if self.count_register == 0 {
                    65536
                } else {
                    self.count_register as u32
                };
                self.output = true;
            }

            remaining -= decrement;
        }
    }

    /// Update counter based on elapsed cycles (Mode 3 - Square Wave Generator)
    fn update_mode3(&mut self, cycles: u64) {
        if !self.gate || self.null_count {
            return;
        }

        let mut remaining = cycles;

        while remaining > 0 {
            if self.counter == 0 {
                // Reload counter
                self.counter = if self.count_register == 0 {
                    65536
                } else {
                    self.count_register as u32
                };

                // Toggle output
                self.output = !self.output;
                self.output_toggle = !self.output_toggle;
            }

            // In Mode 3, counter decrements by 2 each cycle
            // This produces a 50% duty cycle square wave
            let decrement = std::cmp::min(remaining, self.counter as u64);
            let actual_decrement = if decrement >= 2 {
                2 * (decrement / 2) // Round down to even number
            } else {
                decrement
            };

            if actual_decrement > 0 {
                self.counter = self.counter.saturating_sub(actual_decrement as u32);
                remaining -= actual_decrement;
            } else {
                // If we can't decrement by 2, break to avoid infinite loop
                break;
            }
        }
    }
}

/// Intel 8253/8254 Programmable Interval Timer
pub struct Pit {
    channels: [PitChannel; 3],

    /// Accumulated fractional cycles for precise timing
    /// PIT runs at 1.193182 MHz, but our cycle counter may differ
    cycle_accumulator: f64,

    /// Emulated CPU frequency in Hz; used to convert CPU cycles to PIT cycles.
    cpu_freq: u64,
}

impl Pit {
    pub fn new(cpu_freq: u64) -> Self {
        let mut pit = Self {
            channels: [PitChannel::new(), PitChannel::new(), PitChannel::new()],
            cycle_accumulator: 0.0,
            cpu_freq,
        };

        // Initialize channel 0 for 18.2 Hz system timer (standard BIOS configuration)
        // Count of 0 means 65536, which gives PIT_FREQUENCY_HZ / 65536 ≈ 18.2 Hz
        pit.channels[0].count_register = 0; // 0 = 65536
        pit.channels[0].reload_counter(); // Set counter and clear null_count flag

        // Channel 2 gate starts disabled (controlled by port 0x61 bit 0)
        pit.channels[2].gate = false;

        pit
    }

    /// Write command byte to port 0x43
    pub fn write_command(&mut self, command: u8) {
        let channel = (command >> 6) & 0x03;
        let access_mode = (command >> 4) & 0x03;
        let mode = (command >> 1) & 0x07;
        let bcd = (command & 0x01) != 0;

        if channel == 3 {
            // Read-back command (8254 only)
            log::warn!("PIT: Read-back command not implemented");
            return;
        }

        let ch = &mut self.channels[channel as usize];

        if access_mode == 0 {
            // Latch count value
            if ch.latch_value.is_none() {
                ch.latch_value = Some(ch.counter);
                log::trace!("PIT: Channel {} latched count = {}", channel, ch.counter);
            }
            return;
        }

        // Set channel configuration
        ch.access_mode = access_mode;
        ch.mode = if mode > 5 { mode - 4 } else { mode }; // Modes 6,7 -> 2,3
        ch.bcd_mode = bcd;
        ch.null_count = true;
        ch.write_lsb_next = true;
        ch.read_lsb_next = true;

        if bcd {
            log::warn!(
                "PIT: BCD mode not supported on channel {}, treating as binary",
                channel
            );
        }

        log::trace!(
            "PIT: Channel {} configured - Mode: {}, Access: {}",
            channel,
            ch.mode,
            ch.access_mode
        );
    }

    /// Write count value to channel (ports 0x40-0x42)
    pub fn write_channel(&mut self, channel: u8, value: u8) {
        let ch = &mut self.channels[channel as usize];

        match ch.access_mode {
            1 => {
                // LSB only
                ch.count_register = value as u16;
                ch.reload_counter();
            }
            2 => {
                // MSB only
                ch.count_register = (value as u16) << 8;
                ch.reload_counter();
            }
            3 => {
                // LSB then MSB
                if ch.write_lsb_next {
                    ch.write_buffer = value;
                    ch.write_lsb_next = false;
                } else {
                    ch.count_register = ((value as u16) << 8) | (ch.write_buffer as u16);
                    ch.write_lsb_next = true;
                    ch.reload_counter();
                }
            }
            _ => {
                log::warn!("PIT: Invalid access mode on channel {}", channel);
            }
        }

        if ch.access_mode != 3 || ch.write_lsb_next {
            log::trace!(
                "PIT: Channel {} count = {} (0x{:04X})",
                channel,
                ch.count_register,
                ch.count_register
            );
        }
    }

    /// Read current count from channel (ports 0x40-0x42)
    pub fn read_channel(&mut self, channel: u8) -> u8 {
        let ch = &mut self.channels[channel as usize];

        // Use latched value if available, otherwise current counter
        let count = ch.latch_value.unwrap_or(ch.counter);

        let result = match ch.access_mode {
            1 => {
                // LSB only
                (count & 0xFF) as u8
            }
            2 => {
                // MSB only
                (count >> 8) as u8
            }
            3 => {
                // LSB then MSB
                let byte = if ch.read_lsb_next {
                    (count & 0xFF) as u8
                } else {
                    (count >> 8) as u8
                };
                ch.read_lsb_next = !ch.read_lsb_next;

                // Clear latch after both bytes read
                if ch.read_lsb_next {
                    ch.latch_value = None;
                }

                byte
            }
            _ => 0xFF,
        };

        log::trace!("PIT: Channel {} read = 0x{:02X}", channel, result);
        result
    }

    /// Update all channels based on CPU cycles
    /// Called from Computer::increment_cycles()
    pub fn update(&mut self, cpu_cycles: u64) {
        // Convert CPU cycles to PIT cycles
        let pit_cycles_f64 = (cpu_cycles as f64) * (PIT_FREQUENCY_HZ as f64 / self.cpu_freq as f64)
            + self.cycle_accumulator;
        let pit_cycles = pit_cycles_f64 as u64;
        self.cycle_accumulator = pit_cycles_f64 - (pit_cycles as f64);

        // Update each channel
        for (i, channel) in self.channels.iter_mut().enumerate() {
            match channel.mode {
                0 => channel.update_mode0(pit_cycles),
                2 => channel.update_mode2(pit_cycles),
                3 => channel.update_mode3(pit_cycles),
                _ => {
                    if channel.mode != 0 || !channel.null_count {
                        log::warn!(
                            "PIT: Mode {} not fully implemented for channel {}",
                            channel.mode,
                            i
                        );
                    }
                }
            }
        }
    }

    /// Get output state of a channel (for port 0x61 bit 5)
    pub fn get_channel_output(&self, channel: u8) -> bool {
        self.channels[channel as usize].output
    }

    /// Get count register value of a channel (for speaker frequency calculation)
    pub fn get_channel_count(&self, channel: u8) -> u16 {
        self.channels[channel as usize].count_register
    }

    /// Check if a channel is ready (has a valid count loaded)
    ///
    /// Returns false if the channel is waiting for a new count value (null_count flag set).
    /// This is used by the speaker to avoid outputting sound while the PIT is being reprogrammed.
    pub fn is_channel_ready(&self, channel: u8) -> bool {
        !self.channels[channel as usize].null_count
    }

    /// Set gate input for a channel (from port 0x61 bit 0 for channel 2)
    pub fn set_gate(&mut self, channel: u8, gate: bool) {
        let ch = &mut self.channels[channel as usize];
        let old_gate = ch.gate;
        ch.gate = gate;

        // Mode 3: Rising edge of gate reloads counter
        if !old_gate && gate && ch.mode == 3 {
            ch.reload_counter();
        }

        log::trace!("PIT: Channel {} gate = {}", channel, gate);
    }
}
