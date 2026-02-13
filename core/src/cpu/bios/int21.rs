use strum_macros::{Display, FromRepr};

use crate::{
    DriveNumber,
    cpu::{
        Cpu,
        bios::{ExecParams, FindData, SeekMethod, dos_error::DosError},
        cpu_flag,
    },
    memory::Memory,
};

/// File access modes for INT 21h, AH=3Dh
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Display, FromRepr)]
pub enum FileAccess {
    ReadOnly = 0x00,
    WriteOnly = 0x01,
    ReadWrite = 0x02,
}

impl Cpu {
    /// INT 0x21 - DOS Services
    /// AH register contains the function number
    pub(super) fn handle_int21(
        &mut self,
        memory: &mut Memory,
        io: &mut super::Bios,
        video: &mut crate::video::Video,
    ) {
        let function = (self.ax >> 8) as u8; // Get AH directly

        // Log all DOS calls at debug level for easier troubleshooting
        if function != 0x01 && function != 0x02 && function != 0x06 && function != 0x09 {
            log::debug!(
                "INT 21h AH=0x{:02X}, AX=0x{:04X}, BX=0x{:04X}, CX=0x{:04X}, DX=0x{:04X}",
                function,
                self.ax,
                self.bx,
                self.cx,
                self.dx
            );
        }

        match function {
            0x01 => self.int21_read_char_with_echo(io, video),
            0x02 => self.int21_write_char(video),
            0x06 => self.int21_direct_console_io(io, video),
            0x07 => self.int21_direct_console_input(io),
            0x08 => self.int21_console_input_no_echo(io),
            0x09 => self.int21_write_string(memory, video),
            0x0B => self.int21_check_input_status(io),
            0x0C => self.int21_flush_and_input(io, video),
            0x0E => self.int21_select_disk(io),
            0x19 => self.int21_get_current_drive(io),
            0x25 => self.int21_set_interrupt_vector(memory),
            0x2A => self.int21_get_date(io),
            0x2B => self.int21_set_date(),
            0x2C => self.int21_get_time(memory),
            0x2D => self.int21_set_time(memory),
            0x30 => self.int21_get_dos_version(),
            0x31 => self.int21_terminate_stay_resident(memory, io),
            0x32 => self.int21_get_dpb(memory, io),
            0x35 => self.int21_get_interrupt_vector(memory),
            0x36 => self.int21_get_disk_free_space(io),
            0x37 => self.int21_switch_char(),
            0x39 => self.int21_create_dir(memory, io),
            0x3A => self.int21_remove_dir(memory, io),
            0x3B => self.int21_change_dir(memory, io),
            0x3C => self.int21_create_file(memory, io),
            0x3D => self.int21_open_file(memory, io),
            0x3E => self.int21_close_file(io),
            0x3F => self.int21_read_file(memory, io),
            0x40 => self.int21_write_file(memory, io, video),
            0x41 => self.int21_delete_file(memory, io),
            0x42 => self.int21_seek_file(io),
            0x44 => self.int21_ioctl(memory, io),
            0x45 => self.int21_duplicate_file(io),
            0x47 => self.int21_get_current_dir(memory, io),
            0x48 => self.int21_allocate_memory(io),
            0x49 => self.int21_free_memory(io),
            0x4A => self.int21_resize_memory(io),
            0x4B => self.int21_exec(memory, io),
            0x4C => self.int21_exit(memory, io),
            0x4E => self.int21_find_first(memory, io),
            0x4F => self.int21_find_next(memory, io),
            0x50 => self.int21_set_psp(io),
            0x63 => self.int21_get_dbcs_lead_byte_table(memory),
            _ => {
                log::warn!("Unhandled INT 0x21 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 21h, AH=01h - Read Character from STDIN with Echo
    /// Returns: AL = character read
    fn int21_read_char_with_echo(&mut self, io: &mut super::Bios, video: &mut crate::video::Video) {
        if let Some(ch) = io.read_char() {
            // Echo the character via teletype output
            let saved_ax = self.ax;
            self.ax = (self.ax & 0xFF00) | (ch as u16);
            self.int10_teletype_output(video);
            // Store in AL (restore AH, keep AL as the character)
            self.ax = (saved_ax & 0xFF00) | (ch as u16);
        }
    }

    /// INT 21h, AH=02h - Write Character to STDOUT
    /// Input: DL = character to write
    fn int21_write_char(&mut self, video: &mut crate::video::Video) {
        let ch = self.get_reg8(2); // DL register
        // Use teletype output for proper screen handling
        let saved_ax = self.ax;
        self.ax = (self.ax & 0xFF00) | (ch as u16);
        self.int10_teletype_output(video);
        self.ax = saved_ax;
    }

    /// INT 21h, AH=06h - Direct Console I/O
    /// Input: DL = character to output (if DL != 0xFF), or 0xFF to request input
    /// Output: If DL = 0xFF on entry:
    ///   ZF clear: AL = character read from input
    ///   ZF set: No character available (AL = 0)
    fn int21_direct_console_io(&mut self, io: &mut super::Bios, video: &mut crate::video::Video) {
        let dl = (self.dx & 0xFF) as u8;

        if dl == 0xFF {
            // Input mode - check for available character
            if let Some(ch) = io.check_char() {
                self.ax = (self.ax & 0xFF00) | (ch as u16);
                self.set_flag(cpu_flag::ZERO, false); // Character available
            } else {
                self.ax &= 0xFF00;
                self.set_flag(cpu_flag::ZERO, true); // No character available
            }
        } else {
            // Output mode - write character
            let saved_ax = self.ax;
            self.ax = (self.ax & 0xFF00) | (dl as u16);
            self.int10_teletype_output(video);
            self.ax = saved_ax;
        }
    }

    /// INT 21h, AH=07h - Direct Console Input Without Echo
    /// Waits for a character from stdin without echoing it
    /// Output: AL = character read
    fn int21_direct_console_input(&mut self, io: &mut super::Bios) {
        if let Some(ch) = io.read_char() {
            self.ax = (self.ax & 0xFF00) | (ch as u16);
        }
        // If no character available, just return with whatever is in AL
    }

    /// INT 21h, AH=08h - Console Input Without Echo
    /// Same as 07h but checks for Ctrl-Break
    /// Output: AL = character read
    fn int21_console_input_no_echo(&mut self, io: &mut super::Bios) {
        if let Some(ch) = io.read_char() {
            self.ax = (self.ax & 0xFF00) | (ch as u16);
        }
        // If no character available, just return with whatever is in AL
    }

    /// INT 21h, AH=09h - Write String to STDOUT
    /// Input: DS:DX = pointer to '$'-terminated string
    fn int21_write_string(&mut self, memory: &mut Memory, video: &mut crate::video::Video) {
        let mut addr = Self::physical_address(self.ds, self.dx);
        let saved_ax = self.ax;

        loop {
            let ch = memory.read_u8(addr);
            if ch == b'$' {
                break;
            }
            // Use teletype output for each character
            self.ax = (self.ax & 0xFF00) | (ch as u16);
            self.int10_teletype_output(video);
            addr += 1;
        }

        self.ax = saved_ax;
    }

    /// INT 21h, AH=0Bh - Check Standard Input Status
    /// Output:
    ///   AL = 0xFF if character available
    ///   AL = 0x00 if no character available
    fn int21_check_input_status(&mut self, io: &super::Bios) {
        if io.has_char_available() {
            self.ax = (self.ax & 0xFF00) | 0xFF;
        } else {
            self.ax &= 0xFF00;
        }
    }

    /// INT 21h, AH=0Ch - Clear Keyboard Buffer and Invoke Keyboard Function
    /// Input:
    ///   AL = keyboard function to invoke (01h, 06h, 07h, 08h, or 0Ah)
    /// Output: As per the specified function
    fn int21_flush_and_input(&mut self, io: &mut super::Bios, video: &mut crate::video::Video) {
        // Clear the keyboard buffer (consume any pending input)
        while io.check_char().is_some() {}

        // Now invoke the specified function
        let subfunc = (self.ax & 0xFF) as u8;
        match subfunc {
            0x01 => self.int21_read_char_with_echo(io, video),
            0x06 => self.int21_direct_console_io(io, video),
            0x07 => self.int21_direct_console_input(io),
            0x08 => self.int21_console_input_no_echo(io),
            _ => {
                // Function 0x0A (buffered input) not implemented
                log::warn!(
                    "INT 21h AH=0Ch: Unsupported subfunction AL=0x{:02X}",
                    subfunc
                );
            }
        }
    }

    /// INT 21h, AH=19h - Get Current Default Drive
    /// Output: AL = current drive (0=A, 1=B, etc.)
    fn int21_get_current_drive(&mut self, io: &super::Bios) {
        let drive = io.get_current_drive();
        self.ax = (self.ax & 0xFF00) | (drive.to_standard() as u16);
    }

    /// INT 21h, AH=25h - Set Interrupt Vector
    /// Input:
    ///   AL = interrupt number
    ///   DS:DX = new interrupt handler address
    /// Output: None
    fn int21_set_interrupt_vector(&mut self, memory: &mut Memory) {
        let int_num = (self.ax & 0xFF) as u8; // AL
        let segment = self.ds;
        let offset = self.dx;

        // Read old vector for logging
        let ivt_addr = (int_num as usize) * 4;
        let old_offset =
            memory.read_u8(ivt_addr) as u16 | ((memory.read_u8(ivt_addr + 1) as u16) << 8);
        let old_segment =
            memory.read_u8(ivt_addr + 2) as u16 | ((memory.read_u8(ivt_addr + 3) as u16) << 8);

        log::debug!(
            "INT 21h AH=25h: Set INT 0x{:02X} vector from {:04X}:{:04X} to {:04X}:{:04X}",
            int_num,
            old_segment,
            old_offset,
            segment,
            offset
        );

        // Interrupt vector table is at 0000:0000
        // Each entry is 4 bytes: offset (2 bytes) + segment (2 bytes)

        // Write offset (low word)
        memory.write_u8(ivt_addr, (offset & 0xFF) as u8);
        memory.write_u8(ivt_addr + 1, (offset >> 8) as u8);

        // Write segment (high word)
        memory.write_u8(ivt_addr + 2, (segment & 0xFF) as u8);
        memory.write_u8(ivt_addr + 3, (segment >> 8) as u8);
    }

    /// INT 21h, AH=30h - Get DOS Version
    /// Output:
    ///   AL = major version number
    ///   AH = minor version number
    ///   BL:CX = 24-bit user serial number (usually 0)
    fn int21_get_dos_version(&mut self) {
        log::warn!("get dos version should be handled by DOS");

        // Return DOS 3.30 (a common and well-supported version)
        let major = 3;
        let minor = 30;

        self.ax = ((minor as u16) << 8) | (major as u16); // AH=minor, AL=major
        self.bx &= 0xFF00; // BL = 0 (OEM number)
        self.cx = 0; // Serial number = 0
    }

    /// INT 21h, AH=35h - Get Interrupt Vector
    /// Input:
    ///   AL = interrupt number
    /// Output:
    ///   ES:BX = current interrupt handler address
    fn int21_get_interrupt_vector(&mut self, memory: &Memory) {
        let int_num = (self.ax & 0xFF) as u8; // AL

        // Interrupt vector table is at 0000:0000
        // Each entry is 4 bytes: offset (2 bytes) + segment (2 bytes)
        let ivt_addr = (int_num as usize) * 4;

        // Read offset (low word)
        let offset_low = memory.read_u8(ivt_addr) as u16;
        let offset_high = memory.read_u8(ivt_addr + 1) as u16;
        let offset = (offset_high << 8) | offset_low;

        // Read segment (high word)
        let segment_low = memory.read_u8(ivt_addr + 2) as u16;
        let segment_high = memory.read_u8(ivt_addr + 3) as u16;
        let segment = (segment_high << 8) | segment_low;

        // Return in ES:BX
        self.es = segment;
        self.bx = offset;
    }

    /// INT 21h, AH=4Ch - Exit Program
    /// Input: AL = return code
    fn int21_exit(&mut self, memory: &Memory, io: &mut super::Bios) {
        // INT 21h AH=4Ch - Terminate Program
        // Read the terminate address (INT 22h) from the PSP at offset 0x0A
        let psp_segment = io.get_psp();
        let terminate_offset_addr = Self::physical_address(psp_segment, 0x0A);
        let terminate_ip = memory.read_u16(terminate_offset_addr);
        let terminate_cs = memory.read_u16(terminate_offset_addr + 2);

        log::info!(
            "INT 21h AH=4Ch: Terminating from PSP {:04X}, jumping to {:04X}:{:04X}",
            psp_segment,
            terminate_cs,
            terminate_ip
        );

        // Restore parent's PSP
        let parent_psp_addr = Self::physical_address(psp_segment, 0x16);
        let parent_psp = memory.read_u16(parent_psp_addr);
        if parent_psp != 0 {
            io.set_psp(parent_psp);
        }

        // Jump to the terminate address
        if terminate_cs == 0 && terminate_ip == 0 {
            // No return address - halt the CPU (top-level program)
            self.halted = true;
        } else {
            // Return to parent program
            self.cs = terminate_cs;
            self.ip = terminate_ip;
        }
    }

    /// INT 21h, AH=31h - Terminate and Stay Resident (TSR)
    /// Input:
    ///   AL = exit code
    ///   DX = number of paragraphs to keep resident (including PSP)
    /// Output: None (does not return to caller)
    ///
    /// This function terminates the current program but keeps it resident in memory.
    /// The specified number of paragraphs (DX) starting from the PSP are kept allocated.
    /// TSR programs use this to install themselves and return control to DOS.
    fn int21_terminate_stay_resident(&mut self, memory: &Memory, io: &mut super::Bios) {
        let exit_code = (self.ax & 0xFF) as u8;
        let paragraphs_to_keep = self.dx;

        log::info!(
            "INT 21h AH=31h: TSR with {} paragraphs, exit code {}",
            paragraphs_to_keep,
            exit_code
        );

        // In a real DOS system, this would:
        // 1. Mark the memory block as resident
        // 2. Resize the current program's memory to keep only DX paragraphs
        // 3. Restore parent's INT 22h/23h/24h vectors from PSP
        // 4. Return to parent program
        //
        // For our emulator, we treat TSR termination the same as normal termination.
        // The program has already installed its interrupt handlers and they remain
        // in memory. We jump to the terminate address to return control to the parent.

        // Read the terminate address (INT 22h) from the PSP at offset 0x0A
        let psp_segment = io.get_psp();
        let terminate_offset_addr = Self::physical_address(psp_segment, 0x0A);
        let terminate_ip = memory.read_u16(terminate_offset_addr);
        let terminate_cs = memory.read_u16(terminate_offset_addr + 2);

        log::info!(
            "INT 21h AH=31h: Terminating from PSP {:04X}, jumping to {:04X}:{:04X}",
            psp_segment,
            terminate_cs,
            terminate_ip
        );

        // Restore parent's PSP
        let parent_psp_addr = Self::physical_address(psp_segment, 0x16);
        let parent_psp = memory.read_u16(parent_psp_addr);
        if parent_psp != 0 {
            io.set_psp(parent_psp);
        }

        // Jump to the terminate address
        if terminate_cs == 0 && terminate_ip == 0 {
            // No return address - halt the CPU (top-level program)
            self.halted = true;
        } else {
            // Return to parent program
            self.cs = terminate_cs;
            self.ip = terminate_ip;
        }
    }

    /// INT 21h, AH=3Ch - Create or Truncate File
    /// Input:
    ///   DS:DX = pointer to null-terminated filename
    ///   CX = file attributes
    /// Output:
    ///   CF clear if success: AX = file handle
    ///   CF set if error: AX = error code
    fn int21_create_file(&mut self, memory: &Memory, io: &mut super::Bios) {
        let filename = self.read_null_terminated_string(memory, self.ds, self.dx);
        let attributes = (self.cx & 0xFF) as u8;

        match io.file_create(&filename, attributes) {
            Ok(handle) => {
                self.ax = handle;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Dh - Open Existing File
    /// Input:
    ///   DS:DX = pointer to null-terminated filename
    ///   AL = access mode (0=read, 1=write, 2=read/write)
    /// Output:
    ///   CF clear if success: AX = file handle
    ///   CF set if error: AX = error code
    fn int21_open_file(&mut self, memory: &Memory, io: &mut super::Bios) {
        let filename = self.read_null_terminated_string(memory, self.ds, self.dx);
        let access_mode =
            FileAccess::from_repr((self.ax & 0xFF) as u8).unwrap_or(FileAccess::ReadOnly);

        log::info!(
            "INT 21h AH=3Dh: Opening file '{}' with access mode {}",
            filename,
            access_mode
        );

        match io.file_open(&filename, access_mode) {
            Ok(handle) => {
                log::info!(
                    "INT 21h AH=3Dh: File opened successfully, handle = {}",
                    handle
                );
                self.ax = handle;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                log::warn!("INT 21h AH=3Dh: Failed to open file - error {}", error_code);
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Eh - Close File
    /// Input:
    ///   BX = file handle
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_close_file(&mut self, io: &mut super::Bios) {
        let handle = self.bx;

        match io.file_close(handle) {
            Ok(()) => {
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Fh - Read from File or Device
    /// Input:
    ///   BX = file handle
    ///   CX = number of bytes to read
    ///   DS:DX = pointer to buffer
    /// Output:
    ///   CF clear if success: AX = number of bytes read
    ///   CF set if error: AX = error code
    fn int21_read_file(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        let handle = self.bx;
        let max_bytes = self.cx;

        log::debug!(
            "INT 21h AH=3Fh: Reading from handle {} (max {} bytes)",
            handle,
            max_bytes
        );

        match io.file_read(handle, max_bytes) {
            Ok(data) => {
                log::debug!(
                    "INT 21h AH=3Fh: Read {} bytes from handle {}",
                    data.len(),
                    handle
                );
                // Write data to DS:DX
                let buffer_addr = Self::physical_address(self.ds, self.dx);
                for (i, &byte) in data.iter().enumerate() {
                    memory.write_u8(buffer_addr + i, byte);
                }
                self.ax = data.len() as u16;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                log::warn!(
                    "INT 21h AH=3Fh: Failed to read from handle {} - error {}",
                    handle,
                    error_code
                );
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=40h - Write to File or Device
    /// Input:
    ///   BX = file handle
    ///   CX = number of bytes to write
    ///   DS:DX = pointer to data
    /// Output:
    ///   CF clear if success: AX = number of bytes written
    ///   CF set if error: AX = error code
    fn int21_write_file(
        &mut self,
        memory: &mut Memory,
        io: &mut super::Bios,
        video: &mut crate::video::Video,
    ) {
        let handle = self.bx;
        let num_bytes = self.cx;

        // Read data from DS:DX
        let buffer_addr = Self::physical_address(self.ds, self.dx);

        // For stdout (1) and stderr (2), use video teletype output
        if handle == 1 || handle == 2 {
            let saved_ax = self.ax;
            for i in 0..num_bytes {
                let ch = memory.read_u8(buffer_addr + i as usize);
                self.ax = (self.ax & 0xFF00) | (ch as u16);
                self.int10_teletype_output(video);
            }
            self.ax = saved_ax;
            // Report all bytes written
            self.ax = num_bytes;
            self.set_flag(cpu_flag::CARRY, false);
            return;
        }

        // For other handles, use file I/O
        let mut data = Vec::with_capacity(num_bytes as usize);
        for i in 0..num_bytes {
            data.push(memory.read_u8(buffer_addr + i as usize));
        }

        match io.file_write(handle, &data) {
            Ok(bytes_written) => {
                self.ax = bytes_written;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=41h - Delete File (UNLINK)
    /// Input:
    ///   DS:DX = pointer to null-terminated filename
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_delete_file(&mut self, memory: &Memory, io: &mut super::Bios) {
        let filename = self.read_null_terminated_string(memory, self.ds, self.dx);

        log::debug!("INT 21h AH=41h: Deleting file '{}'", filename);

        match io.file_delete(&filename) {
            Ok(()) => {
                log::debug!("INT 21h AH=41h: Successfully deleted file '{}'", filename);
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                log::warn!(
                    "INT 21h AH=41h: Failed to delete file '{}' - error {}",
                    filename,
                    error_code
                );
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=42h - Seek (LSEEK)
    /// Input:
    ///   BX = file handle
    ///   AL = seek method (0=from start, 1=from current, 2=from end)
    ///   CX:DX = signed offset (32-bit)
    /// Output:
    ///   CF clear if success: DX:AX = new file position
    ///   CF set if error: AX = error code
    fn int21_seek_file(&mut self, io: &mut super::Bios) {
        let handle = self.bx;
        let method_code = (self.ax & 0xFF) as u8;

        // Combine CX:DX into a 32-bit signed offset
        let offset = ((self.cx as u32) << 16) | (self.dx as u32);
        let offset_signed = offset as i32;

        let method = match method_code {
            0 => SeekMethod::FromStart,
            1 => SeekMethod::FromCurrent,
            2 => SeekMethod::FromEnd,
            _ => {
                self.ax = DosError::InvalidFunction as u16;
                self.set_flag(cpu_flag::CARRY, true);
                return;
            }
        };

        log::debug!(
            "INT 21h AH=42h: Seek handle {} to offset {} from {:?}",
            handle,
            offset_signed,
            method
        );

        match io.file_seek(handle, offset_signed, method) {
            Ok(new_position) => {
                log::debug!("INT 21h AH=42h: New position = {}", new_position);
                // Return new position in DX:AX
                self.dx = (new_position >> 16) as u16;
                self.ax = (new_position & 0xFFFF) as u16;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                log::warn!("INT 21h AH=42h: Seek failed - error {}", error_code);
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=45h - Duplicate File Handle
    /// Input:
    ///   BX = existing file handle
    /// Output:
    ///   CF clear if success: AX = new file handle (duplicate of BX)
    ///   CF set if error: AX = error code
    fn int21_duplicate_file(&mut self, io: &mut super::Bios) {
        let handle = self.bx;

        match io.file_duplicate(handle) {
            Ok(new_handle) => {
                self.ax = new_handle;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=39h - Create Directory (MKDIR)
    /// Input:
    ///   DS:DX = pointer to null-terminated directory name
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_create_dir(&mut self, memory: &Memory, io: &mut super::Bios) {
        let dirname = self.read_null_terminated_string(memory, self.ds, self.dx);

        match io.dir_create(&dirname) {
            Ok(()) => {
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Ah - Remove Directory (RMDIR)
    /// Input:
    ///   DS:DX = pointer to null-terminated directory name
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_remove_dir(&mut self, memory: &Memory, io: &mut super::Bios) {
        let dirname = self.read_null_terminated_string(memory, self.ds, self.dx);

        match io.dir_remove(&dirname) {
            Ok(()) => {
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=3Bh - Change Current Directory (CHDIR)
    /// Input:
    ///   DS:DX = pointer to null-terminated directory name
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_change_dir(&mut self, memory: &Memory, io: &mut super::Bios) {
        let dirname = self.read_null_terminated_string(memory, self.ds, self.dx);

        match io.dir_change(&dirname) {
            Ok(()) => {
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=36h - Get Disk Free Space
    /// Input:
    ///   DL = drive number (0=default, 1=A, 2=B, 3=C, etc.)
    /// Output:
    ///   AX = sectors per cluster (FFFF if drive invalid)
    ///   BX = number of available clusters
    ///   CX = bytes per sector
    ///   DX = total clusters on drive
    fn int21_get_disk_free_space(&mut self, io: &super::Bios) {
        let drive = DriveNumber::from_dos_with_current((self.dx & 0xFF) as u8); // DL
        let drive = drive.unwrap_or(io.get_current_drive());

        log::debug!("INT 21h AH=36h: Get Disk Free Space for drive {}", drive);

        // Get drive parameters to check if drive exists
        match io.disk_get_params(drive) {
            Ok(_params) => {
                // Drive exists - return reasonable values
                // For simplicity, we'll return fixed cluster size and calculate totals
                // Typical FAT16 values:
                let sectors_per_cluster = 4u16; // 4 sectors = 2KB clusters
                let bytes_per_sector = 512u16;

                // Get total disk size from INT 13h
                match io.disk_get_type(drive) {
                    Ok((_drive_type, total_sectors)) => {
                        let total_clusters = (total_sectors / sectors_per_cluster as u32) as u16;
                        // Report as all free for now (simplification)
                        let free_clusters = total_clusters;

                        self.ax = sectors_per_cluster;
                        self.bx = free_clusters;
                        self.cx = bytes_per_sector;
                        self.dx = total_clusters;

                        log::debug!(
                            "INT 21h AH=36h: Drive {} - spc={}, free={}, bps={}, total={}",
                            drive,
                            sectors_per_cluster,
                            free_clusters,
                            bytes_per_sector,
                            total_clusters
                        );
                    }
                    Err(_) => {
                        // Drive exists but can't get size - return error
                        self.ax = 0xFFFF;
                        log::warn!("INT 21h AH=36h: Drive {} exists but can't get size", drive);
                    }
                }
            }
            Err(_) => {
                // Drive doesn't exist
                self.ax = 0xFFFF;
                log::warn!("INT 21h AH=36h: Invalid drive {}", drive);
            }
        }
    }

    /// INT 21h, AH=47h - Get Current Directory
    /// Input:
    ///   DL = drive number (0=default, 1=A, 2=B, etc.)
    ///   DS:SI = pointer to 64-byte buffer for directory path
    /// Output:
    ///   CF clear if success: buffer filled with path (without drive or leading backslash)
    ///   CF set if error: AX = error code
    fn int21_get_current_dir(&mut self, memory: &mut Memory, io: &super::Bios) {
        let drive = DriveNumber::from_dos_with_current((self.dx & 0xFF) as u8); // DL
        let drive = drive.unwrap_or(io.get_current_drive());

        match io.dir_get_current(drive) {
            Ok(path) => {
                // Write path to DS:SI (null-terminated)
                let buffer_addr = Self::physical_address(self.ds, self.si);
                for (i, &byte) in path.as_bytes().iter().enumerate() {
                    if i >= 63 {
                        break; // Leave room for null terminator
                    }
                    memory.write_u8(buffer_addr + i, byte);
                }
                // Write null terminator
                let len = path.len().min(63);
                memory.write_u8(buffer_addr + len, 0);

                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=4Eh - Find First Matching File
    /// Input:
    ///   DS:DX = pointer to null-terminated file pattern (may include wildcards)
    ///   CX = file attributes to match
    ///   ES:BX = pointer to DTA (Disk Transfer Area, 43 bytes)
    /// Output:
    ///   CF clear if success: DTA filled with file information
    ///   CF set if error: AX = error code
    fn int21_find_first(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        let pattern = self.read_null_terminated_string(memory, self.ds, self.dx);
        let attributes = (self.cx & 0xFF) as u8;

        match io.find_first(&pattern, attributes) {
            Ok((search_id, find_data)) => {
                // Write search ID to a hidden location (we'll use offset 0 of DTA for this)
                let dta_addr = Self::physical_address(self.es, self.bx);

                // DOS DTA format for find first/next:
                // Offset 0-20: Reserved for DOS (we'll store search_id here)
                // Offset 21: File attributes
                // Offset 22-23: File time
                // Offset 24-25: File date
                // Offset 26-29: File size (32-bit little-endian)
                // Offset 30-42: Filename (null-terminated, 13 bytes max)

                // Store search_id in first 8 bytes (as u64)
                for i in 0..8 {
                    memory.write_u8(dta_addr + i, ((search_id >> (i * 8)) & 0xFF) as u8);
                }

                self.write_find_data_to_dta(memory, dta_addr, &find_data);
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=4Fh - Find Next Matching File
    /// Input:
    ///   ES:BX = pointer to DTA (must contain data from previous find first/next)
    /// Output:
    ///   CF clear if success: DTA filled with file information
    ///   CF set if error: AX = error code
    fn int21_find_next(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        let dta_addr = Self::physical_address(self.es, self.bx);

        // Read search_id from DTA
        let mut search_id: usize = 0;
        for i in 0..8 {
            search_id |= (memory.read_u8(dta_addr + i) as usize) << (i * 8);
        }

        match io.find_next(search_id) {
            Ok(find_data) => {
                self.write_find_data_to_dta(memory, dta_addr, &find_data);
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// Helper function to write FindData to DTA
    fn write_find_data_to_dta(&self, memory: &mut Memory, dta_addr: usize, find_data: &FindData) {
        // Offset 21: File attributes
        memory.write_u8(dta_addr + 21, find_data.attributes);

        // Offset 22-23: File time (little-endian)
        memory.write_u8(dta_addr + 22, (find_data.time & 0xFF) as u8);
        memory.write_u8(dta_addr + 23, (find_data.time >> 8) as u8);

        // Offset 24-25: File date (little-endian)
        memory.write_u8(dta_addr + 24, (find_data.date & 0xFF) as u8);
        memory.write_u8(dta_addr + 25, (find_data.date >> 8) as u8);

        // Offset 26-29: File size (32-bit little-endian)
        memory.write_u8(dta_addr + 26, (find_data.size & 0xFF) as u8);
        memory.write_u8(dta_addr + 27, ((find_data.size >> 8) & 0xFF) as u8);
        memory.write_u8(dta_addr + 28, ((find_data.size >> 16) & 0xFF) as u8);
        memory.write_u8(dta_addr + 29, ((find_data.size >> 24) & 0xFF) as u8);

        // Offset 30-42: Filename (null-terminated, max 13 bytes)
        let filename_bytes = find_data.filename.as_bytes();
        for (i, &byte) in filename_bytes.iter().take(12).enumerate() {
            memory.write_u8(dta_addr + 30 + i, byte);
        }
        // Null terminator
        let len = filename_bytes.len().min(12);
        memory.write_u8(dta_addr + 30 + len, 0);
    }

    /// INT 21h, AH=0Eh - Select Default Disk
    /// Input:
    ///   DL = drive number (0=A, 1=B, etc.)
    /// Output:
    ///   AL = number of logical drives in system
    fn int21_select_disk(&mut self, io: &mut super::Bios) {
        let drive = DriveNumber::from_dos((self.dx & 0xFF) as u8); // DL
        log::debug!("INT 21h AH=0Eh: Select disk {}", drive);
        let num_drives = io.set_default_drive(drive);
        log::debug!(
            "INT 21h AH=0Eh: Selected drive {}, returning {} total drives",
            drive,
            num_drives
        );
        self.ax = (self.ax & 0xFF00) | (num_drives as u16);
    }

    /// INT 21h, AH=2Ah - Get System Date
    /// Output:
    ///   CX = year (1980-2099)
    ///   DH = month (1-12)
    ///   DL = day (1-31)
    ///   AL = day of week (0=Sunday, 1=Monday, ..., 6=Saturday)
    fn int21_get_date(&mut self, io: &super::Bios) {
        let date = io.get_local_date();
        let year = (date.century as u16) * 100 + (date.year as u16);
        self.cx = year; // CX = year
        self.dx = ((date.month as u16) << 8) | (date.day as u16); // DH=month, DL=day

        // Calculate day of week (Zeller's congruence)
        let mut m = date.month as i32;
        let mut y = year as i32;
        let d = date.day as i32;

        // January and February are treated as months 13 and 14 of the previous year
        if m < 3 {
            m += 12;
            y -= 1;
        }

        let day_of_week = (d + (13 * (m + 1)) / 5 + y + y / 4 - y / 100 + y / 400) % 7;
        // Adjust: Zeller gives 0=Saturday, we need 0=Sunday
        let day_of_week = ((day_of_week + 6) % 7) as u8;

        self.ax = (self.ax & 0xFF00) | (day_of_week as u16); // AL = day of week

        log::debug!(
            "INT 21h AH=2Ah: Get date - {}-{:02}-{:02} (day of week: {})",
            year,
            date.month,
            date.day,
            day_of_week
        );
    }

    /// INT 21h, AH=2Bh - Set System Date
    /// Input:
    ///   CX = year (1980-2099)
    ///   DH = month (1-12)
    ///   DL = day (1-31)
    /// Output:
    ///   AL = 0x00 if successful, 0xFF if invalid date
    fn int21_set_date(&mut self) {
        let year = self.cx;
        let month = ((self.dx >> 8) & 0xFF) as u8;
        let day = (self.dx & 0xFF) as u8;

        // Validate date
        if !(1980..=2099).contains(&year) || !(1..=12).contains(&month) || !(1..=31).contains(&day)
        {
            self.ax = (self.ax & 0xFF00) | 0xFF; // AL = 0xFF (invalid)
            log::warn!(
                "INT 21h AH=2Bh: Invalid date {}-{:02}-{:02}",
                year,
                month,
                day
            );
            return;
        }

        log::warn!(
            "INT 21h AH=2Bh: Set date to {}-{:02}-{:02} (not implemented - date is read-only in emulator)",
            year,
            month,
            day
        );

        // Note: We don't actually set the system date in the emulator
        // because get_local_date() always reads the host system clock.
        // Return success anyway since programs expect it.
        self.ax &= 0xFF00; // AL = 0x00 (success)
    }

    /// INT 21h, AH=2Ch - Get System Time
    /// Output:
    ///   CH = hours (0-23)
    ///   CL = minutes (0-59)
    ///   DH = seconds (0-59)
    ///   DL = hundredths of seconds (0-99)
    ///
    /// Note: This reads from the BDA timer counter (updated by INT 08h at 18.2 Hz),
    /// not from the RTC. The timer counter is synced before this function is called
    /// to include pending timer ticks.
    fn int21_get_time(&mut self, memory: &Memory) {
        use crate::memory::{BDA_START, BDA_TIMER_COUNTER};

        // Read timer counter from BDA (4 bytes, little-endian)
        let counter_addr = BDA_START + BDA_TIMER_COUNTER;
        let tick_count = memory.read_u32(counter_addr);

        // Convert ticks to time
        // Timer frequency: 18.2065 Hz (exactly 1193182 / 65536)
        // More precisely: ticks = seconds * 1193182 / 65536
        // So: seconds = ticks * 65536 / 1193182
        let total_centiseconds = (tick_count as u64 * 6553600) / 1193182;
        let total_seconds = (total_centiseconds / 100) as u32;
        let hundredths = (total_centiseconds % 100) as u8;

        let hours = (total_seconds / 3600) as u8;
        let minutes = ((total_seconds % 3600) / 60) as u8;
        let seconds = (total_seconds % 60) as u8;

        self.cx = ((hours as u16) << 8) | (minutes as u16); // CH=hours, CL=minutes
        self.dx = ((seconds as u16) << 8) | (hundredths as u16); // DH=seconds, DL=hundredths

        log::debug!(
            "INT 21h AH=2Ch: Get time - {:02}:{:02}:{:02}.{:02} (from {} ticks)",
            hours,
            minutes,
            seconds,
            hundredths,
            tick_count
        );
    }

    /// INT 21h, AH=2Dh - Set System Time
    /// Input:
    ///   CH = hours (0-23)
    ///   CL = minutes (0-59)
    ///   DH = seconds (0-59)
    ///   DL = hundredths of seconds (0-99)
    /// Output:
    ///   AL = 0x00 if successful, 0xFF if invalid time
    fn int21_set_time(&mut self, _memory: &mut Memory) {
        let hours = ((self.cx >> 8) & 0xFF) as u8;
        let minutes = (self.cx & 0xFF) as u8;
        let seconds = ((self.dx >> 8) & 0xFF) as u8;
        let hundredths = (self.dx & 0xFF) as u8;

        // Validate time
        if hours > 23 || minutes > 59 || seconds > 59 || hundredths > 99 {
            self.ax = (self.ax & 0xFF00) | 0xFF; // AL = 0xFF (invalid)
            log::warn!(
                "INT 21h AH=2Dh: Invalid time {:02}:{:02}:{:02}.{:02}",
                hours,
                minutes,
                seconds,
                hundredths
            );
            return;
        }

        log::warn!(
            "INT 21h AH=2Dh: Set time to {:02}:{:02}:{:02}.{:02} (not implemented - time is read-only in emulator)",
            hours,
            minutes,
            seconds,
            hundredths
        );

        // Note: We don't actually set the system time in the emulator
        // because get_local_time() always reads the host system clock.
        // Return success anyway since programs expect it.
        self.ax &= 0xFF00; // AL = 0x00 (success)
    }

    /// INT 21h, AH=32h - Get Drive Parameter Block (DPB) (undocumented)
    /// Input:
    ///   DL = drive number (0=default, 1=A, 2=B, 3=C, etc.)
    /// Output:
    ///   AL = 00h if drive valid, FFh if invalid
    ///   DS:BX = pointer to Drive Parameter Block (DPB)
    fn int21_get_dpb(&mut self, memory: &mut Memory, io: &super::Bios) {
        let drive = DriveNumber::from_dos_with_current((self.dx & 0xFF) as u8); // DL
        let drive = drive.unwrap_or(io.get_current_drive());

        log::debug!("INT 21h AH=32h: Get DPB for drive {:?}", drive);

        // Check if drive exists
        match io.disk_get_params(drive) {
            Ok(_params) => {
                // Drive exists - create a DPB structure in memory
                // We'll use a fixed location in high memory for DPB storage
                // Offset 0x500 (just after BDA) is a safe area
                let dpb_addr = 0x0500 + (drive.to_standard() as usize * 64); // 64 bytes per DPB

                // Build a minimal DPB structure
                // +0: Drive number (0=A, 1=B, 2=C, etc.)
                let dos_drive = drive.to_dos_drive();
                memory.write_u8(dpb_addr, dos_drive);

                // +1: Unit number within driver
                memory.write_u8(dpb_addr + 1, 0);

                // +2: Bytes per sector (word)
                memory.write_u16(dpb_addr + 2, 512);

                // +4: Sectors per cluster - 1 (byte)
                memory.write_u8(dpb_addr + 4, 3); // 4 sectors per cluster

                // +5: Cluster to sector shift (byte)
                memory.write_u8(dpb_addr + 5, 2); // log2(4) = 2

                // +6: Reserved sectors (word)
                memory.write_u16(dpb_addr + 6, 1); // Boot sector

                // +8: Number of FATs (byte)
                memory.write_u8(dpb_addr + 8, 2);

                // +9: Root directory entries (word)
                memory.write_u16(dpb_addr + 9, 512);

                // +11: First data sector (word)
                memory.write_u16(dpb_addr + 11, 33); // After boot, FATs, and root

                // +13: Highest cluster number + 1 (word)
                if let Ok((_drive_type, total_sectors)) = io.disk_get_type(drive) {
                    let clusters = (total_sectors / 4) as u16;
                    memory.write_u16(dpb_addr + 13, clusters);
                } else {
                    memory.write_u16(dpb_addr + 13, 1000); // Default
                }

                // +15: Sectors per FAT (byte in DOS 2.x, word in DOS 3+)
                memory.write_u16(dpb_addr + 15, 9); // Typical for small disk

                // +17: First directory sector (word)
                memory.write_u16(dpb_addr + 17, 19); // After boot and FATs

                // +19: Device driver header (dword)
                memory.write_u16(dpb_addr + 19, 0xFFFF);
                memory.write_u16(dpb_addr + 21, 0xFFFF);

                // +23: Media descriptor (byte)
                let media_id = if drive.is_floppy() {
                    0xF0 // Floppy
                } else {
                    0xF8 // Hard disk
                };
                memory.write_u8(dpb_addr + 23, media_id);

                // +24: Access flag (byte) - 0 = accessed
                memory.write_u8(dpb_addr + 24, 0);

                // +25: Next DPB pointer (dword)
                memory.write_u16(dpb_addr + 25, 0xFFFF);
                memory.write_u16(dpb_addr + 27, 0xFFFF);

                // Return pointer in DS:BX
                let dpb_seg = (dpb_addr >> 4) as u16;
                let dpb_off = (dpb_addr & 0x0F) as u16;
                self.ds = dpb_seg;
                self.bx = dpb_off;
                self.ax &= 0xFF00; // AL = 00h (success)

                log::debug!(
                    "INT 21h AH=32h: DPB created at {:04X}:{:04X} for drive {}",
                    dpb_seg,
                    dpb_off,
                    dos_drive
                );
            }
            Err(_) => {
                // Drive doesn't exist
                self.ax = (self.ax & 0xFF00) | 0xFF; // AL = FFh (invalid)
                log::warn!("INT 21h AH=32h: Invalid drive {}", drive);
            }
        }
    }

    /// INT 21h, AH=37h - Get/Set Switch Character (DOS 2.x)
    /// Input:
    ///   AL = 0: Get switch character
    ///   AL = 1: Set switch character
    ///   DL = new switch character (when AL=1)
    /// Output:
    ///   DL = switch character (when AL=0)
    ///   AL = 0xFF (indicates function not supported in DOS 5+)
    fn int21_switch_char(&mut self) {
        let subfunction = (self.ax & 0xFF) as u8; // AL

        // This function is obsolete in DOS 5.0+
        // Return AL=0xFF to indicate function not supported
        self.ax = (self.ax & 0xFF00) | 0xFF;

        // For compatibility, return '/' as the switch character
        if subfunction == 0x00 {
            self.dx = (self.dx & 0xFF00) | ('/' as u16);
        }
    }

    /// INT 21h, AH=44h - IOCTL (Input/Output Control)
    /// Input:
    ///   AL = subfunction
    ///   BX = file handle (for most subfunctions)
    /// Output: Varies by subfunction
    fn int21_ioctl(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        let subfunction = (self.ax & 0xFF) as u8; // AL
        let handle = self.bx;

        log::info!(
            "INT 21h AH=44h: IOCTL AL=0x{:02X}, BX=0x{:04X}, CX=0x{:04X}, DX=0x{:04X}",
            subfunction,
            self.bx,
            self.cx,
            self.dx
        );

        match subfunction {
            0x00 => {
                // Get device information
                match io.ioctl_get_device_info(handle) {
                    Ok(info) => {
                        self.dx = info;
                        self.set_flag(cpu_flag::CARRY, false);
                    }
                    Err(error_code) => {
                        self.ax = error_code as u16;
                        self.set_flag(cpu_flag::CARRY, true);
                    }
                }
            }
            0x01 => {
                // Set device information
                let info = self.dx;
                match io.ioctl_set_device_info(handle, info) {
                    Ok(()) => {
                        self.set_flag(cpu_flag::CARRY, false);
                    }
                    Err(error_code) => {
                        self.ax = error_code as u16;
                        self.set_flag(cpu_flag::CARRY, true);
                    }
                }
            }
            0x06 => {
                // Get input status
                // Return AL=0xFF if ready, AL=0x00 if not ready
                // For simplicity, always return ready for standard handles
                if handle <= 2 {
                    self.ax = (self.ax & 0xFF00) | 0xFF;
                } else {
                    self.ax &= 0xFF00;
                }
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x07 => {
                // Get output status
                // Return AL=0xFF if ready, AL=0x00 if not ready
                // For simplicity, always return ready for standard handles
                if handle <= 2 {
                    self.ax = (self.ax & 0xFF00) | 0xFF;
                } else {
                    self.ax &= 0xFF00;
                }
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x08 => {
                // Check if block device is removable
                // BL = drive number (0=default, 1=A:, 2=B:, 3=C:, etc.)
                // Return AL=0 if removable, AL=1 if fixed
                let drive = DriveNumber::from_dos_with_current((self.bx & 0xFF) as u8);
                let drive = drive.unwrap_or(io.get_current_drive());

                if drive.is_floppy() {
                    self.ax &= 0xFF00; // Removable (AL=0x00)
                } else {
                    self.ax = (self.ax & 0xFF00) | 0x01; // Fixed (AL=0x01)
                }
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x09 => {
                // Check if block device is remote
                // Return DX bit 12 set if remote
                // For simplicity, all devices are local
                self.dx &= !0x1000; // Clear bit 12
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x0A => {
                // Check if handle is remote
                // Return DX bit 15 set if remote
                // For simplicity, all handles are local
                self.dx &= !0x8000; // Clear bit 15
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x0D => {
                // Generic block device request
                // BL = drive number (0=default, 1=A:, 2=B:, 3=C:, etc.)
                // CH = category (08h = disk drive)
                // CL = function code
                // DS:DX = pointer to parameter block
                let drive = DriveNumber::from_dos_with_current((self.bx & 0xFF) as u8);
                let drive = drive.unwrap_or(io.get_current_drive());
                let category = (self.cx >> 8) as u8;
                let function = (self.cx & 0xFF) as u8;

                log::info!(
                    "INT 21h AH=44h AL=0Dh: Generic IOCTL drive={}, category=0x{:02X}, function=0x{:02X}",
                    drive,
                    category,
                    function
                );

                match (category, function) {
                    (0x08, 0x40) => {
                        // Set device parameters
                        log::info!("  Set device parameters for DOS drive {}", drive);
                        // For now, just acknowledge success without modifying anything
                        self.set_flag(cpu_flag::CARRY, false);
                    }
                    (0x08, 0x42) => {
                        // Format and verify track
                        log::info!("  Format and verify track for DOS drive {}", drive);
                        match self.int21_ioctl_format_track(memory, io, drive) {
                            Ok(()) => {
                                self.set_flag(cpu_flag::CARRY, false);
                            }
                            Err(error_code) => {
                                self.ax = error_code as u16;
                                self.set_flag(cpu_flag::CARRY, true);
                            }
                        }
                    }
                    (0x08, 0x47) => {
                        // Set access flag (for volume label writes during format)
                        log::info!("  Set access flag for DOS drive {}", drive);
                        // Just acknowledge success
                        self.set_flag(cpu_flag::CARRY, false);
                    }
                    (0x08, 0x60) => {
                        // Get device parameters
                        log::info!("  Get device parameters for DOS drive {}", drive);
                        match self.int21_ioctl_get_device_params(memory, io, drive) {
                            Ok(()) => {
                                self.set_flag(cpu_flag::CARRY, false);
                            }
                            Err(error_code) => {
                                self.ax = error_code as u16;
                                self.set_flag(cpu_flag::CARRY, true);
                            }
                        }
                    }
                    _ => {
                        log::warn!(
                            "Unhandled IOCTL 0x0D: category=0x{:02X}, function=0x{:02X}",
                            category,
                            function
                        );
                        self.ax = DosError::InvalidFunction as u16;
                        self.set_flag(cpu_flag::CARRY, true);
                    }
                }
            }
            _ => {
                log::warn!("Unhandled IOCTL subfunction: AL=0x{:02X}", subfunction);
                self.ax = DosError::InvalidFunction as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=44h AL=0Dh CL=60h - Get Device Parameters
    /// Input:
    ///   drive_num = DOS drive number (0=A:, 1=B:, 2=C:, etc.)
    ///   DS:DX = pointer to device parameter block
    /// Output:
    ///   CF clear if success, parameter block filled
    ///   CF set if error: AX = error code
    fn int21_ioctl_get_device_params(
        &mut self,
        memory: &mut Memory,
        io: &mut super::Bios,
        drive: DriveNumber,
    ) -> Result<(), DosError> {
        // Get drive parameters from INT 13h
        let params = io
            .disk_get_params(drive)
            .map_err(|_| DosError::InvalidDrive)?;

        // Convert max values to actual counts
        let cylinders = (params.max_cylinder as u16) + 1;
        let heads = (params.max_head as u16) + 1;
        let sectors_per_track = params.max_sector as u16; // Already 1-based

        // Calculate total sectors
        let total_sectors = cylinders as u32 * heads as u32 * sectors_per_track as u32;

        // Get pointer to parameter block (DS:DX)
        let seg = self.ds;
        let offset = self.dx;
        let addr = (seg as usize) * 16 + offset as usize;

        // Build device parameter block
        // Byte 0: Special functions (0 = default)
        memory.write_u8(addr, 0x00);

        // Byte 1: Device type (5 = hard disk, 0-4 = floppy types)
        let device_type = if drive.is_hard_drive() { 0x05 } else { 0x02 }; // 0x02 = 720KB floppy
        memory.write_u8(addr + 1, device_type);

        // Bytes 2-3: Device attributes (bit 0 = 0 for non-removable, 1 for removable)
        let is_removable = drive.is_floppy(); // Floppies are removable, hard drives are not
        let attributes: u16 = if is_removable { 0x0001 } else { 0x0000 };
        memory.write_u16(addr + 2, attributes);

        // Bytes 4-5: Number of cylinders
        memory.write_u16(addr + 4, cylinders);

        // Byte 6: Media type (0 = default)
        memory.write_u8(addr + 6, 0x00);

        // BPB starts at byte 7 - provide reasonable defaults for unformatted drive
        // Bytes 7-8: Bytes per sector
        memory.write_u16(addr + 7, 512);

        // Byte 9: Sectors per cluster (use 1 for simplicity)
        memory.write_u8(addr + 9, 0x01);

        // Bytes 10-11: Reserved sectors (usually 1 for boot sector)
        memory.write_u16(addr + 10, 1);

        // Byte 12: Number of FATs (usually 2)
        memory.write_u8(addr + 12, 2);

        // Bytes 13-14: Root directory entries (512 is common for hard disks)
        memory.write_u16(addr + 13, 512);

        // Bytes 15-16: Total sectors (if < 32MB)
        if total_sectors < 65536 {
            memory.write_u16(addr + 15, total_sectors as u16);
        } else {
            memory.write_u16(addr + 15, 0); // Use extended field instead
        }

        // Byte 17: Media descriptor (0xF8 = hard disk, 0xF0 = floppy)
        let media_desc = if drive.is_hard_drive() { 0xF8 } else { 0xF0 };
        memory.write_u8(addr + 17, media_desc);

        // Bytes 18-19: Sectors per FAT (calculate based on total sectors)
        let sectors_per_fat = ((total_sectors / 512) / 2).div_ceil(256);
        memory.write_u16(addr + 18, sectors_per_fat as u16);

        // Bytes 20-21: Sectors per track
        memory.write_u16(addr + 20, sectors_per_track);

        // Bytes 22-23: Number of heads
        memory.write_u16(addr + 22, heads);

        // Bytes 24-27: Hidden sectors
        // Since PartitionedDisk handles partition offsets transparently,
        // we report 0 (the OS sees the partition directly, not the whole disk)
        let hidden_sectors = 0u32;
        // Write u32 as two u16 values (little-endian)
        memory.write_u16(addr + 24, (hidden_sectors & 0xFFFF) as u16);
        memory.write_u16(addr + 26, (hidden_sectors >> 16) as u16);

        // Bytes 28-31: Total sectors (extended field for >= 32MB)
        if total_sectors >= 65536 {
            memory.write_u16(addr + 28, (total_sectors & 0xFFFF) as u16);
            memory.write_u16(addr + 30, (total_sectors >> 16) as u16);
        } else {
            memory.write_u16(addr + 28, 0);
            memory.write_u16(addr + 30, 0);
        }

        log::info!(
            "  Device params: cyl={}, head={}, sec={}, total_sec={}, hidden={}",
            cylinders,
            heads,
            sectors_per_track,
            total_sectors,
            hidden_sectors
        );

        Ok(())
    }

    /// INT 21h, AH=44h AL=0Dh CL=42h - Format and Verify Track
    /// Input:
    ///   drive = DOS drive number
    ///   DS:DX = pointer to format parameter block:
    ///     Byte 0: Special functions (0 = default)
    ///     Bytes 1-2: Track/cylinder number (word)
    ///     Bytes 3-4: Head number (word)
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_ioctl_format_track(
        &mut self,
        memory: &mut Memory,
        io: &mut super::Bios,
        drive: DriveNumber,
    ) -> Result<(), DosError> {
        // Get pointer to parameter block (DS:DX)
        let seg = self.ds;
        let offset = self.dx;
        let addr = (seg as usize) * 16 + offset as usize;

        // Read parameters from parameter block
        let _special_functions = memory.read_u8(addr);
        let cylinder = memory.read_u16(addr + 1);
        let head = memory.read_u16(addr + 3);

        log::info!(
            "  Format track: drive={}, cyl={}, head={}",
            drive,
            cylinder,
            head
        );

        // Get drive parameters to determine sectors per track
        let params = io
            .disk_get_params(drive)
            .map_err(|_| DosError::InvalidDrive)?;

        let sectors_per_track = params.max_sector;

        // Format the track using INT 13h function
        io.disk_format_track(drive, cylinder as u8, head as u8, sectors_per_track)
            .map_err(|error_code| {
                log::warn!(
                    "  Format track failed: drive={}, cyl={}, head={}, error={}",
                    drive,
                    cylinder,
                    head,
                    error_code
                );
                DosError::AccessDenied
            })?;

        log::info!("  Format track succeeded");
        Ok(())
    }

    /// INT 21h, AH=48h - Allocate Memory
    /// Input:
    ///   BX = number of paragraphs (16-byte blocks) to allocate
    /// Output:
    ///   CF clear if success: AX = segment of allocated memory
    ///   CF set if error: AX = error code, BX = size of largest available block
    fn int21_allocate_memory(&mut self, io: &mut super::Bios) {
        let paragraphs = self.bx;
        log::info!(
            "INT 21h AH=48h: Allocate memory request: {} paragraphs ({} bytes)",
            paragraphs,
            paragraphs as u32 * 16
        );

        match io.memory_allocate(paragraphs) {
            Ok(segment) => {
                log::info!("INT 21h AH=48h: Allocated at segment 0x{:04X}", segment);
                self.ax = segment;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err((error_code, max_available)) => {
                log::warn!(
                    "INT 21h AH=48h: Allocation failed - error {}, max available: {} paragraphs ({} bytes)",
                    error_code,
                    max_available,
                    max_available as u32 * 16
                );
                self.ax = error_code as u16;
                self.bx = max_available;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=49h - Free Memory
    /// Input:
    ///   ES = segment of block to free
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_free_memory(&mut self, io: &mut super::Bios) {
        let segment = self.es;
        log::info!("INT 21h AH=49h: Free memory at segment 0x{:04X}", segment);

        match io.memory_free(segment) {
            Ok(()) => {
                log::info!(
                    "INT 21h AH=49h: Successfully freed memory at 0x{:04X}",
                    segment
                );
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
                log::warn!("INT 21h AH=49h: Free failed - error {}", error_code);
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=4Ah - Resize Memory Block
    /// Input:
    ///   ES = segment of block to resize
    ///   BX = new size in paragraphs
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code, BX = maximum size available
    fn int21_resize_memory(&mut self, io: &mut super::Bios) {
        let segment = self.es;
        let paragraphs = self.bx;
        log::info!(
            "INT 21h AH=4Ah: Resize memory at segment 0x{:04X} to {} paragraphs ({} bytes)",
            segment,
            paragraphs,
            paragraphs as u32 * 16
        );

        match io.memory_resize(segment, paragraphs) {
            Ok(()) => {
                log::info!(
                    "INT 21h AH=4Ah: Successfully resized to {} paragraphs",
                    paragraphs
                );
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err((error_code, max_available)) => {
                log::warn!(
                    "INT 21h AH=4Ah: Resize failed - error {}, max available: {} paragraphs",
                    error_code,
                    max_available
                );
                self.ax = error_code as u16;
                self.bx = max_available;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=4Bh - EXEC (Load and Execute Program)
    /// Input:
    ///   AL = subfunction (00h=load+execute, 01h=load, 03h=overlay)
    ///   DS:DX = pointer to null-terminated program filename
    ///   ES:BX = pointer to parameter block
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_exec(&mut self, memory: &mut Memory, io: &mut super::Bios) {
        let subfunction = (self.ax & 0xFF) as u8;
        let filename = self.read_null_terminated_string(memory, self.ds, self.dx);

        log::info!("INT 21h AH=4Bh AL={:02X}: EXEC '{}'", subfunction, filename);

        // Read parameter block from ES:BX
        let param_block_addr = Self::physical_address(self.es, self.bx);

        // Parameter block structure for AL=00h (Load and Execute):
        // Offset 0: Word - Environment segment (0 = use parent's)
        // Offset 2: Dword - Command line pointer (far pointer)
        // Offset 6: Dword - First FCB pointer (far pointer)
        // Offset 10: Dword - Second FCB pointer (far pointer)

        let env_segment = memory.read_u16(param_block_addr);
        let cmdline_offset = memory.read_u16(param_block_addr + 2);
        let cmdline_segment = memory.read_u16(param_block_addr + 4);

        // Read command line (Pascal-style string: first byte is length)
        let cmdline_addr = Self::physical_address(cmdline_segment, cmdline_offset);
        let cmdline_len = memory.read_u8(cmdline_addr) as usize;
        let mut command_line = String::new();
        for i in 0..cmdline_len {
            let ch = memory.read_u8(cmdline_addr + 1 + i);
            if ch == 0x0D {
                // CR terminates the command line
                break;
            }
            command_line.push(ch as char);
        }

        log::debug!(
            "INT 21h AH=4Bh: env_segment=0x{:04X}, command_line='{}'",
            env_segment,
            command_line
        );

        let params = ExecParams {
            subfunction,
            filename: filename.clone(),
            env_segment,
            command_line,
        };

        // Load the program data
        let program_data = match io.exec_load_program(&params) {
            Ok(data) => data,
            Err(error_code) => {
                log::warn!(
                    "INT 21h AH=4Bh: Failed to load '{}' - error {}",
                    filename,
                    error_code
                );
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
                return;
            }
        };

        if program_data.is_empty() {
            log::warn!("INT 21h AH=4Bh: Empty program file '{}'", filename);
            self.ax = DosError::InvalidFormat as u16;
            self.set_flag(cpu_flag::CARRY, true);
            return;
        }

        // Check if this is an EXE file (MZ header)
        let is_exe = program_data.len() >= 2 && program_data[0] == 0x4D && program_data[1] == 0x5A;

        if is_exe {
            // EXE file handling
            self.exec_load_exe(memory, io, &program_data, &params);
        } else {
            // COM file handling
            self.exec_load_com(memory, io, &program_data, &params);
        }
    }

    /// Load and execute a COM file
    fn exec_load_com(
        &mut self,
        memory: &mut Memory,
        io: &mut super::Bios,
        program_data: &[u8],
        params: &ExecParams,
    ) {
        // COM files need PSP (256 bytes) + program
        // Max size is 64KB - 256 bytes for PSP - 2 bytes for stack
        let program_size = program_data.len();
        if program_size > 0xFFFE - 0x100 {
            log::warn!(
                "INT 21h AH=4Bh: COM file too large ({} bytes)",
                program_size
            );
            self.ax = DosError::InsufficientMemory as u16;
            self.set_flag(cpu_flag::CARRY, true);
            return;
        }

        // Calculate paragraphs needed: PSP (16 paragraphs) + program + stack
        // Round up program size to paragraph boundary
        let total_bytes = 0x100 + program_size + 0x100; // PSP + program + some stack
        let paragraphs = total_bytes.div_ceil(16) as u16;

        // Allocate memory for the program
        let psp_segment = match io.memory_allocate(paragraphs) {
            Ok(seg) => seg,
            Err((error_code, _)) => {
                log::warn!(
                    "INT 21h AH=4Bh: Failed to allocate memory - error {}",
                    error_code
                );
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
                return;
            }
        };

        log::info!(
            "INT 21h AH=4Bh: Allocated {} paragraphs at segment 0x{:04X}",
            paragraphs,
            psp_segment
        );

        // Build the PSP at psp_segment:0000
        self.build_psp(memory, io, psp_segment, params);

        // Load program at psp_segment:0100
        let load_addr = Self::physical_address(psp_segment, 0x0100);
        for (i, &byte) in program_data.iter().enumerate() {
            memory.write_u8(load_addr + i, byte);
        }

        log::info!(
            "INT 21h AH=4Bh: Loaded {} bytes at {:05X}",
            program_size,
            load_addr
        );

        match params.subfunction {
            0x00 => {
                // Load and execute
                // Save parent's PSP and set new PSP
                let parent_psp = io.get_psp();
                io.set_psp(psp_segment);

                // For COM files: CS=DS=ES=SS=PSP, IP=0100h, SP=FFFEh
                self.cs = psp_segment;
                self.ds = psp_segment;
                self.es = psp_segment;
                self.ss = psp_segment;
                self.ip = 0x0100;
                self.sp = 0xFFFE;

                // Push return address (PSP:0000) for proper termination
                // PSP:0000 contains INT 20h instruction (CD 20)
                self.sp = self.sp.wrapping_sub(2);
                let stack_addr = Self::physical_address(self.ss, self.sp);
                memory.write_u16(stack_addr, 0x0000); // Return offset = 0
                self.sp = self.sp.wrapping_sub(2);
                let stack_addr = Self::physical_address(self.ss, self.sp);
                memory.write_u16(stack_addr, psp_segment); // Return segment = PSP

                // Store parent PSP at offset 0x16 in child's PSP
                let parent_psp_addr = Self::physical_address(psp_segment, 0x16);
                memory.write_u16(parent_psp_addr, parent_psp);

                log::info!(
                    "INT 21h AH=4Bh: Executing COM at {:04X}:{:04X}",
                    self.cs,
                    self.ip
                );

                // Success - clear carry flag
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x01 => {
                // Load but don't execute - return load info
                // BX:CX = entry point (CS:IP)
                self.bx = psp_segment;
                self.cx = 0x0100;
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x03 => {
                // Load overlay - load at ES:BX, no PSP
                // For overlay, we loaded at the wrong place, need to reload
                // This is a simplified implementation
                self.set_flag(cpu_flag::CARRY, false);
            }
            _ => {
                log::warn!(
                    "INT 21h AH=4Bh: Unsupported subfunction 0x{:02X}",
                    params.subfunction
                );
                self.ax = DosError::InvalidFunction as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// Load and execute an EXE file
    fn exec_load_exe(
        &mut self,
        memory: &mut Memory,
        io: &mut super::Bios,
        program_data: &[u8],
        params: &ExecParams,
    ) {
        // Parse EXE header
        if program_data.len() < 28 {
            log::warn!("INT 21h AH=4Bh: EXE header too small");
            self.ax = DosError::InvalidFormat as u16;
            self.set_flag(cpu_flag::CARRY, true);
            return;
        }

        // EXE header fields
        let last_page_bytes = u16::from_le_bytes([program_data[2], program_data[3]]);
        let total_pages = u16::from_le_bytes([program_data[4], program_data[5]]);
        let reloc_count = u16::from_le_bytes([program_data[6], program_data[7]]);
        let header_paragraphs = u16::from_le_bytes([program_data[8], program_data[9]]);
        let min_paragraphs = u16::from_le_bytes([program_data[10], program_data[11]]);
        let _max_paragraphs = u16::from_le_bytes([program_data[12], program_data[13]]);
        let init_ss = u16::from_le_bytes([program_data[14], program_data[15]]);
        let init_sp = u16::from_le_bytes([program_data[16], program_data[17]]);
        let _checksum = u16::from_le_bytes([program_data[18], program_data[19]]);
        let init_ip = u16::from_le_bytes([program_data[20], program_data[21]]);
        let init_cs = u16::from_le_bytes([program_data[22], program_data[23]]);
        let reloc_table_offset = u16::from_le_bytes([program_data[24], program_data[25]]);

        // Calculate load module size
        let header_size = (header_paragraphs as usize) * 16;
        let load_module_size = if last_page_bytes == 0 {
            (total_pages as usize) * 512
        } else {
            ((total_pages as usize) - 1) * 512 + (last_page_bytes as usize)
        } - header_size;

        log::debug!(
            "INT 21h AH=4Bh: EXE header_size={}, load_module_size={}, reloc_count={}",
            header_size,
            load_module_size,
            reloc_count
        );

        // Allocate memory: PSP (16 paragraphs) + load module + min extra
        let load_paragraphs = load_module_size.div_ceil(16) as u16;
        let total_paragraphs = 16 + load_paragraphs + min_paragraphs;

        let psp_segment = match io.memory_allocate(total_paragraphs) {
            Ok(seg) => seg,
            Err((error_code, _)) => {
                log::warn!(
                    "INT 21h AH=4Bh: Failed to allocate memory - error {}",
                    error_code
                );
                self.ax = error_code as u16;
                self.set_flag(cpu_flag::CARRY, true);
                return;
            }
        };

        // Build PSP
        self.build_psp(memory, io, psp_segment, params);

        // Load segment is right after PSP
        let load_segment = psp_segment.wrapping_add(16);

        // Load the program after the header
        let load_addr = Self::physical_address(load_segment, 0);
        if header_size + load_module_size <= program_data.len() {
            for (i, &byte) in program_data[header_size..header_size + load_module_size]
                .iter()
                .enumerate()
            {
                memory.write_u8(load_addr + i, byte);
            }
        }

        // Apply relocations
        let reloc_table_start = reloc_table_offset as usize;
        for i in 0..reloc_count as usize {
            let reloc_entry_offset = reloc_table_start + i * 4;
            if reloc_entry_offset + 4 > program_data.len() {
                break;
            }

            let offset = u16::from_le_bytes([
                program_data[reloc_entry_offset],
                program_data[reloc_entry_offset + 1],
            ]);
            let segment = u16::from_le_bytes([
                program_data[reloc_entry_offset + 2],
                program_data[reloc_entry_offset + 3],
            ]);

            // Calculate address in loaded image
            let reloc_addr = Self::physical_address(load_segment.wrapping_add(segment), offset);

            // Read current value and add load segment
            let current = memory.read_u16(reloc_addr);
            memory.write_u16(reloc_addr, current.wrapping_add(load_segment));
        }

        log::info!(
            "INT 21h AH=4Bh: Loaded EXE at segment 0x{:04X}, {} relocations applied",
            load_segment,
            reloc_count
        );

        match params.subfunction {
            0x00 => {
                // Load and execute
                let parent_psp = io.get_psp();
                io.set_psp(psp_segment);

                // Set up registers for EXE
                self.cs = load_segment.wrapping_add(init_cs);
                self.ip = init_ip;
                self.ss = load_segment.wrapping_add(init_ss);
                self.sp = init_sp;
                self.ds = psp_segment;
                self.es = psp_segment;

                // Store parent PSP
                let parent_psp_addr = Self::physical_address(psp_segment, 0x16);
                memory.write_u16(parent_psp_addr, parent_psp);

                log::info!(
                    "INT 21h AH=4Bh: Executing EXE at {:04X}:{:04X}, SS:SP={:04X}:{:04X}",
                    self.cs,
                    self.ip,
                    self.ss,
                    self.sp
                );

                self.set_flag(cpu_flag::CARRY, false);
            }
            0x01 => {
                // Load but don't execute
                self.bx = load_segment.wrapping_add(init_cs);
                self.cx = init_ip;
                self.set_flag(cpu_flag::CARRY, false);
            }
            0x03 => {
                // Load overlay - simplified
                self.set_flag(cpu_flag::CARRY, false);
            }
            _ => {
                self.ax = DosError::InvalidFunction as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// Build a Program Segment Prefix (PSP)
    fn build_psp(
        &mut self,
        memory: &mut Memory,
        io: &super::Bios,
        psp_segment: u16,
        params: &ExecParams,
    ) {
        let psp_addr = Self::physical_address(psp_segment, 0);

        // Clear PSP area
        for i in 0..256 {
            memory.write_u8(psp_addr + i, 0);
        }

        // Offset 0x00: INT 20h instruction (CD 20)
        memory.write_u8(psp_addr, 0xCD);
        memory.write_u8(psp_addr + 1, 0x20);

        // Offset 0x02: Memory size in paragraphs (segment of first byte beyond allocated memory)
        // For now, use 0xA000 (end of conventional memory)
        memory.write_u16(psp_addr + 0x02, 0xA000);

        // Offset 0x05: Far call to DOS function dispatcher (not implemented - use INT 21h)
        memory.write_u8(psp_addr + 0x05, 0xCD); // INT 21h
        memory.write_u8(psp_addr + 0x06, 0x21);
        memory.write_u8(psp_addr + 0x07, 0xCB); // RETF

        // Offset 0x0A: Terminate address (INT 22h vector)
        // Read current INT 22h vector from IVT (address 0x0088)
        let int22_ip = memory.read_u16(0x22 * 4);
        let int22_cs = memory.read_u16(0x22 * 4 + 2);
        memory.write_u16(psp_addr + 0x0A, int22_ip);
        memory.write_u16(psp_addr + 0x0C, int22_cs);

        // Offset 0x0E: Break address (INT 23h vector)
        let int23_ip = memory.read_u16(0x23 * 4);
        let int23_cs = memory.read_u16(0x23 * 4 + 2);
        memory.write_u16(psp_addr + 0x0E, int23_ip);
        memory.write_u16(psp_addr + 0x10, int23_cs);

        // Offset 0x12: Critical error address (INT 24h vector)
        let int24_ip = memory.read_u16(0x24 * 4);
        let int24_cs = memory.read_u16(0x24 * 4 + 2);
        memory.write_u16(psp_addr + 0x12, int24_ip);
        memory.write_u16(psp_addr + 0x14, int24_cs);

        // Offset 0x16: Parent PSP segment
        memory.write_u16(psp_addr + 0x16, io.get_psp());

        // Offset 0x18: Job File Table (JFT) - 20 bytes, 0xFF = unused
        for i in 0..20 {
            memory.write_u8(psp_addr + 0x18 + i, 0xFF);
        }
        // Set up standard handles
        memory.write_u8(psp_addr + 0x18, 0x01); // stdin -> CON
        memory.write_u8(psp_addr + 0x19, 0x01); // stdout -> CON
        memory.write_u8(psp_addr + 0x1A, 0x01); // stderr -> CON
        memory.write_u8(psp_addr + 0x1B, 0x00); // stdaux -> AUX
        memory.write_u8(psp_addr + 0x1C, 0x02); // stdprn -> PRN

        // Offset 0x2C: Environment segment
        memory.write_u16(psp_addr + 0x2C, params.env_segment);

        // Offset 0x32: JFT size
        memory.write_u16(psp_addr + 0x32, 20);

        // Offset 0x34: JFT pointer (far pointer to JFT at offset 0x18)
        memory.write_u16(psp_addr + 0x34, 0x18);
        memory.write_u16(psp_addr + 0x36, psp_segment);

        // Offset 0x50: DOS function call (INT 21h, RETF)
        memory.write_u8(psp_addr + 0x50, 0xCD);
        memory.write_u8(psp_addr + 0x51, 0x21);
        memory.write_u8(psp_addr + 0x52, 0xCB);

        // Offset 0x5C: First FCB (not populated - zeroed)
        // Offset 0x6C: Second FCB (not populated - zeroed)

        // Offset 0x80: Command line (Pascal-style: length byte followed by string)
        let cmdline_bytes = params.command_line.as_bytes();
        let cmdline_len = cmdline_bytes.len().min(126) as u8;
        memory.write_u8(psp_addr + 0x80, cmdline_len);
        for (i, &byte) in cmdline_bytes.iter().take(126).enumerate() {
            memory.write_u8(psp_addr + 0x81 + i, byte);
        }
        // Terminate with CR
        memory.write_u8(psp_addr + 0x81 + cmdline_len as usize, 0x0D);
    }

    /// INT 21h, AH=50h - Set PSP Address
    /// Input:
    ///   BX = segment of new PSP
    /// Output: None
    fn int21_set_psp(&mut self, io: &mut super::Bios) {
        let segment = self.bx;
        io.set_psp(segment);
    }

    /// INT 21h, AH=63h - Get Lead Byte Table (DBCS)
    /// Input: AL = subfunction
    /// Output:
    ///   If AL=00h: DS:SI = pointer to DBCS lead byte table
    ///   Table format: pairs of lead byte ranges, terminated by 00h 00h
    ///   For no DBCS support, returns pointer to empty table (just 00h 00h)
    fn int21_get_dbcs_lead_byte_table(&mut self, memory: &mut Memory) {
        let subfunction = (self.ax & 0xFF) as u8;

        if subfunction == 0x00 {
            // Return pointer to empty DBCS table (no double-byte character support)
            // We'll use a static location in the BDA area for the empty table
            const DBCS_TABLE_OFFSET: u16 = 0x00F0; // Use unused area in BDA
            const DBCS_TABLE_SEGMENT: u16 = 0x0040; // BDA segment

            // Write empty table (just two null bytes) at the location
            let addr = Self::physical_address(DBCS_TABLE_SEGMENT, DBCS_TABLE_OFFSET);
            memory.write_u8(addr, 0x00);
            memory.write_u8(addr + 1, 0x00);

            // Return DS:SI pointing to the table
            self.ds = DBCS_TABLE_SEGMENT;
            self.si = DBCS_TABLE_OFFSET;

            log::debug!(
                "INT 21h AH=63h AL=00h: Returning empty DBCS table at {:04X}:{:04X}",
                self.ds,
                self.si
            );
        } else {
            log::warn!(
                "INT 21h AH=63h: Unhandled subfunction AL=0x{:02X}",
                subfunction
            );
        }
    }

    /// Helper function to read a null-terminated string from memory
    fn read_null_terminated_string(&self, memory: &Memory, segment: u16, offset: u16) -> String {
        let mut addr = Self::physical_address(segment, offset);
        let mut result = String::new();

        loop {
            let ch = memory.read_u8(addr);
            if ch == 0 {
                break;
            }
            result.push(ch as char);
            addr += 1;
        }

        result
    }
}
