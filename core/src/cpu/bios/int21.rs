use log::warn;

use crate::{
    Bios,
    cpu::{
        Cpu,
        bios::{FindData, SeekMethod},
        cpu_flag,
    },
    memory::Memory,
};

/// INT 21h DOS error codes
pub mod dos_errors {
    pub const SUCCESS: u8 = 0x00;
    pub const INVALID_FUNCTION: u8 = 0x01;
    pub const FILE_NOT_FOUND: u8 = 0x02;
    pub const PATH_NOT_FOUND: u8 = 0x03;
    pub const TOO_MANY_OPEN_FILES: u8 = 0x04;
    pub const ACCESS_DENIED: u8 = 0x05;
    pub const INVALID_HANDLE: u8 = 0x06;
    pub const MEMORY_CONTROL_BLOCKS_DESTROYED: u8 = 0x07;
    pub const INSUFFICIENT_MEMORY: u8 = 0x08;
    pub const INVALID_MEMORY_BLOCK_ADDRESS: u8 = 0x09;
    pub const INVALID_ENVIRONMENT: u8 = 0x0A;
    pub const INVALID_FORMAT: u8 = 0x0B;
    pub const INVALID_ACCESS_CODE: u8 = 0x0C;
    pub const INVALID_DATA: u8 = 0x0D;
    pub const INVALID_DRIVE: u8 = 0x0F;
    pub const ATTEMPT_TO_REMOVE_CURRENT_DIR: u8 = 0x10;
    pub const NOT_SAME_DEVICE: u8 = 0x11;
    pub const NO_MORE_FILES: u8 = 0x12;
}

/// File access modes for INT 21h, AH=3Dh
pub mod file_access {
    pub const READ_ONLY: u8 = 0x00;
    pub const WRITE_ONLY: u8 = 0x01;
    pub const READ_WRITE: u8 = 0x02;
}

impl Cpu {
    /// INT 0x21 - DOS Services
    /// AH register contains the function number
    pub(super) fn handle_int21<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let function = (self.ax >> 8) as u8; // Get AH directly

        match function {
            0x01 => self.int21_read_char_with_echo(io),
            0x02 => self.int21_write_char(io),
            0x09 => self.int21_write_string(memory, io),
            0x0E => self.int21_select_disk(io),
            0x19 => self.int21_get_current_drive(io),
            0x25 => self.int21_set_interrupt_vector(memory),
            0x30 => self.int21_get_dos_version(),
            0x35 => self.int21_get_interrupt_vector(memory),
            0x37 => self.int21_switch_char(),
            0x39 => self.int21_create_dir(memory, io),
            0x3A => self.int21_remove_dir(memory, io),
            0x3B => self.int21_change_dir(memory, io),
            0x3C => self.int21_create_file(memory, io),
            0x3D => self.int21_open_file(memory, io),
            0x3E => self.int21_close_file(io),
            0x3F => self.int21_read_file(memory, io),
            0x40 => self.int21_write_file(memory, io),
            0x42 => self.int21_seek_file(io),
            0x44 => self.int21_ioctl(io),
            0x45 => self.int21_duplicate_file(io),
            0x47 => self.int21_get_current_dir(memory, io),
            0x48 => self.int21_allocate_memory(io),
            0x49 => self.int21_free_memory(io),
            0x4A => self.int21_resize_memory(io),
            0x4C => self.int21_exit(),
            0x4E => self.int21_find_first(memory, io),
            0x4F => self.int21_find_next(memory, io),
            0x50 => self.int21_set_psp(io),
            _ => {
                warn!("Unhandled INT 0x21 function: AH=0x{:02X}", function);
            }
        }
    }

    /// INT 21h, AH=01h - Read Character from STDIN with Echo
    /// Returns: AL = character read
    fn int21_read_char_with_echo<T: Bios>(&mut self, io: &mut T) {
        if let Some(ch) = io.read_char() {
            // Echo the character
            io.write_char(ch);
            // Store in AL
            self.ax = (self.ax & 0xFF00) | (ch as u16);
        }
    }

    /// INT 21h, AH=02h - Write Character to STDOUT
    /// Input: DL = character to write
    fn int21_write_char<T: Bios>(&mut self, io: &mut T) {
        let ch = self.get_reg8(2); // DL register
        io.write_char(ch);
    }

    /// INT 21h, AH=09h - Write String to STDOUT
    /// Input: DS:DX = pointer to '$'-terminated string
    fn int21_write_string<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let mut addr = Self::physical_address(self.ds, self.dx);
        let mut output = String::new();

        loop {
            let ch = memory.read_byte(addr);
            if ch == b'$' {
                break;
            }
            output.push(ch as char);
            addr += 1;
        }

        io.write_str(&output);
    }

    /// INT 21h, AH=19h - Get Current Default Drive
    /// Output: AL = current drive (0=A, 1=B, etc.)
    fn int21_get_current_drive<T: Bios>(&mut self, io: &T) {
        let drive = io.get_current_drive();
        self.ax = (self.ax & 0xFF00) | (drive as u16);
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

        // Interrupt vector table is at 0000:0000
        // Each entry is 4 bytes: offset (2 bytes) + segment (2 bytes)
        let ivt_addr = (int_num as usize) * 4;

        // Write offset (low word)
        memory.write_byte(ivt_addr, (offset & 0xFF) as u8);
        memory.write_byte(ivt_addr + 1, (offset >> 8) as u8);

        // Write segment (high word)
        memory.write_byte(ivt_addr + 2, (segment & 0xFF) as u8);
        memory.write_byte(ivt_addr + 3, (segment >> 8) as u8);
    }

    /// INT 21h, AH=30h - Get DOS Version
    /// Output:
    ///   AL = major version number
    ///   AH = minor version number
    ///   BL:CX = 24-bit user serial number (usually 0)
    fn int21_get_dos_version(&mut self) {
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
        let offset_low = memory.read_byte(ivt_addr) as u16;
        let offset_high = memory.read_byte(ivt_addr + 1) as u16;
        let offset = (offset_high << 8) | offset_low;

        // Read segment (high word)
        let segment_low = memory.read_byte(ivt_addr + 2) as u16;
        let segment_high = memory.read_byte(ivt_addr + 3) as u16;
        let segment = (segment_high << 8) | segment_low;

        // Return in ES:BX
        self.es = segment;
        self.bx = offset;
    }

    /// INT 21h, AH=4Ch - Exit Program
    /// Input: AL = return code
    fn int21_exit(&mut self) {
        // Halt the CPU
        self.halted = true;
    }

    /// INT 21h, AH=3Ch - Create or Truncate File
    /// Input:
    ///   DS:DX = pointer to null-terminated filename
    ///   CX = file attributes
    /// Output:
    ///   CF clear if success: AX = file handle
    ///   CF set if error: AX = error code
    fn int21_create_file<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
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
    fn int21_open_file<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let filename = self.read_null_terminated_string(memory, self.ds, self.dx);
        let access_mode = (self.ax & 0xFF) as u8;

        match io.file_open(&filename, access_mode) {
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

    /// INT 21h, AH=3Eh - Close File
    /// Input:
    ///   BX = file handle
    /// Output:
    ///   CF clear if success
    ///   CF set if error: AX = error code
    fn int21_close_file<T: Bios>(&mut self, io: &mut T) {
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
    fn int21_read_file<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let handle = self.bx;
        let max_bytes = self.cx;

        match io.file_read(handle, max_bytes) {
            Ok(data) => {
                // Write data to DS:DX
                let buffer_addr = Self::physical_address(self.ds, self.dx);
                for (i, &byte) in data.iter().enumerate() {
                    memory.write_byte(buffer_addr + i, byte);
                }
                self.ax = data.len() as u16;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
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
    fn int21_write_file<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
        let handle = self.bx;
        let num_bytes = self.cx;

        // Read data from DS:DX
        let buffer_addr = Self::physical_address(self.ds, self.dx);
        let mut data = Vec::with_capacity(num_bytes as usize);
        for i in 0..num_bytes {
            data.push(memory.read_byte(buffer_addr + i as usize));
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

    /// INT 21h, AH=42h - Seek (LSEEK)
    /// Input:
    ///   BX = file handle
    ///   AL = seek method (0=from start, 1=from current, 2=from end)
    ///   CX:DX = signed offset (32-bit)
    /// Output:
    ///   CF clear if success: DX:AX = new file position
    ///   CF set if error: AX = error code
    fn int21_seek_file<T: Bios>(&mut self, io: &mut T) {
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
                self.ax = dos_errors::INVALID_FUNCTION as u16;
                self.set_flag(cpu_flag::CARRY, true);
                return;
            }
        };

        match io.file_seek(handle, offset_signed, method) {
            Ok(new_position) => {
                // Return new position in DX:AX
                self.dx = (new_position >> 16) as u16;
                self.ax = (new_position & 0xFFFF) as u16;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
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
    fn int21_duplicate_file<T: Bios>(&mut self, io: &mut T) {
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
    fn int21_create_dir<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
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
    fn int21_remove_dir<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
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
    fn int21_change_dir<T: Bios>(&mut self, memory: &Memory, io: &mut T) {
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

    /// INT 21h, AH=47h - Get Current Directory
    /// Input:
    ///   DL = drive number (0=default, 1=A, 2=B, etc.)
    ///   DS:SI = pointer to 64-byte buffer for directory path
    /// Output:
    ///   CF clear if success: buffer filled with path (without drive or leading backslash)
    ///   CF set if error: AX = error code
    fn int21_get_current_dir<T: Bios>(&mut self, memory: &mut Memory, io: &T) {
        let drive = (self.dx & 0xFF) as u8; // DL

        match io.dir_get_current(drive) {
            Ok(path) => {
                // Write path to DS:SI (null-terminated)
                let buffer_addr = Self::physical_address(self.ds, self.si);
                for (i, &byte) in path.as_bytes().iter().enumerate() {
                    if i >= 63 {
                        break; // Leave room for null terminator
                    }
                    memory.write_byte(buffer_addr + i, byte);
                }
                // Write null terminator
                let len = path.len().min(63);
                memory.write_byte(buffer_addr + len, 0);

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
    fn int21_find_first<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
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
                    memory.write_byte(dta_addr + i, ((search_id >> (i * 8)) & 0xFF) as u8);
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
    fn int21_find_next<T: Bios>(&mut self, memory: &mut Memory, io: &mut T) {
        let dta_addr = Self::physical_address(self.es, self.bx);

        // Read search_id from DTA
        let mut search_id: usize = 0;
        for i in 0..8 {
            search_id |= (memory.read_byte(dta_addr + i) as usize) << (i * 8);
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
        memory.write_byte(dta_addr + 21, find_data.attributes);

        // Offset 22-23: File time (little-endian)
        memory.write_byte(dta_addr + 22, (find_data.time & 0xFF) as u8);
        memory.write_byte(dta_addr + 23, (find_data.time >> 8) as u8);

        // Offset 24-25: File date (little-endian)
        memory.write_byte(dta_addr + 24, (find_data.date & 0xFF) as u8);
        memory.write_byte(dta_addr + 25, (find_data.date >> 8) as u8);

        // Offset 26-29: File size (32-bit little-endian)
        memory.write_byte(dta_addr + 26, (find_data.size & 0xFF) as u8);
        memory.write_byte(dta_addr + 27, ((find_data.size >> 8) & 0xFF) as u8);
        memory.write_byte(dta_addr + 28, ((find_data.size >> 16) & 0xFF) as u8);
        memory.write_byte(dta_addr + 29, ((find_data.size >> 24) & 0xFF) as u8);

        // Offset 30-42: Filename (null-terminated, max 13 bytes)
        let filename_bytes = find_data.filename.as_bytes();
        for (i, &byte) in filename_bytes.iter().take(12).enumerate() {
            memory.write_byte(dta_addr + 30 + i, byte);
        }
        // Null terminator
        let len = filename_bytes.len().min(12);
        memory.write_byte(dta_addr + 30 + len, 0);
    }

    /// INT 21h, AH=0Eh - Select Default Disk
    /// Input:
    ///   DL = drive number (0=A, 1=B, etc.)
    /// Output:
    ///   AL = number of logical drives in system
    fn int21_select_disk<T: Bios>(&mut self, io: &mut T) {
        let drive = (self.dx & 0xFF) as u8; // DL
        let num_drives = io.set_default_drive(drive);
        self.ax = (self.ax & 0xFF00) | (num_drives as u16);
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
    fn int21_ioctl<T: Bios>(&mut self, io: &mut T) {
        let subfunction = (self.ax & 0xFF) as u8; // AL
        let handle = self.bx;

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
                // Return AL=0 if removable, AL=1 if fixed
                // For compatibility, return fixed (most emulated drives are fixed)
                self.ax = (self.ax & 0xFF00) | 0x01;
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
            _ => {
                warn!("Unhandled IOCTL subfunction: AL=0x{:02X}", subfunction);
                self.ax = dos_errors::INVALID_FUNCTION as u16;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=48h - Allocate Memory
    /// Input:
    ///   BX = number of paragraphs (16-byte blocks) to allocate
    /// Output:
    ///   CF clear if success: AX = segment of allocated memory
    ///   CF set if error: AX = error code, BX = size of largest available block
    fn int21_allocate_memory<T: Bios>(&mut self, io: &mut T) {
        let paragraphs = self.bx;

        match io.memory_allocate(paragraphs) {
            Ok(segment) => {
                self.ax = segment;
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err((error_code, max_available)) => {
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
    fn int21_free_memory<T: Bios>(&mut self, io: &mut T) {
        let segment = self.es;

        match io.memory_free(segment) {
            Ok(()) => {
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err(error_code) => {
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
    fn int21_resize_memory<T: Bios>(&mut self, io: &mut T) {
        let segment = self.es;
        let paragraphs = self.bx;

        match io.memory_resize(segment, paragraphs) {
            Ok(()) => {
                self.set_flag(cpu_flag::CARRY, false);
            }
            Err((error_code, max_available)) => {
                self.ax = error_code as u16;
                self.bx = max_available;
                self.set_flag(cpu_flag::CARRY, true);
            }
        }
    }

    /// INT 21h, AH=50h - Set PSP Address
    /// Input:
    ///   BX = segment of new PSP
    /// Output: None
    fn int21_set_psp<T: Bios>(&mut self, io: &mut T) {
        let segment = self.bx;
        io.set_psp(segment);
    }

    /// Helper function to read a null-terminated string from memory
    fn read_null_terminated_string(&self, memory: &Memory, segment: u16, offset: u16) -> String {
        let mut addr = Self::physical_address(segment, offset);
        let mut result = String::new();

        loop {
            let ch = memory.read_byte(addr);
            if ch == 0 {
                break;
            }
            result.push(ch as char);
            addr += 1;
        }

        result
    }
}
