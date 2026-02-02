use anyhow::Result;

use crate::{
    Bios, DriveNumber, KeyboardInput, MouseInput, NullVideoController, SerialDevice, SpeakerOutput,
    TextCell, Video, VideoController,
    cpu::Cpu,
    cpu::bios::KeyPress,
    io::IoDevice,
    memory::{self, Memory},
};

pub struct Computer<K: KeyboardInput, V: VideoController = NullVideoController> {
    cpu: Cpu,
    memory: Memory,
    bios: Bios<K>,
    io_device: IoDevice,
    video: Video,
    video_controller: V,
    speaker: Box<dyn SpeakerOutput>,
    /// Cycle counter for timer emulation (resets each tick)
    cycle_count: u64,
    /// Total cycles executed (never resets)
    total_cycles: u64,
    /// Cycles per timer tick (PIT frequency / 18.2 Hz)
    /// 8086 at 4.77 MHz: approximately 262144 cycles per tick
    cycles_per_tick: u64,
    /// Instruction step counter for debugging
    step_count: u64,

    /// if set to true, opcode execution will be logged as info level
    pub exec_logging_enabled: bool,
    /// if set to true, interrupts will be logged as info level
    log_interrupts_enabled: bool,
    log_steps: u32,

    /// Queue of pending keyboard IRQs (INT 09h)
    pending_keyboard_irqs: std::collections::VecDeque<KeyPress>,
    pending_serial_irqs: std::collections::VecDeque<u8>, // Serial port numbers (0=COM1, 1=COM2)
    /// Pending timer IRQs (INT 08h) - counter to handle multiple ticks if CPU is slow
    pending_timer_irqs: u32,
    /// Debug: track if we've logged timer IRQ blocking (to avoid spam)
    timer_irq_blocked_logged: bool,
}

impl<K: KeyboardInput, V: VideoController> Computer<K, V> {
    pub fn new(
        keyboard: K,
        mouse: Box<dyn MouseInput>,
        video_controller: V,
        speaker: Box<dyn SpeakerOutput>,
    ) -> Self {
        let bios = Bios::new(keyboard, mouse);

        let mut memory = Memory::new();
        memory.initialize_ivt();
        memory.initialize_bda();

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

        Self {
            cpu: Cpu::new(),
            memory,
            bios,
            io_device: IoDevice::new(),
            video: Video::new(),
            video_controller,
            speaker,
            cycle_count: 0,
            total_cycles: 0,
            // 8086 at 4.77 MHz with PIT at 18.2 Hz: ~262144 cycles per tick
            // This is approximate: 4770000 / 18.2 ≈ 262088
            cycles_per_tick: 262088,
            step_count: 0,
            exec_logging_enabled: false,
            log_interrupts_enabled: false,
            log_steps: 0,
            pending_keyboard_irqs: std::collections::VecDeque::new(),
            pending_serial_irqs: std::collections::VecDeque::new(),
            pending_timer_irqs: 0,
            timer_irq_blocked_logged: false,
        }
    }

    pub fn load_bios(&mut self, bios_data: &[u8]) -> Result<()> {
        self.memory.load_bios(bios_data)?;
        Ok(())
    }

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = Cpu::physical_address(segment, offset);
        self.memory.load_at(physical_addr, program_data)?;

        // Set CPU to start at this location
        self.cpu.cs = segment;
        self.cpu.ip = offset;

        // Initialize other segments to reasonable defaults
        self.cpu.ds = segment;
        self.cpu.es = segment;
        self.cpu.ss = segment;
        self.cpu.sp = 0xFFFE; // Stack grows down from top of segment

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
        if boot_sector[510] != 0x55 || boot_sector[511] != 0xAA {
            return Err(anyhow::anyhow!(
                "Invalid boot sector signature: expected 0x55AA, got 0x{:02X}{:02X}",
                boot_sector[511],
                boot_sector[510]
            ));
        }

        // Load boot sector to 0x0000:0x7C00 (physical address 0x7C00)
        const BOOT_SEGMENT: u16 = 0x0000;
        const BOOT_OFFSET: u16 = 0x7C00;
        let boot_addr = Cpu::physical_address(BOOT_SEGMENT, BOOT_OFFSET);
        self.memory.load_at(boot_addr, &boot_sector)?;

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

        Ok(())
    }

    pub fn reset(&mut self) {
        self.cpu.reset();
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
        log::debug!(
            "Queueing keyboard IRQ: scan=0x{:02X}, ascii=0x{:02X}",
            key.scan_code,
            key.ascii_code
        );
        self.pending_keyboard_irqs.push_back(key);
    }

    /// Fire INT 09h (keyboard hardware interrupt)
    ///
    /// This adds the key to the BIOS keyboard buffer and calls the INT 09h handler.
    /// Programs can install custom INT 09h handlers to intercept keyboard input.
    fn fire_keyboard_irq(&mut self, key: KeyPress) {
        use memory::{BDA_KEYBOARD_BUFFER_HEAD, BDA_KEYBOARD_BUFFER_TAIL, BDA_START};

        // Add key to BIOS keyboard buffer
        let head_addr = BDA_START + BDA_KEYBOARD_BUFFER_HEAD;
        let tail_addr = BDA_START + BDA_KEYBOARD_BUFFER_TAIL;
        let head = self.memory.read_u16(head_addr);
        let tail = self.memory.read_u16(tail_addr);

        // Calculate what tail would be after adding this key
        let buffer_start: u16 = 0x001E; // Relative to BDA
        let new_tail = if tail == buffer_start + 30 {
            buffer_start // Wrap around
        } else {
            tail + 2
        };

        // Check if buffer would become full
        if new_tail == head {
            // Buffer full - discard key
            log::warn!(
                "INT 09h: Keyboard buffer full! Discarding scan=0x{:02X}, ascii=0x{:02X}",
                key.scan_code,
                key.ascii_code
            );
            return;
        }

        // Add key to buffer
        let char_addr = BDA_START + tail as usize;
        self.memory.write_u8(char_addr, key.scan_code);
        self.memory.write_u8(char_addr + 1, key.ascii_code);
        self.memory.write_u16(tail_addr, new_tail);

        log::debug!(
            "INT 09h: Buffered key - Scan: 0x{:02X}, ASCII: 0x{:02X}",
            key.scan_code,
            key.ascii_code
        );

        // Call INT 09h handler
        let int_num = 0x09u8;
        let ivt_addr = (int_num as usize) * 4;
        let offset = self.memory.read_u16(ivt_addr);
        let segment = self.memory.read_u16(ivt_addr + 2);

        log::debug!("INT 09h: Calling handler at {:04X}:{:04X}", segment, offset);

        // Push flags, CS, IP (simulating INT instruction)
        self.cpu.push(self.cpu.flags, &mut self.memory);
        self.cpu.push(self.cpu.cs, &mut self.memory);
        self.cpu.push(self.cpu.ip, &mut self.memory);

        // Clear IF and TF flags (standard INT behavior)
        use crate::cpu::cpu_flag;
        self.cpu.set_flag(cpu_flag::INTERRUPT, false);
        self.cpu.set_flag(cpu_flag::TRAP, false);

        // Jump to INT 09h handler
        self.cpu.cs = segment;
        self.cpu.ip = offset;
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
    fn fire_serial_irq(&mut self, port_num: u8) {
        // COM1 = IRQ4 = INT 0x0C, COM2 = IRQ3 = INT 0x0B
        let int_num = if port_num == 0 { 0x0C } else { 0x0B };

        let ivt_addr = (int_num as usize) * 4;
        let offset = self.memory.read_u16(ivt_addr);
        let segment = self.memory.read_u16(ivt_addr + 2);

        log::debug!(
            "INT 0x{:02X}: Firing serial IRQ for COM{} - handler at {:04X}:{:04X}",
            int_num,
            port_num + 1,
            segment,
            offset
        );

        // Push flags, CS, IP (simulating INT instruction)
        self.cpu.push(self.cpu.flags, &mut self.memory);
        self.cpu.push(self.cpu.cs, &mut self.memory);
        self.cpu.push(self.cpu.ip, &mut self.memory);

        // Clear IF and TF flags (standard INT behavior)
        use crate::cpu::cpu_flag;
        self.cpu.set_flag(cpu_flag::INTERRUPT, false);
        self.cpu.set_flag(cpu_flag::TRAP, false);

        // Jump to interrupt handler
        self.cpu.cs = segment;
        self.cpu.ip = offset;
    }

    /// Fire INT 0x08 (Timer Hardware Interrupt / IRQ0)
    ///
    /// This fires the system timer interrupt that occurs 18.2 times per second.
    /// The INT 0x08 handler increments the BDA timer counter and chains to INT 0x1C.
    ///
    /// Returns true if the IRQ was fired, false if interrupts were disabled.
    fn fire_timer_irq(&mut self) -> bool {
        use crate::cpu::cpu_flag;

        // Only fire if interrupts are enabled
        if !self.cpu.get_flag(cpu_flag::INTERRUPT) {
            // Return false - caller should NOT decrement pending count
            return false;
        }

        // Wake CPU from HLT state
        if self.cpu.is_halted() {
            self.cpu.clear_halt();
        }

        let int_num: u8 = 0x08;
        let ivt_addr = (int_num as usize) * 4;
        let offset = self.memory.read_u16(ivt_addr);
        let segment = self.memory.read_u16(ivt_addr + 2);

        if self.log_interrupts_enabled {
            log::info!(
                "INT 0x08: Firing timer IRQ - handler at {:04X}:{:04X}",
                segment,
                offset
            );
        }

        // Push flags, CS, IP (simulating INT instruction)
        self.cpu.push(self.cpu.flags, &mut self.memory);
        self.cpu.push(self.cpu.cs, &mut self.memory);
        self.cpu.push(self.cpu.ip, &mut self.memory);

        // Clear IF and TF flags (standard INT behavior)
        self.cpu.set_flag(cpu_flag::INTERRUPT, false);
        self.cpu.set_flag(cpu_flag::TRAP, false);

        // Jump to INT 0x08 handler
        self.cpu.cs = segment;
        self.cpu.ip = offset;

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
        if self.log_steps > 0 {
            self.log_steps -= 1;
            if self.log_steps == 0 {
                self.set_log_interrupts(false);
                self.exec_logging_enabled = false;
            }
        }

        self.step_count += 1;

        // Process pending keyboard IRQs before executing the next instruction
        // This simulates hardware interrupts that preempt normal execution
        if let Some(key) = self.pending_keyboard_irqs.pop_front() {
            self.fire_keyboard_irq(key);
            // After firing the IRQ, return to let the INT 09h handler execute
            // The handler will run on subsequent step() calls
            return;
        }

        // Process pending serial IRQs
        if let Some(port_num) = self.pending_serial_irqs.pop_front() {
            self.fire_serial_irq(port_num);
            return;
        }

        // Process pending timer IRQs (INT 0x08)
        // Timer has lowest priority among hardware interrupts
        if self.pending_timer_irqs > 0 {
            // Only decrement if we actually fired the IRQ (IF was enabled)
            if self.fire_timer_irq() {
                self.pending_timer_irqs -= 1;
                self.timer_irq_blocked_logged = false; // Reset for next potential block
                return;
            }
            // IF=0, log periodically that timer IRQs are being blocked
            // Log every 10000 steps to avoid spam but show ongoing issues
            if self.step_count % 10000 == 0 {
                use crate::cpu::cpu_flag;
                log::warn!(
                    "Timer IRQs blocked: pending={}, IF={}, CS:IP={:04X}:{:04X}",
                    self.pending_timer_irqs,
                    self.cpu.get_flag(cpu_flag::INTERRUPT),
                    self.cpu.cs,
                    self.cpu.ip
                );
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
            let ret_offset = self.cpu.pop(&self.memory);
            let ret_segment = self.cpu.pop(&self.memory);

            // Call our BIOS handler directly
            // We need to bypass the IVT check since we're already being called via CALL FAR from DOS
            // Handle the interrupt directly based on int_num
            self.cpu.handle_bios_interrupt_direct(
                int_num,
                &mut self.memory,
                &mut self.bios,
                &mut self.video,
            );

            // Check if the handler called chain_to_interrupt() to chain to another handler
            // (e.g., INT 0x08 chains to INT 0x1C for user timer tick)
            // If CS:IP changed from F000:int_num, a chain is in progress - don't overwrite
            let chained = !(self.cpu.cs == 0xF000 && self.cpu.ip == current_ip);
            if int_num == 0x08 {
                log::info!(
                    "INT 0x08 handler done: chained={}, CS:IP={:04X}:{:04X}",
                    chained,
                    self.cpu.cs,
                    self.cpu.ip
                );
            }
            if !chained {
                // No chaining occurred - normal return path
                // Pop the FLAGS that DOS pushed before CALL FAR
                let saved_flags = self.cpu.pop(&self.memory);
                // BIOS may have modified flags (especially CF for error indication)
                // Merge: keep the modified CF, ZF, etc. from BIOS, but restore IF from DOS
                let old_if = (self.cpu.flags >> 9) & 1;
                let new_if = (saved_flags >> 9) & 1;
                self.cpu.flags = (self.cpu.flags & 0xF8FF) | (saved_flags & 0x0700); // Restore IF, TF, DF
                if int_num == 0x08 {
                    log::info!(
                        "INT 0x08 return: saved_flags=0x{:04X}, old_IF={}, new_IF={}, returning to {:04X}:{:04X}",
                        saved_flags, old_if, new_if, ret_segment, ret_offset
                    );
                }

                // Return to DOS
                self.cpu.ip = ret_offset;
                self.cpu.cs = ret_segment;
            } else {
                // Handler chained to another interrupt (e.g., INT 0x08 chains to INT 0x1C)
                //
                // Stack layout at this point (from bottom/high address to top/low address):
                // - original_flags (from fire_timer_irq, at SP before chain_to_interrupt)
                // - chained_flags (from chain_to_interrupt)
                // - 0xF000 (from chain_to_interrupt)
                // - 0x0008 (from chain_to_interrupt) <- current SP
                //
                // We need to fix up the stack so when INT 1C does IRET:
                // - It returns to the original interrupted code (ret_segment:ret_offset)
                // - Stack is properly balanced

                // Pop what chain_to_interrupt pushed
                let _chained_ip = self.cpu.pop(&self.memory);
                let _chained_cs = self.cpu.pop(&self.memory);
                let _chained_flags = self.cpu.pop(&self.memory);

                // Pop the original flags left from fire_timer_irq (step() only popped 2 of 3)
                // These have IF=1 (interrupts were enabled before the timer IRQ)
                let original_flags = self.cpu.pop(&self.memory);

                // Push proper return frame for INT 1C's IRET
                // Use original_flags (with IF=1) so interrupts are re-enabled after IRET
                self.cpu.push(original_flags, &mut self.memory);
                self.cpu.push(ret_segment, &mut self.memory);
                self.cpu.push(ret_offset, &mut self.memory);

                log::debug!(
                    "INT 0x{:02X}: Chained to {:04X}:{:04X}, IRET will return to {:04X}:{:04X}",
                    int_num,
                    self.cpu.cs,
                    self.cpu.ip,
                    ret_segment,
                    ret_offset
                );
            }

            return;
        }

        let addr = Cpu::physical_address(current_cs, current_ip);
        let opcode = self.memory.read_u8(addr);

        if self.exec_logging_enabled {
            let decoded = crate::decoder::decode_instruction_with_regs(
                &self.memory,
                current_cs,
                current_ip,
                Some(&self.cpu),
            );
            log::info!(
                "OP {:04X}:{:04X} {:30} {}",
                current_cs,
                current_ip,
                decoded.text,
                decoded.reg_values,
            );
        }

        // Check if it's an INT instruction
        match opcode {
            0xCD => {
                // INT with immediate - need to fetch the interrupt number
                let int_num = self.memory.read_u8(addr + 1);

                // Manually advance IP past the INT instruction
                self.cpu.ip = self.cpu.ip.wrapping_add(2);
                // Execute with BIOS I/O
                self.cpu.execute_int_with_io(
                    int_num,
                    &mut self.memory,
                    &mut self.bios,
                    &mut self.video,
                );
            }
            0xCC => {
                // INT 3 - advance IP and execute INT 3
                log::info!("INT 0x03 (breakpoint)");
                self.cpu.ip = self.cpu.ip.wrapping_add(1);
                self.cpu
                    .execute_int_with_io(3, &mut self.memory, &mut self.bios, &mut self.video);
            }
            _ => {
                // Normal instruction - use execute_with_io
                let opcode = self.cpu.fetch_byte(&self.memory);
                self.cpu.execute_with_io(
                    opcode,
                    &mut self.memory,
                    &mut self.bios,
                    &mut self.io_device,
                );
            }
        }

        // Process any video memory writes that occurred during instruction execution
        for (offset, value) in self.memory.drain_video_writes() {
            self.video.write_byte(offset, value);
        }

        // Increment cycle counter and update timer
        // Approximate: assume each instruction takes 10 cycles
        // Real 8086 instructions vary from 2 to 100+ cycles
        self.increment_cycles(10);

        // Update serial devices every 1000 instructions (~18 times per second)
        if self.step_count % 1000 == 0 {
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
        if self.video.is_dirty() {
            self.video_controller
                .update_display(self.video.get_buffer());
            self.video.clear_dirty();
        }
        // Always update cursor position (cursor moves don't dirty the buffer)
        self.video_controller.update_cursor(self.video.get_cursor());
    }

    /// Update speaker output (call periodically for platforms that need it)
    pub fn update_speaker_output(&mut self) {
        self.speaker.update();
    }

    /// Force a full video redraw regardless of dirty state
    /// Used when terminal state is known to be out of sync (e.g., after clearing screen)
    pub fn force_video_redraw(&mut self) {
        self.video_controller.force_redraw(self.video.get_buffer());
        self.video.clear_dirty();
        self.video_controller.update_cursor(self.video.get_cursor());
    }

    /// Get video buffer for inspection
    pub fn get_video_buffer(
        &self,
    ) -> &[TextCell; crate::video::TEXT_MODE_COLS * crate::video::TEXT_MODE_ROWS] {
        self.video.get_buffer()
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

    /// Get a reference to the BIOS (for disk saving on exit, etc.)
    pub fn bios(&self) -> &Bios<K> {
        &self.bios
    }

    /// Get a mutable reference to the BIOS (for runtime operations like disk swapping)
    pub fn bios_mut(&mut self) -> &mut Bios<K> {
        &mut self.bios
    }

    /// Get a mutable reference to the video controller (for platform-specific rendering)
    pub fn video_controller_mut(&mut self) -> &mut V {
        &mut self.video_controller
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
        let mut flags = self.memory.read_u8(flags_addr);

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

        self.memory.write_u8(flags_addr, flags);
    }

    /// Increment cycle counter and queue timer interrupts when tick threshold reached
    /// This simulates the PIT (Programmable Interval Timer) running at 18.2 Hz
    fn increment_cycles(&mut self, cycles: u64) {
        self.cycle_count += cycles;
        self.total_cycles += cycles;

        // Update PIT counters
        self.io_device.update_pit(cycles);

        // Update speaker based on PIT state
        self.update_speaker();

        // Queue timer interrupts when tick threshold reached
        // The INT 0x08 handler will update BDA timer counter and chain to INT 0x1C
        while self.cycle_count >= self.cycles_per_tick {
            self.cycle_count -= self.cycles_per_tick;
            self.pending_timer_irqs += 1;
        }
    }

    /// Update speaker output based on PIT Channel 2 state and port 0x61 control bits
    fn update_speaker(&mut self) {
        let control_bits = self.io_device.system_control_port().get_control_bits();
        let timer2_gate = (control_bits & 0x01) != 0;
        let speaker_data = (control_bits & 0x02) != 0;

        // Speaker enabled when both gate and data bits set
        let enabled = timer2_gate && speaker_data;

        if enabled {
            let count = self.io_device.pit().get_channel_count(2);
            if count > 0 {
                let frequency = 1193182.0 / (count as f32);
                log::debug!(
                    "Speaker: Enabled - count={}, frequency={:.2} Hz, control_bits=0x{:02X}",
                    count,
                    frequency,
                    control_bits
                );
                self.speaker.set_frequency(true, frequency);
            } else {
                self.speaker.set_frequency(false, 0.0);
            }
        } else {
            self.speaker.set_frequency(false, 0.0);
        }
    }

    pub fn set_log_interrupts(&mut self, enable: bool) {
        self.log_interrupts_enabled = enable;
        self.cpu.log_interrupts_enabled = enable;
    }

    pub fn set_log_steps(&mut self, steps: u32) {
        self.exec_logging_enabled = true;
        self.set_log_interrupts(true);
        self.log_steps = steps;
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
