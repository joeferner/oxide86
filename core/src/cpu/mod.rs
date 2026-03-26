use crate::{
    Computer,
    bus::Bus,
    cpu::bios::int21_dos_services::{
        DosFileHandleTable, PendingDosOpen, PendingDosRead, PendingDosSeek,
    },
    cpu::{
        bios::BIOS_CODE_SEGMENT,
        instructions::{RepeatPrefix, decoder, fpu::FPU_DEFAULT_CONTROL_WORD},
    },
    debugger::DebugSnapshot,
    disk::DriveNumber,
};

pub mod bios;
mod cpu_type;
pub(crate) mod f80;
pub(crate) mod f80_trig;
pub(crate) mod instructions;
mod timing;

pub use cpu_type::CpuType;

// IVT (Interrupt Vector Table) constants
pub const IVT_START: usize = 0x0000;
pub const IVT_END: usize = 0x0400;
pub const IVT_ENTRY_SIZE: usize = 4; // Each entry is 4 bytes (offset, segment)

/// Flag bit positions
#[allow(dead_code)]
mod cpu_flag {
    pub const CARRY: u16 = 1 << 0;
    pub const PARITY: u16 = 1 << 2;
    pub const AUXILIARY: u16 = 1 << 4;
    pub const ZERO: u16 = 1 << 6;
    pub const SIGN: u16 = 1 << 7;
    pub const TRAP: u16 = 1 << 8;
    pub const INTERRUPT: u16 = 1 << 9;
    pub const DIRECTION: u16 = 1 << 10;
    pub const OVERFLOW: u16 = 1 << 11;
}

pub(crate) struct Cpu {
    cpu_type: CpuType,

    // General purpose registers
    ax: u16,
    bx: u16,
    cx: u16,
    dx: u16,

    // Index and pointer registers
    si: u16,
    di: u16,
    sp: u16,
    bp: u16,

    // Segment registers
    cs: u16,
    ds: u16,
    ss: u16,
    es: u16,
    fs: u16, // 80386+
    gs: u16, // 80386+

    /// Instruction pointer
    ip: u16,

    /// Flags (start with just carry, zero, sign)
    flags: u16,

    /// Program Segment Prefix
    current_psp: u16,

    /// clock speed in Hz
    clock_speed: u32,

    /// Halted flag
    halted: bool,

    /// If INT 21h, AH=4Ch - Exit Program is called without a place to return it will set this value
    exit_code: Option<u8>,

    /// Segment override prefix (for next instruction only)
    segment_override: Option<u16>,

    /// Repeat prefix for string instructions
    repeat_prefix: Option<RepeatPrefix>,

    /// Pending CPU exception interrupt number (e.g. 0 = divide error)
    /// Set by instructions that trigger CPU exceptions; fired by Computer::step()
    pending_exception: Option<u8>,

    /// Last disk operation status (for INT 13h AH=01h)
    last_disk_status: u8,

    /// In INT 16h, AH=00h - Read Character if a key is not available we need to return to allow
    /// key presses to be handled
    wait_for_key_press: bool,
    //// if this flag is set when returning from a key press we need to patch the flags
    wait_for_key_press_patch_flags: bool,

    /// if set to true, opcode execution will be logged as info level
    pub exec_logging_enabled: bool,

    /// Set by INT 19h (bootstrap loader) to signal that the next reboot should
    /// try drives in boot order rather than only the configured boot drive.
    bootstrap_request: bool,

    /// Pending INT 0x21 AH=3Fh: buffer location to dump on return from DOS.
    pending_dos_read: Option<PendingDosRead>,

    /// Pending INT 0x21 AH=3C/3Dh: filename waiting for the returned handle.
    pending_dos_open: Option<PendingDosOpen>,

    /// Pending INT 0x21 AH=42h: handle waiting for the returned file position.
    pending_dos_seek: Option<PendingDosSeek>,

    /// Side-table: open DOS file handle → filename + current position.
    dos_file_handles: DosFileHandleTable,

    /// Buffer for consecutive INT 10h AH=0Eh / INT 29h teletype characters.
    /// Flushed as a single log line on CR, LF, or any non-teletype interrupt.
    teletype_log_buffer: String,

    /// 8087 math coprocessor present
    math_coprocessor: bool,

    /// 8087 control word (CW). Reset to 0x037F by FNINIT.
    fpu_control_word: u16,

    /// 8087 status word (SW). Reset to 0x0000 by FNINIT.
    fpu_status_word: u16,

    /// 8087 register stack (8 x 80-bit extended precision)
    fpu_stack: [f80::F80; 8],

    /// 8087 stack top pointer (0-7). ST(i) = fpu_stack[(fpu_top + i) & 7].
    fpu_top: u8,

    /// When true, the single-step trap (INT 1) is suppressed for the next instruction.
    /// Set after `mov ss, ...` or `pop ss` to allow the paired SP load to run atomically.
    suppress_trap: bool,
    /// When true, skip exec-log output for the next instruction.
    /// Set when a prefix (WAIT, REP, REPNE) is folded with the following instruction
    /// in the log so that instruction doesn't get logged a second time on its own step.
    suppress_next_exec_log: bool,
}

/// Snapshot of CPU state captured before instruction execution, used by the
/// decoder so that annotations can show pre-execution values where needed
/// (e.g. CS before a far jump, FPU ST(0) before a store-and-pop).
struct PreExecState {
    cs: u16,
    fpu_st0: ([u8; 10], f64),
}

/// Adapter that implements `Computer` by combining a `Cpu` and a `Bus`.
struct CpuBusComputer<'a> {
    cpu: &'a Cpu,
    bus: &'a Bus,
    pre: PreExecState,
}

impl Computer for CpuBusComputer<'_> {
    fn ax(&self) -> u16 {
        self.cpu.ax
    }
    fn bx(&self) -> u16 {
        self.cpu.bx
    }
    fn cx(&self) -> u16 {
        self.cpu.cx
    }
    fn dx(&self) -> u16 {
        self.cpu.dx
    }
    fn sp(&self) -> u16 {
        self.cpu.sp
    }
    fn bp(&self) -> u16 {
        self.cpu.bp
    }
    fn si(&self) -> u16 {
        self.cpu.si
    }
    fn di(&self) -> u16 {
        self.cpu.di
    }
    fn cs(&self) -> u16 {
        self.pre.cs
    }
    fn ds(&self) -> u16 {
        self.cpu.ds
    }
    fn ss(&self) -> u16 {
        self.cpu.ss
    }
    fn es(&self) -> u16 {
        self.cpu.es
    }
    fn read_u8(&self, phys: u32) -> u8 {
        self.bus.memory_read_u8(phys as usize)
    }
    fn fpu_st(&self, i: u8) -> ([u8; 10], f64) {
        let idx = (self.cpu.fpu_top.wrapping_add(i)) as usize & 7;
        let val = self.cpu.fpu_stack[idx];
        (val.to_bytes(), val.to_f64())
    }
    fn fpu_st_pre(&self, i: u8) -> ([u8; 10], f64) {
        if i == 0 {
            self.pre.fpu_st0
        } else {
            self.fpu_st(i)
        }
    }
}

impl Cpu {
    pub(crate) fn new(cpu_type: CpuType, clock_speed: u32, math_coprocessor: bool) -> Self {
        Self {
            cpu_type,
            clock_speed,
            ax: 0,
            bx: 0,
            cx: 0,
            dx: 0,
            si: 0,
            di: 0,
            sp: 0,
            bp: 0,
            cs: 0,
            ds: 0,
            ss: 0,
            es: 0,
            fs: 0,
            gs: 0,
            ip: 0,
            flags: 0,
            current_psp: 0x100,
            halted: false,
            exit_code: None,
            segment_override: None,
            repeat_prefix: None,
            pending_exception: None,
            last_disk_status: 0,
            wait_for_key_press: false,
            wait_for_key_press_patch_flags: false,
            exec_logging_enabled: false,
            bootstrap_request: false,
            pending_dos_read: None,
            pending_dos_open: None,
            pending_dos_seek: None,
            dos_file_handles: DosFileHandleTable::new(),
            teletype_log_buffer: String::new(),
            suppress_trap: false,
            suppress_next_exec_log: false,
            math_coprocessor,
            fpu_control_word: FPU_DEFAULT_CONTROL_WORD,
            fpu_status_word: 0,
            fpu_stack: [f80::F80::ZERO; 8],
            fpu_top: 0,
        }
    }

    pub(crate) fn math_coprocessor(&self) -> bool {
        self.math_coprocessor
    }

    pub(crate) fn clock_speed(&self) -> u32 {
        self.clock_speed
    }

    pub(crate) fn cs(&self) -> u16 {
        self.cs
    }

    pub(crate) fn ip(&self) -> u16 {
        self.ip
    }

    pub(crate) fn snapshot(&self) -> DebugSnapshot {
        DebugSnapshot {
            cs: self.cs,
            ip: self.ip,
            ax: self.ax,
            bx: self.bx,
            cx: self.cx,
            dx: self.dx,
            si: self.si,
            di: self.di,
            sp: self.sp,
            bp: self.bp,
            ds: self.ds,
            es: self.es,
            ss: self.ss,
            fs: self.fs,
            gs: self.gs,
            flags: self.flags,
            fpu_top: self.fpu_top,
            fpu_stack: {
                let mut arr = [0.0f64; 8];
                for (i, v) in self.fpu_stack.iter().enumerate() {
                    arr[i] = v.to_f64();
                }
                arr
            },
            fpu_status_word: self.fpu_status_word,
            fpu_control_word: self.fpu_control_word,
        }
    }

    /// Set a specific flag
    fn set_flag(&mut self, flag: u16, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    /// Get a specific flag
    fn get_flag(&self, flag: u16) -> bool {
        (self.flags & flag) != 0
    }

    /// Log the current CPU state (registers, flags, etc.)
    pub(crate) fn log_state(&self) {
        let ax = self.ax;
        let bx = self.bx;
        let cx = self.cx;
        let dx = self.dx;
        log::info!("CPU State ({}):", self.cpu_type);
        log::info!("  AX={ax:04X}  BX={bx:04X}  CX={cx:04X}  DX={dx:04X}",);
        log::info!(
            "  SI={:04X}  DI={:04X}  SP={:04X}  BP={:04X}",
            self.si,
            self.di,
            self.sp,
            self.bp
        );
        log::info!(
            "  CS={:04X}  DS={:04X}  SS={:04X}  ES={:04X}  FS={:04X}  GS={:04X}",
            self.cs,
            self.ds,
            self.ss,
            self.es,
            self.fs,
            self.gs
        );
        log::info!("  IP={:04X}", self.ip);
        log::info!(
            "  Flags={:04X}  CF={} PF={} AF={} ZF={} SF={} TF={} IF={} DF={} OF={}",
            self.flags,
            self.get_flag(cpu_flag::CARRY) as u8,
            self.get_flag(cpu_flag::PARITY) as u8,
            self.get_flag(cpu_flag::AUXILIARY) as u8,
            self.get_flag(cpu_flag::ZERO) as u8,
            self.get_flag(cpu_flag::SIGN) as u8,
            self.get_flag(cpu_flag::TRAP) as u8,
            self.get_flag(cpu_flag::INTERRUPT) as u8,
            self.get_flag(cpu_flag::DIRECTION) as u8,
            self.get_flag(cpu_flag::OVERFLOW) as u8,
        );
        log::info!("  halted={}  exit_code={:?}", self.halted, self.exit_code);
    }

    pub(crate) fn step(&mut self, bus: &mut Bus) {
        // service any interrupts coming from the PIC
        if self.get_flag(cpu_flag::INTERRUPT) {
            let irq = bus.pic_mut().take_irq(bus.cycle_count());
            if let Some(irq) = irq {
                let ivt_addr = (irq as usize) * 4;
                let ivt_off = bus.memory_read_u16(ivt_addr);
                let ivt_seg = bus.memory_read_u16(ivt_addr + 2);
                if irq != 0x08 {
                    log::debug!(
                        "CPU: dispatching IRQ 0x{irq:02X} -> {:04X}:{:04X} (cycle={})",
                        ivt_seg,
                        ivt_off,
                        bus.cycle_count()
                    );
                } else {
                    log::trace!(
                        "CPU: dispatching IRQ 0x{irq:02X} -> {:04X}:{:04X} (cycle={})",
                        ivt_seg,
                        ivt_off,
                        bus.cycle_count()
                    );
                }
                self.dispatch_interrupt(bus, irq);
                return;
            }
        }

        // If halted, wait for an interrupt to wake us — do not execute instructions.
        // Advance cycles so the PIT timer can fire and eventually deliver an IRQ.
        if self.halted {
            bus.increment_cycle_count(timing::cycles::HLT);
            return;
        }

        // service any bios routines
        if self.cs == BIOS_CODE_SEGMENT {
            self.step_bios_segment(bus);
            return;
        }

        let pre_ip = self.ip;
        let pre = if self.exec_logging_enabled {
            let idx = self.fpu_top as usize & 7;
            let val = self.fpu_stack[idx];
            Some(PreExecState {
                cs: self.cs,
                fpu_st0: (val.to_bytes(), val.to_f64()),
            })
        } else {
            None
        };

        // Capture TF before execution — it may be cleared by the instruction (e.g. POPF).
        let trap_before = self.get_flag(cpu_flag::TRAP);

        let pre_cs = self.cs;
        bus.set_current_ip(pre_cs, pre_ip);
        self.exec_instruction(bus);

        self.check_int21_dos_call(bus);

        // Decode after execution so register annotations reflect post-exec state.
        // Instruction bytes at pre_cs:pre_ip are still in memory (code is not modified).
        let decoded = pre.map(|pre| {
            decoder::decode(
                &CpuBusComputer {
                    cpu: self,
                    bus,
                    pre,
                },
                pre_cs,
                pre_ip,
            )
        });

        if self.exec_logging_enabled
            && let Some(ref instr) = decoded
        {
            if self.suppress_next_exec_log {
                self.suppress_next_exec_log = false;
            } else {
                log::info!("{}", instr.format_line());
                // If this instruction folded a prefix+body (e.g. "wait fpatan",
                // "rep movsb"), the body will execute as the next step — suppress
                // that log entry so it doesn't appear twice.
                if instr.prefix.is_some() {
                    self.suppress_next_exec_log = true;
                }
            }
        }

        // Single-step trap: if TF was set before the instruction, fire INT 1 now
        // unless this instruction was a SS-load (pop ss / mov ss,...) which inhibits
        // the trap for the immediately following instruction (8086/286 behaviour).
        if trap_before {
            if self.suppress_trap {
                self.suppress_trap = false;
            } else {
                self.dispatch_interrupt(bus, 0x01);
            }
        }
    }

    pub(crate) fn wait_for_key_press(&self) -> bool {
        self.wait_for_key_press
    }

    /// Signal that a key has been pressed, if we were waiting handle it
    pub(crate) fn key_press(&mut self, bus: &mut Bus) {
        if self.wait_for_key_press {
            log::debug!("INT 0x16 was waiting for keypress, continuing");
            self.int16_read_char(bus);
            self.wait_for_key_press = false;
            if self.wait_for_key_press_patch_flags {
                self.patch_flags_and_iret(bus);
            }
            self.wait_for_key_press_patch_flags = false;
        }
    }

    /// Dispatch an interrupt: push FLAGS/CS/IP, clear IF/TF, load CS:IP from IVT.
    /// Common mechanism for INT instructions and hardware IRQs.
    fn dispatch_interrupt(&mut self, bus: &mut Bus, int_num: u8) {
        if int_num == 0x10 {
            self.log_int10_video_services();
        } else if int_num == 0x21 {
            self.log_int21_dos_call(bus);
        }
        if self.exec_logging_enabled {
            log::info!(
                "pushing flags={:04X}  CF={} PF={} AF={} ZF={} SF={} TF={} IF={} DF={} OF={}",
                self.flags,
                self.get_flag(cpu_flag::CARRY) as u8,
                self.get_flag(cpu_flag::PARITY) as u8,
                self.get_flag(cpu_flag::AUXILIARY) as u8,
                self.get_flag(cpu_flag::ZERO) as u8,
                self.get_flag(cpu_flag::SIGN) as u8,
                self.get_flag(cpu_flag::TRAP) as u8,
                self.get_flag(cpu_flag::INTERRUPT) as u8,
                self.get_flag(cpu_flag::DIRECTION) as u8,
                self.get_flag(cpu_flag::OVERFLOW) as u8,
            );
            log::info!("pushing CS={:04X}", self.cs);
            log::info!("pushing IP={:04X}", self.ip);
        }
        self.halted = false;
        self.push(self.flags, bus);
        self.push(self.cs, bus);
        self.push(self.ip, bus);
        self.set_flag(cpu_flag::INTERRUPT, false);
        self.set_flag(cpu_flag::TRAP, false);
        let ivt_addr = (int_num as usize) * 4;
        let offset = bus.memory_read_u16(ivt_addr);
        let segment = bus.memory_read_u16(ivt_addr + 2);
        self.ip = offset;
        self.cs = segment;
    }

    pub(crate) fn patch_flags_and_iret(&mut self, bus: &mut Bus) {
        // Patch the stacked FLAGS with any changes the handler made (CF, ZF, SF, etc.),
        // mirroring how a real BIOS handler modifies the caller's FLAGS before iret.
        // Stack layout after `int`: SP+0=IP, SP+2=CS, SP+4=FLAGS
        //
        // We preserve IF from the original stacked flags: dispatch_interrupt cleared IF,
        // so self.flags has IF=0, but the stacked copy reflects the caller's IF state
        // (typically IF=1).  Clobbering IF here would leave the caller running with
        // interrupts disabled after every BIOS call.
        let stacked_flags_addr = bus.physical_address(self.ss, self.sp.wrapping_add(4));
        let original_stacked = bus.memory_read_u16(stacked_flags_addr);
        let patched =
            (self.flags & !cpu_flag::INTERRUPT) | (original_stacked & cpu_flag::INTERRUPT);
        bus.memory_write_u16(stacked_flags_addr, patched);
        self.iret(bus);
    }

    /// Fetch a byte from memory at CS:IP and increment IP
    fn fetch_byte(&mut self, bus: &Bus) -> u8 {
        let addr = bus.physical_address(self.cs, self.ip);
        let byte = bus.memory_read_u8(addr);
        self.ip = self.ip.wrapping_add(1);
        byte
    }

    /// Fetch a word (2 bytes, little-endian) from memory at CS:IP
    fn fetch_word(&mut self, bus: &Bus) -> u16 {
        let low = self.fetch_byte(bus) as u16;
        let high = self.fetch_byte(bus) as u16;
        (high << 8) | low
    }

    pub(crate) fn reset(&mut self, segment: u16, offset: u16, boot_drive: Option<DriveNumber>) {
        self.ax = 0;
        self.bx = 0;
        self.cx = 0;
        self.dx = 0;
        self.si = 0;
        self.di = 0;
        self.bp = 0;
        self.cs = 0;
        self.ds = 0;
        self.es = 0;
        self.fs = 0;
        self.gs = 0;
        self.ip = 0;
        // On 8086, bits 12-15 of FLAGS are physically pulled high (always 1).
        // On 286+, bits 12-15 are 0 after reset.
        self.flags = if self.cpu_type == CpuType::I8086 {
            0xF002
        } else {
            0x0002
        };
        self.halted = false;
        self.suppress_trap = false;
        self.exit_code = None;
        self.segment_override = None;
        self.repeat_prefix = None;
        self.pending_exception = None;
        self.wait_for_key_press = false;
        self.wait_for_key_press_patch_flags = false;
        self.fpu_control_word = FPU_DEFAULT_CONTROL_WORD;
        self.fpu_status_word = 0;
        self.fpu_stack = [f80::F80::ZERO; 8];
        self.fpu_top = 0;

        // Set CPU to start at this location
        self.cs = segment;
        self.ip = offset;

        if let Some(boot_drive) = boot_drive {
            // DL contains boot drive number (0x00 for floppy A:, 0x80 for first hard disk)
            self.dx = (self.dx & 0xFF00) | (boot_drive.as_standard() as u16);
            // Set up stack at 0x0000:0x7C00 (just below boot sector)
            // Some boot loaders expect this, others set up their own stack
            self.ss = 0x0000;
            self.sp = 0x7C00;
            self.current_psp = 0x0000;
        } else {
            // Initialize other segments to reasonable defaults
            self.ds = segment;
            self.es = segment;
            self.ss = segment;
            self.sp = 0xFFFE; // Stack grows down from top of segment
            // For a direct COM load, the PSP is at segment:0x0000.
            // Memory there is zero-initialized, so the INT 22h terminate vector
            // at PSP+0x0A reads as 0x0000:0x0000, causing int21_exit to halt.
            self.current_psp = segment;
        }

        // Enable interrupts - DOS programs expect IF=1 (inherited from DOS environment)
        self.set_flag(cpu_flag::INTERRUPT, true);
    }

    fn step_bios_segment(&mut self, bus: &mut Bus) {
        if self.ip > 0xff {
            log::error!("Invalid BIOS handler 0x{:02X}", self.ip);
            return;
        }
        self.step_bios_int(bus, self.ip as u8);
        if self.wait_for_key_press {
            self.wait_for_key_press_patch_flags = true;
            return;
        }
        // If the BIOS handler did a FAR CALL into user code (e.g. the PS/2
        // mouse callback) CS will no longer point to the BIOS segment.
        // Skip patch_flags_and_iret — the callback will RETF to the
        // trampoline at PS2_MOUSE_RETURN_IP, which then lets
        // patch_flags_and_iret clean up the original INT frame.
        if self.cs != BIOS_CODE_SEGMENT {
            return;
        }
        // Terminal halt (e.g. INT 21h AH=4Ch with no parent): skip IRET so that
        // IF=0 (set by int21_exit) is preserved and no pending IRQ can wake the
        // CPU and resume execution in the terminated program.
        if self.halted {
            return;
        }
        // If the BIOS handler enabled interrupts (STI equivalent), check for a
        // pending timer IRQ and deliver it — mirrors real BIOS behavior where the
        // CPU takes IRQ0 at the next instruction boundary after STI inside the handler.
        // This is critical when caller code runs with IF=0 but the BIOS service does STI.
        //
        // When IVT[0x08] still points to our BIOS handler we run it inline.
        // When a guest has hooked INT 08h we push a trampoline return frame so the
        // guest's handler IRETs back to BIOS_CODE_SEGMENT:TIMER_INLINE_RETURN_IP
        // (a no-op), after which patch_flags_and_iret IRETs to the original caller.
        if self.get_flag(cpu_flag::INTERRUPT) {
            let timer_ivt_off = bus.memory_read_u16(crate::devices::pic::PIT_CPU_IRQ as usize * 4);
            let timer_ivt_seg =
                bus.memory_read_u16(crate::devices::pic::PIT_CPU_IRQ as usize * 4 + 2);
            if bus.pic_mut().take_timer_irq(bus.cycle_count()) {
                if timer_ivt_seg == BIOS_CODE_SEGMENT {
                    self.step_bios_int(bus, crate::devices::pic::PIT_CPU_IRQ);
                } else {
                    // Guest has hooked INT 08h.  Push a trampoline return frame so
                    // that when the guest's handler IRETs, execution resumes at the
                    // BIOS no-op trampoline and then patch_flags_and_iret handles
                    // the original caller's IRET frame.
                    self.push(self.flags, bus);
                    self.push(BIOS_CODE_SEGMENT, bus);
                    self.push(
                        crate::cpu::bios::int08_timer_interrupt::TIMER_INLINE_RETURN_IP,
                        bus,
                    );
                    self.set_flag(cpu_flag::INTERRUPT, false);
                    self.cs = timer_ivt_seg;
                    self.ip = timer_ivt_off;
                    return;
                }
            }
        }
        self.patch_flags_and_iret(bus);
    }

    fn step_bios_int(&mut self, bus: &mut Bus, irq: u8) {
        if self.exec_logging_enabled {
            log::info!("running internal bios interrupt code 0x{irq:02X}");
        }
        match irq {
            0x08 => self.handle_int08_timer_interrupt(bus),
            0x09 => self.handle_int09_keyboard_hardware_interrupt(bus),
            0x10 => self.handle_int10_video_services(bus),
            0x11 => self.handle_int11_get_equipment_list(bus),
            0x12 => self.handle_int12_get_memory_size(bus),
            0x13 => self.handle_int13_disk_services(bus),
            0x14 => self.handle_int14_serial_port_services(bus),
            0x15 => self.handle_int15_miscellaneous(bus),
            0x16 => self.handle_int16_keyboard_services(bus),
            0x17 => self.handle_int17_printer_services(bus),
            0x19 => self.handle_int19_bootstrap_loader(),
            0x1a => self.handle_int1a_time_services(bus),
            0x21 => self.handle_int21_dos_services(bus),
            0x70 => self.handle_int70_rtc_alarm_interrupt(bus),
            0x74 => self.handle_int74_ps2_mouse_interrupt(bus),
            // Default INT 06h handler (invalid opcode): no-op, just IRET.
            // A real BIOS typically does nothing here; programs can install their own handler.
            0x06 => {}
            // Default INT 1Ch handler (user timer tick): no-op, just IRET.
            0x1C => {}
            // Default INT 4Ah handler (user alarm interrupt): no-op, just IRET.
            // Programs that need alarm notification install their own INT 4Ah handler.
            0x4A => {}
            // INT 1Ch IRET trampoline — the chained INT 1Ch handler returned here.
            // Nothing to do; step() will call patch_flags_and_iret to IRET
            // back to wherever INT 08h originally interrupted.
            0xF5 => {}
            // Timer inline-dispatch trampoline — guest INT 08h handler returned here.
            // Nothing to do; step() will call patch_flags_and_iret to IRET
            // back to wherever the original BIOS call interrupted.
            0xF6 => {}
            // PS/2 mouse callback RETF trampoline — the application's handler
            // returned here.  Nothing to do; step() will call patch_flags_and_iret
            // to IRET back to wherever INT 74h originally interrupted.
            0xF4 => {}
            // INT 4Ah IRET trampoline — the chained INT 4Ah user alarm handler returned here.
            // Nothing to do; step() will call patch_flags_and_iret to IRET
            // back to wherever INT 70h originally interrupted.
            0xF3 => {}
            _ => log::error!("unhandled BIOS interrupt 0x{irq:02X}"),
        }
    }

    /// INT 19h — Bootstrap loader.
    /// Sets the bootstrap request flag and jumps to FFFF:0000 (reset vector) without IRET.
    /// Computer::step() detects at_reset_vector() + take_bootstrap_request() and tries
    /// drives in boot order (floppy first, then the configured boot drive).
    fn handle_int19_bootstrap_loader(&mut self) {
        log::debug!("INT 19h: bootstrap loader requested");
        self.bootstrap_request = true;
        // Jump to reset vector; step() will see CS != BIOS_CODE_SEGMENT and skip IRET.
        self.cs = 0xFFFF;
        self.ip = 0x0000;
    }

    pub(crate) fn get_exit_code(&self) -> Option<u8> {
        self.exit_code
    }

    pub(crate) fn at_reset_vector(&self) -> bool {
        self.cs == 0xFFFF && self.ip == 0x0000
    }

    /// Returns true and clears the flag if INT 19h requested a bootstrap.
    pub(crate) fn take_bootstrap_request(&mut self) -> bool {
        let v = self.bootstrap_request;
        self.bootstrap_request = false;
        v
    }

    pub(crate) fn is_terminal_halt(&self) -> bool {
        self.halted && !self.get_flag(cpu_flag::INTERRUPT)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::{Arc, RwLock};

    use crate::cpu::CpuType;
    use crate::devices::pc_speaker::NullPcSpeaker;
    use crate::devices::rtc::tests::MockClock;
    use crate::video::VideoCardType;
    use crate::{
        bus::{Bus, BusConfig},
        cpu::Cpu,
        memory::Memory,
        video::{VideoBuffer, VideoCard},
    };

    pub(crate) fn create_test_cpu() -> (Cpu, Bus) {
        let cpu_clock_speed = 8_000_000;
        let cpu = Cpu::new(CpuType::I8086, cpu_clock_speed, false);
        let video_buffer = Arc::new(RwLock::new(VideoBuffer::new()));
        let bus = Bus::new(BusConfig {
            memory: Memory::new(1024),
            cpu_clock_speed,
            clock: Some(Box::new(MockClock::new())),
            hard_disks: vec![],
            video_card: Rc::new(RefCell::new(VideoCard::new(
                VideoCardType::VGA,
                video_buffer,
                cpu_clock_speed,
            ))),
            pc_speaker: Box::new(NullPcSpeaker::new()),
        });

        (cpu, bus)
    }
}
