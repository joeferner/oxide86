use anyhow::Result;

use crate::{
    Bios, DriveNumber, IoDevice, NullBios, NullIoDevice, NullVideoController, TextCell, Video,
    VideoController,
    cpu::Cpu,
    io_port::IoPort,
    memory::{self, Memory},
};

pub struct Computer<
    B: Bios = NullBios,
    I: IoDevice = NullIoDevice,
    V: VideoController = NullVideoController,
> {
    cpu: Cpu,
    memory: Memory,
    bios: B,
    io_port: IoPort<I>,
    video: Video,
    video_controller: V,
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
}

impl<B: Bios, I: IoDevice, V: VideoController> Computer<B, I, V> {
    pub fn new(bios: B, io_device: I, video_controller: V) -> Self {
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
            io_port: IoPort::new(io_device),
            video: Video::new(),
            video_controller,
            cycle_count: 0,
            total_cycles: 0,
            // 8086 at 4.77 MHz with PIT at 18.2 Hz: ~262144 cycles per tick
            // This is approximate: 4770000 / 18.2 ≈ 262088
            cycles_per_tick: 262088,
            step_count: 0,
            exec_logging_enabled: false,
            log_interrupts_enabled: false,
            log_steps: 0,
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

            // Pop the FLAGS that DOS pushed before CALL FAR
            let saved_flags = self.cpu.pop(&self.memory);
            // BIOS may have modified flags (especially CF for error indication)
            // Merge: keep the modified CF, ZF, etc. from BIOS, but restore IF from DOS
            self.cpu.flags = (self.cpu.flags & 0xF8FF) | (saved_flags & 0x0700); // Restore IF, TF, DF

            // Return to DOS
            self.cpu.ip = ret_offset;
            self.cpu.cs = ret_segment;

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
                self.cpu
                    .execute_with_io(opcode, &mut self.memory, &mut self.io_port);
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
    pub fn bios(&self) -> &B {
        &self.bios
    }

    /// Get a mutable reference to the BIOS (for runtime operations like disk swapping)
    pub fn bios_mut(&mut self) -> &mut B {
        &mut self.bios
    }

    /// Increment cycle counter and update system timer if needed
    /// This simulates the PIT (Programmable Interval Timer) running at 18.2 Hz
    fn increment_cycles(&mut self, cycles: u64) {
        self.cycle_count += cycles;
        self.total_cycles += cycles;

        // Check if we've accumulated enough cycles for a timer tick
        if self.cycle_count >= self.cycles_per_tick {
            self.cycle_count -= self.cycles_per_tick;

            // Read current timer counter from BDA
            let counter_addr = memory::BDA_START + memory::BDA_TIMER_COUNTER;
            let low_word = self.memory.read_u16(counter_addr);
            let high_word = self.memory.read_u16(counter_addr + 2);
            let mut tick_count = ((high_word as u32) << 16) | (low_word as u32);

            // Increment tick count
            tick_count = tick_count.wrapping_add(1);

            // Check for midnight rollover (0x001800B0 ticks = 24 hours)
            if tick_count >= 0x001800B0 {
                tick_count = 0;
                // Set midnight overflow flag
                let overflow_addr = memory::BDA_START + memory::BDA_TIMER_OVERFLOW;
                self.memory.write_u8(overflow_addr, 1);
            }

            // Write updated tick count back to BDA
            self.memory
                .write_u16(counter_addr, (tick_count & 0xFFFF) as u16);
            self.memory
                .write_u16(counter_addr + 2, (tick_count >> 16) as u16);
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
}
