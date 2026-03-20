use std::collections::HashMap;

use crate::{
    bus::Bus,
    cpu::{Cpu, cpu_flag},
};

/// Tracks an open DOS file handle.
pub(in crate::cpu) struct DosFileHandle {
    pub(in crate::cpu) filename: String,
    pub(in crate::cpu) position: u32,
}

/// Saved state for an in-flight AH=3C/3D open/create call; resolved on return.
pub(in crate::cpu) struct PendingDosOpen {
    pub(in crate::cpu) filename: String,
    pub(in crate::cpu) ret_cs: u16,
    pub(in crate::cpu) ret_ip: u16,
}

/// Saved state for an in-flight AH=42 seek call; resolved on return.
pub(in crate::cpu) struct PendingDosSeek {
    pub(in crate::cpu) handle: u16,
    pub(in crate::cpu) ret_cs: u16,
    pub(in crate::cpu) ret_ip: u16,
}

pub(in crate::cpu) struct PendingDosRead {
    pub(in crate::cpu) ret_cs: u16,
    pub(in crate::cpu) ret_ip: u16,
    pub(in crate::cpu) ds: u16,
    pub(in crate::cpu) dx: u16,
    pub(in crate::cpu) handle: u16,
    pub(in crate::cpu) file_pos: u32,
}

/// Side-table: DOS file handle → open file info.
pub(in crate::cpu) type DosFileHandleTable = HashMap<u16, DosFileHandle>;

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
        let mut addr = bus.physical_address(self.ds, self.dx);
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
        let terminate_offset_addr = bus.physical_address(psp_segment, 0x0A);
        let terminate_ip = bus.memory_read_u16(terminate_offset_addr);
        let terminate_cs = bus.memory_read_u16(terminate_offset_addr + 2);

        log::info!(
            "INT 0x21 AH=0x4C: Terminating from PSP {:04X}, jumping to {:04X}:{:04X}",
            psp_segment,
            terminate_cs,
            terminate_ip
        );

        // Restore parent's PSP
        let parent_psp_addr = bus.physical_address(psp_segment, 0x16);
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

    /// Returns a short annotation string like ` ("foo.txt" pos=42)` for a handle,
    /// or an empty string if the handle is not in the table.
    fn file_handle_annotation(&self, handle: u16) -> String {
        match self.dos_file_handles.get(&handle) {
            Some(fh) => format!(" (\"{}\", pos={})", fh.filename, fh.position),
            None => String::new(),
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
                let name = bus.read_c_string(bus.physical_address(ds, dx));
                log::debug!("[DOS] AH=3C create \"{name}\" attr={cx:04X}");
                self.pending_dos_open = Some(PendingDosOpen {
                    filename: name,
                    ret_cs: self.cs,
                    ret_ip: self.ip,
                });
            }
            0x3D => {
                let name = bus.read_c_string(bus.physical_address(ds, dx));
                log::debug!("[DOS] AH=3D open \"{name}\" mode={al:02X}");
                self.pending_dos_open = Some(PendingDosOpen {
                    filename: name,
                    ret_cs: self.cs,
                    ret_ip: self.ip,
                });
            }
            0x3E => {
                let ann = self.file_handle_annotation(bx);
                log::debug!("[DOS] AH=3E close handle={bx}{ann}");
                self.dos_file_handles.remove(&bx);
            }
            0x3F => {
                let file_pos = self
                    .dos_file_handles
                    .get(&bx)
                    .map(|fh| fh.position)
                    .unwrap_or(0);
                self.pending_dos_read = Some(PendingDosRead {
                    ret_cs: self.cs,
                    ret_ip: self.ip,
                    ds,
                    dx,
                    handle: bx,
                    file_pos,
                });
            }
            0x40 => {
                let ann = self.file_handle_annotation(bx);
                log::debug!("[DOS] AH=40 write handle={bx}{ann} buf={ds:04X}:{dx:04X} len={cx}");
            }
            0x41 => {
                let name = bus.read_c_string(bus.physical_address(ds, dx));
                log::debug!("[DOS] AH=41 delete \"{name}\"");
            }
            0x42 => {
                let offset = ((cx as i32) << 16) | (dx as i32);
                let ann = self.file_handle_annotation(bx);
                log::debug!("[DOS] AH=42 seek handle={bx}{ann} origin={al} offset={offset}");
                self.pending_dos_seek = Some(PendingDosSeek {
                    handle: bx,
                    ret_cs: self.cs,
                    ret_ip: self.ip,
                });
            }
            0x4E => {
                let name = bus.read_c_string(bus.physical_address(ds, dx));
                log::debug!("[DOS] AH=4E find_first \"{name}\" attr={cx:02X}");
            }
            0x4F => {
                log::debug!("[DOS] AH=4F find_next");
            }
            _ => {}
        }
    }

    pub(in crate::cpu) fn check_int21_dos_call(&mut self, bus: &Bus) {
        self.check_pending_dos_open();
        self.check_pending_dos_seek();
        self.check_pending_dos_read(bus);
    }

    /// If a pending AH=3C/3Dh open/create has just returned, register the new handle.
    fn check_pending_dos_open(&mut self) {
        let Some(ref pdo) = self.pending_dos_open else {
            return;
        };
        if self.cs != pdo.ret_cs || self.ip != pdo.ret_ip {
            return;
        }
        if !self.get_flag(cpu_flag::CARRY) {
            let handle = self.ax;
            let filename = pdo.filename.clone();
            log::debug!("[DOS] open/create → handle={handle} \"{filename}\"");
            self.dos_file_handles.insert(
                handle,
                DosFileHandle {
                    filename,
                    position: 0,
                },
            );
        }
        self.pending_dos_open = None;
    }

    /// If a pending AH=42h seek has just returned, update the stored file position.
    fn check_pending_dos_seek(&mut self) {
        let Some(ref pds) = self.pending_dos_seek else {
            return;
        };
        if self.cs != pds.ret_cs || self.ip != pds.ret_ip {
            return;
        }
        if !self.get_flag(cpu_flag::CARRY) {
            let new_pos = ((self.dx as u32) << 16) | (self.ax as u32);
            let handle = pds.handle;
            if let Some(fh) = self.dos_file_handles.get_mut(&handle) {
                fh.position = new_pos;
            }
        }
        self.pending_dos_seek = None;
    }

    /// If a pending AH=3Fh read has just returned (cs:ip matches the saved return address),
    /// log the bytes-read count and a hex+ASCII dump of the first 16 bytes of the buffer.
    fn check_pending_dos_read(&mut self, bus: &Bus) {
        let Some(ref pdr) = self.pending_dos_read else {
            return;
        };
        if self.cs != pdr.ret_cs || self.ip != pdr.ret_ip {
            return;
        }
        let bytes_read = self.ax as usize;
        let handle = pdr.handle;
        let ds = pdr.ds;
        let dx = pdr.dx;
        let file_pos = pdr.file_pos;
        let base = bus.physical_address(ds, dx);

        let filename = self
            .dos_file_handles
            .get(&handle)
            .map(|fh| fh.filename.clone())
            .unwrap_or_default();

        if bytes_read > 0 {
            let phys_end = base + bytes_read - 1;
            log::debug!(
                "[DOS] AH=3F read \"{filename}\" pos={file_pos} → phys 0x{base:05X}–0x{phys_end:05X} ({bytes_read} bytes)"
            );
            let dump_len = bytes_read.min(16);
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
            log::debug!("[DOS] AH=3F   {hex} {asc}");
        } else {
            log::debug!("[DOS] AH=3F read \"{filename}\" pos={file_pos} → 0 bytes");
        }

        if let Some(fh) = self.dos_file_handles.get_mut(&handle) {
            fh.position += bytes_read as u32;
        }
        self.pending_dos_read = None;
    }
}
