use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag},
    physical_address,
};

pub(in crate::cpu) struct PendingDosRead {
    pub(in crate::cpu) ret_cs: u16,
    pub(in crate::cpu) ret_ip: u16,
    pub(in crate::cpu) ds: u16,
    pub(in crate::cpu) dx: u16,
}

impl Cpu {
    pub(in crate::cpu) fn handle_int21_dos_services(&mut self, bus: &mut Bus) {
        bus.increment_cycle_count(500);
        // Enable interrupts during DOS services (DOS runs with IF=1)
        self.set_flag(cpu_flag::INTERRUPT, true);
        let function = (self.ax >> 8) as u8; // Get AH directly
        match function {
            0x02 => self.int21_write_char(bus),
            0x09 => self.int21_write_string(bus),
            0x4c => self.int21_exit(bus),
            _ => log::warn!("Unhandled INT 0x21 function: AH=0x{function:02X}"),
        }
    }

    /// INT 21h, AH=02h - Write Character to STDOUT
    /// Input: DL = character to write
    fn int21_write_char(&mut self, bus: &mut Bus) {
        let ch = self.get_reg8(2); // DL register
        // Use teletype output for proper screen handling
        let saved_ax = self.ax;
        self.ax = (self.ax & 0xFF00) | (ch as u16);
        self.int10_teletype_output(bus);
        self.ax = saved_ax;
    }

    /// INT 21h, AH=09h - Write String to STDOUT
    /// Input: DS:DX = pointer to '$'-terminated string
    fn int21_write_string(&mut self, bus: &mut Bus) {
        let mut addr = physical_address(self.ds, self.dx);
        let saved_ax = self.ax;

        loop {
            let ch = bus.memory_read_u8(addr);
            if ch == b'$' {
                break;
            }
            // Use teletype output for each character
            self.ax = (self.ax & 0xFF00) | (ch as u16);
            self.int10_teletype_output(bus);
            addr += 1;
        }

        self.ax = saved_ax;
    }

    /// INT 21h, AH=4Ch - Exit Program
    /// Input: AL = return code
    fn int21_exit(&mut self, bus: &mut Bus) {
        let return_code = (self.ax & 0xff) as u8;

        // Read the terminate address (INT 22h) from the PSP at offset 0x0A
        let psp_segment = self.current_psp;
        let terminate_offset_addr = physical_address(psp_segment, 0x0A);
        let terminate_ip = bus.memory_read_u16(terminate_offset_addr);
        let terminate_cs = bus.memory_read_u16(terminate_offset_addr + 2);

        log::info!(
            "INT 0x21 AH=0x4C: Terminating from PSP {:04X}, jumping to {:04X}:{:04X}",
            psp_segment,
            terminate_cs,
            terminate_ip
        );

        // Restore parent's PSP
        let parent_psp_addr = physical_address(psp_segment, 0x16);
        let parent_psp = bus.memory_read_u16(parent_psp_addr);
        if parent_psp != 0 {
            self.current_psp = parent_psp;
        }

        // Jump to the terminate address
        if terminate_cs == 0 && terminate_ip == 0 {
            // No return address - halt the CPU (top-level program).
            // Clear IF so that pending IRQs (e.g. timer) cannot wake the CPU
            // and resume execution after the INT 21h instruction.
            self.halted = true;
            self.set_flag(cpu_flag::INTERRUPT, false);
            self.exit_code = Some(return_code);
        } else {
            // Return to parent program
            self.cs = terminate_cs;
            self.ip = terminate_ip;
        }
    }

    /// Log INT 0x21 call parameters before the call is dispatched to any handler.
    /// Called from dispatch_interrupt so it fires for both the built-in Rust DOS
    /// handler and a real DOS kernel running in guest memory.
    pub(in crate::cpu) fn log_int21_dos_call(&mut self, bus: &Bus) {
        let ah = (self.ax >> 8) as u8;
        let al = (self.ax & 0xff) as u8;
        let bx = self.bx;
        let cx = self.cx;
        let dx = self.dx;
        let ds = self.ds;

        match ah {
            0x3C => {
                let name = bus.read_c_string(physical_address(ds, dx));
                log::info!("[DOS] AH=3C create \"{name}\" attr={cx:04X}");
            }
            0x3D => {
                let name = bus.read_c_string(physical_address(ds, dx));
                log::info!("[DOS] AH=3D open  \"{name}\" mode={al:02X}");
            }
            0x3E => {
                log::info!("[DOS] AH=3E close handle={bx}");
            }
            0x3F => {
                log::info!("[DOS] AH=3F read  handle={bx} buf={ds:04X}:{dx:04X} max={cx}");
                self.pending_dos_read = Some(PendingDosRead {
                    ret_cs: self.cs,
                    ret_ip: self.ip,
                    ds,
                    dx,
                });
            }
            0x40 => {
                log::info!("[DOS] AH=40 write handle={bx} buf={ds:04X}:{dx:04X} len={cx}");
            }
            0x41 => {
                let name = bus.read_c_string(physical_address(ds, dx));
                log::info!("[DOS] AH=41 delete \"{name}\"");
            }
            0x42 => {
                let offset = ((cx as i32) << 16) | (dx as i32);
                log::info!("[DOS] AH=42 seek  handle={bx} origin={al} offset={offset}");
            }
            0x4E => {
                let name = bus.read_c_string(physical_address(ds, dx));
                log::info!("[DOS] AH=4E find_first \"{name}\" attr={cx:02X}");
            }
            0x4F => {
                log::info!("[DOS] AH=4F find_next");
            }
            _ => {}
        }
    }

    /// If a pending AH=3Fh read has just returned (cs:ip matches the saved return address),
    /// log the bytes-read count and a hex+ASCII dump of the first 16 bytes of the buffer.
    pub(in crate::cpu) fn check_pending_dos_read(&mut self, bus: &Bus) {
        let Some(ref pdr) = self.pending_dos_read else {
            return;
        };
        if self.cs != pdr.ret_cs || self.ip != pdr.ret_ip {
            return;
        }
        let bytes_read = self.ax as usize;
        let dump_len = bytes_read.min(16);
        if dump_len > 0 {
            let base = physical_address(pdr.ds, pdr.dx);
            let mut hex = String::new();
            let mut asc = String::new();
            for i in 0..dump_len {
                let b = bus.memory_read_u8(base + i);
                hex.push_str(&format!("{b:02X} "));
                asc.push(if b.is_ascii_graphic() || b == b' ' {
                    b as char
                } else {
                    '.'
                });
            }
            log::info!("[DOS] AH=3F → read={bytes_read}  {hex} {asc}");
        } else {
            log::info!("[DOS] AH=3F → read={bytes_read}");
        }
        self.pending_dos_read = None;
    }
}
