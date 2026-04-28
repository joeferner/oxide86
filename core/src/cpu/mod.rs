use crate::{
    Computer,
    bus::Bus,
    cpu::bios::{
        BIOS_CODE_SEGMENT,
        int08_timer_interrupt::{INT1C_RETURN_IP, TIMER_INLINE_RETURN_IP},
        int21_dos_services::{DosFileHandleTable, PendingDosOpen, PendingDosRead, PendingDosSeek},
        int70_rtc_alarm_interrupt::INT4A_RETURN_IP,
        int74_ps2_mouse_interrupt::PS2_MOUSE_RETURN_IP,
    },
    cpu::instructions::{RepeatPrefix, decoder, fpu::FPU_DEFAULT_CONTROL_WORD},
    debugger::DebugSnapshot,
    disk::DriveNumber,
};

pub mod bios;
mod cpu_type;
pub(crate) mod f80;
pub(crate) mod f80_trig;
pub(crate) mod instructions;
mod protected_mode;
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

/// A pending CPU exception to be dispatched at the start of the next step.
#[derive(Debug, Clone, Copy)]
struct PendingException {
    /// Interrupt vector number (e.g. 0 = #DE, 11 = #NP, 13 = #GP)
    int_num: u8,
    /// Error code pushed onto the stack for certain exceptions (#GP, #NP, #SS, #TS).
    /// `None` for exceptions that don't push an error code (#DE, #UD, etc.).
    error_code: Option<u16>,
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

    // Cached descriptor state (hidden part of segment registers)
    cs_cache: protected_mode::SegmentCache,
    ds_cache: protected_mode::SegmentCache,
    ss_cache: protected_mode::SegmentCache,
    es_cache: protected_mode::SegmentCache,

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

    /// Last effective segment and offset from decode_modrm (for word limit checks)
    last_ea_seg: u16,
    last_ea_offset: u16,

    /// Repeat prefix for string instructions
    repeat_prefix: Option<RepeatPrefix>,

    /// Pending CPU exception, fired at the start of step().
    pending_exception: Option<PendingException>,

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

    /// Registers saved before INT 0x74 calls the application's PS/2 mouse callback.
    /// Restored in the 0xF4 trampoline so the interrupted code sees the original
    /// register values on return (mirrors real BIOS PUSHA/POPA around the FAR CALL).
    saved_for_int74: Option<(u16, u16, u16, u16)>,

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

    // --- 286+ system registers ---
    /// Machine Status Word / Control Register 0 (low 16 bits on 286).
    /// Bit 0 = PE (Protection Enable), Bit 1 = MP, Bit 2 = EM, Bit 3 = TS.
    /// Bits 15-4 are undefined but hardwired to 1 on real 286 hardware; SMSW
    /// therefore returns 0xFFF0 in real mode on a genuine 286.
    cr0: u16,

    /// Global Descriptor Table Register: base address (24-bit on 286)
    gdtr_base: u32,
    /// Global Descriptor Table Register: table limit in bytes
    gdtr_limit: u16,

    /// Interrupt Descriptor Table Register: base address (24-bit on 286)
    idtr_base: u32,
    /// Interrupt Descriptor Table Register: table limit in bytes
    idtr_limit: u16,

    /// Local Descriptor Table Register (selector)
    ldtr: u16,
    /// Cached LDT base address (loaded from GDT descriptor on LLDT)
    ldtr_base: u32,
    /// Cached LDT limit (loaded from GDT descriptor on LLDT)
    ldtr_limit: u16,

    /// Task Register (selector)
    tr: u16,
    /// Cached TSS base address (loaded from GDT descriptor on LTR)
    tr_base: u32,
    /// Cached TSS limit (loaded from GDT descriptor on LTR)
    tr_limit: u16,

    /// Current Privilege Level (0–3)
    cpl: u8,
}

/// Snapshot of CPU state captured before instruction execution, used by the
/// decoder so that annotations can show pre-execution values where needed
/// (e.g. CS before a far jump, FPU ST(0) before a store-and-pop).
struct PreExecState {
    cs: u16,
    cs_cache: protected_mode::SegmentCache,
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
    fn seg_to_phys(&self, seg: u16, offset: u16) -> u32 {
        // Use descriptor cache in both real and protected mode: LOADALL can set a
        // non-standard base while leaving PE clear (e.g. SVARDOS XMS driver).
        // For the pre-execution CS (code fetch), use the pre-execution CS cache so
        // that instructions like LOADALL — which overwrite all segment registers —
        // don't cause the decoder to read from the wrong physical address.
        let cache = if seg == self.pre.cs {
            &self.pre.cs_cache
        } else {
            self.cpu.cache_for_seg_value(seg)
        };
        self.bus
            .apply_a20_pub(cache.base.wrapping_add(offset as u32) as usize) as u32
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
            cs_cache: protected_mode::SegmentCache::default(),
            ds_cache: protected_mode::SegmentCache::default(),
            ss_cache: protected_mode::SegmentCache::default(),
            es_cache: protected_mode::SegmentCache::default(),
            ip: 0,
            flags: 0,
            current_psp: 0x100,
            halted: false,
            exit_code: None,
            segment_override: None,
            last_ea_seg: 0,
            last_ea_offset: 0,
            repeat_prefix: None,
            pending_exception: None,
            last_disk_status: 0,
            wait_for_key_press: false,
            wait_for_key_press_patch_flags: false,
            exec_logging_enabled: false,
            bootstrap_request: false,
            saved_for_int74: None,
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
            // Bits 15-4 hardwired to 1 on real 286 hardware; SMSW returns 0xFFF0 in real mode.
            cr0: if cpu_type.is_286_or_later() {
                0xFFF0
            } else {
                0
            },
            gdtr_base: 0,
            gdtr_limit: 0,
            idtr_base: 0,
            idtr_limit: 0x03FF, // Real-mode default: IVT at 0x0000, 1024 bytes
            ldtr: 0,
            ldtr_base: 0,
            ldtr_limit: 0,
            tr: 0,
            tr_base: 0,
            tr_limit: 0,
            cpl: 0,
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

    /// Returns true if the CPU is in protected mode (PE bit set in CR0/MSW).
    pub(crate) fn in_protected_mode(&self) -> bool {
        self.cr0 & 1 != 0
    }

    /// Set CS and update cache (real-mode base = seg << 4).
    /// Use `load_segment_register(1, ...)` for PM descriptor lookup instead.
    fn set_cs_real(&mut self, value: u16) {
        self.cs = value;
        self.cs_cache = protected_mode::SegmentCache::from_real_mode(value);
    }

    /// Set DS and update cache (real-mode base = seg << 4).
    fn set_ds_real(&mut self, value: u16) {
        self.ds = value;
        self.ds_cache = protected_mode::SegmentCache::from_real_mode(value);
    }

    /// Set ES and update cache (real-mode base = seg << 4).
    fn set_es_real(&mut self, value: u16) {
        self.es = value;
        self.es_cache = protected_mode::SegmentCache::from_real_mode(value);
    }

    /// Set SS and update cache (real-mode base = seg << 4).
    fn set_ss_real(&mut self, value: u16) {
        self.ss = value;
        self.ss_cache = protected_mode::SegmentCache::from_real_mode(value);
    }

    /// Resolve segment:offset to a physical address.
    /// In real mode: descriptor cache base + offset with A20 masking.
    /// In protected mode: cached_base + offset with A20 masking, with limit check.
    /// If the offset exceeds the segment limit, sets pending_exception to #GP(13)
    /// and returns a dummy address (0). Callers should check pending_exception
    /// or let Computer::step() fire the exception before the next instruction.
    ///
    /// The 286 always uses the hidden descriptor cache for address translation.
    /// In normal real mode the cache mirrors segment*16, but LOADALL can set an
    /// arbitrary cache base while leaving PE=0 (e.g. the SVARDOS XMS driver sets
    /// CS_selector=0 and CS_cache.base to a physical address in extended memory).
    fn seg_offset_to_phys(&mut self, segment: u16, offset: u16, bus: &Bus) -> usize {
        if self.in_protected_mode() {
            let cache = *self.cache_for_seg_value(segment);
            if offset as u32 > cache.limit as u32 {
                log::warn!(
                    "#GP: segment 0x{:04X} offset 0x{:04X} exceeds limit 0x{:04X}",
                    segment,
                    offset,
                    cache.limit
                );
                self.pending_exception = Some(PendingException {
                    int_num: 13,
                    error_code: Some(0),
                });
                return 0;
            }
            let addr = cache.base as usize + offset as usize;
            bus.apply_a20_pub(addr)
        } else {
            let base = self.cache_for_seg_value(segment).base;
            bus.apply_a20_pub(base as usize + offset as usize)
        }
    }

    /// Like `seg_offset_to_phys` but also checks that offset+size-1 is within limits.
    /// Used for word (2-byte) accesses where the last byte might exceed the limit.
    fn seg_offset_to_phys_word(&mut self, segment: u16, offset: u16, bus: &Bus) -> usize {
        if self.in_protected_mode() {
            let cache = *self.cache_for_seg_value(segment);
            let last_byte = offset as u32 + 1; // offset of second byte
            if last_byte > cache.limit as u32 {
                log::warn!(
                    "#GP: segment 0x{:04X} word access at offset 0x{:04X} exceeds limit 0x{:04X}",
                    segment,
                    offset,
                    cache.limit
                );
                self.pending_exception = Some(PendingException {
                    int_num: 13,
                    error_code: Some(0),
                });
                return 0;
            }
            let addr = cache.base as usize + offset as usize;
            bus.apply_a20_pub(addr)
        } else {
            let base = self.cache_for_seg_value(segment).base;
            bus.apply_a20_pub(base as usize + offset as usize)
        }
    }

    /// Resolve a physical address from an explicit descriptor cache and offset.
    /// Unlike `seg_offset_to_phys`, this bypasses `cache_for_seg_value` and uses
    /// the supplied cache directly — required when two segment registers hold the
    /// same selector value (e.g. after LOADALL sets ES == DS for XMS transfers).
    fn seg_cache_to_phys(
        &mut self,
        cache: protected_mode::SegmentCache,
        offset: u16,
        bus: &Bus,
    ) -> usize {
        if self.in_protected_mode() && offset as u32 > cache.limit as u32 {
            log::warn!(
                "#GP: cache base 0x{:06X} offset 0x{:04X} exceeds limit 0x{:04X}",
                cache.base,
                offset,
                cache.limit
            );
            self.pending_exception = Some(PendingException {
                int_num: 13,
                error_code: Some(0),
            });
            return 0;
        }
        bus.apply_a20_pub(cache.base as usize + offset as usize)
    }

    /// Get the segment cache for a segment register value.
    /// In protected mode, looks up which segment register holds this value.
    fn cache_for_seg_value(&self, segment: u16) -> &protected_mode::SegmentCache {
        if segment == self.cs {
            &self.cs_cache
        } else if segment == self.ds {
            &self.ds_cache
        } else if segment == self.ss {
            &self.ss_cache
        } else if segment == self.es {
            &self.es_cache
        } else {
            // Fallback: treat as real-mode style (shouldn't happen in PM)
            &self.ds_cache
        }
    }

    /// Load a segment register with descriptor lookup in protected mode.
    /// In real mode, just sets the register value and updates the cache.
    fn load_segment_register(&mut self, reg: u8, selector: u16, bus: &Bus) {
        if self.in_protected_mode() {
            // Null selector is allowed for DS/ES (not CS/SS)
            if selector & 0xFFF8 == 0 {
                match reg & 0x03 {
                    0 => {
                        self.es = selector;
                        self.es_cache = protected_mode::SegmentCache::default();
                    }
                    3 => {
                        self.ds = selector;
                        self.ds_cache = protected_mode::SegmentCache::default();
                    }
                    _ => {
                        log::warn!("Null selector loaded into CS or SS — #GP(0)");
                        self.pending_exception = Some(PendingException {
                            int_num: 13,
                            error_code: Some(0),
                        });
                    }
                }
                return;
            }

            // Look up descriptor from GDT (TI=0) or LDT (TI=1)
            let descriptor = if selector & 0x04 == 0 {
                // GDT
                protected_mode::load_descriptor_from_table(
                    bus,
                    self.gdtr_base,
                    self.gdtr_limit,
                    selector,
                )
            } else {
                // LDT
                let result = protected_mode::load_descriptor_from_table(
                    bus,
                    self.ldtr_base,
                    self.ldtr_limit,
                    selector,
                );
                match &result {
                    None => {
                        let byte_offset = (selector & 0xFFF8) as u32;
                        let raw: [u8; 8] = std::array::from_fn(|i| {
                            bus.memory_read_u8((self.ldtr_base + byte_offset + i as u32) as usize)
                        });
                        log::warn!(
                            "LDT selector 0x{:04X} out of bounds: ldtr_base=0x{:06X} ldtr_limit=0x{:04X} byte_offset=0x{:04X} raw=[{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}]",
                            selector, self.ldtr_base, self.ldtr_limit, byte_offset,
                            raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7]
                        );
                    }
                    Some(desc) if desc.limit == 0 => {
                        let byte_offset = (selector & 0xFFF8) as u32;
                        let raw: [u8; 8] = std::array::from_fn(|i| {
                            bus.memory_read_u8((self.ldtr_base + byte_offset + i as u32) as usize)
                        });
                        log::warn!(
                            "LDT selector 0x{:04X} has limit=0: ldtr_base=0x{:06X} ldtr_limit=0x{:04X} byte_offset=0x{:04X} raw=[{:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X} {:02X}]",
                            selector, self.ldtr_base, self.ldtr_limit, byte_offset,
                            raw[0], raw[1], raw[2], raw[3], raw[4], raw[5], raw[6], raw[7]
                        );
                    }
                    _ => {}
                }
                result
            };

            match descriptor {
                Some(desc) => {
                    if !desc.is_present() {
                        log::warn!(
                            "Segment 0x{:04X} not present — #NP(0x{:04X})",
                            selector,
                            selector
                        );
                        self.pending_exception = Some(PendingException {
                            int_num: 11,
                            error_code: Some(selector),
                        });
                        return;
                    }
                    let cache = desc.to_cache();
                    match reg & 0x03 {
                        0 => {
                            self.es = selector;
                            self.es_cache = cache;
                        }
                        1 => {
                            self.cs = selector;
                            self.cs_cache = cache;
                            self.cpl = (selector & 0x03) as u8;
                        }
                        2 => {
                            self.ss = selector;
                            self.ss_cache = cache;
                            self.suppress_trap = true;
                        }
                        3 => {
                            self.ds = selector;
                            self.ds_cache = cache;
                        }
                        _ => unreachable!(),
                    }
                }
                None => {
                    log::warn!(
                        "Selector 0x{:04X} out of bounds (GDTR limit=0x{:04X}) — #GP(0x{:04X})",
                        selector,
                        self.gdtr_limit,
                        selector
                    );
                    self.pending_exception = Some(PendingException {
                        int_num: 13,
                        error_code: Some(selector),
                    });
                }
            }
        } else {
            // Real mode: set register and update cache
            match reg & 0x03 {
                0 => {
                    self.es = selector;
                    self.es_cache = protected_mode::SegmentCache::from_real_mode(selector);
                }
                1 => {
                    self.cs = selector;
                    self.cs_cache = protected_mode::SegmentCache::from_real_mode(selector);
                }
                2 => {
                    self.ss = selector;
                    self.ss_cache = protected_mode::SegmentCache::from_real_mode(selector);
                    self.suppress_trap = true;
                }
                3 => {
                    self.ds = selector;
                    self.ds_cache = protected_mode::SegmentCache::from_real_mode(selector);
                }
                _ => unreachable!(),
            }
        }
    }

    /// Get the DPL of a selector's descriptor. Returns 0 if descriptor not found.
    fn get_selector_dpl(&self, selector: u16, bus: &Bus) -> u8 {
        let (table_base, table_limit) = if selector & 0x04 == 0 {
            (self.gdtr_base, self.gdtr_limit)
        } else {
            (self.ldtr_base, self.ldtr_limit)
        };
        if let Some(raw) =
            protected_mode::load_raw_descriptor(bus, table_base, table_limit, selector)
        {
            (raw[5] >> 5) & 0x03
        } else {
            0
        }
    }

    /// Read ring 0 SS:SP from the 286 TSS.
    /// 286 TSS layout: +02 = SP0, +04 = SS0
    fn read_tss_ring0_stack(&self, bus: &Bus) -> (u16, u16) {
        let sp0 = bus.memory_read_u16(self.tr_base as usize + 2);
        let ss0 = bus.memory_read_u16(self.tr_base as usize + 4);
        (ss0, sp0)
    }

    /// Handle a far CALL in protected mode.
    /// Checks if the selector points to a code segment (direct transfer)
    /// or a call gate (indirect transfer through the gate).
    fn far_call_pm(&mut self, selector: u16, offset: u16, bus: &mut Bus) {
        let (table_base, table_limit) = if selector & 0x04 == 0 {
            (self.gdtr_base, self.gdtr_limit)
        } else {
            (self.ldtr_base, self.ldtr_limit)
        };

        let raw = protected_mode::load_raw_descriptor(bus, table_base, table_limit, selector);
        let raw = match raw {
            Some(r) => r,
            None => {
                log::warn!("Far CALL: selector 0x{:04X} out of bounds — #GP", selector);
                self.pending_exception = Some(PendingException {
                    int_num: 13,
                    error_code: Some(selector),
                });
                return;
            }
        };

        let access = raw[5];
        if protected_mode::is_call_gate(access) {
            // Call gate: may involve privilege transition with stack switch
            let gate = protected_mode::GateDescriptor::from_bytes(&raw);
            if !gate.is_present() {
                log::warn!("Far CALL: call gate 0x{:04X} not present — #NP", selector);
                self.pending_exception = Some(PendingException {
                    int_num: 11,
                    error_code: Some(selector),
                });
                return;
            }

            // Determine target DPL from the target code segment descriptor
            let target_dpl = self.get_selector_dpl(gate.selector, bus);

            if target_dpl < self.cpl {
                // Inter-privilege call: switch stacks via TSS
                let old_ss = self.ss;
                let old_sp = self.sp;
                let old_cs = self.cs;
                let old_ip = self.ip;

                // Read new SS:SP from TSS for the target privilege level
                let (new_ss, new_sp) = self.read_tss_ring0_stack(bus);

                // Switch to new stack
                self.load_segment_register(2, new_ss, bus); // SS
                self.sp = new_sp;

                // Push caller's SS:SP onto new stack
                self.push(old_ss, bus);
                self.push(old_sp, bus);

                // Push return CS:IP
                self.push(old_cs, bus);
                self.push(old_ip, bus);

                // Update CPL to target privilege level
                self.cpl = target_dpl;

                // Load CS:IP from gate
                self.ip = gate.offset;
                self.load_segment_register(1, gate.selector, bus);
            } else {
                // Same-privilege call gate: no stack switch
                self.push(self.cs, bus);
                self.push(self.ip, bus);
                self.ip = gate.offset;
                self.load_segment_register(1, gate.selector, bus);
            }
        } else {
            // Code segment: direct far call
            self.push(self.cs, bus);
            self.push(self.ip, bus);
            self.ip = offset;
            self.load_segment_register(1, selector, bus);
        }
    }

    /// Handle a far JMP in protected mode.
    /// Checks if the selector points to a code segment (direct transfer)
    /// or a call gate (indirect transfer through the gate, no return address pushed).
    fn far_jmp_pm(&mut self, selector: u16, offset: u16, bus: &mut Bus) {
        let (table_base, table_limit) = if selector & 0x04 == 0 {
            (self.gdtr_base, self.gdtr_limit)
        } else {
            (self.ldtr_base, self.ldtr_limit)
        };

        let raw = protected_mode::load_raw_descriptor(bus, table_base, table_limit, selector);
        let raw = match raw {
            Some(r) => r,
            None => {
                log::warn!("Far JMP: selector 0x{:04X} out of bounds — #GP", selector);
                self.pending_exception = Some(PendingException {
                    int_num: 13,
                    error_code: Some(selector),
                });
                return;
            }
        };

        let access = raw[5];
        if protected_mode::is_call_gate(access) {
            let gate = protected_mode::GateDescriptor::from_bytes(&raw);
            if !gate.is_present() {
                log::warn!("Far JMP: call gate 0x{:04X} not present — #NP", selector);
                self.pending_exception = Some(PendingException {
                    int_num: 11,
                    error_code: Some(selector),
                });
                return;
            }
            // JMP through call gate: no return address pushed
            self.ip = gate.offset;
            self.load_segment_register(1, gate.selector, bus);
        } else {
            // Code segment: direct far jump
            self.ip = offset;
            self.load_segment_register(1, selector, bus);
        }
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
        // Fire any pending CPU exception (e.g. #DE from divide, #GP from limit violation)
        if let Some(exc) = self.pending_exception.take() {
            if self.in_protected_mode() {
                self.dispatch_exception_pm(bus, exc.int_num, exc.error_code);
            } else {
                self.dispatch_interrupt(bus, exc.int_num);
            }
            return;
        }

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
                cs_cache: self.cs_cache,
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
                // WAIT (0x9B) is a two-step prefix: 0x9B advances IP by one byte,
                // then the FPU body executes as the next step. Suppress that body
                // so it doesn't appear as a duplicate log line.
                // REP/REPNE/LOCK all call exec_instruction() recursively in the
                // same step, so their body is already done — no suppression needed.
                if instr.prefix == Some("wait") {
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

    /// Dispatch an interrupt: push FLAGS/CS/IP, load CS:IP from IVT (real mode)
    /// or IDT (protected mode).
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

        if self.in_protected_mode() {
            self.dispatch_interrupt_pm(bus, int_num);
        } else {
            self.dispatch_interrupt_real(bus, int_num);
        }
    }

    /// Real-mode interrupt dispatch: push FLAGS/CS/IP, clear IF/TF, load from IVT.
    fn dispatch_interrupt_real(&mut self, bus: &mut Bus, int_num: u8) {
        self.push(self.flags, bus);
        self.push(self.cs, bus);
        self.push(self.ip, bus);
        self.set_flag(cpu_flag::INTERRUPT, false);
        self.set_flag(cpu_flag::TRAP, false);
        let ivt_addr = (int_num as usize) * 4;
        let offset = bus.memory_read_u16(ivt_addr);
        let segment = bus.memory_read_u16(ivt_addr + 2);
        self.ip = offset;
        self.set_cs_real(segment);
    }

    /// Protected-mode exception dispatch: push error code (if any), then FLAGS/CS/IP
    /// via the IDT. Used for #GP, #NP, #SS, #TS, #DE, etc.
    fn dispatch_exception_pm(&mut self, bus: &mut Bus, int_num: u8, error_code: Option<u16>) {
        let gate = protected_mode::load_idt_gate(bus, self.idtr_base, self.idtr_limit, int_num);

        let gate = match gate {
            Some(g) if g.is_present() && (g.is_interrupt_gate() || g.is_trap_gate()) => g,
            _ => {
                if int_num == 0x08 {
                    // No gate for #DF → triple fault → halt
                    log::error!("Triple fault: no IDT gate for #DF (0x08), halting CPU");
                    self.halted = true;
                    return;
                }
                log::warn!(
                    "Exception 0x{:02X}: no valid IDT gate, raising #DF (double fault)",
                    int_num
                );
                self.dispatch_exception_pm(bus, 0x08, Some(0));
                return;
            }
        };

        // Check for inter-privilege transition (exception handlers typically run at ring 0)
        let target_dpl = self.get_selector_dpl(gate.selector, bus);
        if target_dpl < self.cpl {
            // Inter-privilege: switch to ring 0 stack from TSS, push old SS:SP
            let old_ss = self.ss;
            let old_sp = self.sp;
            let (new_ss, new_sp) = self.read_tss_ring0_stack(bus);
            self.load_segment_register(2, new_ss, bus); // SS
            self.sp = new_sp;
            self.push(old_ss, bus);
            self.push(old_sp, bus);
        }

        // Push FLAGS, CS, IP
        self.push(self.flags, bus);
        self.push(self.cs, bus);
        self.push(self.ip, bus);

        // Push error code (exceptions like #GP, #NP, #SS, #TS push one)
        if let Some(code) = error_code {
            self.push(code, bus);
        }

        self.set_flag(cpu_flag::TRAP, false);
        if gate.is_interrupt_gate() {
            self.set_flag(cpu_flag::INTERRUPT, false);
        }

        self.ip = gate.offset;
        self.load_segment_register(1, gate.selector, bus);
    }

    /// Protected-mode interrupt dispatch: push FLAGS/CS/IP, load from IDT gate.
    /// Interrupt gates clear IF; trap gates preserve IF.
    fn dispatch_interrupt_pm(&mut self, bus: &mut Bus, int_num: u8) {
        let gate = protected_mode::load_idt_gate(bus, self.idtr_base, self.idtr_limit, int_num);

        let gate = match gate {
            Some(g) => g,
            None => {
                log::warn!(
                    "INT 0x{:02X}: IDT entry out of bounds (IDTR limit=0x{:04X})",
                    int_num,
                    self.idtr_limit
                );
                // Fall back to real-mode IVT dispatch (e.g. for BIOS calls
                // that haven't been redirected through IDT entries)
                self.dispatch_interrupt_real(bus, int_num);
                return;
            }
        };

        if !gate.is_present() || (!gate.is_interrupt_gate() && !gate.is_trap_gate()) {
            // Gate not present or not a recognized gate type.
            // Fall back to real-mode IVT dispatch. This handles the case where
            // the default IDTR (base=0, limit=0x3FF) overlaps the real-mode IVT
            // and the bytes there don't form valid gate descriptors.
            log::debug!(
                "INT 0x{:02X}: IDT gate not usable (access=0x{:02X}), falling back to IVT",
                int_num,
                gate.access
            );
            self.dispatch_interrupt_real(bus, int_num);
            return;
        }

        // Check for inter-privilege transition (handlers typically run at ring 0)
        let target_dpl = self.get_selector_dpl(gate.selector, bus);
        if target_dpl < self.cpl {
            // Inter-privilege: switch to ring 0 stack from TSS, push old SS:SP
            let old_ss = self.ss;
            let old_sp = self.sp;
            let (new_ss, new_sp) = self.read_tss_ring0_stack(bus);
            self.load_segment_register(2, new_ss, bus); // SS
            self.sp = new_sp;
            self.push(old_ss, bus);
            self.push(old_sp, bus);
        }

        // Push FLAGS, CS, IP
        self.push(self.flags, bus);
        self.push(self.cs, bus);
        self.push(self.ip, bus);

        // Clear TF always
        self.set_flag(cpu_flag::TRAP, false);

        // Interrupt gates clear IF; trap gates leave IF unchanged
        if gate.is_interrupt_gate() {
            self.set_flag(cpu_flag::INTERRUPT, false);
        }

        // Load CS from the gate's selector (with descriptor lookup)
        self.ip = gate.offset;
        self.load_segment_register(1, gate.selector, bus); // 1 = CS
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
        let stacked_flags_addr = self.seg_offset_to_phys(self.ss, self.sp.wrapping_add(4), bus);
        let original_stacked = bus.memory_read_u16(stacked_flags_addr);
        let patched =
            (self.flags & !cpu_flag::INTERRUPT) | (original_stacked & cpu_flag::INTERRUPT);
        bus.memory_write_u16(stacked_flags_addr, patched);
        self.iret(bus);
    }

    /// Fetch a byte from memory at CS:IP and increment IP
    fn fetch_byte(&mut self, bus: &Bus) -> u8 {
        let addr = self.seg_offset_to_phys(self.cs, self.ip, bus);
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
        self.set_cs_real(0);
        self.set_ds_real(0);
        self.set_es_real(0);
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
        self.saved_for_int74 = None;
        self.fpu_control_word = FPU_DEFAULT_CONTROL_WORD;
        self.fpu_status_word = 0;
        self.fpu_stack = [f80::F80::ZERO; 8];
        self.fpu_top = 0;

        // Reset 286+ system registers
        // Bits 15-4 hardwired to 1 on real 286 hardware; SMSW returns 0xFFF0 in real mode.
        self.cr0 = if self.cpu_type.is_286_or_later() {
            0xFFF0
        } else {
            0
        };
        self.gdtr_base = 0;
        self.gdtr_limit = 0;
        self.idtr_base = 0;
        self.idtr_limit = 0x03FF;
        self.ldtr = 0;
        self.ldtr_base = 0;
        self.ldtr_limit = 0;
        self.tr = 0;
        self.tr_base = 0;
        self.tr_limit = 0;
        self.cpl = 0;

        // Set CPU to start at this location
        self.set_cs_real(segment);
        self.ip = offset;

        if let Some(boot_drive) = boot_drive {
            // DL contains boot drive number (0x00 for floppy A:, 0x80 for first hard disk)
            self.dx = (self.dx & 0xFF00) | (boot_drive.as_standard() as u16);
            // Set up stack at 0x0000:0x7C00 (just below boot sector)
            // Some boot loaders expect this, others set up their own stack
            self.set_ss_real(0x0000);
            self.sp = 0x7C00;
            self.current_psp = 0x0000;
        } else {
            // Initialize other segments to reasonable defaults
            self.set_ds_real(segment);
            self.set_es_real(segment);
            self.set_ss_real(segment);
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
        let entry_ip = self.ip;
        self.step_bios_int(bus, entry_ip as u8);
        if self.wait_for_key_press {
            // Trampoline IPs are no-ops whose sole purpose is to trigger patch_flags_and_iret
            // to unwind a nested interrupt frame (e.g. a timer IRQ that fired while we were
            // waiting for a keypress).  Skipping patch_flags_and_iret here would leave the
            // stack unwound and spin the CPU forever at the trampoline with no cycle progress.
            let is_trampoline = matches!(
                entry_ip,
                INT1C_RETURN_IP | TIMER_INLINE_RETURN_IP | PS2_MOUSE_RETURN_IP | INT4A_RETURN_IP
            );
            if !is_trampoline {
                self.wait_for_key_press_patch_flags = true;
                return;
            }
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
                    self.set_cs_real(timer_ivt_seg);
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
            // returned here.  Restore registers saved before the FAR CALL so that
            // the interrupted code sees the original values on return; then
            // step() will call patch_flags_and_iret to IRET back to the
            // interrupted instruction.
            0xF4 => {
                if let Some((ax, bx, cx, dx)) = self.saved_for_int74.take() {
                    self.ax = ax;
                    self.bx = bx;
                    self.cx = cx;
                    self.dx = dx;
                }
            }
            // INT 4Ah IRET trampoline — the chained INT 4Ah user alarm handler returned here.
            // Nothing to do; step() will call patch_flags_and_iret to IRET
            // back to wherever INT 70h originally interrupted.
            0xF3 => {}
            // INT 68h - IBM PC LAN Program / NetBIOS presence check.
            // Without network software installed, IRET with no side-effects is correct.
            0x68 => {}
            // Unhandled PIC1 hardware IRQ vectors (0x0A–0x0F).
            // A real BIOS sends EOI before returning from any hardware IRQ handler so the PIC
            // can clear its in_service bit and deliver future IRQs on the same line.
            // Without EOI, in_service stays set and the PIC silently drops all subsequent IRQs
            // on that line — e.g. IRQ5 (INT 0x0D, CD-ROM) before SBPCD.SYS installs its handler.
            0x0A..=0x0F => {
                log::debug!("BIOS default handler for hardware IRQ 0x{irq:02X}: sending EOI");
                bus.io_write_u8(
                    crate::devices::pic::PIC_IO_PORT_COMMAND,
                    crate::devices::pic::PIC_COMMAND_EOI,
                );
            }
            _ => log::warn!("unhandled BIOS interrupt 0x{irq:02X}"),
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
        self.set_cs_real(0xFFFF);
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
            cpu_type: CpuType::I8086,
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
