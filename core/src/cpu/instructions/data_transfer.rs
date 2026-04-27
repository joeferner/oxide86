use crate::{
    bus::Bus,
    cpu::{Cpu, CpuType, PendingException, cpu_flag, timing},
};

impl Cpu {
    /// MOV immediate to register (opcodes B0-BF)
    /// B0-B7: MOV reg8, imm8
    /// B8-BF: MOV reg16, imm16
    pub(in crate::cpu) fn mov_imm_to_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let reg = opcode & 0x07;
        let is_word = opcode & 0x08 != 0;

        if is_word {
            // 16-bit register
            let value = self.fetch_word(bus);
            self.set_reg16(reg, value);
        } else {
            // 8-bit register
            let value = self.fetch_byte(bus);
            self.set_reg8(reg, value);
        }

        // MOV immediate to register: 4 cycles
        bus.increment_cycle_count(timing::cycles::MOV_IMM_REG)
    }

    /// MOV accumulator to/from direct bus offset (opcodes A0-A3)
    /// A0: MOV AL, [moffs8] - Move byte at direct address to AL
    /// A1: MOV AX, [moffs16] - Move word at direct address to AX
    /// A2: MOV [moffs8], AL - Move AL to byte at direct address
    /// A3: MOV [moffs16], AX - Move AX to word at direct address
    pub(in crate::cpu) fn mov_acc_moffs(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let to_acc = opcode & 0x02 == 0; // 0 = to accumulator, 1 = from accumulator

        // Fetch the direct bus offset (16-bit address)
        let offset = self.fetch_word(bus);
        // Use segment override if present, otherwise use DS
        let segment = self.segment_override.unwrap_or(self.ds);
        let addr = self.seg_offset_to_phys(segment, offset, bus);

        if is_word {
            if to_acc {
                self.ax = bus.memory_read_u16(addr);
            } else {
                bus.memory_write_u16(addr, self.ax);
            }
        } else if to_acc {
            let value = bus.memory_read_u8(addr);
            self.ax = (self.ax & 0xFF00) | (value as u16);
        } else {
            let value = (self.ax & 0xFF) as u8;
            bus.memory_write_u8(addr, value);
        }

        bus.increment_cycle_count(if to_acc {
            timing::cycles::MOV_MEM_ACC
        } else {
            timing::cycles::MOV_ACC_MEM
        });
    }

    /// MOV r/m16 to segment register (opcode 8E)
    /// 8E: MOV segreg, r/m16
    /// Copies a 16-bit register or bus value to a segment register (ES, CS, SS, DS)
    /// Note: MOV to CS is not recommended as it affects instruction fetching
    pub(in crate::cpu) fn mov_rm_to_segreg(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, seg_reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field specifies which segment register (ES=0, CS=1, SS=2, DS=3)
        let value = self.read_rm16(mode, rm, addr, bus);

        if self.in_protected_mode() {
            self.load_segment_register(seg_reg, value, bus);
        } else {
            self.set_segreg(seg_reg, value);
            // Loading SS suppresses the single-step trap for the next instruction so
            // the paired SP load runs atomically (8086/286 behaviour).
            if seg_reg == 2 {
                self.suppress_trap = true;
            }
        }

        // Calculate cycle timing
        bus.increment_cycle_count(if mode == 0b11 {
            // MOV segreg, reg: 2 cycles
            timing::cycles::MOV_RM_SEGREG_REG
        } else {
            // MOV segreg, mem: 8 + EA cycles
            timing::cycles::MOV_RM_SEGREG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        });
    }

    /// POP 16-bit register (opcodes 58-5F)
    /// Pop from stack to register
    pub(in crate::cpu) fn pop_reg16(&mut self, opcode: u8, bus: &mut Bus) {
        let reg = opcode & 0x07;
        let value = self.pop(bus);
        self.set_reg16(reg, value);

        // POP register: 8 cycles
        bus.increment_cycle_count(timing::cycles::POP_REG)
    }

    /// PUSH 16-bit register (opcodes 50-57)
    /// Push register onto stack
    /// 8086 PUSH SP behavior: pushes SP-2 (value after decrement)
    /// 80286+ PUSH SP behavior: pushes original SP value
    pub(in crate::cpu) fn push_reg16(&mut self, opcode: u8, bus: &mut Bus) {
        let reg = opcode & 0x07;
        if reg == 4 && self.cpu_type == CpuType::I8086 {
            // PUSH SP on 8086: push the decremented value (post-decrement SP)
            self.sp = self.sp.wrapping_sub(2);
            let value = self.sp;
            let addr = self.seg_offset_to_phys(self.ss, self.sp, bus);
            bus.memory_write_u16(addr, value);
        } else {
            let value = self.get_reg16(reg);
            self.push(value, bus);
        }

        // PUSH register: 11 cycles
        bus.increment_cycle_count(timing::cycles::PUSH_REG)
    }

    /// PUSH r/m16 (opcode FF /6) - Group 5
    /// FF /6: PUSH r/m16
    /// Pushes a word from register or bus location onto stack
    pub(in crate::cpu) fn push_rm16(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg_field, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field should be 6 for PUSH (it's an opcode extension)
        if reg_field != 6 {
            panic!(
                "Invalid opcode extension for FF /6: expected /6, got /{}",
                reg_field
            );
        }

        let value = self.read_rm16(mode, rm, addr, bus);
        self.push(value, bus);

        // Calculate cycle timing
        bus.increment_cycle_count(if mode == 0b11 {
            // PUSH reg: 11 cycles
            timing::cycles::PUSH_REG
        } else {
            // PUSH mem: 16 + EA cycles
            timing::cycles::PUSH_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        });
    }

    /// MOV register to/from r/m (opcodes 88-8B)
    /// 88: MOV r/m8, r8
    /// 89: MOV r/m16, r16
    /// 8A: MOV r8, r/m8
    /// 8B: MOV r16, r/m16
    pub(in crate::cpu) fn mov_reg_rm(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let dir = opcode & 0x02 != 0; // 0 = reg is source, 1 = reg is dest

        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            // 16-bit move
            if dir {
                // MOV reg16, r/m16
                let value = self.read_rm16(mode, rm, addr, bus);
                self.set_reg16(reg, value);
            } else {
                // MOV r/m16, reg16
                let value = self.get_reg16(reg);
                self.write_rm16(mode, rm, addr, value, bus);
            }
        } else {
            // 8-bit move
            if dir {
                // MOV reg8, r/m8
                let value = self.read_rm8(mode, rm, addr, bus);
                self.set_reg8(reg, value);
            } else {
                // MOV r/m8, reg8
                let value = self.get_reg8(reg);
                self.write_rm8(mode, rm, addr, value, bus);
            }
        }

        // Calculate cycle timing based on operands
        bus.increment_cycle_count(if mode == 0b11 {
            // MOV reg, reg: 2 cycles
            timing::cycles::MOV_REG_REG
        } else if dir {
            // MOV reg, mem: 8 + EA cycles
            timing::cycles::MOV_MEM_REG
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        } else {
            // MOV mem, reg: 9 + EA cycles
            timing::cycles::MOV_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        });
    }

    /// MOV immediate to r/m (opcodes C6-C7)
    /// C6: MOV r/m8, imm8
    /// C7: MOV r/m16, imm16
    pub(in crate::cpu) fn mov_imm_to_rm(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(bus);
        let (mode, _reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field should be 0 for MOV immediate
        // (it's part of the opcode extension)

        if is_word {
            // MOV r/m16, imm16
            let value = self.fetch_word(bus);
            self.write_rm16(mode, rm, addr, value, bus);
        } else {
            // MOV r/m8, imm8
            let value = self.fetch_byte(bus);
            self.write_rm8(mode, rm, addr, value, bus);
        }

        // Calculate cycle timing
        bus.increment_cycle_count(if mode == 0b11 {
            // MOV reg, imm: 4 cycles
            timing::cycles::MOV_IMM_REG
        } else {
            // MOV mem, imm: 10 + EA cycles
            timing::cycles::MOV_IMM_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        });
    }

    /// PUSH segment register (opcodes 06, 0E, 16, 1E)
    /// 06: PUSH ES
    /// 0E: PUSH CS
    /// 16: PUSH SS
    /// 1E: PUSH DS
    pub(in crate::cpu) fn push_segreg(&mut self, opcode: u8, bus: &mut Bus) {
        let seg = match opcode {
            0x06 => 0, // ES
            0x0E => 1, // CS
            0x16 => 2, // SS
            0x1E => 3, // DS
            _ => unreachable!(),
        };
        let value = self.get_segreg(seg);
        self.push(value, bus);

        // PUSH segment register: 10 cycles
        bus.increment_cycle_count(timing::cycles::PUSH_SEGREG)
    }

    /// LDS - Load Pointer using DS (opcode 0xC5)
    /// Loads far pointer from bus into register and DS
    pub(in crate::cpu) fn lds(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, _rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // LDS only works with bus operands
        if mode == 0b11 {
            panic!("LDS cannot use register operand");
        }

        // Read offset and segment from bus (4 bytes total)
        let offset = bus.memory_read_u16(addr);
        let segment = bus.memory_read_u16(addr + 2);

        self.set_reg16(reg, offset);
        if self.in_protected_mode() {
            self.load_segment_register(3, segment, bus); // 3 = DS
        } else {
            self.set_ds_real(segment);
        }

        // LDS: 16 + EA cycles
        let rm = modrm & 0x07;
        bus.increment_cycle_count(
            timing::cycles::LDS
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some()),
        );
    }

    /// MOV segment register to r/m16 (opcode 8C)
    /// 8C: MOV r/m16, segreg
    /// Copies a segment register (ES, CS, SS, DS) to a 16-bit register or bus location
    pub(in crate::cpu) fn mov_segreg_to_rm(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, seg_reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field specifies which segment register (ES=0, CS=1, SS=2, DS=3)
        let value = self.get_segreg(seg_reg);
        self.write_rm16(mode, rm, addr, value, bus);

        // Calculate cycle timing
        bus.increment_cycle_count(if mode == 0b11 {
            // MOV reg, segreg: 2 cycles
            timing::cycles::MOV_SEGREG_RM_REG
        } else {
            // MOV mem, segreg: 9 + EA cycles
            timing::cycles::MOV_SEGREG_RM_MEM
                + timing::calculate_ea_cycles(mode, seg_reg, self.segment_override.is_some())
        });
    }

    /// POP segment register (opcodes 07, 0F, 17, 1F)
    /// 07: POP ES
    /// 0F: POP CS (note: POP CS is unusual, typically not used)
    /// 17: POP SS
    /// 1F: POP DS
    pub(in crate::cpu) fn pop_segreg(&mut self, opcode: u8, bus: &mut Bus) {
        let seg = match opcode {
            0x07 => 0, // ES
            0x0F => 1, // CS
            0x17 => 2, // SS
            0x1F => 3, // DS
            _ => unreachable!(),
        };
        let value = self.pop(bus);

        if self.in_protected_mode() {
            self.load_segment_register(seg, value, bus);
        } else {
            self.set_segreg(seg, value);
            // POP SS suppresses the single-step trap for the next instruction so
            // the paired SP load runs atomically (8086/286 behaviour).
            if seg == 2 {
                self.suppress_trap = true;
            }
        }

        // POP segment register: 8 cycles
        bus.increment_cycle_count(timing::cycles::POP_SEGREG)
    }

    /// Handle opcode 0x0F:
    /// - 8086: POP CS (dangerous but valid)
    /// - 286+: two-byte instruction prefix
    pub(in crate::cpu) fn exec_opcode_0f(&mut self, bus: &mut Bus) {
        if self.cpu_type.is_286_or_later() {
            let second = self.fetch_byte(bus);
            match second {
                // SLDT/STR/LLDT/LTR — reg field in ModRM selects operation
                0x00 => self.exec_0f_00(bus),

                // SGDT/SIDT/LGDT/LIDT/SMSW/LMSW — reg field in ModRM selects operation
                0x01 => self.exec_0f_01(bus),

                // LOADALL — load all CPU state from physical 0x800
                0x05 => self.exec_0f_05(bus),

                // CLTS — clear task-switched flag (TS, bit 3) in CR0
                0x06 => {
                    log::debug!(
                        "CLTS (0F 06) at {:04X}:{:04X} — clearing TS bit",
                        self.cs,
                        self.ip.wrapping_sub(2)
                    );
                    self.cr0 &= !0x0008;
                }
                _ => {
                    log::warn!(
                        "Unimplemented 286 two-byte opcode 0F {:02X} at {:04X}:{:04X} — firing INT 6",
                        second,
                        self.cs,
                        self.ip.wrapping_sub(2)
                    );
                    self.dispatch_interrupt(bus, 6);
                }
            }
        } else {
            log::warn!(
                "POP CS at {:04X}:{:04X} (8086 instruction, dangerous!)",
                self.cs,
                self.ip.wrapping_sub(1)
            );
            self.pop_segreg(0x0F, bus);
        }
    }

    /// 0F 05 — LOADALL (286, undocumented)
    ///
    /// Reads 102 bytes from physical address 0x800 and atomically loads all CPU
    /// state. Table layout (little-endian words unless noted):
    ///
    ///   +0x00  reserved (6 bytes)
    ///   +0x06  MSW (CR0 low 16 bits)
    ///   +0x08  reserved (14 bytes)
    ///   +0x16  TR selector
    ///   +0x18  FLAGS
    ///   +0x1A  IP
    ///   +0x1C  LDTR selector
    ///   +0x1E  DS  +0x20  SS  +0x22  CS  +0x24  ES
    ///   +0x26  DI  +0x28  SI  +0x2A  BP  +0x2C  SP
    ///   +0x2E  BX  +0x30  DX  +0x32  CX  +0x34  AX
    ///
    /// Descriptor cache entries (6 bytes each):
    ///   bytes [0-1]: base low word, byte [2]: base high, byte [3]: access, bytes [4-5]: limit
    ///   +0x36  ES  +0x3C  CS  +0x42  SS  +0x48  DS
    ///
    /// System descriptor pseudo-descriptors (6 bytes each):
    ///   bytes [0-1]: base low word, byte [2]: base high, byte [3]: unused/access, bytes [4-5]: limit
    ///   +0x4E  GDT  +0x54  LDT  +0x5A  IDT  +0x60  TR
    ///
    /// Used by HIMEM.SYS to access XMS extended memory from real mode by setting
    /// arbitrary hidden descriptor cache bases while leaving PE=0.
    fn exec_0f_05(&mut self, bus: &mut Bus) {
        use crate::cpu::protected_mode::SegmentCache;

        const BASE: usize = 0x800;

        let r16 = |bus: &Bus, off: usize| bus.memory_read_u16(BASE + off);
        let r8 = |bus: &Bus, off: usize| bus.memory_read_u8(BASE + off);
        // Each descriptor cache entry is 6 bytes:
        //   [0-1] base_lo word, [2] base_hi byte, [3] access byte, [4-5] limit word
        let cache = |bus: &Bus, off: usize| SegmentCache {
            base: (bus.memory_read_u8(BASE + off) as u32)
                | ((bus.memory_read_u8(BASE + off + 1) as u32) << 8)
                | ((bus.memory_read_u8(BASE + off + 2) as u32) << 16),
            limit: bus.memory_read_u16(BASE + off + 4),
        };

        // MSW is at +0x06 (bytes 0x00-0x05 are reserved).
        // Preserve the current PE bit: LOADALL can set MSW fields but hardware
        // prevents clearing PE once set (same behaviour as 86Box).
        let new_msw = (self.cr0 & 1) | r16(bus, 0x06);
        let new_tr = r16(bus, 0x16);
        // FLAGS: clear reserved bits (mask 0xffd5) and force the always-1 bit 1.
        let new_flags = (r16(bus, 0x18) & 0xffd5) | 0x0002;
        let new_ip = r16(bus, 0x1A);
        let new_ldtr = r16(bus, 0x1C);
        let new_ds = r16(bus, 0x1E);
        let new_ss = r16(bus, 0x20);
        let new_cs = r16(bus, 0x22);
        let new_es = r16(bus, 0x24);
        let new_di = r16(bus, 0x26);
        let new_si = r16(bus, 0x28);
        let new_bp = r16(bus, 0x2A);
        let new_sp = r16(bus, 0x2C);
        let new_bx = r16(bus, 0x2E);
        let new_dx = r16(bus, 0x30);
        let new_cx = r16(bus, 0x32);
        let new_ax = r16(bus, 0x34);

        let new_es_cache = cache(bus, 0x36);
        let new_cs_cache = cache(bus, 0x3C);
        let new_ss_cache = cache(bus, 0x42);
        let new_ds_cache = cache(bus, 0x48);

        // System descriptors: [base_lo(2), base_hi(1), unused/access(1), limit(2)]
        // GDT at +0x4E, LDT at +0x54, IDT at +0x5A, TR cache at +0x60
        let gdtr_base = r16(bus, 0x4E) as u32 | ((r8(bus, 0x50) as u32) << 16);
        let gdtr_limit = r16(bus, 0x52);
        let idtr_base = r16(bus, 0x5A) as u32 | ((r8(bus, 0x5C) as u32) << 16);
        let idtr_limit = r16(bus, 0x5E);

        // Apply all state atomically
        self.cr0 = new_msw;
        self.ip = new_ip;
        self.flags = new_flags;

        self.ax = new_ax;
        self.bx = new_bx;
        self.cx = new_cx;
        self.dx = new_dx;
        self.si = new_si;
        self.di = new_di;
        self.bp = new_bp;
        self.sp = new_sp;

        self.cs = new_cs;
        self.cs_cache = new_cs_cache;
        self.ds = new_ds;
        self.ds_cache = new_ds_cache;
        self.ss = new_ss;
        self.ss_cache = new_ss_cache;
        self.es = new_es;
        self.es_cache = new_es_cache;

        self.ldtr = new_ldtr;
        self.tr = new_tr;

        self.gdtr_base = gdtr_base;
        self.gdtr_limit = gdtr_limit;
        self.idtr_base = idtr_base;
        self.idtr_limit = idtr_limit;

        bus.increment_cycle_count(timing::cycles::LOADALL);

        if self.exec_logging_enabled && log::log_enabled!(log::Level::Debug) {
            let mut row = String::new();
            for i in (0..0x60).step_by(2) {
                if i % 16 == 0 && !row.is_empty() {
                    log::debug!("LOADALL table {:02X}: {}", i - 16, row);
                    row.clear();
                }
                row.push_str(&format!("{:04X} ", r16(bus, i)));
            }
            if !row.is_empty() {
                log::debug!("LOADALL table {:02X}: {}", 0x50, row);
            }
            log::debug!(
                "LOADALL: new CS:IP={:04X}:{:04X} MSW={:04X} SS={:04X} SP={:04X} AX={:04X} CX={:04X} FLAGS={:04X}",
                new_cs,
                new_ip,
                new_msw,
                new_ss,
                new_sp,
                new_ax,
                new_cx,
                new_flags,
            );
        }
    }

    /// 0F 01 — SGDT/SIDT/LGDT/LIDT/SMSW/LMSW (286+)
    /// 0F 00 — SLDT/STR/LLDT/LTR (286+)
    fn exec_0f_00(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);
        match reg {
            // SLDT r/m16 — store Local Descriptor Table register
            0 => {
                if mode == 0b11 {
                    self.set_reg16(rm, self.ldtr);
                } else {
                    bus.memory_write_u16(addr, self.ldtr);
                }
            }
            // STR r/m16 — store Task Register
            1 => {
                if mode == 0b11 {
                    self.set_reg16(rm, self.tr);
                } else {
                    bus.memory_write_u16(addr, self.tr);
                }
            }
            // LLDT r/m16 — load Local Descriptor Table register from selector
            2 => {
                let selector = if mode == 0b11 {
                    self.get_reg16(rm)
                } else {
                    bus.memory_read_u16(addr)
                };
                if selector & 0xFFF8 == 0 {
                    // Null selector: clear LDT
                    self.ldtr = 0;
                    self.ldtr_base = 0;
                    self.ldtr_limit = 0;
                } else {
                    // Look up the LDT descriptor in the GDT
                    let desc = crate::cpu::protected_mode::load_descriptor_from_table(
                        bus,
                        self.gdtr_base,
                        self.gdtr_limit,
                        selector,
                    );
                    match desc {
                        Some(d) if d.is_present() => {
                            self.ldtr = selector;
                            self.ldtr_base = d.base;
                            self.ldtr_limit = d.limit;
                            log::debug!(
                                "LLDT: selector=0x{:04X}, base=0x{:06X}, limit=0x{:04X}",
                                selector,
                                d.base,
                                d.limit
                            );
                        }
                        Some(_) => {
                            log::warn!(
                                "LLDT: descriptor not present for selector 0x{:04X}",
                                selector
                            );
                            self.pending_exception = Some(PendingException {
                                int_num: 11, // #NP
                                error_code: Some(selector),
                            });
                        }
                        None => {
                            log::warn!("LLDT: selector 0x{:04X} out of GDT bounds", selector);
                            self.pending_exception = Some(PendingException {
                                int_num: 13, // #GP
                                error_code: Some(selector),
                            });
                        }
                    }
                }
            }
            // LTR r/m16 — load Task Register from selector
            3 => {
                let selector = if mode == 0b11 {
                    self.get_reg16(rm)
                } else {
                    bus.memory_read_u16(addr)
                };
                if selector & 0xFFF8 == 0 {
                    log::warn!("LTR: null selector — #GP(0)");
                    self.pending_exception = Some(PendingException {
                        int_num: 13,
                        error_code: Some(0),
                    });
                } else {
                    // Look up the TSS descriptor in the GDT
                    let desc = crate::cpu::protected_mode::load_descriptor_from_table(
                        bus,
                        self.gdtr_base,
                        self.gdtr_limit,
                        selector,
                    );
                    match desc {
                        Some(d) if d.is_present() => {
                            self.tr = selector;
                            self.tr_base = d.base;
                            self.tr_limit = d.limit;
                            log::debug!(
                                "LTR: selector=0x{:04X}, base=0x{:06X}, limit=0x{:04X}",
                                selector,
                                d.base,
                                d.limit
                            );
                        }
                        Some(_) => {
                            log::warn!(
                                "LTR: descriptor not present for selector 0x{:04X}",
                                selector
                            );
                            self.pending_exception = Some(PendingException {
                                int_num: 11,
                                error_code: Some(selector),
                            });
                        }
                        None => {
                            log::warn!("LTR: selector 0x{:04X} out of GDT bounds", selector);
                            self.pending_exception = Some(PendingException {
                                int_num: 13,
                                error_code: Some(selector),
                            });
                        }
                    }
                }
            }
            _ => {
                log::warn!(
                    "Unimplemented 0F 00 /{} at {:04X}:{:04X} — firing INT 6",
                    reg,
                    self.cs,
                    self.ip.wrapping_sub(3)
                );
                self.dispatch_interrupt(bus, 6);
            }
        }
    }

    fn exec_0f_01(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);
        match reg {
            // SGDT m — store GDTR (6 bytes: limit16, base24, reserved8)
            0 => {
                if mode == 0b11 {
                    log::warn!("SGDT with register operand is invalid");
                    self.dispatch_interrupt(bus, 6);
                    return;
                }
                bus.memory_write_u16(addr, self.gdtr_limit);
                bus.memory_write_u8(addr + 2, (self.gdtr_base & 0xFF) as u8);
                bus.memory_write_u8(addr + 3, ((self.gdtr_base >> 8) & 0xFF) as u8);
                bus.memory_write_u8(addr + 4, ((self.gdtr_base >> 16) & 0xFF) as u8);
                bus.memory_write_u8(addr + 5, 0xFF); // 286: upper byte is undefined
            }
            // SIDT m — store IDTR (6 bytes: limit16, base24, reserved8)
            1 => {
                if mode == 0b11 {
                    log::warn!("SIDT with register operand is invalid");
                    self.dispatch_interrupt(bus, 6);
                    return;
                }
                bus.memory_write_u16(addr, self.idtr_limit);
                bus.memory_write_u8(addr + 2, (self.idtr_base & 0xFF) as u8);
                bus.memory_write_u8(addr + 3, ((self.idtr_base >> 8) & 0xFF) as u8);
                bus.memory_write_u8(addr + 4, ((self.idtr_base >> 16) & 0xFF) as u8);
                bus.memory_write_u8(addr + 5, 0xFF); // 286: upper byte is undefined
            }
            // LGDT m — load GDTR from 6 bytes (limit16, base24)
            2 => {
                if mode == 0b11 {
                    log::warn!("LGDT with register operand is invalid");
                    self.dispatch_interrupt(bus, 6);
                    return;
                }
                self.gdtr_limit = bus.memory_read_u16(addr);
                self.gdtr_base = bus.memory_read_u8(addr + 2) as u32
                    | ((bus.memory_read_u8(addr + 3) as u32) << 8)
                    | ((bus.memory_read_u8(addr + 4) as u32) << 16);
                log::debug!(
                    "LGDT at {:04X}:{:04X} — base=0x{:06X}, limit=0x{:04X}",
                    self.cs,
                    self.ip.wrapping_sub(4),
                    self.gdtr_base,
                    self.gdtr_limit
                );
            }
            // LIDT m — load IDTR from 6 bytes (limit16, base24)
            3 => {
                if mode == 0b11 {
                    log::warn!("LIDT with register operand is invalid");
                    self.dispatch_interrupt(bus, 6);
                    return;
                }
                self.idtr_limit = bus.memory_read_u16(addr);
                self.idtr_base = bus.memory_read_u8(addr + 2) as u32
                    | ((bus.memory_read_u8(addr + 3) as u32) << 8)
                    | ((bus.memory_read_u8(addr + 4) as u32) << 16);
                log::debug!(
                    "LIDT at {:04X}:{:04X} — base=0x{:06X}, limit=0x{:04X}",
                    self.cs,
                    self.ip.wrapping_sub(4),
                    self.idtr_base,
                    self.idtr_limit
                );
            }
            // SMSW r/m16 — store Machine Status Word (CR0 low 16 bits) to r/m
            4 => {
                let msw = self.cr0;
                if self.exec_logging_enabled {
                    log::debug!(
                        "SMSW at {:04X}:{:04X} — returning MSW=0x{:04X}",
                        self.cs,
                        self.ip.wrapping_sub(3),
                        msw
                    );
                }
                if mode == 0b11 {
                    self.set_reg16(rm, msw);
                } else {
                    bus.memory_write_u16(addr, msw);
                }
            }
            // LMSW r/m16 — load Machine Status Word (low 4 bits of CR0).
            // PE (bit 0) can be set but not cleared; bits 1-3 (MP, EM, TS) can
            // be freely set or cleared.
            6 => {
                let val = if mode == 0b11 {
                    self.get_reg16(rm)
                } else {
                    bus.memory_read_u16(addr)
                };
                // Preserve PE if already set (cannot clear PE via LMSW on 286)
                let new_cr0 =
                    (self.cr0 & !0x000E) | (val & 0x000E) | (val & 0x0001) | (self.cr0 & 0x0001);
                log::debug!(
                    "LMSW at {:04X}:{:04X} — CR0: 0x{:04X} -> 0x{:04X}",
                    self.cs,
                    self.ip.wrapping_sub(3),
                    self.cr0,
                    new_cr0
                );
                self.cr0 = new_cr0;
            }
            _ => {
                log::warn!(
                    "Unimplemented 0F 01 /{} at {:04X}:{:04X} — firing INT 6",
                    reg,
                    self.cs,
                    self.ip.wrapping_sub(3)
                );
                self.dispatch_interrupt(bus, 6);
            }
        }
    }

    /// LES - Load Pointer using ES (opcode 0xC4)
    /// Loads far pointer from bus into register and ES
    pub(in crate::cpu) fn les(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, _rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // LES only works with bus operands
        if mode == 0b11 {
            panic!("LES cannot use register operand");
        }

        // Read offset and segment from bus (4 bytes total)
        let offset = bus.memory_read_u16(addr);
        let segment = bus.memory_read_u16(addr + 2);

        self.set_reg16(reg, offset);
        if self.in_protected_mode() {
            self.load_segment_register(0, segment, bus); // 0 = ES
        } else {
            self.set_es_real(segment);
        }

        // LES: 16 + EA cycles
        let rm = modrm & 0x07;
        bus.increment_cycle_count(
            timing::cycles::LES
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some()),
        );
    }

    /// XCHG register with accumulator (opcodes 90-97)
    /// 90: NOP (XCHG AX, AX) - special case
    /// 91-97: XCHG AX, reg16
    pub(in crate::cpu) fn xchg_ax_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let reg = opcode & 0x07;
        if reg == 0 {
            // NOP - XCHG AX, AX does nothing
            bus.increment_cycle_count(timing::cycles::NOP);
            return;
        }
        let temp = self.ax;
        self.ax = self.get_reg16(reg);
        self.set_reg16(reg, temp);

        // XCHG AX, reg: 3 cycles
        bus.increment_cycle_count(timing::cycles::XCHG_REG_ACC)
    }

    /// XCHG register/bus with register (opcodes 86-87)
    /// 86: XCHG r/m8, r8
    /// 87: XCHG r/m16, r16
    pub(in crate::cpu) fn xchg_rm_reg(&mut self, opcode: u8, bus: &mut Bus) {
        let is_word = opcode & 0x01 != 0;
        let modrm = self.fetch_byte(bus);
        let (mode, reg, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        if is_word {
            // 16-bit exchange
            let reg_val = self.get_reg16(reg);
            let rm_val = self.read_rm16(mode, rm, addr, bus);
            self.set_reg16(reg, rm_val);
            self.write_rm16(mode, rm, addr, reg_val, bus);
        } else {
            // 8-bit exchange
            let reg_val = self.get_reg8(reg);
            let rm_val = self.read_rm8(mode, rm, addr, bus);
            self.set_reg8(reg, rm_val);
            self.write_rm8(mode, rm, addr, reg_val, bus);
        }

        // Calculate cycle timing
        bus.increment_cycle_count(if mode == 0b11 {
            // XCHG reg, reg: 4 cycles
            timing::cycles::XCHG_REG_REG
        } else {
            // XCHG reg, mem: 17 + EA cycles
            timing::cycles::XCHG_REG_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        });
    }

    /// LEA - Load Effective Address (opcode 0x8D)
    /// Loads the offset of the source operand into destination register
    pub(in crate::cpu) fn lea(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let mode = modrm >> 6;
        let reg = (modrm >> 3) & 0x07;
        let rm = modrm & 0x07;

        // LEA only works with bus operands (mode != 11)
        if mode == 0b11 {
            panic!("LEA cannot use register operand");
        }

        // Calculate the effective address offset (not physical address)
        let offset = match rm {
            0b000 => self.bx.wrapping_add(self.si), // [BX + SI]
            0b001 => self.bx.wrapping_add(self.di), // [BX + DI]
            0b010 => self.bp.wrapping_add(self.si), // [BP + SI]
            0b011 => self.bp.wrapping_add(self.di), // [BP + DI]
            0b100 => self.si,                       // [SI]
            0b101 => self.di,                       // [DI]
            0b110 => {
                if mode == 0b00 {
                    // Special case: direct address
                    self.fetch_word(bus)
                } else {
                    self.bp // [BP]
                }
            }
            0b111 => self.bx, // [BX]
            _ => unreachable!(),
        };

        // Add displacement based on mode
        let effective_offset = match mode {
            0b00 => offset, // No displacement (except for direct addressing handled above)
            0b01 => {
                // 8-bit signed displacement
                let disp = self.fetch_byte(bus) as i8;
                offset.wrapping_add(disp as i16 as u16)
            }
            0b10 => {
                // 16-bit displacement
                let disp = self.fetch_word(bus);
                offset.wrapping_add(disp)
            }
            _ => unreachable!(),
        };

        self.set_reg16(reg, effective_offset);

        // LEA: 2 + EA cycles (EA calculation is done even though bus isn't accessed)
        bus.increment_cycle_count(
            timing::cycles::LEA
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some()),
        );
    }

    /// LAHF - Load AH from Flags (opcode 0x9F)
    /// Loads SF, ZF, AF, PF, CF into AH
    pub(in crate::cpu) fn lahf(&mut self, bus: &mut Bus) {
        let ah = (self.flags & 0xFF) as u8;
        self.ax = (self.ax & 0x00FF) | ((ah as u16) << 8);

        // LAHF: 4 cycles
        bus.increment_cycle_count(timing::cycles::LAHF)
    }

    /// SAHF - Store AH into Flags (opcode 0x9E)
    /// Stores AH into SF, ZF, AF, PF, CF
    pub(in crate::cpu) fn sahf(&mut self, bus: &mut Bus) {
        let ah = ((self.ax >> 8) & 0xFF) as u8;
        // Only update lower 8 bits of flags (SF, ZF, 0, AF, 0, PF, 1, CF)
        // Preserve upper 8 bits
        self.flags = (self.flags & 0xFF00) | (ah as u16);

        // SAHF: 4 cycles
        bus.increment_cycle_count(timing::cycles::SAHF)
    }

    /// XLAT - Table Look-up Translation (opcode 0xD7)
    /// Translates AL using lookup table at DS:BX
    /// AL = [DS:BX + AL]
    pub(in crate::cpu) fn xlat(&mut self, bus: &mut Bus) {
        let al = (self.ax & 0xFF) as u8;
        let offset = self.bx.wrapping_add(al as u16);
        // Use segment override if present, otherwise use DS
        let segment = self.segment_override.unwrap_or(self.ds);
        let addr = self.seg_offset_to_phys(segment, offset, bus);
        let value = bus.memory_read_u8(addr);
        self.ax = (self.ax & 0xFF00) | (value as u16);

        // XLAT: 11 cycles
        bus.increment_cycle_count(timing::cycles::XLAT)
    }

    /// PUSHF - Push Flags Register (opcode 9C)
    /// Pushes the FLAGS register onto the stack
    pub(in crate::cpu) fn pushf(&mut self, bus: &mut Bus) {
        self.push(self.flags, bus);

        // PUSHF: 10 cycles
        bus.increment_cycle_count(timing::cycles::PUSHF)
    }

    /// POPF - Pop Flags Register (opcode 9D)
    /// Pops a word from the stack into the FLAGS register
    /// 8086: bits 12-15 are physically pulled high and cannot be cleared
    /// 286 real mode: bits 12-15 (IOPL, NT) cannot be set by POPF (remain 0)
    /// 386+ real mode: IOPL (bits 12-13) and NT (bit 14) are freely settable
    pub(in crate::cpu) fn popf(&mut self, bus: &mut Bus) {
        let value = self.pop(bus);
        let old_if = self.get_flag(cpu_flag::INTERRUPT);
        self.flags = match self.cpu_type {
            CpuType::I8086 => (value & 0x0FFF) | 0xF002, // bits 12-15 always 1 on 8086
            CpuType::I80286 => (value & 0x0FFF) | 0x0002, // bits 12-15 always 0 in 286 real mode
            _ => (value & 0x7FFF) | 0x0002, // 386+: IOPL/NT settable, bit 15 reserved 0
        };
        let new_if = self.get_flag(cpu_flag::INTERRUPT);
        if self.exec_logging_enabled && old_if != new_if {
            log::info!(
                "POPF: IF {} -> {} (FLAGS={:04X})",
                old_if as u8,
                new_if as u8,
                value
            );
        }

        // POPF: 8 cycles
        bus.increment_cycle_count(timing::cycles::POPF)
    }

    /// PUSHA - Push All General Registers (opcode 0x60)
    /// Pushes AX, CX, DX, BX, original SP, BP, SI, DI onto the stack
    /// 80186+ instruction
    pub(in crate::cpu) fn pusha(&mut self, bus: &mut Bus) {
        let original_sp = self.sp;
        self.push(self.ax, bus);
        self.push(self.cx, bus);
        self.push(self.dx, bus);
        self.push(self.bx, bus);
        self.push(original_sp, bus);
        self.push(self.bp, bus);
        self.push(self.si, bus);
        self.push(self.di, bus);

        // PUSHA: 36 cycles (80186+)
        bus.increment_cycle_count(timing::cycles::PUSHA)
    }

    /// POPA - Pop All General Registers (opcode 0x61)
    /// Pops DI, SI, BP, (discard), BX, DX, CX, AX from stack
    /// 80186+ instruction
    pub(in crate::cpu) fn popa(&mut self, bus: &mut Bus) {
        self.di = self.pop(bus);
        self.si = self.pop(bus);
        self.bp = self.pop(bus);
        let _discard = self.pop(bus); // SP is discarded
        self.bx = self.pop(bus);
        self.dx = self.pop(bus);
        self.cx = self.pop(bus);
        self.ax = self.pop(bus);

        // POPA: 51 cycles (80186+)
        bus.increment_cycle_count(timing::cycles::POPA);
    }

    /// PUSH immediate (opcode 68: 16-bit, 6A: sign-extended 8-bit)
    pub(in crate::cpu) fn push_imm(&mut self, opcode: u8, bus: &mut Bus) {
        let value = if opcode == 0x68 {
            // PUSH imm16
            self.fetch_word(bus)
        } else {
            // PUSH imm8 (sign-extended to 16 bits)
            let imm8 = self.fetch_byte(bus);
            if imm8 & 0x80 != 0 {
                0xFF00 | (imm8 as u16)
            } else {
                imm8 as u16
            }
        };
        self.push(value, bus);

        // PUSH immediate: 10 cycles (80186+)
        bus.increment_cycle_count(timing::cycles::PUSH_IMM)
    }

    /// POP r/m16 (opcode 8F) - Group 1A
    /// 8F /0: POP r/m16
    /// Pops a word from stack to register or bus location
    pub(in crate::cpu) fn pop_rm16(&mut self, bus: &mut Bus) {
        let modrm = self.fetch_byte(bus);
        let (mode, reg_field, rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // The reg field should be 0 for POP (it's an opcode extension)
        if reg_field != 0 {
            panic!(
                "Invalid opcode extension for 8F: expected /0, got /{}",
                reg_field
            );
        }

        let value = self.pop(bus);
        self.write_rm16(mode, rm, addr, value, bus);

        // Calculate cycle timing
        bus.increment_cycle_count(if mode == 0b11 {
            // POP reg: 8 cycles
            timing::cycles::POP_REG
        } else {
            // POP mem: 17 + EA cycles
            timing::cycles::POP_MEM
                + timing::calculate_ea_cycles(mode, rm, self.segment_override.is_some())
        });
    }

    /// BOUND - Check Array Index Against Bounds (opcode 0x62)
    /// Checks if a signed register value is within bounds stored in bus
    /// If index < lower_bound or index > upper_bound, triggers INT 5
    /// 80186+ instruction
    pub(in crate::cpu) fn bound(&mut self, bus: &mut Bus) -> bool {
        let modrm = self.fetch_byte(bus);
        let (mode, reg, _rm, addr, _seg) = self.decode_modrm(modrm, bus);

        // BOUND only works with bus operands
        if mode == 0b11 {
            panic!("BOUND cannot use register operand");
        }

        // Get the index value from register (signed)
        let index = self.get_reg16(reg) as i16;

        // Read lower and upper bounds from bus (two consecutive signed words)
        let lower_bound = bus.memory_read_u16(addr) as i16;
        let upper_bound = bus.memory_read_u16(addr + 2) as i16;

        // Check if index is out of bounds
        if index < lower_bound || index > upper_bound {
            // Out of bounds - caller should trigger INT 5
            bus.increment_cycle_count(timing::cycles::BOUND_OUT); // 48-51 cycles
            return true;
        }

        // Within bounds - no exception
        bus.increment_cycle_count(timing::cycles::BOUND_IN); // 33-35 cycles
        false
    }
}

#[cfg(test)]
mod tests {
    // Assuming these exist in your project structure
    use crate::cpu::tests::create_test_cpu;

    #[test_log::test]
    fn test_mov_imm_to_reg_8bit() {
        // 1. Setup: Initialize CPU and Memory
        let (mut cpu, mut bus) = create_test_cpu();

        // 2. Test 8-bit Move: MOV AL, 0x42 (Opcode 0xB0)
        // AL is usually register index 0
        let opcode_al = 0xB0;
        let imm_val_8 = 0x42;

        // Place the immediate value in memory at the current IP
        bus.memory_write_u8(bus.physical_address(0, cpu.ip), imm_val_8);

        cpu.mov_imm_to_reg(opcode_al, &mut bus);

        assert_eq!(cpu.get_reg8(0), imm_val_8, "AL should contain 0x42");
        assert_eq!(bus.cycle_count(), 4, "Should take 4 cycles");
        assert_eq!(cpu.ip, 1, "IP should have advanced by 1 bytes");
    }

    #[test_log::test]
    fn test_mov_imm_to_reg_16bit() {
        // 1. Setup: Initialize CPU and Memory
        let (mut cpu, mut bus) = create_test_cpu();

        // 2. Test 16-bit Move: MOV AX, 0x1234 (Opcode 0xB8)
        // AX is usually register index 0 (with the is_word bit set)
        let opcode_ax = 0xB8;
        let imm_val_16 = 0x1234;

        // Place the word in memory (handling little-endian if applicable)
        bus.memory_write_u16(bus.physical_address(0, cpu.ip), imm_val_16);

        cpu.mov_imm_to_reg(opcode_ax, &mut bus);

        assert_eq!(cpu.get_reg16(0), imm_val_16, "AX should contain 0x1234");
        assert_eq!(cpu.ip, 2, "IP should have advanced by 2 bytes");
    }
}
