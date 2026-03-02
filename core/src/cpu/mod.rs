use crate::{Bus, CpuType, io::IoDevice};

pub mod bios;
mod instructions;
pub(super) mod timing;

pub struct Cpu {
// MIGRATED      // General purpose registers
// MIGRATED      pub ax: u16,
// MIGRATED      pub bx: u16,
// MIGRATED      pub cx: u16,
// MIGRATED      pub dx: u16,
// MIGRATED  
// MIGRATED      // Index and pointer registers
// MIGRATED      pub si: u16,
// MIGRATED      pub di: u16,
// MIGRATED      pub sp: u16,
// MIGRATED      pub bp: u16,
// MIGRATED  
// MIGRATED      // Segment registers
// MIGRATED      pub cs: u16,
// MIGRATED      pub ds: u16,
// MIGRATED      pub ss: u16,
// MIGRATED      pub es: u16,
// MIGRATED      pub fs: u16, // 80386+
// MIGRATED      pub gs: u16, // 80386+
// MIGRATED  
// MIGRATED      // Instruction pointer
// MIGRATED      pub ip: u16,
// MIGRATED  
// MIGRATED      // Flags (start with just carry, zero, sign)
// MIGRATED      pub flags: u16,
// MIGRATED  
// MIGRATED      // Halted flag
// MIGRATED      halted: bool,

    // Wait state (paused waiting for external event)
    wait_state: CpuWaitState,

// MIGRATED      // Segment override prefix (for next instruction only)
// MIGRATED      segment_override: Option<u16>,

// MIGRATED      // Repeat prefix for string instructions
// MIGRATED      repeat_prefix: Option<RepeatPrefix>,

    /// if true logs interrupts at info level
    pub log_interrupts_enabled: bool,

    /// if set to true, opcode execution will be logged as info level
    pub exec_logging_enabled: bool,

    /// Cycle count for the last executed instruction
    /// Used by Computer::step() to accurately track CPU cycles
    pub(super) last_instruction_cycles: u64,

    /// Pending sleep cycles (set by INT 15h AH=86h)
    /// When > 0, Computer's step() will burn cycles instead of executing instructions
    pub(super) pending_sleep_cycles: u64,

    /// CPU clock frequency in Hz (e.g. 4_770_000 for 4.77 MHz)
    /// Used by interrupt handlers that need to convert real time to cycles
    pub(super) cpu_freq: u64,

    /// IRQ chain context - tracks nested interrupt chaining
    /// None = normal execution
    /// Some(IrqChainContext) = currently processing a chained interrupt
    irq_chain_context: Option<IrqChainContext>,
}

/// IRQ chain context - tracks state when one interrupt chains to another
/// (e.g., INT 08h -> INT 1Ch)
#[derive(Debug, Clone, Copy)]
pub(crate) struct IrqChainContext {
    /// The original interrupt that started the chain (e.g., 0x08)
    original_int: u8,
    /// Stack pointer before chain started (for validation)
    original_sp: u16,
    /// Return address after chain completes
    return_cs: u16,
    return_ip: u16,
    /// Flags to restore after chain
    return_flags: u16,
}

/// CPU wait state - indicates the CPU is paused waiting for an external event
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CpuWaitState {
    /// CPU is executing instructions normally
    Running,
    /// CPU is waiting for keyboard input from INT 16h AH=00h
    /// When resumed, INT 16h handler will be retried
    WaitingForKeyboardInt16,
}

// Flag bit positions
pub mod cpu_flag {
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

impl Cpu {
    pub fn new() -> Self {
        Self {
            wait_state: CpuWaitState::Running,
            log_interrupts_enabled: false,
            exec_logging_enabled: false,
            last_instruction_cycles: 0,
            pending_sleep_cycles: 0,
            cpu_freq: 4_770_000,
            irq_chain_context: None,
        }
    }

    // Reset CPU to initial state (as if powered on)
    pub fn reset(&mut self) {
        // Other typical reset values
        self.wait_state = CpuWaitState::Running;
        // Other registers are undefined on reset
    }

    // Fetch a byte from memory at CS:IP and increment IP
    pub(crate) fn fetch_byte(&mut self, bus: &Bus) -> u8 {
        let addr = Self::physical_address(self.cs, self.ip);
        let byte = bus.read_u8(addr);
        self.ip = self.ip.wrapping_add(1);
        byte
    }

    // Fetch a word (2 bytes, little-endian) from memory at CS:IP
    pub(super) fn fetch_word(&mut self, bus: &Bus) -> u16 {
        let low = self.fetch_byte(bus) as u16;
        let high = self.fetch_byte(bus) as u16;
        (high << 8) | low
    }

    // Check if CPU is halted
    pub fn is_halted(&self) -> bool {
        self.halted
    }

    /// Clear the halted state (used when an interrupt wakes the CPU from HLT)
    pub fn clear_halt(&mut self) {
        self.halted = false;
    }

    /// Check if CPU is waiting for keyboard input
    pub fn is_waiting_for_keyboard(&self) -> bool {
        self.wait_state == CpuWaitState::WaitingForKeyboardInt16
    }

    /// Get the current wait state
    pub fn wait_state(&self) -> CpuWaitState {
        self.wait_state
    }

    /// Set CPU to wait for keyboard input (INT 16h will be retried when resumed)
    pub fn set_waiting_for_keyboard(&mut self) {
        self.wait_state = CpuWaitState::WaitingForKeyboardInt16;
    }

    /// Resume CPU from wait state, returns true if INT 16h should be retried
    pub fn resume_from_wait(&mut self) -> bool {
        let should_retry = self.wait_state == CpuWaitState::WaitingForKeyboardInt16;
        self.wait_state = CpuWaitState::Running;
        should_retry
    }

    /// Begin an IRQ chain (e.g., INT 08h -> INT 1Ch)
    ///
    /// Called when a BIOS interrupt handler needs to chain to another interrupt.
    /// Sets up the chain context and transfers control to the target interrupt.
    ///
    /// # Arguments
    /// * `from_int` - The original interrupt number (e.g., 0x08)
    /// * `to_int` - The target interrupt to chain to (e.g., 0x1C)
    /// * `return_cs` - Code segment to return to after chain completes
    /// * `return_ip` - Instruction pointer to return to after chain completes
    /// * `return_flags` - Flags to restore after chain completes
    /// * `bus` - Bus for reading IVT and pushing stack frame
    pub(crate) fn begin_irq_chain(
        &mut self,
        from_int: u8,
        to_int: u8,
        return_cs: u16,
        return_ip: u16,
        return_flags: u16,
        bus: &mut Bus,
    ) {
        // Save chain context
        self.irq_chain_context = Some(IrqChainContext {
            original_int: from_int,
            original_sp: self.sp,
            return_cs,
            return_ip,
            return_flags,
        });

        // Push return frame for chained handler's IRET
        self.push(return_flags, bus);
        self.push(return_cs, bus);
        self.push(return_ip, bus);

        // Clear IF and TF flags (standard INT behavior)
        self.set_flag(cpu_flag::INTERRUPT, false);
        self.set_flag(cpu_flag::TRAP, false);

        // Load interrupt vector from IVT
        let ivt_addr = (to_int as usize) * 4;
        let offset = bus.read_u16(ivt_addr);
        let segment = bus.read_u16(ivt_addr + 2);

        // Transfer control to chained handler
        self.cs = segment;
        self.ip = offset;

        log::debug!(
            "IRQ Chain Begin: INT 0x{:02X} -> INT 0x{:02X} at {:04X}:{:04X}",
            from_int,
            to_int,
            segment,
            offset
        );
    }

    /// Check if CPU is in an IRQ chain
    pub(crate) fn is_in_irq_chain(&self) -> bool {
        self.irq_chain_context.is_some()
    }

    /// Complete an IRQ chain (called when chained handler does IRET)
    ///
    /// Validates stack state and clears chain context.
    /// Returns the chain context if one was active.
    pub(crate) fn complete_irq_chain(&mut self) -> Option<IrqChainContext> {
        if let Some(context) = self.irq_chain_context.take() {
            // Validate that stack pointer matches expected value
            // IRET already popped IP, CS, FLAGS (6 bytes)
            if self.sp != context.original_sp {
                log::warn!(
                    "Stack mismatch after IRQ chain: expected SP={:04X}, got SP={:04X}",
                    context.original_sp,
                    self.sp
                );
            }
            Some(context)
        } else {
            None
        }
    }

    // Execute an INT instruction with BIOS I/O handler
    pub fn execute_int_with_io(
        &mut self,
        int_num: u8,
        bus: &mut Bus,
        io: &mut crate::cpu::bios::Bios,
        cpu_type: crate::CpuType,
    ) {
        // If DOS has installed its own handler (IVT not pointing to BIOS ROM),
        // let DOS handle it instead of intercepting
        let is_bios_handler = Self::is_bios_handler(bus, int_num);

        if self.log_interrupts_enabled
            && int_num != 0x10
            && int_num != 0x16
            && int_num != 0x2a
            && int_num != 0x28
            && int_num != 0x29
        {
            log::info!(
                "INT 0x{:02X} AX={:04X} BX={:04X} CX={:04X} DX={:04X} BIOS={} IF={}",
                int_num,
                self.ax,
                self.bx,
                self.cx,
                self.dx,
                is_bios_handler,
                if self.get_flag(cpu_flag::INTERRUPT) {
                    1
                } else {
                    0
                }
            );
        }

        if is_bios_handler {
            self.handle_bios_interrupt_impl(int_num, bus, io, cpu_type);
        } else {
            // Not handled, do normal INT
            // Push flags, CS, and IP
            self.push(self.flags, bus);
            self.push(self.cs, bus);
            self.push(self.ip, bus);
            // Clear IF and TF
            self.set_flag(cpu_flag::INTERRUPT, false);
            self.set_flag(cpu_flag::TRAP, false);
            // Load interrupt vector from IVT
            let ivt_addr = (int_num as usize) * 4;
            let offset = bus.read_u16(ivt_addr);
            let segment = bus.read_u16(ivt_addr + 2);
            self.ip = offset;
            self.cs = segment;
        }
    }

    // Decode and execute instruction with I/O port support
    pub(crate) fn execute_with_io(
        &mut self,
        opcode: u8,
        bus: &mut Bus,
        bios: &mut crate::cpu::bios::Bios,
        io_device: &mut IoDevice,
    ) {
        match opcode {
            // POP SS (17)
            0x17 => self.pop_segreg(opcode, bus),


            // POPA - Pop All General Registers (61)
            0x61 => self.popa(bus),

            // BOUND - Check Array Index Against Bounds (62)
            0x62 => {
                if self.bound(bus) {
                    // Index out of bounds - trigger INT 5
                    self.push(self.flags, bus);
                    self.push(self.cs, bus);
                    // IP points after BOUND instruction; we need to point at it
                    // The modrm byte and any displacement were already consumed by bound()
                    // so we save the current IP (which is past the instruction)
                    self.push(self.ip, bus);
                    self.set_flag(cpu_flag::INTERRUPT, false);
                    self.set_flag(cpu_flag::TRAP, false);
                    // Load INT 5 vector
                    let ivt_addr = 5 * 4;
                    self.ip = bus.read_u16(ivt_addr);
                    self.cs = bus.read_u16(ivt_addr + 2);
                }
            }

            // GS: segment override prefix (65) - 80386+
            0x65 => {
                self.segment_override = Some(self.gs);
                let next_opcode = self.fetch_byte(bus);
                self.execute_with_io(next_opcode, bus, bios, io_device);
                self.segment_override = None;
            }

            // IMUL - Signed Multiply with Immediate (69: imm16, 6B: imm8 sign-extended)
            0x69 => self.imul_imm16(bus),
            0x6B => self.imul_imm8(bus),

            // OUTS - Output String to Port (6E-6F)
            0x6E..=0x6F => self.outs(opcode, bus, bios, io_device),

            // CALL far (9A)
            0x9A => self.call_far(bus),

            // LEAVE - High Level Procedure Exit (C9, 80186+)
            0xC9 => self.leave(bus),

            // INTO - Interrupt on Overflow (CE)
            0xCE => self.into(bus),

            // IN AX, imm8 (E5)
            0xE5 => self.in_ax_imm8(bus, bios, io_device),

            // OUT imm8, AX (E7)
            0xE7 => self.out_imm8_ax(bus, bios, io_device),

            // IN AX, DX (ED)
            0xED => self.in_ax_dx(bios, io_device),

            // OUT DX, AX (EF)
            0xEF => self.out_dx_ax(bus, bios, io_device),

        }
    }

    // Set 8-bit register
    pub(super) fn set_reg8(&mut self, reg: u8, value: u8) {
        match reg {
            0 => self.ax = (self.ax & 0xFF00) | value as u16, // AL
            1 => self.cx = (self.cx & 0xFF00) | value as u16, // CL
            2 => self.dx = (self.dx & 0xFF00) | value as u16, // DL
            3 => self.bx = (self.bx & 0xFF00) | value as u16, // BL
            4 => self.ax = (self.ax & 0x00FF) | ((value as u16) << 8), // AH
            5 => self.cx = (self.cx & 0x00FF) | ((value as u16) << 8), // CH
            6 => self.dx = (self.dx & 0x00FF) | ((value as u16) << 8), // DH
            7 => self.bx = (self.bx & 0x00FF) | ((value as u16) << 8), // BH
            _ => unreachable!(),
        }
    }

    // Set 16-bit register
    pub(super) fn set_reg16(&mut self, reg: u8, value: u16) {
        match reg & 0x07 {
            0 => self.ax = value,
            1 => self.cx = value,
            2 => self.dx = value,
            3 => self.bx = value,
            4 => self.sp = value,
            5 => self.bp = value,
            6 => self.si = value,
            7 => self.di = value,
            _ => unreachable!(),
        }
    }

    // Get 8-bit register value
    pub(super) fn get_reg8(&self, reg: u8) -> u8 {
        match reg {
            0 => (self.ax & 0xFF) as u8, // AL
            1 => (self.cx & 0xFF) as u8, // CL
            2 => (self.dx & 0xFF) as u8, // DL
            3 => (self.bx & 0xFF) as u8, // BL
            4 => (self.ax >> 8) as u8,   // AH
            5 => (self.cx >> 8) as u8,   // CH
            6 => (self.dx >> 8) as u8,   // DH
            7 => (self.bx >> 8) as u8,   // BH
            _ => unreachable!(),
        }
    }

    // Get 16-bit register value
    pub(super) fn get_reg16(&self, reg: u8) -> u16 {
        match reg & 0x07 {
            0 => self.ax,
            1 => self.cx,
            2 => self.dx,
            3 => self.bx,
            4 => self.sp,
            5 => self.bp,
            6 => self.si,
            7 => self.di,
            _ => unreachable!(),
        }
    }

    // Set segment register value
    pub(super) fn set_segreg(&mut self, reg: u8, value: u16) {
        match reg & 0x03 {
            0 => self.es = value,
            1 => self.cs = value,
            2 => self.ss = value,
            3 => self.ds = value,
            _ => unreachable!(),
        }
    }

    // Set a specific flag
    pub(super) fn set_flag(&mut self, flag: u16, value: bool) {
        if self.exec_logging_enabled && flag == cpu_flag::INTERRUPT {
            let old_if = (self.flags & cpu_flag::INTERRUPT) != 0;
            if old_if != value {
                log::debug!(
                    "IF: {} -> {} at {:04X}:{:04X}",
                    old_if as u8,
                    value as u8,
                    self.cs,
                    self.ip,
                );
            }
        }
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    // Get a specific flag
    pub(super) fn get_flag(&self, flag: u16) -> bool {
        (self.flags & flag) != 0
    }

    // Decode ModR/M byte and calculate effective address
    // Returns (mod, reg, r/m, effective_address, default_segment)
    // mod: 00=no disp (except r/m=110), 01=8-bit disp, 10=16-bit disp, 11=register
    // For mod=11, effective_address is unused
    pub(super) fn decode_modrm(&mut self, modrm: u8, bus: &Bus) -> (u8, u8, u8, usize, u16) {
        let mode = modrm >> 6;
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;

        if mode == 0b11 {
            // Register mode - no memory access
            return (mode, reg, rm, 0, self.ds);
        }

        // Calculate base address from r/m field
        let (base_addr, default_seg) = match rm {
            0b000 => (self.bx.wrapping_add(self.si), self.ds), // [BX + SI]
            0b001 => (self.bx.wrapping_add(self.di), self.ds), // [BX + DI]
            0b010 => (self.bp.wrapping_add(self.si), self.ss), // [BP + SI]
            0b011 => (self.bp.wrapping_add(self.di), self.ss), // [BP + DI]
            0b100 => (self.si, self.ds),                       // [SI]
            0b101 => (self.di, self.ds),                       // [DI]
            0b110 => {
                if mode == 0b00 {
                    // Special case: direct address (16-bit displacement, no base)
                    let disp = self.fetch_word(bus);
                    let seg = self.segment_override.unwrap_or(self.ds);
                    return (mode, reg, rm, Self::physical_address(seg, disp), seg);
                } else {
                    (self.bp, self.ss) // [BP]
                }
            }
            0b111 => (self.bx, self.ds), // [BX]
            _ => unreachable!(),
        };

        // Add displacement based on mode
        let effective_offset = match mode {
            0b00 => base_addr, // No displacement
            0b01 => {
                // 8-bit signed displacement
                let disp = self.fetch_byte(bus) as i8;
                base_addr.wrapping_add(disp as i16 as u16)
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word(bus);
                base_addr.wrapping_add(disp)
            }
            _ => unreachable!(),
        };

        // Use segment override if present, otherwise use default segment
        let effective_seg = self.segment_override.unwrap_or(default_seg);
        let effective_addr = Self::physical_address(effective_seg, effective_offset);
        (mode, reg, rm, effective_addr, effective_seg)
    }

// MIGRATED      // Read 8-bit value from register or memory based on mod field
// MIGRATED      pub(super) fn read_rm8(&self, mode: u8, rm: u8, addr: usize, bus: &Bus) -> u8 {
// MIGRATED          if mode == 0b11 {
// MIGRATED              // Register mode
// MIGRATED              self.get_reg8(rm)
// MIGRATED          } else {
// MIGRATED              // Memory mode
// MIGRATED              bus.read_u8(addr)
// MIGRATED          }
// MIGRATED      }

// MIGRATED      // Read 16-bit value from register or memory based on mod field
// MIGRATED      pub(super) fn read_rm16(&self, mode: u8, rm: u8, addr: usize, bus: &Bus) -> u16 {
// MIGRATED          if mode == 0b11 {
// MIGRATED              // Register mode
// MIGRATED              self.get_reg16(rm)
// MIGRATED          } else {
// MIGRATED              // Memory mode
// MIGRATED              bus.read_u16(addr)
// MIGRATED          }
// MIGRATED      }

// MIGRATED      // Write 8-bit value to register or memory based on mod field
// MIGRATED      pub(super) fn write_rm8(&mut self, mode: u8, rm: u8, addr: usize, value: u8, bus: &mut Bus) {
// MIGRATED          if mode == 0b11 {
// MIGRATED              // Register mode
// MIGRATED              self.set_reg8(rm, value);
// MIGRATED          } else {
// MIGRATED              // Memory mode
// MIGRATED              bus.write_u8(addr, value);
// MIGRATED          }
// MIGRATED      }
// MIGRATED  
// MIGRATED      // Write 16-bit value to register or memory based on mod field
// MIGRATED      pub(super) fn write_rm16(&mut self, mode: u8, rm: u8, addr: usize, value: u16, bus: &mut Bus) {
// MIGRATED          if mode == 0b11 {
// MIGRATED              // Register mode
// MIGRATED              self.set_reg16(rm, value);
// MIGRATED          } else {
// MIGRATED              // Memory mode
// MIGRATED              bus.write_u16(addr, value);
// MIGRATED          }
// MIGRATED      }

// MIGRATED      // Calculate and set flags for 8-bit result
// MIGRATED      pub(super) fn set_flags_8(&mut self, result: u8) {
// MIGRATED          self.set_flag(cpu_flag::ZERO, result == 0);
// MIGRATED          self.set_flag(cpu_flag::SIGN, (result & 0x80) != 0);
// MIGRATED          self.set_flag(cpu_flag::PARITY, result.count_ones().is_multiple_of(2));
// MIGRATED      }

// MIGRATED      // Calculate and set flags for 16-bit result
// MIGRATED      pub(super) fn set_flags_16(&mut self, result: u16) {
// MIGRATED          self.set_flag(cpu_flag::ZERO, result == 0);
// MIGRATED          self.set_flag(cpu_flag::SIGN, (result & 0x8000) != 0);
// MIGRATED          self.set_flag(
// MIGRATED              cpu_flag::PARITY,
// MIGRATED              (result as u8).count_ones().is_multiple_of(2),
// MIGRATED          );
// MIGRATED      }

    // Dump register state
    pub fn dump_registers(&self) {
        log::info!(
            "AX={:04X}  BX={:04X}  CX={:04X}  DX={:04X}",
            self.ax,
            self.bx,
            self.cx,
            self.dx
        );
        log::info!(
            "SI={:04X}  DI={:04X}  BP={:04X}  SP={:04X}",
            self.si,
            self.di,
            self.bp,
            self.sp
        );
        log::info!(
            "CS={:04X}  DS={:04X}  SS={:04X}  ES={:04X}",
            self.cs,
            self.ds,
            self.ss,
            self.es
        );
        log::info!("IP={:04X}  FLAGS={:04X}", self.ip, self.flags);
        log::info!(
            "CF={}  PF={}  AF={}  ZF={}  SF={}  TF={}  IF={}  DF={}  OF={}",
            if self.get_flag(cpu_flag::CARRY) { 1 } else { 0 },
            if self.get_flag(cpu_flag::PARITY) {
                1
            } else {
                0
            },
            if self.get_flag(cpu_flag::AUXILIARY) {
                1
            } else {
                0
            },
            if self.get_flag(cpu_flag::ZERO) { 1 } else { 0 },
            if self.get_flag(cpu_flag::SIGN) { 1 } else { 0 },
            if self.get_flag(cpu_flag::TRAP) { 1 } else { 0 },
            if self.get_flag(cpu_flag::INTERRUPT) {
                1
            } else {
                0
            },
            if self.get_flag(cpu_flag::DIRECTION) {
                1
            } else {
                0
            },
            if self.get_flag(cpu_flag::OVERFLOW) {
                1
            } else {
                0
            },
        );
    }
}
