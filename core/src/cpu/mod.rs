use crate::{
    bus::Bus,
    cpu::{bios::BIOS_CODE_SEGMENT, instructions::RepeatPrefix},
    disk::DriveNumber,
    physical_address,
};

pub mod bios;
mod cpu_type;
mod instructions;
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
}

impl Cpu {
    pub(crate) fn new(cpu_type: CpuType, clock_speed: u32) -> Self {
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
        }
    }

    pub(crate) fn clock_speed(&self) -> u32 {
        self.clock_speed
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

    pub(crate) fn step(&mut self, bus: &mut Bus) {
        // service any interrupts coming from the PIC
        if self.get_flag(cpu_flag::INTERRUPT) {
            let irq = bus.pic_mut().take_irq(bus.cycle_count());
            if let Some(irq) = irq {
                self.dispatch_interrupt(bus, irq);
                return;
            }
        }

        // service any bios routines
        if self.cs == BIOS_CODE_SEGMENT {
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
            self.patch_flags_and_iret(bus);
            return;
        }

        if self.exec_logging_enabled {
            let decoded = self.decode_instruction_with_regs(bus);

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
                "{}",
                format!(
                    "OP {:04X}:{:04X} {:<18} {:30} {}",
                    self.cs, self.ip, bytes_hex, decoded.text, values,
                )
                .trim()
            );
        }

        self.exec_instruction(bus);
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
        let stacked_flags_addr = physical_address(self.ss, self.sp.wrapping_add(4));
        let original_stacked = bus.memory_read_u16(stacked_flags_addr);
        let patched =
            (self.flags & !cpu_flag::INTERRUPT) | (original_stacked & cpu_flag::INTERRUPT);
        bus.memory_write_u16(stacked_flags_addr, patched);
        self.iret(bus);
    }

    /// Fetch a byte from memory at CS:IP and increment IP
    fn fetch_byte(&mut self, bus: &Bus) -> u8 {
        let addr = physical_address(self.cs, self.ip);
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
        self.flags = 0x0002; // Reserved bit always set
        self.halted = false;
        self.exit_code = None;
        self.segment_override = None;
        self.repeat_prefix = None;
        self.pending_exception = None;
        self.wait_for_key_press = false;
        self.wait_for_key_press_patch_flags = false;

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
        } else {
            // Initialize other segments to reasonable defaults
            self.ds = segment;
            self.es = segment;
            self.ss = segment;
            self.sp = 0xFFFE; // Stack grows down from top of segment
        }

        // Enable interrupts - DOS programs expect IF=1 (inherited from DOS environment)
        self.set_flag(cpu_flag::INTERRUPT, true);
    }

    fn step_bios_int(&mut self, bus: &mut Bus, irq: u8) {
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
            0x1a => self.handle_int1a_time_services(bus),
            0x21 => self.handle_int21_dos_services(bus),
            0x74 => self.handle_int74_ps2_mouse_interrupt(bus),
            // PS/2 mouse callback RETF trampoline — the application's handler
            // returned here.  Nothing to do; step() will call patch_flags_and_iret
            // to IRET back to wherever INT 74h originally interrupted.
            0xF4 => {}
            _ => log::error!("unhandled BIOS interrupt 0x{irq:02X}"),
        }
    }

    pub(crate) fn get_exit_code(&self) -> Option<u8> {
        self.exit_code
    }

    pub(crate) fn at_reset_vector(&self) -> bool {
        self.cs == 0xFFFF && self.ip == 0x0000
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
        let cpu = Cpu::new(CpuType::I8086, cpu_clock_speed);
        let video_buffer = Arc::new(RwLock::new(VideoBuffer::new()));
        let bus = Bus::new(BusConfig {
            memory: Memory::new(1024),
            cpu_clock_speed,
            clock: Some(Box::new(MockClock::new())),
            hard_disks: vec![],
            video_card: Rc::new(RefCell::new(VideoCard::new(
                VideoCardType::VGA,
                video_buffer,
            ))),
            pc_speaker: Box::new(NullPcSpeaker::new()),
        });

        (cpu, bus)
    }
}
