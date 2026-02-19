use anyhow::Result;

use crate::{
    Bios, Bus, Clock, CpuType, DriveNumber, MouseInput, NullVideoController, SerialDevice,
    SpeakerOutput, Video, VideoCardType, VideoController,
    audio::SoundCard,
    cpu::{Cpu, bios::KeyPress},
    io::IoDevice,
    joystick::JoystickInput,
    keyboard::KeyboardInput,
    memory::{self, Memory},
    video::text::TextBuffer,
};

#[derive(Clone)]
struct LoadedProgram {
    data: Vec<u8>,
    segment: u16,
    offset: u16,
}

/// Configuration for creating a new [`Computer`] instance.
pub struct ComputerConfig {
    /// CPU type to emulate (default: I8086)
    pub cpu_type: CpuType,
    /// Memory size in KB (default: 1024)
    pub memory_kb: u32,
    /// Video card type (default: VideoCardType::default())
    pub video_card_type: VideoCardType,
}

impl Default for ComputerConfig {
    fn default() -> Self {
        Self {
            cpu_type: CpuType::I8086,
            memory_kb: 1024,
            video_card_type: VideoCardType::default(),
        }
    }
}

pub struct Computer<V: VideoController = NullVideoController> {
    cpu: Cpu,
    cpu_type: CpuType,
    bus: Bus,
    bios: Bios,
    io_device: IoDevice,
    video_controller: V,
    speaker: Box<dyn SpeakerOutput>,
    /// Cycle counter for timer emulation (resets each tick)
    cycle_count: u64,
    /// Total cycles executed (never resets)
    total_cycles: u64,
    /// Instruction step counter for debugging
    step_count: u64,

    /// if set to true, opcode execution will be logged as info level
    pub exec_logging_enabled: bool,
    /// if set to true, interrupts will be logged as info level
    log_interrupts_enabled: bool,

    /// Queue of pending keyboard IRQs (INT 09h)
    pending_keyboard_irqs: std::collections::VecDeque<KeyPress>,
    pending_serial_irqs: std::collections::VecDeque<u8>, // Serial port numbers (0=COM1, 1=COM2)
    /// Pending timer IRQs (INT 08h) - counter to handle multiple ticks if CPU is slow
    pending_timer_irqs: u32,
    /// Counter for periodic speaker updates (reduces overhead)
    speaker_update_cycles: u64,
    /// Boot drive for reset/reboot operations
    boot_drive: Option<DriveNumber>,
    /// Loaded program for reset/reload operations
    loaded_program: Option<LoadedProgram>,
}

impl<V: VideoController> Computer<V> {
    // Need to keep VideoController as a generic because native-gui uses functions only available
    // to it's implementation directly.

    pub fn new(
        keyboard: Box<dyn KeyboardInput>,
        mouse: Box<dyn MouseInput>,
        joystick: Box<dyn JoystickInput>,
        clock: Box<dyn Clock>,
        video_controller: V,
        speaker: Box<dyn SpeakerOutput>,
        config: ComputerConfig,
    ) -> Self {
        let cpu_type = config.cpu_type;
        let memory_kb = config.memory_kb;
        let video_card_type = config.video_card_type;

        let bios = Bios::new(keyboard, mouse, clock);

        // Create Memory and Video, then wrap in Bus
        let mut memory = memory::Memory::new_with_size(memory_kb);
        memory.initialize_ivt();
        memory.initialize_bda();
        // Override BDA memory size with the configured value
        let conventional_kb = memory.conventional_memory_kb();
        memory.write_u16(memory::BDA_START + memory::BDA_MEMORY_SIZE, conventional_kb);
        memory.initialize_fonts();

        // Initialize BDA timer counter from host system time
        let initial_ticks = bios.get_system_ticks();
        memory.write_u16(
            memory::BDA_START + memory::BDA_TIMER_COUNTER,
            (initial_ticks & 0xFFFF) as u16,
        );
        memory.write_u16(
            memory::BDA_START + memory::BDA_TIMER_COUNTER + 2,
            (initial_ticks >> 16) as u16,
        );
        // BDA timer counter initialized above from host system time

        // Initialize BDA hard drive count
        // Query drive 0x80 to get the number of installed hard drives
        let hard_drive_count = bios
            .disk_get_params(DriveNumber::hard_drive_c())
            .map(|params| params.drive_count)
            .unwrap_or(0);
        memory.write_u8(
            memory::BDA_START + memory::BDA_NUM_HARD_DRIVES,
            hard_drive_count,
        );
        log::info!(
            "BDA: Set hard drive count to {} at offset 0x{:04X}",
            hard_drive_count,
            memory::BDA_START + memory::BDA_NUM_HARD_DRIVES
        );

        log::info!(
            "Emulating CPU type: {}, video card: {}",
            cpu_type,
            video_card_type
        );

        // Create Bus with Memory and Video
        let video = Video::new_with_card_type(video_card_type);
        let bus = Bus::new(memory, video);

        let mut computer = Self {
            cpu: Cpu::new(),
            cpu_type,
            bus,
            bios,
            io_device: IoDevice::new(joystick),
            video_controller,
            speaker,
            cycle_count: 0,
            total_cycles: 0,
            step_count: 0,
            exec_logging_enabled: false,
            log_interrupts_enabled: false,
            pending_keyboard_irqs: std::collections::VecDeque::new(),
            pending_serial_irqs: std::collections::VecDeque::new(),
            pending_timer_irqs: 0,
            speaker_update_cycles: 0,
            boot_drive: None,
            loaded_program: None,
        };

        computer.draw_bios_splash();
        computer
    }

    /// Write the BIOS splash screen to the video buffer.
    ///
    /// Writes system info text to VRAM (visible on all renderers) and pushes
    /// an RGBA pixel overlay to the video controller so GUI / WASM renderers
    /// display the graphical logo on top.  CLI renderers ignore the overlay
    /// (default no-op in `VideoController::draw_logo_overlay`).
    fn draw_bios_splash(&mut self) {
        const INFO1_ATTR: u8 = 0x0B; // Bright cyan on black
        const INFO2_ATTR: u8 = 0x07; // Light gray on black
        // Each text-mode character cell is 8 px wide and 16 px tall
        const CHAR_WIDTH_PX: usize = 8;
        const CHAR_HEIGHT_PX: usize = 16;

        // Gather system data before borrowing the bus mutably
        let cpu_name = self.cpu_type.name();
        let memory_kb = self.bus.memory().conventional_memory_kb();
        let line1 = format!("Oxide86 {} (C)2026", cpu_name);
        let line2 = format!("{} KB OK", memory_kb);

        let (logo_pixels, logo_w, logo_h) = crate::bios_logo::generate_bios_logo();

        // In GUI/WASM the PNG logo overlays the top-left, so indent the text
        // to start right of the logo.  In CLI there is no overlay so start at 0.
        let text_start_col = if self.video_controller.shows_logo_overlay() {
            logo_w.div_ceil(CHAR_WIDTH_PX) + 1 // +1 col padding after the logo
        } else {
            0
        };

        // Number of text rows the logo occupies (rounded up), at minimum 2 for
        // the two info lines written below.
        let cursor_row = logo_h.div_ceil(CHAR_HEIGHT_PX).max(2);

        {
            let video = self.bus.video_mut();

            // Write system info beside (or at start of) the logo
            for (i, ch) in line1.bytes().enumerate() {
                let offset = (text_start_col + i) * 2;
                video.write_byte(offset, ch);
                video.write_byte(offset + 1, INFO1_ATTR);
            }
            for (i, ch) in line2.bytes().enumerate() {
                let offset = (80 + text_start_col + i) * 2;
                video.write_byte(offset, ch);
                video.write_byte(offset + 1, INFO2_ATTR);
            }

            // Position cursor below the logo so program output starts there
            video.set_cursor(cursor_row, 0);
        }

        // Sync BDA cursor position for page 0 (high byte = row, low byte = col)
        self.bus.write_u16(
            memory::BDA_START + memory::BDA_CURSOR_POS,
            (cursor_row as u16) << 8,
        );

        // GUI / WASM: push the graphical pixel overlay.
        // CLI renderers use the default no-op.
        self.video_controller
            .draw_logo_overlay(logo_pixels, logo_w, logo_h);
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        self.bus.load_bios(bios_data)?;
        Ok(())
    }

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = Cpu::physical_address(segment, offset);
        self.bus.load_at(physical_addr, program_data)?;

        // Set CPU to start at this location
        self.cpu.cs = segment;
        self.cpu.ip = offset;

        // Initialize other segments to reasonable defaults
        self.cpu.ds = segment;
        self.cpu.es = segment;
        self.cpu.ss = segment;
        self.cpu.sp = 0xFFFE; // Stack grows down from top of segment

        // Store program for reset/reload
        self.loaded_program = Some(LoadedProgram {
            data: program_data.to_vec(),
            segment,
            offset,
        });

        // Clear boot_drive since we're loading a program, not booting
        self.boot_drive = None;

        // Enable interrupts - DOS programs expect IF=1 (inherited from DOS environment)
        use crate::cpu::cpu_flag;
        self.cpu.set_flag(cpu_flag::INTERRUPT, true);

        Ok(())
    }

    /// Boot from disk by loading boot sector to 0x0000:0x7C00
    /// This simulates the BIOS boot process:
    /// 1. Read sector 0 (cylinder 0, head 0, sector 1) from the specified drive
    /// 2. Load it to physical address 0x7C00
    /// 3. Set CS:IP to 0x0000:0x7C00
    /// 4. Set DL to boot drive number
    pub fn boot(&mut self, drive: DriveNumber) -> Result<()> {
        // Read boot sector using BIOS disk services
        // Boot sector is at cylinder 0, head 0, sector 1
        let boot_sector = self
            .bios
            .disk_read_sectors(drive, 0, 0, 1, 1)
            .map_err(|error_code| {
                anyhow::anyhow!("Failed to read boot sector: error {}", error_code)
            })?;

        if boot_sector.len() != 512 {
            return Err(anyhow::anyhow!(
                "Boot sector must be exactly 512 bytes, got {}",
                boot_sector.len()
            ));
        }

        // Verify boot signature (0x55AA at offset 510-511)
        // Some old "booter" games predate the convention and lack this signature; warn but continue.
        if boot_sector[510] != 0x55 || boot_sector[511] != 0xAA {
            log::warn!(
                "Boot sector missing 0x55AA signature (got 0x{:02X}{:02X}); proceeding anyway",
                boot_sector[511],
                boot_sector[510]
            );
        }

        // Load boot sector to 0x0000:0x7C00 (physical address 0x7C00)
        const BOOT_SEGMENT: u16 = 0x0000;
        const BOOT_OFFSET: u16 = 0x7C00;
        let boot_addr = Cpu::physical_address(BOOT_SEGMENT, BOOT_OFFSET);
        self.bus.load_at(boot_addr, &boot_sector)?;

        // Set up CPU registers as BIOS would
        self.cpu.cs = BOOT_SEGMENT;
        self.cpu.ip = BOOT_OFFSET;

        // DL contains boot drive number (0x00 for floppy A:, 0x80 for first hard disk)
        self.cpu.dx = (self.cpu.dx & 0xFF00) | (drive.to_standard() as u16);

        // Set up stack at 0x0000:0x7C00 (just below boot sector)
        // Some boot loaders expect this, others set up their own stack
        self.cpu.ss = 0x0000;
        self.cpu.sp = 0x7C00;

        // Initialize data segments
        self.cpu.ds = 0x0000;
        self.cpu.es = 0x0000;

        // Set current drive to match boot drive
        // Convert BIOS drive number to DOS drive number: 0x00->0, 0x01->1, 0x80->2, 0x81->3
        self.bios.set_default_drive(drive);

        // Pre-allocate memory for DOS kernel
        // In a real system, DOS would already be loaded in memory before
        // the memory allocator starts. We simulate this by pre-allocating
        // a block for DOS, reducing the amount of "free" memory available.
        // Typically DOS + COMMAND.COM takes about 64-128KB.
        // We'll allocate 4096 paragraphs (64KB) for DOS.
        const DOS_PARAGRAPHS: u16 = 4096; // 64KB for DOS kernel and COMMAND.COM
        match self.bios.memory_allocate(DOS_PARAGRAPHS) {
            Ok(seg) => {
                log::info!(
                    "Pre-allocated {} KB at segment 0x{:04X} for DOS kernel",
                    (DOS_PARAGRAPHS as u32 * 16) / 1024,
                    seg
                );
            }
            Err((error_code, available)) => {
                log::warn!(
                    "Failed to pre-allocate DOS memory: error {}, available {} paragraphs",
                    error_code,
                    available
                );
            }
        }

        // Store boot drive for reset/reboot operations
        self.boot_drive = Some(drive);

        // Clear loaded_program since we're booting, not loading
        self.loaded_program = None;

        // Enable interrupts - real BIOS enables IF before jumping to boot sector
        // This allows timer IRQs (INT 0x08) to fire and update the BDA timer counter
        use crate::cpu::cpu_flag;
        self.cpu.set_flag(cpu_flag::INTERRUPT, true);

        Ok(())
    }

    /// Reset the computer and re-boot from the previously booted drive.
    /// If no drive was previously booted, only resets the CPU.
    pub fn reset(&mut self) {
        log::info!("Resetting computer...");

        // Reset CPU state
        self.cpu.reset();
        self.cpu.clear_halt(); // Ensure CPU is not halted after reset

        // Clear conventional memory so stale data from the previous boot
        // (e.g. HIMEM.SYS/VDISK signatures, mouse driver state) does not persist
        self.bus.clear_conventional_memory();

        // Reset memory (BDA and IVT)
        self.bus.initialize_ivt();
        self.bus.initialize_bda();
        self.bus.initialize_fonts();

        // Reset BIOS state (memory allocator, open files, device handles)
        // But keep the attached drives (they're the "hardware")
        self.bios.reset_state();

        // Reset video to blank screen
        *self.bus.video_mut() = Video::new();

        // Notify video controller of mode reset (Video::new() defaults to mode 0x03)
        self.video_controller.set_video_mode(0x03);
        self.video_controller
            .update_vga_dac_palette(self.bus.video().get_vga_dac_palette());

        // Reset IO devices (preserves joystick connection)
        self.io_device.reset();

        // Clear pending interrupts
        self.pending_keyboard_irqs.clear();
        self.pending_serial_irqs.clear();
        self.pending_timer_irqs = 0;

        // Reset cycle counters
        self.cycle_count = 0;
        // Note: total_cycles is NOT reset - it's used for timing/throttling in native CLI
        // Resetting it would cause the throttling to think it's behind schedule
        self.step_count = 0;
        self.speaker_update_cycles = 0;

        // Force a video redraw to clear the screen
        self.video_controller
            .force_redraw(self.bus.video().get_buffer());

        // Reload program or re-boot from the stored boot drive if one exists
        if let Some(program) = self.loaded_program.clone() {
            log::info!(
                "Reloading program at {:04X}:{:04X}",
                program.segment,
                program.offset
            );
            // Ignore load errors during reset - just log them
            if let Err(e) = self.load_program(&program.data, program.segment, program.offset) {
                log::error!("Failed to reload program during reset: {}", e);
            }
        } else if let Some(drive) = self.boot_drive {
            log::info!("Rebooting from drive {}", drive.to_letter());
            // Ignore boot errors during reset - just log them
            if let Err(e) = self.boot(drive) {
                log::error!("Failed to reboot during reset: {}", e);
            }
        } else {
            log::info!("Reset called but no boot drive or program stored");
        }
    }

    /// Queue a keyboard IRQ to be processed before the next instruction
    ///
    /// This method should be called from the event loop when a keyboard event is detected.
    /// The IRQ will be processed at the next opportunity (before the next instruction),
    /// which simulates the asynchronous nature of hardware interrupts.
    ///
    /// The INT 09h handler will:
    /// 1. Add the key to the BIOS keyboard buffer
    /// 2. Call any custom INT 09h handlers installed by the program
    ///
    /// Programs like edit.exe install custom INT 09h handlers to implement enhanced
    /// keyboard features and maintain their own keyboard buffers.
    pub fn process_keyboard_irq(&mut self, key: KeyPress) {
        log::trace!(
            "Queueing keyboard IRQ: scan=0x{:02X}, ascii=0x{:02X}",
            key.scan_code,
            key.ascii_code
        );
        self.pending_keyboard_irqs.push_back(key);
    }

    /// Fire INT 09h (keyboard hardware interrupt)
    ///
    /// The key data (scan code and ASCII) is stored for the INT 09h handler to read.
    /// - Port 0x60 stores the scan code (for programs that read the keyboard controller directly)
    /// - The BIOS struct stores both scan code and ASCII for the default BIOS handler
    /// - The default BIOS INT 09h handler adds keys to the keyboard buffer
    /// - Programs can install custom INT 09h handlers to intercept keyboard input directly
    ///
    /// Returns true if the interrupt was fired, false if blocked (IF=0).
    fn fire_keyboard_irq(&mut self, key: KeyPress) -> bool {
        use crate::cpu::cpu_flag;

        // Only fire if interrupts are enabled
        if !self.cpu.get_flag(cpu_flag::INTERRUPT) {
            // Return false - caller should NOT remove from queue
            return false;
        }

        // Set keyboard data:
        // 1. Port 0x60 for custom INT 09h handlers that read the keyboard controller
        // 2. BIOS pending fields for the default BIOS INT 09h handler
        self.io_device
            .set_keyboard_data(key.scan_code, key.ascii_code);
        self.bios.pending_scan_code = key.scan_code;
        self.bios.pending_ascii_code = key.ascii_code;

        log::trace!(
            "INT 09h: Firing keyboard IRQ - Scan: 0x{:02X}, ASCII: 0x{:02X}",
            key.scan_code,
            key.ascii_code
        );

        // Check if custom INT 09h handler is installed
        let int_num: u8 = 0x09;
        let is_bios_handler = Cpu::is_bios_handler(&mut self.bus, int_num);

        if !is_bios_handler {
            // Custom INT 09h handler installed by program (e.g., CheckIt, edit.exe)
            //
            // Pre-buffer the key for custom handlers (like real PC hardware behavior).
            // Custom handlers expect the BIOS to have already processed the key.
            // If they chain to F000:0009, our BIOS handler will detect the duplicate
            // and skip re-adding it.
            //
            // Skip key releases (bit 7 set) - only buffer key presses
            if key.scan_code & 0x80 == 0 {
                use memory::{BDA_KEYBOARD_BUFFER_HEAD, BDA_KEYBOARD_BUFFER_TAIL, BDA_START};

                let head_addr = BDA_START + BDA_KEYBOARD_BUFFER_HEAD;
                let tail_addr = BDA_START + BDA_KEYBOARD_BUFFER_TAIL;
                let head = self.bus.read_u16(head_addr);
                let tail = self.bus.read_u16(tail_addr);

                // Calculate what tail would be after adding this key
                let buffer_start: u16 = 0x001E; // Relative to BDA
                let new_tail = if tail == buffer_start + 30 {
                    buffer_start // Wrap around
                } else {
                    tail + 2
                };

                // Check if buffer would become full
                if new_tail != head {
                    // Add key to buffer
                    let char_addr = BDA_START + tail as usize;
                    self.bus.write_u8(char_addr, key.scan_code);
                    self.bus.write_u8(char_addr + 1, key.ascii_code);
                    self.bus.write_u16(tail_addr, new_tail);
                    self.bios.key_was_prebuffered = true; // Mark as pre-buffered
                    log::debug!("INT 09h: Pre-buffered key for custom handler");
                } else {
                    log::warn!("INT 09h: Keyboard buffer full, discarding key");
                }
            } else {
                // Key release - not pre-buffered
                self.bios.key_was_prebuffered = false;
            }

            // Call the custom INT 09h handler
            let ivt_addr = (int_num as usize) * 4;
            let offset = self.bus.read_u16(ivt_addr);
            let segment = self.bus.read_u16(ivt_addr + 2);

            log::trace!(
                "INT 09h: Calling custom handler at {:04X}:{:04X}",
                segment,
                offset
            );

            if self.exec_logging_enabled {
                log::info!(
                    "IRQ 09h: {:04X}:{:04X} -> {:04X}:{:04X}",
                    self.cpu.cs,
                    self.cpu.ip,
                    segment,
                    offset
                );
            }

            // Push flags, CS, IP (simulating INT instruction)
            self.cpu.push(self.cpu.flags, &mut self.bus);
            self.cpu.push(self.cpu.cs, &mut self.bus);
            self.cpu.push(self.cpu.ip, &mut self.bus);

            // Clear IF and TF flags (standard INT behavior)
            self.cpu.set_flag(cpu_flag::INTERRUPT, false);
            self.cpu.set_flag(cpu_flag::TRAP, false);

            // Jump to INT 09h handler
            self.cpu.cs = segment;
            self.cpu.ip = offset;
        } else {
            // BIOS INT 09h handler - call Rust handler directly
            // The Rust handler will add the key to the buffer (no pre-buffering)
            self.bios.key_was_prebuffered = false; // Not pre-buffered for default handler
            log::debug!("INT 09h: Calling default BIOS handler");
            self.cpu.handle_bios_interrupt_direct(
                int_num,
                &mut self.bus,
                &mut self.bios,
                self.cpu_type,
            );
        }

        true
    }

    /// Queue a serial port interrupt (IRQ3 for COM2, IRQ4 for COM1)
    ///
    /// This should be called when serial data arrives and interrupts are enabled.
    /// port_num: 0 for COM1, 1 for COM2
    pub fn process_serial_irq(&mut self, port_num: u8) {
        log::debug!("Queueing serial IRQ for COM{}", port_num + 1);
        self.pending_serial_irqs.push_back(port_num);
    }

    /// Fire INT 0x0C (IRQ4, COM1) or INT 0x0B (IRQ3, COM2)
    ///
    /// This is the hardware interrupt for serial port data reception.
    /// Returns true if the interrupt was fired, false if blocked (IF=0).
    fn fire_serial_irq(&mut self, port_num: u8) -> bool {
        use crate::cpu::cpu_flag;

        // Only fire if interrupts are enabled
        if !self.cpu.get_flag(cpu_flag::INTERRUPT) {
            // Return false - caller should NOT remove from queue
            return false;
        }

        // COM1 = IRQ4 = INT 0x0C, COM2 = IRQ3 = INT 0x0B
        let int_num = if port_num == 0 { 0x0C } else { 0x0B };

        let ivt_addr = (int_num as usize) * 4;
        let offset = self.bus.read_u16(ivt_addr);
        let segment = self.bus.read_u16(ivt_addr + 2);

        log::debug!(
            "INT 0x{:02X}: Firing serial IRQ for COM{} - handler at {:04X}:{:04X}",
            int_num,
            port_num + 1,
            segment,
            offset
        );

        if self.exec_logging_enabled {
            log::info!(
                "IRQ {:02X}h: {:04X}:{:04X} -> {:04X}:{:04X}",
                int_num,
                self.cpu.cs,
                self.cpu.ip,
                segment,
                offset
            );
        }

        // Push flags, CS, IP (simulating INT instruction)
        self.cpu.push(self.cpu.flags, &mut self.bus);
        self.cpu.push(self.cpu.cs, &mut self.bus);
        self.cpu.push(self.cpu.ip, &mut self.bus);

        // Clear IF and TF flags (standard INT behavior)
        self.cpu.set_flag(cpu_flag::INTERRUPT, false);
        self.cpu.set_flag(cpu_flag::TRAP, false);

        // Jump to interrupt handler
        self.cpu.cs = segment;
        self.cpu.ip = offset;

        true
    }

    /// Fire INT 0x08 (Timer Hardware Interrupt / IRQ0)
    ///
    /// This fires the system timer interrupt that occurs 18.2 times per second.
    /// The INT 0x08 handler increments the BDA timer counter and chains to INT 0x1C.
    ///
    /// Returns true if the IRQ was fired, false if interrupts were disabled.
    /// Unified timer IRQ processing - handles both normal and inline cases
    ///
    /// Returns true if IRQ was processed (and caller should decrement pending count)
    fn process_timer_irq(&mut self) -> bool {
        use crate::cpu::cpu_flag;

        // Only process if interrupts are enabled
        if !self.cpu.get_flag(cpu_flag::INTERRUPT) {
            return false;
        }

        // Wake CPU from HLT state
        if self.cpu.is_halted() {
            self.cpu.clear_halt();
        }

        // Check if custom INT 08h handler is installed
        let int_num: u8 = 0x08;
        let is_bios_handler = Cpu::is_bios_handler(&mut self.bus, int_num);

        if !is_bios_handler {
            // Custom INT 08h handler - use full interrupt with stack frame
            let ivt_addr = (int_num as usize) * 4;
            let offset = self.bus.read_u16(ivt_addr);
            let segment = self.bus.read_u16(ivt_addr + 2);

            if self.log_interrupts_enabled {
                log::info!(
                    "INT 0x08: Firing timer IRQ (custom) - handler at {:04X}:{:04X}",
                    segment,
                    offset
                );
            }

            if self.exec_logging_enabled {
                log::info!(
                    "IRQ 08h: {:04X}:{:04X} -> {:04X}:{:04X}",
                    self.cpu.cs,
                    self.cpu.ip,
                    segment,
                    offset
                );
            }

            // Push flags, CS, IP (simulating INT instruction)
            self.cpu.push(self.cpu.flags, &mut self.bus);
            self.cpu.push(self.cpu.cs, &mut self.bus);
            self.cpu.push(self.cpu.ip, &mut self.bus);

            // Clear IF and TF flags (standard INT behavior)
            self.cpu.set_flag(cpu_flag::INTERRUPT, false);
            self.cpu.set_flag(cpu_flag::TRAP, false);

            // Jump to INT 0x08 handler
            self.cpu.cs = segment;
            self.cpu.ip = offset;
        } else {
            // BIOS INT 08h handler - update BDA counter directly from memory
            let counter_addr = memory::BDA_START + memory::BDA_TIMER_COUNTER;
            let lo = self.bus.read_u16(counter_addr) as u32;
            let hi = self.bus.read_u16(counter_addr + 2) as u32;
            let mut tick_count = (hi << 16) | lo;
            tick_count = tick_count.wrapping_add(1);

            // Check for midnight rollover (ticks per day = 0x001800B0)
            let ticks_per_day = 0x001800B0;
            if tick_count >= ticks_per_day {
                tick_count = 0;
                // Set midnight overflow flag
                let overflow_addr = memory::BDA_START + memory::BDA_TIMER_OVERFLOW;
                self.bus.write_u8(overflow_addr, 1);
            }

            // Write updated counter to BDA
            self.bus
                .write_u16(counter_addr, (tick_count & 0xFFFF) as u16);
            self.bus
                .write_u16(counter_addr + 2, (tick_count >> 16) as u16);

            // Check if custom INT 1Ch handler is installed
            let is_custom_1ch = !Cpu::is_bios_handler(&mut self.bus, 0x1C);

            if is_custom_1ch {
                // Custom INT 1Ch - chain to it
                let return_cs = self.cpu.cs;
                let return_ip = self.cpu.ip;
                let return_flags = self.cpu.flags;

                let ivt_1c = 0x1C * 4;
                let handler_off = self.bus.read_u16(ivt_1c);
                let handler_seg = self.bus.read_u16(ivt_1c + 2);

                if self.exec_logging_enabled {
                    log::info!(
                        "IRQ 08h (->1Ch): {:04X}:{:04X} -> {:04X}:{:04X}",
                        return_cs,
                        return_ip,
                        handler_seg,
                        handler_off
                    );
                }

                self.cpu.begin_irq_chain(
                    0x08,
                    0x1C,
                    return_cs,
                    return_ip,
                    return_flags,
                    &mut self.bus,
                );

                if self.log_interrupts_enabled {
                    log::info!(
                        "INT 0x08: Chaining to custom INT 0x1Ch at {:04X}:{:04X}",
                        self.cpu.cs,
                        self.cpu.ip
                    );
                }
            } else if self.exec_logging_enabled {
                log::info!("IRQ 08h: tick at {:04X}:{:04X}", self.cpu.cs, self.cpu.ip);
            }
            // BIOS INT 1Ch is a no-op, so nothing more to do
        }

        true
    }

    pub fn run(&mut self) {
        while !self.cpu.is_halted() {
            self.step();
            self.update_video();
        }
    }

    /// Execute a single instruction
    pub fn step(&mut self) {
        self.step_count += 1;

        // If CPU is busy-waiting (INT 15h AH=86h), burn cycles instead of executing
        // This simulates busy-waiting like real hardware, works for both native and WASM
        if self.cpu.pending_sleep_cycles > 0 {
            // Burn cycles equivalent to a NOP instruction (~3 cycles)
            const CYCLES_PER_WAIT_STEP: u64 = 3;
            let cycles_to_burn = CYCLES_PER_WAIT_STEP.min(self.cpu.pending_sleep_cycles);

            self.increment_cycles(cycles_to_burn);
            self.cpu.pending_sleep_cycles -= cycles_to_burn;

            // Return early - don't execute an instruction while waiting
            return;
        }

        // Check if CPU is waiting for keyboard input
        if self.cpu.is_waiting_for_keyboard() {
            // Drain any pending keyboard IRQs, looking for a key press.
            // Key releases (scan_code bit 7 set) are discarded - INT 16h AH=00h only
            // cares about key presses, and processing releases via fire_keyboard_irq
            // would corrupt IF (custom INT 09h handlers clear IF on entry) causing
            // subsequent key presses to be lost.
            while let Some(key) = self.pending_keyboard_irqs.pop_front() {
                if key.scan_code & 0x80 != 0 {
                    // Key release - discard silently
                    log::debug!(
                        "Discarding key release 0x{:02X} while waiting for INT 16h keyboard input",
                        key.scan_code
                    );
                    continue;
                }
                // Key press - directly pre-buffer in BDA without dispatching INT 09h.
                // Bypassing INT 09h avoids IF corruption from custom handlers and
                // correctly unblocks INT 16h AH=00h regardless of DOS hooks.
                log::debug!(
                    "Pre-buffering key press (scan=0x{:02X}, ascii=0x{:02X}) for INT 16h wait-state resume",
                    key.scan_code,
                    key.ascii_code
                );
                {
                    use memory::{BDA_KEYBOARD_BUFFER_HEAD, BDA_KEYBOARD_BUFFER_TAIL, BDA_START};
                    let head_addr = BDA_START + BDA_KEYBOARD_BUFFER_HEAD;
                    let tail_addr = BDA_START + BDA_KEYBOARD_BUFFER_TAIL;
                    let head = self.bus.read_u16(head_addr);
                    let tail = self.bus.read_u16(tail_addr);
                    let buffer_start: u16 = 0x001E;
                    let new_tail = if tail == buffer_start + 30 {
                        buffer_start
                    } else {
                        tail + 2
                    };
                    if new_tail != head {
                        let char_addr = BDA_START + tail as usize;
                        self.bus.write_u8(char_addr, key.scan_code);
                        self.bus.write_u8(char_addr + 1, key.ascii_code);
                        self.bus.write_u16(tail_addr, new_tail);
                    } else {
                        log::warn!("Keyboard buffer full while in wait state, discarding key");
                    }
                }
                break;
            }

            // Check if a key is available in the BDA keyboard buffer
            let head_addr = memory::BDA_START + memory::BDA_KEYBOARD_BUFFER_HEAD;
            let tail_addr = memory::BDA_START + memory::BDA_KEYBOARD_BUFFER_TAIL;
            let head = self.bus.read_u16(head_addr);
            let tail = self.bus.read_u16(tail_addr);

            if head != tail {
                log::debug!(
                    "Key available in BDA buffer, resuming from wait state and retrying INT 16h"
                );
                if self.cpu.resume_from_wait() {
                    self.cpu.int16_read_char(&mut self.bus, &mut self.bios);
                    return;
                }
                // Fall through to execute the next instruction
            } else {
                // Still waiting - return without executing
                return;
            }
        }

        // Process pending keyboard IRQs before executing the next instruction
        // This simulates hardware interrupts that preempt normal execution
        // Only pop from queue if we actually fire (IF=1)
        if !self.pending_keyboard_irqs.is_empty()
            && self.cpu.get_flag(crate::cpu::cpu_flag::INTERRUPT)
            && let Some(key) = self.pending_keyboard_irqs.pop_front()
        {
            self.fire_keyboard_irq(key);
            // After firing the IRQ, return to let the INT 09h handler execute
            // The handler will run on subsequent step() calls
            return;
        }
        // If IF=0, leave the IRQ in queue for later

        // Process pending serial IRQs
        // Only pop from queue if we actually fire (IF=1)
        if let Some(&port_num) = self.pending_serial_irqs.front()
            && self.fire_serial_irq(port_num)
        {
            self.pending_serial_irqs.pop_front();
            return;
        }
        // If IF=0, leave the IRQ in queue for later

        // Process pending timer IRQs (INT 0x08)
        // Timer has lowest priority among hardware interrupts
        if self.pending_timer_irqs > 0 {
            // Only decrement if we actually processed the IRQ (IF was enabled)
            if self.process_timer_irq() {
                self.dec_pending_timer_irqs();
                return;
            }
        }

        // Get current IP to check what opcode we're about to execute
        let current_ip = self.cpu.ip;
        let current_cs = self.cpu.cs;

        // Check if we're executing in BIOS ROM area (0xF000 segment)
        // This handles DOS interrupt handlers that chain back to BIOS via PUSHF + CALL FAR
        if current_cs == 0xF000 {
            // The IVT was initialized with unique offsets for each interrupt:
            // INT 0x13 -> F000:0013, INT 0x21 -> F000:0021, etc.
            // The offset tells us which interrupt this is!
            let int_num = (current_ip & 0xFF) as u8;

            // Extra logging for INT 09h to debug custom handlers
            if int_num == 0x09 {
                log::debug!(
                    "INT 09h: Custom handler chained to BIOS at F000:{:04X}, scan=0x{:02X}, ascii=0x{:02X}",
                    current_ip,
                    self.bios.pending_scan_code,
                    self.bios.pending_ascii_code
                );
            }

            if self.log_interrupts_enabled {
                log::info!(
                    "BIOS ROM execution detected at {:04X}:{:04X}, handling as INT 0x{:02X}",
                    current_cs,
                    current_ip,
                    int_num
                );
            }

            // DOS typically does: PUSHF, CALL FAR old_handler
            // The stack has: [SP] = IP, [SP+2] = CS, [SP+4] = FLAGS
            // Pop the return address (simulating return from CALL FAR)
            let ret_offset = self.cpu.pop(&self.bus);
            let ret_segment = self.cpu.pop(&self.bus);

            // Call our BIOS handler directly
            // We need to bypass the IVT check since we're already being called via CALL FAR from DOS
            self.cpu.handle_bios_interrupt_direct(
                int_num,
                &mut self.bus,
                &mut self.bios,
                self.cpu_type,
            );

            // Check if handler started an IRQ chain (e.g., INT 0x08 chains to INT 0x1C)
            if self.cpu.is_in_irq_chain() {
                // Chain in progress - let it complete naturally via IRET
                // The chain context has already been set up with proper return address
                return;
            }

            // No chaining - normal return path
            //
            // BIOS interrupt handlers return via IRET, which pops IP, CS, FLAGS.
            // We already popped ret_IP and ret_CS above. Now we need to pop FLAGS
            // to complete the IRET simulation.
            //
            // Two scenarios with same stack handling:
            // 1. Direct INT/IRQ to F000: Stack had [IP, CS, FLAGS] from INT instruction
            // 2. Chained via PUSHF+CALL FAR: Stack had [ret_IP, ret_CS, PUSHF'd FLAGS, ...]
            //    The PUSHF'd FLAGS is what we pop here (simulating BIOS doing IRET)
            //
            // In both cases, popping FLAGS is correct:
            // - Case 1: We restore the original caller's FLAGS
            // - Case 2: We restore the DOS handler's PUSHF'd FLAGS (IF=0), and the
            //           DOS handler will later do IRET to restore the original caller's FLAGS
            let saved_flags = self.cpu.pop(&self.bus);
            // Restore IF, TF, DF from saved flags (keep CF, ZF, etc. from BIOS handler)
            self.cpu.flags = (self.cpu.flags & 0xF8FF) | (saved_flags & 0x0700);

            // Return to caller
            self.cpu.ip = ret_offset;
            self.cpu.cs = ret_segment;

            // Process pending timer IRQs inline if IF=1
            // This simulates the timer IRQs that would fire during disk I/O on real hardware.
            // Note: We do NOT process keyboard IRQs inline because custom INT 09h handlers
            // need proper stack frames for IRET. Keyboard IRQs will fire immediately after
            // this return completes (if IF=1) via the normal IRQ processing in step().
            while self.cpu.get_flag(crate::cpu::cpu_flag::INTERRUPT) && self.pending_timer_irqs > 0
            {
                if self.process_timer_irq() {
                    self.dec_pending_timer_irqs();
                }
            }

            return;
        }

        let addr = Cpu::physical_address(current_cs, current_ip);
        let opcode = self.bus.read_u8(addr);

        if self.exec_logging_enabled {
            let decoded = crate::decoder::decode_instruction_with_regs(
                self.bus.memory(),
                current_cs,
                current_ip,
                Some(&self.cpu),
            );

            // Combine register and memory values for logging
            let mut values = String::new();
            if !decoded.reg_values.is_empty() {
                values.push_str(&decoded.reg_values);
            }
            if !decoded.mem_values.is_empty() {
                if !values.is_empty() {
                    values.push(' ');
                }
                values.push_str(&decoded.mem_values);
            }

            let bytes_hex = decoded
                .bytes
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            log::info!(
                "OP {:04X}:{:04X} {:<18} {:30} {}",
                current_cs,
                current_ip,
                bytes_hex,
                decoded.text,
                values,
            );
        }

        // Check if it's an INT instruction
        match opcode {
            0xCD => {
                // INT with immediate - need to fetch the interrupt number
                let int_num = self.bus.read_u8(addr + 1);

                // Before executing INT 16h (keyboard read), flush any pending video
                // updates so the screen is current before we potentially block.
                if int_num == 0x16 && self.bus.video().is_dirty() {
                    self.update_video();
                }

                // Before executing INT 1Ah (time) or INT 21h (DOS), sync BDA timer
                // to include pending timer ticks. This ensures accurate time reading
                // even when IF has been 0 for extended periods.
                if int_num == 0x1A || int_num == 0x21 {
                    self.sync_bda_timer();
                }

                // Manually advance IP past the INT instruction
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                // Execute with BIOS I/O
                self.cpu
                    .execute_int_with_io(int_num, &mut self.bus, &mut self.bios, self.cpu_type);
                // Set cycle count for INT instruction (51 cycles)
                // Only set if the interrupt handler didn't already set a custom cycle count
                // (e.g., INT 15h AH=86h sets cycles for the wait duration)
                if self.cpu.last_instruction_cycles == 0 {
                    self.cpu.last_instruction_cycles = crate::cpu::timing::cycles::INT;
                }
            }
            0xCC => {
                // INT 3 - advance IP and execute INT 3
                log::info!("INT 0x03 (breakpoint)");
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                self.cpu
                    .execute_int_with_io(3, &mut self.bus, &mut self.bios, self.cpu_type);
                // Set cycle count for INT 3 instruction (52 cycles)
                self.cpu.last_instruction_cycles = crate::cpu::timing::cycles::INT3;
            }
            _ => {
                // Normal instruction - use execute_with_io
                let opcode = self.cpu.fetch_byte(&self.bus);
                self.cpu.execute_with_io(
                    opcode,
                    &mut self.bus,
                    &mut self.bios,
                    &mut self.io_device,
                );
                // Check for CPU exceptions raised during instruction (e.g. divide error = INT 0)
                if let Some(exc) = self.cpu.pending_exception.take() {
                    self.cpu
                        .execute_int_with_io(exc, &mut self.bus, &mut self.bios, self.cpu_type);
                    self.cpu.last_instruction_cycles = crate::cpu::timing::cycles::INT;
                }
            }
        }

        // Sync A20 gate state from keyboard controller to memory
        // This must happen after instruction execution in case OUT instructions changed A20
        let a20_enabled = self.io_device.is_a20_enabled();
        self.bus.set_a20_enabled(a20_enabled);

        // Increment cycle counter and update timer
        // Use accurate cycle count from instruction, or fall back to 10 cycles
        // if timing not yet implemented for this instruction
        let cycles = if self.cpu.last_instruction_cycles > 0 {
            self.cpu.last_instruction_cycles
        } else {
            10 // Fallback for instructions without timing implemented
        };
        self.increment_cycles(cycles);
        self.cpu.last_instruction_cycles = 0; // Reset for next instruction

        // Update serial devices every 1000 instructions (~18 times per second)
        if self.step_count.is_multiple_of(1000) {
            let ports_with_interrupts = self.bios.update_serial_devices();
            for port_num in ports_with_interrupts {
                self.process_serial_irq(port_num);
            }
        }
    }

    pub fn dump_registers(&self) {
        self.cpu.dump_registers();
    }

    /// Update video display if needed (call periodically or after step)
    pub fn update_video(&mut self) {
        // Hide the BIOS logo the first time the screen scrolls
        if self.bus.video_mut().take_scroll_occurred() {
            self.video_controller.clear_logo_overlay();
        }

        // Check if video mode changed and notify controller
        if self.bus.video_mut().take_mode_changed() {
            let mode = self.bus.video().get_mode();
            log::info!(
                "Notifying video controller of mode change to 0x{:02X}",
                mode
            );
            // A program-initiated mode change clears the BIOS logo overlay
            self.video_controller.clear_logo_overlay();
            self.video_controller.set_video_mode(mode);
            // Update VGA DAC palette when mode changes (palette is reset on mode change)
            log::info!("Computer: Passing palette to renderer (mode change)");
            self.video_controller
                .update_vga_dac_palette(self.bus.video().get_vga_dac_palette());

            // Sync BDA video state (may have been set via CGA hardware registers, not INT 10h)
            let cols = self.bus.video().get_cols();
            let rows = self.bus.video().get_rows();
            self.bus.write_u8(
                crate::memory::BDA_START + crate::memory::BDA_VIDEO_MODE,
                mode,
            );
            self.bus.write_u16(
                crate::memory::BDA_START + crate::memory::BDA_SCREEN_COLUMNS,
                cols as u16,
            );
            let page_size = cols * rows * 2;
            self.bus.write_u16(
                crate::memory::BDA_START + crate::memory::BDA_VIDEO_PAGE_SIZE,
                page_size as u16,
            );
        }

        if self.bus.video().is_dirty() {
            // Update VGA DAC palette (in case it was modified via INT 10h or I/O ports)
            log::trace!("Computer: Passing palette to renderer (dirty)");
            self.video_controller
                .update_vga_dac_palette(self.bus.video().get_vga_dac_palette());

            // Rebuild rendering cache from VRAM (VRAM is the single source of truth)
            self.bus.video_mut().rebuild_cache();

            // Update video controller based on current mode
            match self.bus.video().get_mode_type() {
                crate::video::VideoMode::Text { .. } => {
                    self.video_controller
                        .update_display(self.bus.video().get_buffer());
                }
                crate::video::VideoMode::Graphics320x200 => {
                    let pixels = self.bus.video().get_cga_pixels();
                    if self.bus.video().is_composite_mode() {
                        // Composite CGA: render 320x200 2bpp data as composite artifact colors
                        self.video_controller
                            .update_graphics_640x200(&pixels, 15, 0, true);
                    } else {
                        // Pass AC palette registers 0-3 as the color map
                        // These map pixel values 0-3 to VGA DAC indices
                        let ac = self.bus.video().get_ac_palette();
                        let color_map = [ac[0], ac[1], ac[2], ac[3]];
                        self.video_controller
                            .update_graphics_320x200(&pixels, color_map);
                    }
                }
                crate::video::VideoMode::Graphics640x200 => {
                    let pixels = self.bus.video().get_cga_pixels();
                    self.video_controller.update_graphics_640x200(
                        &pixels,
                        15, // Foreground: always bright white
                        0,  // Background: always black
                        self.bus.video().is_composite_mode(),
                    );
                }
                crate::video::VideoMode::Graphics320x200x16 => {
                    let pixels = self.bus.video().get_ega_pixels();
                    self.video_controller.update_graphics_320x200x16(&pixels);
                }
                crate::video::VideoMode::Graphics320x200x256 => {
                    let pixels = self.bus.video().get_vga_pixels();
                    self.video_controller.update_graphics_320x200x256(&pixels);
                }
            }
            self.bus.video_mut().clear_dirty();
        }
        // Always update cursor position (cursor moves don't dirty the buffer)
        self.video_controller
            .update_cursor(self.bus.video().get_cursor());
    }

    /// Update speaker output (call periodically for platforms that need it)
    pub fn update_speaker_output(&mut self) {
        self.speaker.update();
    }

    /// Force a full video redraw regardless of dirty state
    /// Used when terminal state is known to be out of sync (e.g., after clearing screen)
    pub fn force_video_redraw(&mut self) {
        // Force redraw based on current mode
        match self.bus.video().get_mode_type() {
            crate::video::VideoMode::Text { .. } => {
                self.video_controller
                    .force_redraw(self.bus.video().get_buffer());
            }
            crate::video::VideoMode::Graphics320x200 => {
                let pixels = self.bus.video().get_cga_pixels();
                if self.bus.video().is_composite_mode() {
                    self.video_controller
                        .update_graphics_640x200(&pixels, 15, 0, true);
                } else {
                    let ac = self.bus.video().get_ac_palette();
                    let color_map = [ac[0], ac[1], ac[2], ac[3]];
                    self.video_controller
                        .update_graphics_320x200(&pixels, color_map);
                }
            }
            crate::video::VideoMode::Graphics640x200 => {
                let pixels = self.bus.video().get_cga_pixels();
                self.video_controller.update_graphics_640x200(
                    &pixels,
                    15,
                    0,
                    self.bus.video().is_composite_mode(),
                );
            }
            crate::video::VideoMode::Graphics320x200x16 => {
                let pixels = self.bus.video().get_ega_pixels();
                self.video_controller.update_graphics_320x200x16(&pixels);
            }
            crate::video::VideoMode::Graphics320x200x256 => {
                let pixels = self.bus.video().get_vga_pixels();
                self.video_controller.update_graphics_320x200x256(&pixels);
            }
        }
        self.bus.video_mut().clear_dirty();
        self.video_controller
            .update_cursor(self.bus.video().get_cursor());
    }

    /// Get video buffer for inspection
    pub fn get_video_buffer(&self) -> &TextBuffer {
        self.bus.video().get_buffer()
    }

    /// Check if CPU is halted
    pub fn is_halted(&self) -> bool {
        self.cpu.is_halted()
    }

    /// Get total cycles executed
    pub fn get_cycle_count(&self) -> u64 {
        // Return total cycles: (cycles_per_tick * number of ticks) + remaining cycles
        // For simplicity, we track a separate total
        self.total_cycles
    }

    /// Get the CPU type being emulated
    pub fn cpu_type(&self) -> CpuType {
        self.cpu_type
    }

    /// Get a reference to the BIOS (for disk saving on exit, etc.)
    pub fn bios(&self) -> &Bios {
        &self.bios
    }

    /// Get a mutable reference to the BIOS (for runtime operations like disk swapping)
    pub fn bios_mut(&mut self) -> &mut Bios {
        &mut self.bios
    }

    /// Update BDA hard drive count after adding/removing drives
    /// This should be called after any operation that changes the number of hard drives
    pub fn update_bda_hard_drive_count(&mut self) {
        let hard_drive_count = self
            .bios
            .disk_get_params(DriveNumber::hard_drive_c())
            .map(|params| params.drive_count)
            .unwrap_or(0);
        self.bus.write_u8(
            memory::BDA_START + memory::BDA_NUM_HARD_DRIVES,
            hard_drive_count,
        );
        log::info!(
            "Updated BDA hard drive count to {} at offset 0x{:04X}",
            hard_drive_count,
            memory::BDA_START + memory::BDA_NUM_HARD_DRIVES
        );
    }

    /// Get a mutable reference to the video controller (for platform-specific rendering)
    pub fn video_controller_mut(&mut self) -> &mut V {
        &mut self.video_controller
    }

    /// Get an immutable reference to the video controller
    pub fn video_controller(&self) -> &V {
        &self.video_controller
    }

    /// Get a reference to the memory
    pub fn memory(&self) -> &Memory {
        self.bus.memory()
    }

    /// Update keyboard shift flags in the BIOS Data Area
    /// This should be called when modifier key state changes (Shift, Ctrl, Alt)
    ///
    /// # Arguments
    ///
    /// * `shift` - Shift key is pressed
    /// * `ctrl` - Ctrl key is pressed
    /// * `alt` - Alt key is pressed
    pub fn update_keyboard_flags(&mut self, shift: bool, ctrl: bool, alt: bool) {
        let flags_addr = memory::BDA_START + memory::BDA_KEYBOARD_FLAGS1;
        let mut flags = self.bus.read_u8(flags_addr);

        // Bit 0: Right Shift pressed
        // Bit 1: Left Shift pressed
        // We don't distinguish between left/right shift, so set both if shift is pressed
        if shift {
            flags |= 0x03; // Set both left and right shift bits
        } else {
            flags &= !0x03; // Clear both shift bits
        }

        // Bit 2: Ctrl pressed
        if ctrl {
            flags |= 0x04;
        } else {
            flags &= !0x04;
        }

        // Bit 3: Alt pressed
        if alt {
            flags |= 0x08;
        } else {
            flags &= !0x08;
        }

        self.bus.write_u8(flags_addr, flags);
    }

    /// Increment cycle counter and queue timer interrupts when tick threshold reached.
    /// The tick rate is derived from PIT Channel 0's actual count register, so programs
    /// that reprogram the PIT (e.g., games wanting 100 Hz or 1000 Hz timers) will
    /// automatically get the correct interrupt rate.
    fn increment_cycles(&mut self, cycles: u64) {
        self.cycle_count += cycles;
        self.total_cycles += cycles;
        self.speaker_update_cycles += cycles;

        // Update PIT counters
        self.io_device.update_pit(cycles);

        // Update joystick cycle counter (for axis timing)
        self.io_device.update_cycles(self.total_cycles);

        // Update speaker periodically (every ~100 cycles) to reduce overhead
        // This is ~47,700 times per second at 4.77 MHz, plenty for audio
        if self.speaker_update_cycles >= 100 {
            self.speaker_update_cycles = 0;
            self.update_speaker();
        }

        // Advance sound card (AdLib OPL2 tick, sample generation).
        self.io_device.tick_sound_card(cycles);

        // Derive cycles_per_tick from PIT Channel 0's current count register.
        // Default count of 0 means 65536, giving ~18.2 Hz. Programs can write
        // a smaller divisor for faster timer rates (e.g., 1193 for ~1000 Hz).
        // Formula: cpu_cycles_per_tick = pit_count * (CPU_FREQ / PIT_FREQ)
        let cycles_per_tick = self.get_cycles_per_tick();

        // Queue timer interrupts when tick threshold reached
        // The INT 0x08 handler will update BDA timer counter and chain to INT 0x1C
        while self.cycle_count >= cycles_per_tick {
            self.cycle_count -= cycles_per_tick;
            self.inc_pending_timer_irqs();
            // Warn if many timer IRQs are pending (IF has been 0 for too long)
            if self.pending_timer_irqs == 10 {
                log::warn!(
                    "10 timer IRQs pending! IF=0 for extended period. CS:IP={:04X}:{:04X} FLAGS=0x{:04X}",
                    self.cpu.cs,
                    self.cpu.ip,
                    self.cpu.flags
                );
            }
        }
    }

    /// Calculate CPU cycles per timer tick from PIT Channel 0's count register.
    /// PIT base frequency is 1,193,182 Hz. CPU runs at 4,770,000 Hz.
    /// cycles_per_tick = pit_count * (4_770_000 / 1_193_182)
    fn get_cycles_per_tick(&self) -> u64 {
        let pit_count = self.io_device.pit().get_channel_count(0);
        let count = if pit_count == 0 {
            65536u64
        } else {
            pit_count as u64
        };
        // Use integer math: count * 4_770_000 / 1_193_182
        // For default count 65536: 65536 * 4770000 / 1193182 = 261,887 (~262K cycles)
        (count * 4_770_000) / 1_193_182
    }

    /// Increment pending timer IRQs
    #[inline]
    fn inc_pending_timer_irqs(&mut self) {
        self.pending_timer_irqs += 1;
    }

    /// Decrement pending timer IRQs counter
    #[inline]
    fn dec_pending_timer_irqs(&mut self) {
        self.pending_timer_irqs -= 1;
    }

    /// Write BDA timer counter directly to memory
    fn write_bda_timer(&mut self, tick_count: u32) {
        let counter_addr = memory::BDA_START + memory::BDA_TIMER_COUNTER;
        self.bus
            .write_u16(counter_addr, (tick_count & 0xFFFF) as u16);
        self.bus
            .write_u16(counter_addr + 2, (tick_count >> 16) as u16);
    }

    /// Read BDA timer counter (includes pending IRQs)
    ///
    /// Returns the accurate current tick count by reading memory and adding pending IRQs.
    /// This ensures accurate time even when interrupts are disabled.
    fn read_bda_timer(&self) -> u32 {
        let counter_addr = memory::BDA_START + memory::BDA_TIMER_COUNTER;
        let lo = self.bus.read_u16(counter_addr) as u32;
        let hi = self.bus.read_u16(counter_addr + 2) as u32;
        let bda_value = (hi << 16) | lo;
        bda_value.wrapping_add(self.pending_timer_irqs)
    }

    /// Synchronize BDA timer counter with pending timer ticks
    ///
    /// This updates the BDA timer counter to reflect all accumulated timer ticks,
    /// including those that are pending (queued but INT 08h hasn't fired yet due to IF=0).
    /// This ensures that time-reading functions (INT 1Ah, INT 21h AH=2Ch) return accurate
    /// time even when interrupts have been disabled for extended periods.
    fn sync_bda_timer(&mut self) {
        if self.pending_timer_irqs == 0 {
            return; // No pending ticks, BDA is already up to date
        }

        // Calculate the accurate current tick count
        let new_counter = self.read_bda_timer();

        // Check for midnight rollover (ticks per day = 0x001800B0)
        let ticks_per_day = 0x001800B0;
        let (final_counter, overflow) = if new_counter >= ticks_per_day {
            (new_counter - ticks_per_day, true)
        } else {
            (new_counter, false)
        };

        // Write updated counter to BDA
        self.write_bda_timer(final_counter);

        // Set midnight overflow flag if we rolled over
        if overflow {
            let overflow_addr = memory::BDA_START + memory::BDA_TIMER_OVERFLOW;
            self.bus.write_u8(overflow_addr, 1);
        }

        // Clear pending IRQs since we've applied them to the BDA
        // Note: We don't actually fire INT 08h here, just update the counter
        log::debug!(
            "Synced BDA timer: applied {} pending ticks, counter now {}",
            self.pending_timer_irqs,
            final_counter
        );
        self.pending_timer_irqs = 0;
    }

    /// Update speaker output based on PIT Channel 2 state and port 0x61 control bits
    fn update_speaker(&mut self) {
        let control_bits = self.io_device.system_control_port().get_control_bits();
        let timer2_gate = (control_bits & 0x01) != 0;
        let speaker_data = (control_bits & 0x02) != 0;

        // Speaker enabled when both gate and data bits set
        let enabled = timer2_gate && speaker_data;

        // Also check if PIT channel 2 has a valid count loaded (not being reprogrammed)
        // This prevents high-pitched sounds during note transitions when the PIT is
        // being reconfigured with a new frequency
        let pit_ready = self.io_device.pit().is_channel_ready(2);

        if enabled && pit_ready {
            let count = self.io_device.pit().get_channel_count(2);
            if count > 0 {
                let frequency = 1193182.0 / (count as f32);
                self.speaker.set_frequency(true, frequency);
            } else {
                // count of 0 means 65536 (lowest frequency ~18.2 Hz)
                let frequency = 1193182.0 / 65536.0;
                self.speaker.set_frequency(true, frequency);
            }
        } else {
            self.speaker.set_frequency(false, 0.0);
        }
    }

    /// Set the sound card. Call before starting emulation.
    pub fn set_sound_card(&mut self, card: Box<dyn SoundCard>) {
        self.io_device.set_sound_card(card);
    }

    /// Pop `count` samples from the sound card's internal buffer (for WASM audio callbacks).
    /// Returns zeros if the sound card produces no audio or the buffer is empty.
    pub fn get_sound_card_samples(&mut self, count: usize) -> Vec<f32> {
        self.io_device.pop_sound_card_samples(count)
    }

    pub fn set_log_interrupts(&mut self, enable: bool) {
        self.log_interrupts_enabled = enable;
        self.cpu.log_interrupts_enabled = enable;
    }

    pub fn set_exec_logging(&mut self, enable: bool) {
        self.exec_logging_enabled = enable;
        log::info!(
            "exec logging {}",
            if enable { "enabled" } else { "disabled" }
        );
    }

    // Serial port device management

    /// Attach a device to COM1
    pub fn set_com1_device(&mut self, device: Box<dyn SerialDevice>) {
        self.bios.serial_ports[0].attach_device(device);
    }

    /// Attach a device to COM2
    pub fn set_com2_device(&mut self, device: Box<dyn SerialDevice>) {
        self.bios.serial_ports[1].attach_device(device);
    }

    /// Remove device from COM1
    pub fn clear_com1_device(&mut self) {
        self.bios.serial_ports[0].detach_device();
    }

    /// Remove device from COM2
    pub fn clear_com2_device(&mut self) {
        self.bios.serial_ports[1].detach_device();
    }
}
