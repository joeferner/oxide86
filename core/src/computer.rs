use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crate::debugger::{DebugCommand, DebugResponse, DebugShared};
use crate::devices::game_port::GamePortDevice;
use crate::devices::parallel_port::LptPortDevice;
use crate::devices::pc_speaker::PcSpeaker;
use crate::devices::uart::ComPortDevice;
use crate::{
    Device,
    bus::{Bus, BusConfig},
    byte_to_printable_char,
    cpu::{
        Cpu, CpuType,
        bios::{
            bda::bda_set_math_coprocessor, int09_keyboard_hardware_interrupt::scan_code_to_ascii,
        },
    },
    devices::{
        clock::Clock,
        rtc::{CMOS_REG_FLOPPY_TYPES, RTC_IO_PORT_DATA, RTC_IO_PORT_REGISTER_SELECT},
    },
    disk::{Disk, DiskError, DriveNumber},
    memory::Memory,
    video::{VideoBuffer, VideoCard, VideoCardType},
};
use anyhow::{Result, anyhow};

/// Pre-assembled x86 stub loaded when a hard drive has no valid boot signature.
/// Prints "Non-system disk or disk error", waits for a keypress, then halts.
/// Assembled for load address 0x0000:0x7C00; message starts at offset 0x1C (SI=0x7C1C).
#[rustfmt::skip]
const NON_SYSTEM_DISK_STUB: &[u8] = &[
    0xFA,                               // cli
    0x31, 0xC0,                         // xor ax, ax
    0x8E, 0xD8,                         // mov ds, ax
    0x31, 0xDB,                         // xor bx, bx       (BH=0: video page 0)
    0xBE, 0x1B, 0x7C,                   // mov si, 0x7C1B   (message offset)
    0xB4, 0x0E,                         // mov ah, 0x0E     (BIOS TTY print)
    // loop_start (0x0C):
    0xAC,                               // lodsb
    0x08, 0xC0,                         // or al, al
    0x74, 0x04,                         // jz wait_key      (+4 -> 0x15)
    0xCD, 0x10,                         // int 0x10
    0xEB, 0xF7,                         // jmp loop_start   (-9 -> 0x0C)
    // wait_key (0x15):
    0x30, 0xE4,                         // xor ah, ah
    0xCD, 0x16,                         // int 0x16         (wait for key)
    0xCD, 0x19,                         // int 0x19         (bootstrap: try drives in boot order)
    // message (0x1B):
    b'N', b'o', b'n', b'-', b's', b'y', b's', b't', b'e', b'm', b' ',
    b'd', b'i', b's', b'k', b' ', b'o', b'r', b' ', b'd', b'i', b's', b'k', b' ',
    b'e', b'r', b'r', b'o', b'r', b'\r', b'\n',
    b'R', b'e', b'p', b'l', b'a', b'c', b'e', b' ', b'a', b'n', b'd', b' ',
    b'p', b'r', b'e', b's', b's', b' ', b'a', b'n', b'y', b' ', b'k', b'e', b'y', b' ',
    b'w', b'h', b'e', b'n', b' ', b'r', b'e', b'a', b'd', b'y', b'\r', b'\n',
    0x00,
];

pub struct ComputerConfig {
    pub cpu_type: CpuType,
    pub clock_speed: u32,
    pub memory_size: usize,
    pub clock: Box<dyn Clock>,
    pub hard_disks: Vec<Box<dyn Disk>>,
    pub video_card_type: VideoCardType,
    pub video_buffer: Arc<RwLock<VideoBuffer>>,
    pub pc_speaker: Box<dyn PcSpeaker>,
    pub math_coprocessor: bool,
}

pub struct Computer {
    cpu: Cpu,
    bus: Bus,
    key_presses: VecDeque<u8>,
    boot_drive: Option<DriveNumber>,
    loaded_program: Option<(Vec<u8>, u16, u16)>,
    debug: Option<Arc<DebugShared>>,
}

impl Computer {
    pub fn new(config: ComputerConfig) -> Self {
        let cpu = Cpu::new(config.cpu_type, config.clock_speed, config.math_coprocessor);
        log::info!("Memory {}kb", config.memory_size / 1024);
        let memory = Memory::new(config.memory_size);
        let cpu_clock_speed = cpu.clock_speed();
        let clock = if config.cpu_type == CpuType::I8086 {
            None
        } else {
            Some(config.clock)
        };
        let video_card = Rc::new(RefCell::new(VideoCard::new(
            config.video_card_type,
            config.video_buffer,
            cpu_clock_speed,
        )));
        let mut computer = Self {
            cpu,
            bus: Bus::new(BusConfig {
                memory,
                cpu_clock_speed,
                clock,
                hard_disks: config.hard_disks,
                video_card,
                pc_speaker: config.pc_speaker,
            }),
            key_presses: VecDeque::new(),
            boot_drive: None,
            loaded_program: None,
            debug: None,
        };
        computer.reset();
        computer
    }

    pub fn set_debug(&mut self, debug: Arc<DebugShared>) {
        self.bus.set_debug(Arc::clone(&debug));
        self.debug = Some(debug);
    }

    pub fn add_device<T: Device + 'static>(&mut self, device: T) {
        self.bus.add_device(device);
    }

    pub fn add_sound_card<T: Device + crate::devices::SoundCard + 'static>(&mut self, device: T) {
        self.bus.add_sound_card(device);
    }

    /// Insert or eject the floppy disk for the given drive. Returns the previous disk if any.
    /// Pass `None` to eject; the CMOS drive-type register is only updated on insert.
    pub fn set_floppy_disk(
        &mut self,
        drive: DriveNumber,
        disk: Option<Box<dyn Disk>>,
    ) -> Option<Box<dyn Disk>> {
        if let Some(ref d) = disk {
            // Derive CMOS type code from geometry.
            // Values: 0=none, 1=360KB 5.25", 2=1.2MB 5.25", 3=720KB 3.5", 4=1.44MB 3.5", 5=2.88MB 3.5"
            let cmos_type: u8 = match d.disk_geometry().total_size {
                1_474_560 => 0x04,
                737_280 => 0x03,
                368_640 | 163_840 => 0x01,
                _ => 0x00,
            };

            // Update CMOS register 0x10 (floppy drive types).
            // Read current value first to preserve the other drive's nibble.
            self.bus
                .io_write_u8(RTC_IO_PORT_REGISTER_SELECT, CMOS_REG_FLOPPY_TYPES);
            let current = self.bus.io_read_u8(RTC_IO_PORT_DATA);
            let current = if current == 0xFF { 0x00 } else { current };
            let floppy_types = if drive.as_floppy_index() == 0 {
                (cmos_type << 4) | (current & 0x0F) // drive A in bits 7:4
            } else {
                (current & 0xF0) | (cmos_type & 0x0F) // drive B in bits 3:0
            };
            self.bus
                .io_write_u8(RTC_IO_PORT_REGISTER_SELECT, CMOS_REG_FLOPPY_TYPES);
            self.bus.io_write_u8(RTC_IO_PORT_DATA, floppy_types);
        }

        self.bus.floppy_controller_mut().set_drive_disk(drive, disk)
    }

    /// Load a program at the specified segment:offset and set CPU to start there
    pub fn load_program(&mut self, program_data: &[u8], segment: u16, offset: u16) -> Result<()> {
        let physical_addr = self.bus.physical_address(segment, offset);
        self.bus.load_at(physical_addr, program_data)?;
        self.cpu.reset(segment, offset, None);
        self.loaded_program = Some((program_data.to_vec(), segment, offset));
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn run(&mut self) {
        while self.get_exit_code().is_none() && !self.cpu.wait_for_key_press() {
            self.step();
        }
    }

    pub fn step(&mut self) {
        if let Some(debug) = &self.debug
            && self.debug_check(&debug.clone())
        {
            return;
        }
        self.process_key_presses();
        self.cpu.step(&mut self.bus);
        if self.cpu.at_reset_vector() {
            if self.cpu.take_bootstrap_request() {
                // INT 19h: try floppy A: first, then fall back to the configured boot drive.
                let floppy_a = DriveNumber::floppy_a();
                if self.boot(floppy_a).is_ok() {
                    log::info!("Bootstrap: booting from floppy A:");
                    self.boot_drive = Some(floppy_a);
                } else if let Some(drive) = self.boot_drive {
                    log::info!(
                        "Bootstrap: no bootable floppy, retrying drive {}",
                        drive.to_letter()
                    );
                    if let Err(e) = self.boot(drive) {
                        log::error!("Bootstrap reboot failed: {e}");
                    }
                }
            } else if let Some(drive) = self.boot_drive {
                log::info!("Rebooting from drive {}", drive.to_letter());
                if let Err(e) = self.boot(drive) {
                    log::error!("Reboot failed: {e}");
                }
            } else {
                if self.loaded_program.is_none() {
                    log::warn!("Reset vector reached but no boot drive set; resetting CPU");
                }
                self.reset();
            }
        }
    }

    pub fn reset(&mut self) {
        self.key_presses.clear();
        if let Some((data, segment, offset)) = self.loaded_program.clone() {
            self.bus.reset();
            self.apply_coprocessor_bda();
            let physical_addr = self.bus.physical_address(segment, offset);
            if let Err(e) = self.bus.load_at(physical_addr, &data) {
                log::error!("Failed to reload program on reset: {e}");
            }
            self.cpu.reset(segment, offset, None);
        } else if let Some(drive) = self.boot_drive {
            if let Err(e) = self.boot(drive) {
                log::error!("Reset boot failed: {e}");
            }
        } else {
            self.bus.reset();
            self.apply_coprocessor_bda();
            self.cpu.reset(0xffff, 0x0000, None);
        }
    }

    fn apply_coprocessor_bda(&mut self) {
        if self.cpu.math_coprocessor() {
            bda_set_math_coprocessor(&mut self.bus);
        }
    }

    /// Boot from disk by loading boot sector to 0x0000:0x7C00
    /// This simulates the BIOS boot process:
    /// 1. Read sector 0 (cylinder 0, head 0, sector 1) from the specified drive
    /// 2. Load it to physical address 0x7C00
    /// 3. Set CS:IP to 0x0000:0x7C00
    /// 4. Set DL to boot drive number
    pub fn boot(&mut self, drive: DriveNumber) -> Result<()> {
        // If floppy boot fails, fall back to hard drive C — same behavior as a real BIOS.
        if drive.is_floppy() {
            let hdd = DriveNumber::hard_drive_c();
            return match self.boot_from(drive) {
                Ok(()) => Ok(()),
                Err(e) => {
                    log::warn!(
                        "Boot from {} failed ({}), falling back to hard drive C:",
                        drive.to_letter(),
                        e
                    );
                    self.boot_from(hdd)
                }
            };
        }
        self.boot_from(drive)
    }

    fn boot_from(&mut self, drive: DriveNumber) -> Result<()> {
        // Reset bus state (IVT, BDA, devices) before booting so that any modifications
        // made by a previous session (e.g. DOS replacing INT 13h at 0x0070) don't persist.
        self.bus.reset();
        self.apply_coprocessor_bda();

        // Read boot sector: cylinder 0, head 0, sector 1
        let boot_sector = if drive.is_floppy() {
            self.bus.floppy_controller().read_sectors(drive, 0, 0, 1, 1)
        } else {
            self.bus
                .hard_disk_controller()
                .get_disk(drive)
                .ok_or(DiskError::DriveNotReady)
                .and_then(|disk| disk.read_sectors(0, 0, 1, 1))
        }
        .map_err(|err| anyhow!("failed to read boot sector: {err}"))?;

        if boot_sector.len() != 512 {
            return Err(anyhow::anyhow!(
                "Boot sector must be exactly 512 bytes, got {}",
                boot_sector.len()
            ));
        }

        // Verify boot signature (0x55AA at offset 510-511)
        // For hard drives, a missing signature means the disk is not bootable (not formatted/no OS).
        // For floppies, some old "booter" games predate the convention, so we warn but continue.
        let boot_data: &[u8] = if boot_sector[510] != 0x55 || boot_sector[511] != 0xAA {
            if drive.is_floppy() {
                log::warn!(
                    "Boot sector missing 0x55AA signature (got 0x{:02X}{:02X}); proceeding anyway",
                    boot_sector[511],
                    boot_sector[510]
                );
                &boot_sector
            } else {
                log::warn!(
                    "Hard drive boot sector missing 0x55AA signature; loading non-system disk stub"
                );
                NON_SYSTEM_DISK_STUB
            }
        } else {
            &boot_sector
        };

        // Load boot sector to 0x0000:0x7C00 (physical address 0x7C00)
        const BOOT_SEGMENT: u16 = 0x0000;
        const BOOT_OFFSET: u16 = 0x7C00;
        let boot_addr = self.bus.physical_address(BOOT_SEGMENT, BOOT_OFFSET);
        self.bus.load_at(boot_addr, boot_data)?;
        self.cpu.reset(BOOT_SEGMENT, BOOT_OFFSET, Some(drive));
        self.boot_drive = Some(drive);

        // TODO
        // // Set current drive to match boot drive
        // // Convert BIOS drive number to DOS drive number: 0x00->0, 0x01->1, 0x80->2, 0x81->3
        // self.bios.set_default_drive(drive);
        //
        // // Pre-allocate memory for DOS kernel
        // // In a real system, DOS would already be loaded in memory before
        // // the memory allocator starts. We simulate this by pre-allocating
        // // a block for DOS, reducing the amount of "free" memory available.
        // // Typically DOS + COMMAND.COM takes about 64-128KB.
        // // We'll allocate 4096 paragraphs (64KB) for DOS.
        // const DOS_PARAGRAPHS: u16 = 4096; // 64KB for DOS kernel and COMMAND.COM
        // match self.bios.memory_allocate(DOS_PARAGRAPHS) {
        //     Ok(seg) => {
        //         log::info!(
        //             "Pre-allocated {} KB at segment 0x{:04X} for DOS kernel",
        //             (DOS_PARAGRAPHS as u32 * 16) / 1024,
        //             seg
        //         );
        //     }
        //     Err((error_code, available)) => {
        //         log::warn!(
        //             "Failed to pre-allocate DOS memory: error {}, available {} paragraphs",
        //             error_code,
        //             available
        //         );
        //     }
        // }

        // // Store boot drive for reset/reboot operations
        // self.boot_drive = Some(drive);

        // // Clear loaded_program since we're booting, not loading
        // self.loaded_program = None;

        Ok(())
    }

    pub fn push_key_press(&mut self, scan_code: u8) {
        log::debug!(
            "pushing key 0x{scan_code:02X} '{}'",
            byte_to_printable_char(scan_code_to_ascii(scan_code, false, false, false))
        );
        self.key_presses.push_back(scan_code);
        self.process_key_presses();
    }

    fn process_key_presses(&mut self) {
        // Gate on output_buffer_full (obf) rather than pending_key.
        // pending_key is cleared by the PIC as soon as it dispatches the IRQ,
        // but the BIOS INT 09h handler hasn't read port 0x60 yet at that point.
        // If we load the next scan code then, we overwrite the previous one
        // before INT 09h can read it, so it never reaches the BDA buffer.
        // obf stays true until port 0x60 is actually read, which is the right gate.
        //
        // Additionally, gate on keyboard IRQ not being in service. When a program
        // installs a custom INT 09h handler, it may read port 0x60 directly (clearing
        // obf) and then chain to the old BIOS INT 09h handler. Without this extra gate,
        // process_key_presses would load the next key between those two events, causing
        // the chained BIOS handler to read the wrong scan code and miss modifier state
        // (e.g. ALT+key would arrive as plain key because 0x38/ALT was processed by the
        // custom handler but the BIOS handler read the next key instead).
        let obf = self.bus.keyboard_controller().output_buffer_full();
        let irq_in_service = self.bus.is_keyboard_irq_in_service();
        if !obf
            && !irq_in_service
            && let Some(scan_code) = self.key_presses.pop_front()
        {
            {
                let mut keyboard_controller = self.bus.keyboard_controller_mut();
                keyboard_controller.key_press(scan_code);
            }
            self.bus.notify_irq_pending();
            self.cpu.key_press(&mut self.bus);
        }
    }

    #[cfg(test)]
    pub(crate) fn read_hard_disk_sectors(
        &self,
        drive: DriveNumber,
        cylinder: u8,
        head: u8,
        sector: u8,
        count: u8,
    ) -> Result<Vec<u8>, DiskError> {
        self.bus
            .hard_disk_controller()
            .get_disk(drive)
            .ok_or(DiskError::DriveNotReady)?
            .read_sectors(cylinder, head, sector, count)
    }

    pub fn get_exit_code(&self) -> Option<u8> {
        self.cpu.get_exit_code()
    }

    pub fn wait_for_key_press(&self) -> bool {
        self.cpu.wait_for_key_press()
    }

    pub fn joystick_mut(&self) -> std::cell::RefMut<'_, GamePortDevice> {
        self.bus.game_port_mut()
    }

    pub fn set_com_port_device(
        &mut self,
        port: u8,
        device: Option<Arc<RwLock<dyn ComPortDevice>>>,
    ) {
        self.bus.uart_mut().set_com_port_device(port, device)
    }

    pub fn set_lpt_device(&mut self, port: u8, device: Option<Arc<RwLock<dyn LptPortDevice>>>) {
        self.bus.parallel_port_mut().set_lpt_device(port, device)
    }

    /// Inject a PS/2 mouse event.  The event is encoded as a 3-byte PS/2 packet
    /// and queued through the keyboard controller's auxiliary port, exactly as
    /// real hardware would do.  IRQ12 fires on the next step() call (provided
    /// interrupts are enabled and the aux port has been enabled via
    /// INT 15h AH=C2h AL=00h BH=01h).
    ///
    /// `buttons`: bit 0 = left, bit 1 = right, bit 2 = middle.
    pub fn push_ps2_mouse_event(&mut self, dx: i8, dy: i8, buttons: u8) {
        self.bus.push_ps2_mouse_event(dx, dy, buttons);
    }

    /// Returns true once the guest has both enabled the PS/2 aux port
    /// (INT 15h AH=C2h AL=00h BH=01h) and registered a callback handler
    /// (INT 15h AH=C2h AL=07h).  Use this as the trigger for injecting
    /// test mouse events so the packet is not discarded before the handler exists.
    #[cfg(test)]
    pub(crate) fn is_ps2_mouse_ready(&self) -> bool {
        if !self.bus.keyboard_controller().is_aux_enabled() {
            return false;
        }
        let (seg, off, _mask) = crate::cpu::bios::bda::bda_get_ps2_mouse_handler(&self.bus);
        seg != 0 || off != 0
    }

    pub fn set_watch_addresses(&mut self, addrs: Vec<usize>) {
        self.bus.set_watch_addresses(addrs);
    }

    pub fn set_exec_logging_enabled(&mut self, enabled: bool) {
        log::info!(
            "exec logging {}",
            if enabled { "enabled" } else { "disabled" }
        );
        self.cpu.exec_logging_enabled = enabled;
    }

    pub fn exec_logging_enabled(&self) -> bool {
        self.cpu.exec_logging_enabled
    }

    pub fn get_cycle_count(&self) -> u64 {
        self.bus.cycle_count() as u64
    }

    pub fn get_clock_speed(&self) -> u32 {
        self.cpu.clock_speed()
    }

    pub fn is_terminal_halt(&self) -> bool {
        self.cpu.is_terminal_halt()
    }

    /// Returns `true` when the MCP debug server has paused execution.
    pub fn is_debug_paused(&self) -> bool {
        self.debug
            .as_ref()
            .is_some_and(|d| d.paused.load(Ordering::Relaxed))
    }

    pub fn log_cpu_state(&self) {
        self.cpu.log_state();
    }

    /// Returns `true` if the emulator was paused this call (so the caller
    /// should skip executing the next CPU instruction).
    fn debug_check(&mut self, dbg: &Arc<DebugShared>) -> bool {
        if dbg.paused.load(Ordering::Relaxed) {
            self.service_debug_commands(dbg);
            return true;
        }

        if dbg.pause_requested.load(Ordering::Relaxed) {
            dbg.pause_requested.store(false, Ordering::Relaxed);
            self.do_pause(dbg);
            return true;
        }

        if self.check_breakpoint(dbg) {
            self.do_pause(dbg);
            return true;
        }

        false
    }

    /// Returns `true` if the current CS:IP matches a breakpoint.
    fn check_breakpoint(&self, dbg: &Arc<DebugShared>) -> bool {
        if !dbg.has_breakpoints.load(Ordering::Relaxed) {
            return false;
        }
        let cs = self.cpu.cs();
        let ip = self.cpu.ip();
        dbg.breakpoints.lock().unwrap().contains(&(cs, ip))
    }

    fn do_pause(&mut self, dbg: &Arc<DebugShared>) {
        *dbg.snapshot.lock().unwrap() = Some(self.cpu.snapshot());
        dbg.paused.store(true, Ordering::SeqCst);
        dbg.cond_paused.notify_all();

        self.service_debug_commands(dbg);
    }

    fn service_debug_commands(&mut self, dbg: &Arc<DebugShared>) {
        loop {
            let cmd = {
                let mut lock = dbg.pending_command.lock().unwrap();
                // Non-blocking: if no command is ready, return and let the
                // caller (step()) yield back to the GUI event loop.  The next
                // frame will call debug_check again and try once more.
                match lock.take() {
                    Some(cmd) => cmd,
                    None => return,
                }
            };

            match cmd {
                DebugCommand::Continue => {
                    *dbg.watchpoint_hit.lock().unwrap() = None;
                    // Execute one step first so CS:IP advances past the breakpoint
                    // before resuming; otherwise the same breakpoint fires again
                    // immediately on the next debug_check call.
                    self.cpu.step(&mut self.bus);
                    dbg.paused.store(false, Ordering::SeqCst);
                    *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Ok);
                    dbg.cond_paused.notify_all();
                    break;
                }
                DebugCommand::Step(n) => {
                    for _ in 0..n {
                        self.cpu.step(&mut self.bus);
                        // Stop early if a watchpoint or breakpoint fired
                        if dbg.pause_requested.load(Ordering::Relaxed) || self.check_breakpoint(dbg)
                        {
                            dbg.pause_requested.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                    *dbg.snapshot.lock().unwrap() = Some(self.cpu.snapshot());
                    *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Ok);
                    dbg.cond_paused.notify_all();
                    // Stay paused — wait for next command
                }
                DebugCommand::ReadMemory { addr, len } => {
                    let bytes: Vec<u8> = (addr..addr + len)
                        .map(|a| self.bus.memory_read_u8(a as usize))
                        .collect();
                    *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Memory(bytes));
                    dbg.cond_paused.notify_all();
                    // Stay paused — wait for next command
                }
                DebugCommand::SendKey(scan_code) => {
                    self.push_key_press(scan_code);
                    *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Ok);
                    dbg.cond_paused.notify_all();
                    // Stay paused — wait for next command
                }
                DebugCommand::AddWriteWatchpoint(addr) => {
                    dbg.add_write_watchpoint(addr);
                    *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Ok);
                    dbg.cond_paused.notify_all();
                }
                DebugCommand::RemoveWriteWatchpoint(addr) => {
                    dbg.remove_write_watchpoint(addr);
                    *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Ok);
                    dbg.cond_paused.notify_all();
                }
            }
        }
    }
}
