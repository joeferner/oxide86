mod cga_ports;
mod joystick_port;
mod pit;
mod system_control_port;

use crate::joystick::JoystickInput;
use crate::audio::{NullSoundCard, SoundCard};
use crate::video::Video;
use cga_ports::CgaModeControl;
use joystick_port::JoystickPort;
pub use pit::Pit;
use std::collections::HashMap;
pub use system_control_port::SystemControlPort;

/// I/O device implementation.
pub struct IoDevice {
    /// Track last written values for debugging
    last_write: HashMap<u16, u8>,
    /// System Control Port (port 61h)
    system_control_port: SystemControlPort,
    /// Programmable Interval Timer (ports 40h-43h)
    pit: Pit,
    /// Joystick Port (port 201h)
    joystick: JoystickPort,
    /// Current cycle count (for joystick timing)
    current_cycle: u64,
    /// CGA Mode Control Register (port 3D8h)
    cga_mode_control: CgaModeControl,
    /// Sound card (ports depend on card type; AdLib uses 388h-389h)
    sound_card: Box<dyn SoundCard>,
    /// Keyboard controller data port (port 60h) - stores last scan code
    keyboard_scan_code: u8,
    /// ASCII code corresponding to the last scan code (for BIOS INT 09h handler)
    keyboard_ascii_code: u8,
    /// Keyboard controller status register (port 64h)
    keyboard_status: u8,
    /// Keyboard controller command (last command sent to port 64h)
    keyboard_command: u8,
    /// Keyboard controller output port (controls A20 line via bit 1)
    keyboard_output_port: u8,
    /// Counter for CGA status register (port 3DAh) reads.
    /// Used to simulate toggling retrace states so programs don't spin forever.
    cga_status_counter: u32,
    /// EGA Sequencer index register (port 0x3C4)
    ega_sequencer_index: u8,
    /// EGA Sequencer registers (indexed via 0x3C4/0x3C5)
    ega_sequencer_regs: [u8; 8],
    /// EGA Graphics Controller index register (port 0x3CE)
    ega_graphics_index: u8,
    /// EGA Graphics Controller registers (indexed via 0x3CE/0x3CF)
    ega_graphics_regs: [u8; 16],
    /// VGA Attribute Controller address/data flip-flop
    /// false = next write to 0x3C0 is address, true = next write is data
    ac_flip_flop: bool,
    /// VGA Attribute Controller index register (set by address write)
    ac_index: u8,
    /// VGA DAC write mode index (port 0x3C8): next palette entry to write
    dac_write_index: u8,
    /// VGA DAC read mode index (port 0x3C7): next palette entry to read
    dac_read_index: u8,
    /// VGA DAC component counter: 0=R, 1=G, 2=B
    dac_component: u8,
    /// VGA DAC accumulation buffer while writing R/G/B components
    dac_write_buf: [u8; 3],
}

impl IoDevice {
    pub fn new(joystick: Box<dyn JoystickInput>) -> Self {
        let mut ega_sequencer_regs = [0u8; 8];
        ega_sequencer_regs[2] = 0x0F; // Map Mask: all 4 planes enabled by default

        Self {
            last_write: HashMap::new(),
            system_control_port: SystemControlPort::new(),
            pit: Pit::new(),
            joystick: JoystickPort::new(joystick),
            current_cycle: 0,
            cga_mode_control: CgaModeControl::new(),
            sound_card: Box::new(NullSoundCard),
            keyboard_scan_code: 0x00,
            keyboard_ascii_code: 0x00,
            keyboard_status: 0x14, // Bit 2: system flag, bit 4: command/data (ready for commands)
            keyboard_command: 0x00,
            keyboard_output_port: 0x02, // Bit 1: A20 enabled by default
            cga_status_counter: 0,
            ega_sequencer_index: 0,
            ega_sequencer_regs,
            ega_graphics_index: 0,
            ega_graphics_regs: [0u8; 16],
            ac_flip_flop: false,
            ac_index: 0,
            dac_write_index: 0,
            dac_read_index: 0,
            dac_component: 0,
            dac_write_buf: [0u8; 3],
        }
    }

    /// Read a byte from the specified I/O port.
    pub fn read_byte(&mut self, port: u16) -> u8 {
        let value = match port {
            // PIT channel data ports
            0x40..=0x42 => self.pit.read_channel((port - 0x40) as u8),

            // PIT command port (write-only, return 0xFF on read)
            0x43 => 0xFF,

            // Keyboard controller data port
            0x60 => {
                // If last command was 0xD0 (read output port), return output port
                if self.keyboard_command == 0xD0 {
                    self.keyboard_command = 0x00; // Clear command after read
                    self.keyboard_output_port
                } else {
                    // Otherwise return last scan code
                    self.keyboard_scan_code
                }
            }

            // Keyboard controller status port
            0x64 => self.keyboard_status,

            // PS/2 System Control Port (port 0x92) - Fast A20 gate
            0x92 => {
                // Bit 0: Fast reset (write-only, read as 0)
                // Bit 1: Fast A20 gate (1 = enabled)
                if self.keyboard_output_port & 0x02 != 0 {
                    0x02 // A20 enabled
                } else {
                    0x00 // A20 disabled
                }
            }

            // System control port with Timer 2 output
            0x61 => {
                let mut value = self.system_control_port.read();
                // Set bit 5 to reflect Timer 2 output state
                if self.pit.get_channel_output(2) {
                    value |= 0x20;
                }
                value
            }

            // CGA Mode Control Register (read-only in practice)
            0x3D8 => self.cga_mode_control.read(),

            // CGA Color Select Register (write-only, return 0xFF on read)
            0x3D9 => 0xFF,

            // CGA Status Register (port 3DAh)
            // Bit 0: Horizontal retrace active
            // Bit 3: Vertical retrace active
            // Toggle state on each read so programs waiting for retrace
            // start/end don't spin forever.
            // VGA DAC state register (port 0x3C7): returns 0=read mode, 3=write mode ready
            0x3C7 => 3,

            // VGA DAC read index port: returns the current read index
            0x3C8 => self.dac_read_index,

            // VGA Attribute Controller (port 0x3C0) - read returns current index
            0x3C0 => self.ac_index,

            0x3DA => {
                // Reading port 0x3DA resets the AC address/data flip-flop
                self.ac_flip_flop = false;

                self.cga_status_counter = self.cga_status_counter.wrapping_add(1);
                // Every other read: alternate between active display (0x00) and
                // retrace (0x09 = both hsync and vsync bits set)
                if self.cga_status_counter & 1 == 0 {
                    0x00
                } else {
                    0x09
                }
            }

            // EGA Sequencer index port (write-only address, return 0xFF)
            0x3C4 => self.ega_sequencer_index,

            // EGA Sequencer data port
            0x3C5 => {
                let idx = self.ega_sequencer_index as usize;
                if idx < self.ega_sequencer_regs.len() {
                    self.ega_sequencer_regs[idx]
                } else {
                    0xFF
                }
            }

            // EGA Graphics Controller index port
            0x3CE => self.ega_graphics_index,

            // EGA Graphics Controller data port
            0x3CF => {
                let idx = self.ega_graphics_index as usize;
                if idx < self.ega_graphics_regs.len() {
                    self.ega_graphics_regs[idx]
                } else {
                    0xFF
                }
            }

            // Sound card ports (AdLib: 388h-389h)
            0x388 | 0x389 => self.sound_card.read_port(port),

            // Joystick port
            0x201 => self.joystick.read(self.current_cycle),

            // All other ports return 0xFF (floating high)
            _ => self.last_write.get(&port).copied().unwrap_or(0xFF),
        };

        log::trace!("I/O Read:  Port 0x{:04X} -> 0x{:02X}", port, value);

        value
    }

    /// Write a byte to the specified I/O port.
    pub fn write_byte(&mut self, port: u16, value: u8, video: &mut Video) {
        log::trace!("I/O Write: Port 0x{:04X} <- 0x{:02X}", port, value);

        match port {
            // PIT channel data ports
            0x40..=0x42 => self.pit.write_channel((port - 0x40) as u8, value),

            // PIT command port
            0x43 => self.pit.write_command(value),

            // Keyboard controller data port
            0x60 => {
                // If last command was 0xD1 (write output port), update output port
                if self.keyboard_command == 0xD1 {
                    self.keyboard_output_port = value;
                    self.keyboard_command = 0x00; // Clear command after write
                    log::debug!(
                        "Keyboard controller: output port = 0x{:02X}, A20 = {}",
                        value,
                        if value & 0x02 != 0 {
                            "enabled"
                        } else {
                            "disabled"
                        }
                    );
                }
                // Otherwise, this would be keyboard input data (not used in emulator)
            }

            // Keyboard controller command port
            0x64 => {
                self.keyboard_command = value;
                match value {
                    0xAA => {
                        // Self-test - always succeed
                        log::debug!("Keyboard controller: self-test command");
                        // Test result (0x55 = success) will be available at port 0x60
                        self.keyboard_scan_code = 0x55;
                    }
                    0xAB => {
                        // Interface test - always succeed
                        log::debug!("Keyboard controller: interface test command");
                        // Test result (0x00 = success) will be available at port 0x60
                        self.keyboard_scan_code = 0x00;
                    }
                    0xAD => {
                        // Disable keyboard - acknowledge but don't actually disable
                        log::debug!("Keyboard controller: disable keyboard");
                    }
                    0xAE => {
                        // Enable keyboard - acknowledge
                        log::debug!("Keyboard controller: enable keyboard");
                    }
                    0xD0 => {
                        // Read output port - next read from 0x60 will return output port
                        log::debug!("Keyboard controller: read output port command");
                    }
                    0xD1 => {
                        // Write output port - next write to 0x60 will update output port
                        log::debug!("Keyboard controller: write output port command");
                    }
                    0xDD => {
                        // Enable A20 line
                        self.keyboard_output_port |= 0x02;
                        log::debug!("Keyboard controller: A20 enabled (via 0xDD)");
                    }
                    0xDF => {
                        // Disable A20 line
                        self.keyboard_output_port &= !0x02;
                        log::debug!("Keyboard controller: A20 disabled (via 0xDF)");
                    }
                    0xFF => {
                        // Reset keyboard controller
                        log::debug!("Keyboard controller: reset");
                        self.keyboard_status = 0x14;
                        self.keyboard_command = 0x00;
                        // Reset result (0xAA = success) will be available at port 0x60
                        self.keyboard_scan_code = 0xAA;
                    }
                    _ => {
                        log::warn!("Unimplemented keyboard controller command: 0x{:02X}", value);
                    }
                }
            }

            // System control port
            0x61 => {
                self.system_control_port.write(value);
                // Update PIT Channel 2 gate from bit 0
                self.pit.set_gate(2, (value & 0x01) != 0);
            }

            // PS/2 System Control Port (port 0x92) - Fast A20 gate
            0x92 => {
                // Bit 0: Fast reset (0 = reset system, ignored in emulator)
                // Bit 1: Fast A20 gate (1 = enabled, 0 = disabled)
                if value & 0x02 != 0 {
                    self.keyboard_output_port |= 0x02;
                    log::debug!("Fast A20 gate: enabled (via port 0x92)");
                } else {
                    self.keyboard_output_port &= !0x02;
                    log::debug!("Fast A20 gate: disabled (via port 0x92)");
                }
            }

            // VGA DAC: set write-mode palette index (port 0x3C8)
            // Subsequent writes to 0x3C9 set R, G, B for this entry, then advance the index.
            0x3C8 => {
                self.dac_write_index = value;
                self.dac_component = 0;
                log::debug!("VGA DAC: write index set to {}", value);
            }

            // VGA DAC: set read-mode palette index (port 0x3C7)
            0x3C7 => {
                self.dac_read_index = value;
                self.dac_component = 0;
                log::debug!("VGA DAC: read index set to {}", value);
            }

            // VGA DAC data port (port 0x3C9): write R/G/B components sequentially
            0x3C9 => {
                self.dac_write_buf[self.dac_component as usize] = value & 0x3F;
                if self.dac_component == 2 {
                    // All three components received — commit to the palette
                    let [r, g, b] = self.dac_write_buf;
                    video.set_vga_dac_register(self.dac_write_index, r, g, b);
                    log::debug!(
                        "VGA DAC port: palette[{}] = RGB({}, {}, {})",
                        self.dac_write_index,
                        r,
                        g,
                        b
                    );
                    self.dac_write_index = self.dac_write_index.wrapping_add(1);
                    self.dac_component = 0;
                } else {
                    self.dac_component += 1;
                }
            }

            // VGA Attribute Controller (port 0x3C0)
            // Alternates between address and data writes (flip-flop reset by reading 0x3DA)
            0x3C0 => {
                if !self.ac_flip_flop {
                    // Address write: select which AC register to modify
                    self.ac_index = value & 0x1F; // 5-bit index
                } else {
                    // Data write: set the selected AC register
                    let index = self.ac_index & 0x0F;
                    if index < 16 {
                        // AC palette registers 0-15: map attribute values to VGA DAC indices
                        video.set_ac_register(index, value);
                    }
                }
                self.ac_flip_flop = !self.ac_flip_flop;
            }

            // CGA Mode Control Register
            0x3D8 => {
                self.cga_mode_control.write(value);
                log::debug!("CGA Mode Control: 0x{:02X}", value);
                // Decode mode from CGA mode control register bits and update video mode.
                // Bit 3 (0x08) = video enable; only switch mode when display is enabled.
                if value & 0x08 != 0 {
                    let mode = if value & 0x02 != 0 {
                        // Graphics mode
                        if value & 0x10 != 0 {
                            0x06 // 640x200 2-color (bit 4 = high-res)
                        } else if value & 0x04 != 0 {
                            0x05 // 320x200 B&W (bit 2 = monochrome)
                        } else {
                            0x04 // 320x200 4-color
                        }
                    } else {
                        // Text mode
                        if value & 0x01 != 0 {
                            0x03 // 80x25 color text (bit 0 = 80-col)
                        } else {
                            0x01 // 40x25 color text
                        }
                    };
                    if mode == 0x06 && video.get_mode() == 0x04 {
                        // Hires bit flipped while in mode 0x04 (e.g., AGI games set mode
                        // 0x04 via INT 10h then flip hires bit for NTSC composite artifact
                        // colors). DON'T change the video mode - keep mode 0x04's
                        // pixel format (2bpp) so programs continue writing correctly.
                        // Only the renderer changes to composite decoding.
                        video.set_composite_mode(true);
                        video.set_dirty();
                        log::info!(
                            "CGA Mode Control 0x{:02X}: enabling composite mode (keeping mode 0x{:02X})",
                            value,
                            video.get_mode()
                        );
                    } else {
                        // Non-composite mode change or disable composite
                        let was_composite = video.is_composite_mode();
                        video.set_composite_mode(false);
                        if mode != video.get_mode() {
                            log::info!(
                                "CGA Mode Control 0x{:02X}: switching to video mode 0x{:02X}",
                                value,
                                mode,
                            );
                            video.set_mode(mode, true); // Port-based: preserve B800 data (e.g. MS Flight Simulator)
                        } else if was_composite {
                            // Composite mode was disabled but video mode didn't change
                            // Still need to trigger re-render
                            video.set_dirty();
                            log::info!(
                                "CGA Mode Control 0x{:02X}: disabling composite mode (keeping mode 0x{:02X})",
                                value,
                                video.get_mode()
                            );
                        }
                    }
                }
            }

            // CGA Color Select Register
            0x3D9 => {
                video.set_palette(value);
                log::debug!("CGA Color Select: 0x{:02X}", value);
            }

            // EGA Sequencer index register
            0x3C4 => {
                self.ega_sequencer_index = value;
            }

            // EGA Sequencer data register
            0x3C5 => {
                let idx = self.ega_sequencer_index as usize;
                if idx < self.ega_sequencer_regs.len() {
                    self.ega_sequencer_regs[idx] = value;
                    // Register 2: Map Mask - controls which planes receive writes
                    if idx == 2 {
                        video.set_ega_map_mask(value);
                        log::debug!("EGA Sequencer Map Mask: 0x{:02X}", value);
                    }
                }
            }

            // EGA Graphics Controller index register
            0x3CE => {
                self.ega_graphics_index = value;
            }

            // EGA Graphics Controller data register
            0x3CF => {
                let idx = self.ega_graphics_index as usize;
                if idx < self.ega_graphics_regs.len() {
                    self.ega_graphics_regs[idx] = value;
                    // Register 4: Read Map Select - which plane to read
                    if idx == 4 {
                        video.set_ega_read_plane(value);
                        log::debug!("EGA Graphics Read Map Select: plane {}", value & 3);
                    }
                }
            }

            // Sound card ports (AdLib: 388h-389h)
            0x388 | 0x389 => {
                self.sound_card.write_port(port, value);
            }

            // Joystick port - fire one-shots
            0x201 => {
                self.joystick.fire(self.current_cycle);
            }

            _ => {}
        }

        self.last_write.insert(port, value);
    }

    /// Read a word (16-bit) from the specified I/O port.
    /// Reads from port and port+1 in little-endian order.
    pub fn read_word(&mut self, port: u16) -> u16 {
        let low = self.read_byte(port);
        let high = self.read_byte(port.wrapping_add(1));
        (high as u16) << 8 | low as u16
    }

    /// Write a word (16-bit) to the specified I/O port.
    /// Writes to port and port+1 in little-endian order.
    pub fn write_word(&mut self, port: u16, value: u16, video: &mut Video) {
        let low = (value & 0xFF) as u8;
        let high = (value >> 8) as u8;
        self.write_byte(port, low, video);
        self.write_byte(port.wrapping_add(1), high, video);
    }

    /// Update PIT counters based on CPU cycles
    /// Called from Computer::increment_cycles()
    pub fn update_pit(&mut self, cycles: u64) {
        self.pit.update(cycles);
    }

    /// Get reference to the PIT (for speaker integration)
    pub fn pit(&self) -> &Pit {
        &self.pit
    }

    /// Get reference to the system control port (for speaker integration)
    pub fn system_control_port(&self) -> &SystemControlPort {
        &self.system_control_port
    }

    /// Set the keyboard scan code and ASCII code for INT 09h
    /// Called when firing keyboard IRQ
    pub fn set_keyboard_data(&mut self, scan_code: u8, ascii_code: u8) {
        self.keyboard_scan_code = scan_code;
        self.keyboard_ascii_code = ascii_code;
    }

    /// Get the keyboard ASCII code (for BIOS INT 09h handler)
    pub fn get_keyboard_ascii_code(&self) -> u8 {
        self.keyboard_ascii_code
    }

    /// Set the sound card. Call before starting emulation.
    pub fn set_sound_card(&mut self, card: Box<dyn SoundCard>) {
        self.sound_card = card;
    }

    /// Advance the sound card by `cpu_cycles` and accumulate samples.
    pub fn tick_sound_card(&mut self, cpu_cycles: u64) {
        self.sound_card.tick(cpu_cycles);
    }

    /// Pop `count` samples from the sound card's internal buffer.
    pub fn pop_sound_card_samples(&mut self, count: usize) -> Vec<f32> {
        self.sound_card.pop_samples(count)
    }

    /// Check if A20 line is enabled (bit 1 of keyboard controller output port)
    pub fn is_a20_enabled(&self) -> bool {
        (self.keyboard_output_port & 0x02) != 0
    }

    /// Update the current cycle count (for joystick timing)
    /// Called from Computer::increment_cycles()
    pub fn update_cycles(&mut self, total_cycles: u64) {
        self.current_cycle = total_cycles;
    }

    /// Reset I/O device state while preserving joystick connection
    /// Called during computer reset to clear temporary state
    pub fn reset(&mut self) {
        self.last_write.clear();
        self.system_control_port = SystemControlPort::new();
        self.pit = Pit::new();
        // Keep joystick - it's "hardware" that persists across resets
        self.current_cycle = 0;
        self.cga_mode_control = CgaModeControl::new();
        self.keyboard_scan_code = 0x00;
        self.keyboard_ascii_code = 0x00;
        self.keyboard_status = 0x14;
        self.keyboard_command = 0x00;
        self.keyboard_output_port = 0x02; // A20 enabled by default
        self.cga_status_counter = 0;
        self.ega_sequencer_index = 0;
        self.ega_sequencer_regs = [0u8; 8];
        self.ega_sequencer_regs[2] = 0x0F; // Map Mask default
        self.ega_graphics_index = 0;
        self.ega_graphics_regs = [0u8; 16];
        self.sound_card.reset();
    }
}
