//! WebAssembly bindings for emu86 8086 emulator.
//!
//! This crate provides browser-based implementations of the emulator's
//! platform-independent traits (KeyboardInput, MouseInput, VideoController, SpeakerOutput)
//! and exposes a JavaScript API for controlling the emulator from web applications.

use emu86_core::{
    BackedDisk, Computer, DiskController, DiskGeometry, DriveNumber, KeyPress, KeyboardInput,
    MemoryDiskBackend, MouseInput, MouseState, NullSpeaker, PartitionedDisk, cpu::bios::FileAccess,
    parse_mbr,
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, Window};

mod web_keyboard;
mod web_mouse;
mod web_speaker;
mod web_video;

use web_keyboard::WebKeyboard;
use web_mouse::WebMouse;
use web_speaker::WebSpeaker;
use web_video::WebVideo;

/// Wrapper around WebKeyboard that shares ownership via Rc<RefCell<>>
/// This allows both the Computer and Emu86Computer to access the keyboard
struct SharedKeyboard(Rc<RefCell<WebKeyboard>>);

impl KeyboardInput for SharedKeyboard {
    fn read_char(&mut self) -> Option<u8> {
        self.0.borrow_mut().read_char()
    }

    fn check_char(&mut self) -> Option<u8> {
        self.0.borrow_mut().check_char()
    }

    fn has_char_available(&self) -> bool {
        self.0.borrow().has_char_available()
    }

    fn read_key(&mut self) -> Option<KeyPress> {
        self.0.borrow_mut().read_key()
    }

    fn check_key(&mut self) -> Option<KeyPress> {
        self.0.borrow_mut().check_key()
    }
}

/// Wrapper around WebMouse that shares ownership via Rc<RefCell<>>
/// This allows both the Computer and Emu86Computer to access the mouse
struct SharedMouse(Rc<RefCell<WebMouse>>);

impl MouseInput for SharedMouse {
    fn get_state(&self) -> MouseState {
        self.0.borrow().get_state()
    }

    fn get_motion(&mut self) -> (i16, i16) {
        self.0.borrow_mut().get_motion()
    }

    fn is_present(&self) -> bool {
        self.0.borrow().is_present()
    }

    fn update_window_size(&mut self, width: f64, height: f64) {
        self.0.borrow_mut().update_window_size(width, height);
    }
}

/// Initialize WASM module (call this first from JavaScript)
#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages in browser console
    console_error_panic_hook::set_once();

    // Initialize logging to browser console (default to info level)
    wasm_logger::init(wasm_logger::Config::new(log::Level::Info));

    log::info!("emu86 WASM module initialized");
}

/// WASM wrapper for the Computer emulator
#[wasm_bindgen]
pub struct Emu86Computer {
    computer: Computer<WebVideo>,
    mouse: Rc<RefCell<WebMouse>>,
    // Performance tracking
    perf_last_update_time: f64,
    perf_last_cycle_count: u64,
    perf_current_mhz: f64,
    perf_update_interval_ms: f64,
}

#[wasm_bindgen]
impl Emu86Computer {
    /// Create a new emulator instance.
    ///
    /// # Arguments
    /// * `canvas_id` - The ID of the canvas element to render to
    #[wasm_bindgen(constructor)]
    pub fn new(canvas_id: &str) -> Result<Emu86Computer, JsValue> {
        let window: Window =
            web_sys::window().ok_or_else(|| JsValue::from_str("No window object"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("No document object"))?;

        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("Canvas {} not found", canvas_id)))?
            .dyn_into::<HtmlCanvasElement>()?;

        // Get canvas dimensions for mouse coordinate scaling
        let canvas_width = canvas.width() as f64;
        let canvas_height = canvas.height() as f64;

        // Create keyboard and mouse
        let keyboard = Rc::new(RefCell::new(WebKeyboard::new()?));
        let mouse = Rc::new(RefCell::new(WebMouse::new(canvas_width, canvas_height)?));

        // Create wrappers for the Computer
        let keyboard_wrapper = Box::new(SharedKeyboard(keyboard));
        let mouse_wrapper = Box::new(SharedMouse(mouse.clone()));

        let video = WebVideo::new(canvas)?;

        // Try to initialize Web Audio API, fall back to NullSpeaker if it fails
        let speaker: Box<dyn emu86_core::SpeakerOutput> = match WebSpeaker::new() {
            Ok(s) => {
                log::info!("Web Audio API initialized successfully");
                Box::new(s)
            }
            Err(e) => {
                log::warn!(
                    "Failed to initialize Web Audio API: {:?}, using NullSpeaker",
                    e
                );
                Box::new(NullSpeaker)
            }
        };

        let mut computer = Computer::new(keyboard_wrapper, mouse_wrapper, video, speaker);

        // Force initial video render to show blank screen
        computer.force_video_redraw();

        Ok(Self {
            computer,
            mouse,
            perf_last_update_time: 0.0,
            perf_last_cycle_count: 0,
            perf_current_mhz: 0.0,
            perf_update_interval_ms: 200.0,
        })
    }

    /// Load a floppy disk image from a byte array.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:)
    /// * `data` - Disk image data as Uint8Array from JavaScript
    #[wasm_bindgen]
    pub fn load_floppy(&mut self, drive: u8, data: Vec<u8>) -> Result<(), JsValue> {
        let geometry = DiskGeometry::from_size(data.len())
            .ok_or_else(|| JsValue::from_str("Invalid floppy disk size"))?;

        if !geometry.is_floppy() {
            return Err(JsValue::from_str("Image size is not a valid floppy disk"));
        }

        let backend = MemoryDiskBackend::new(data);
        let disk = BackedDisk::new(backend).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let drive_number = match drive {
            0 => DriveNumber::floppy_a(),
            1 => DriveNumber::floppy_b(),
            _ => {
                return Err(JsValue::from_str(
                    "Invalid floppy drive number (use 0 or 1)",
                ));
            }
        };

        self.computer
            .bios_mut()
            .insert_floppy(drive_number, Box::new(disk))
            .map_err(|e| JsValue::from_str(&e))?;

        log::info!(
            "Loaded floppy {}: ({} bytes)",
            drive_number.to_letter(),
            geometry.total_size
        );

        Ok(())
    }

    /// Eject a floppy disk.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:)
    #[wasm_bindgen]
    pub fn eject_floppy(&mut self, drive: u8) -> Result<(), JsValue> {
        let drive_number = match drive {
            0 => DriveNumber::floppy_a(),
            1 => DriveNumber::floppy_b(),
            _ => {
                return Err(JsValue::from_str(
                    "Invalid floppy drive number (use 0 or 1)",
                ));
            }
        };

        self.computer
            .bios_mut()
            .eject_floppy(drive_number)
            .map_err(|e| JsValue::from_str(&e))?;

        log::info!("Ejected floppy {}", drive_number.to_letter());
        Ok(())
    }

    /// Load a hard drive image from a byte array.
    ///
    /// # Arguments
    /// * `data` - Disk image data as Uint8Array from JavaScript
    #[wasm_bindgen]
    pub fn add_hard_drive(&mut self, data: Vec<u8>) -> Result<(), JsValue> {
        let geometry = DiskGeometry::from_size(data.len())
            .ok_or_else(|| JsValue::from_str("Invalid hard drive size"))?;

        if geometry.is_floppy() {
            return Err(JsValue::from_str(
                "Image size is too small for a hard drive",
            ));
        }

        let backend = MemoryDiskBackend::new(data.clone());
        let disk = BackedDisk::new(backend).map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Check if disk has MBR and partitions
        let sector_0 = disk.read_sector_lba(0).ok();
        let has_partitions = sector_0
            .as_ref()
            .and_then(parse_mbr)
            .and_then(|parts| parts[0]);

        let drive_number = if let Some(partition) = has_partitions {
            log::info!(
                "Detected MBR: partition 1 at sector {}, {} sectors",
                partition.start_sector,
                partition.sector_count
            );

            // Create raw disk for INT 13h operations (MBR access)
            let raw_backend = MemoryDiskBackend::new(data);
            let raw_disk =
                BackedDisk::new(raw_backend).map_err(|e| JsValue::from_str(&e.to_string()))?;

            // Create partitioned disk for DOS filesystem operations
            let partitioned =
                PartitionedDisk::new(disk, partition.start_sector, partition.sector_count);
            self.computer
                .bios_mut()
                .add_hard_drive_with_partition(Box::new(partitioned), Box::new(raw_disk))
        } else {
            self.computer.bios_mut().add_hard_drive(Box::new(disk))
        };

        log::info!(
            "Added hard drive {}: ({} bytes)",
            drive_number.to_letter(),
            geometry.total_size
        );

        Ok(())
    }

    /// Update performance metrics (called periodically)
    fn update_performance(&mut self, current_time_ms: f64) {
        if current_time_ms - self.perf_last_update_time >= self.perf_update_interval_ms {
            let current_cycles = self.computer.get_cycle_count();
            let cycle_delta = current_cycles.saturating_sub(self.perf_last_cycle_count);
            let time_delta_ms = current_time_ms - self.perf_last_update_time;

            // Calculate instantaneous MHz: cycles / milliseconds / 1000
            let instant_mhz = (cycle_delta as f64) / time_delta_ms / 1000.0;

            // Exponential moving average for smoothing
            if self.perf_current_mhz == 0.0 {
                self.perf_current_mhz = instant_mhz;
            } else {
                self.perf_current_mhz = 0.7 * self.perf_current_mhz + 0.3 * instant_mhz;
            }

            self.perf_last_update_time = current_time_ms;
            self.perf_last_cycle_count = current_cycles;
        }
    }

    /// Load a program into memory and set CPU to start executing it.
    ///
    /// # Arguments
    /// * `data` - Program binary data as Uint8Array from JavaScript
    /// * `segment` - Starting segment address (e.g., 0x0000)
    /// * `offset` - Starting offset address (e.g., 0x0100 for .COM files)
    #[wasm_bindgen]
    pub fn load_program(
        &mut self,
        data: Vec<u8>,
        segment: u16,
        offset: u16,
    ) -> Result<(), JsValue> {
        self.computer
            .load_program(&data, segment, offset)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Force initial video render to show blank screen
        self.computer.force_video_redraw();

        log::info!(
            "Loaded program: {} bytes at {:04X}:{:04X}",
            data.len(),
            segment,
            offset
        );

        Ok(())
    }

    /// Boot from a drive.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0x00 = A:, 0x01 = B:, 0x80 = C:)
    #[wasm_bindgen]
    pub fn boot(&mut self, drive: u8) -> Result<(), JsValue> {
        let drive_number = DriveNumber::from_standard(drive);
        self.computer
            .boot(drive_number)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        // Force initial video render to show blank screen
        self.computer.force_video_redraw();

        Ok(())
    }

    /// Execute one instruction and return whether CPU is still running.
    #[wasm_bindgen]
    pub fn step(&mut self) -> bool {
        self.computer.step();
        self.computer.update_video();
        !self.computer.is_halted()
    }

    /// Execute instructions for approximately the given number of milliseconds.
    ///
    /// # Arguments
    /// * `ms` - Milliseconds to run (approximately)
    /// * `current_time_ms` - Current timestamp from performance.now() in JavaScript
    ///
    /// # Returns
    /// true if CPU is still running, false if halted
    #[wasm_bindgen]
    pub fn run_for_ms(&mut self, ms: f64, current_time_ms: f64) -> bool {
        // Update performance metrics
        self.update_performance(current_time_ms);

        // 8086 at 4.77 MHz: approximately 4770 cycles per ms
        let cycles = (ms * 4770.0) as u64;
        let mut remaining = cycles;

        while remaining > 0 {
            if self.computer.is_halted() {
                // Update video one last time before returning
                self.computer.update_video();
                return false;
            }

            self.computer.step();

            // Rough approximation: assume average instruction takes ~10 cycles
            remaining = remaining.saturating_sub(10);
        }

        // Update video after batch execution
        self.computer.update_video();

        !self.computer.is_halted()
    }

    /// Reset the computer.
    #[wasm_bindgen]
    pub fn reset(&mut self) {
        self.computer.reset();
        log::info!("Computer reset");
    }

    /// Get the target clock rate in MHz (always 4.77 for 8086).
    #[wasm_bindgen]
    pub fn get_target_mhz(&self) -> f64 {
        4.77
    }

    /// Get the actual measured clock rate in MHz.
    #[wasm_bindgen]
    pub fn get_actual_mhz(&self) -> f64 {
        self.perf_current_mhz
    }

    /// Handle keyboard event from JavaScript.
    ///
    /// # Arguments
    /// * `code` - KeyboardEvent.code (e.g., "KeyA", "Enter")
    /// * `key` - KeyboardEvent.key (e.g., "a", "Enter")
    /// * `shift` - Shift key state
    /// * `ctrl` - Control key state
    /// * `alt` - Alt key state
    #[wasm_bindgen]
    pub fn handle_key_event(
        &mut self,
        code: String,
        key: String,
        shift: bool,
        ctrl: bool,
        alt: bool,
    ) {
        // Convert the keyboard event to a KeyPress and queue it for IRQ processing
        // Don't add to WebKeyboard buffer - let INT 09h handle it
        if let Some(key_press) = web_keyboard::event_to_keypress(&code, &key, shift, ctrl, alt) {
            self.computer.process_keyboard_irq(key_press);
        }
    }

    /// Handle mouse move event from JavaScript.
    ///
    /// # Arguments
    /// * `offset_x` - Mouse X coordinate relative to canvas
    /// * `offset_y` - Mouse Y coordinate relative to canvas
    #[wasm_bindgen]
    pub fn handle_mouse_move(&mut self, offset_x: f64, offset_y: f64) {
        self.mouse
            .borrow_mut()
            .inject_mouse_move(offset_x, offset_y);
    }

    /// Handle mouse movement delta from JavaScript (for pointer lock mode).
    ///
    /// When the pointer is locked, use movementX/movementY from the browser
    /// to inject relative mouse movement without absolute position.
    ///
    /// # Arguments
    /// * `delta_x` - Horizontal movement in pixels
    /// * `delta_y` - Vertical movement in pixels
    #[wasm_bindgen]
    pub fn handle_mouse_delta(&mut self, delta_x: f64, delta_y: f64) {
        self.mouse.borrow_mut().inject_mouse_delta(delta_x, delta_y);
    }

    /// Handle mouse button event from JavaScript.
    ///
    /// # Arguments
    /// * `button` - Button number (0=left, 1=middle, 2=right)
    /// * `pressed` - true for mousedown, false for mouseup
    #[wasm_bindgen]
    pub fn handle_mouse_button(&mut self, button: u8, pressed: bool) {
        self.mouse.borrow_mut().inject_mouse_button(button, pressed);
    }

    /// Attach a serial mouse to COM1.
    ///
    /// This enables Microsoft Serial Mouse protocol on COM1 (typically at 1200 baud, 7N1).
    /// Programs like CTMOUSE.EXE and CUTE.COM will detect the mouse on this port.
    #[wasm_bindgen]
    pub fn attach_serial_mouse_com1(&mut self) {
        use emu86_core::SerialMouse;
        let mouse_clone =
            Box::new(SharedMouse(self.mouse.clone())) as Box<dyn emu86_core::MouseInput>;
        self.computer
            .set_com1_device(Box::new(SerialMouse::new(mouse_clone)));
        log::info!("Serial mouse attached to COM1");
    }

    /// Attach a serial mouse to COM2.
    ///
    /// This enables Microsoft Serial Mouse protocol on COM2 (typically at 1200 baud, 7N1).
    /// Programs like CTMOUSE.EXE and CUTE.COM will detect the mouse on this port.
    #[wasm_bindgen]
    pub fn attach_serial_mouse_com2(&mut self) {
        use emu86_core::SerialMouse;
        let mouse_clone =
            Box::new(SharedMouse(self.mouse.clone())) as Box<dyn emu86_core::MouseInput>;
        self.computer
            .set_com2_device(Box::new(SerialMouse::new(mouse_clone)));
        log::info!("Serial mouse attached to COM2");
    }

    /// Get floppy disk data as a byte array for download.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:)
    ///
    /// # Returns
    /// Complete disk image as Vec<u8>
    #[wasm_bindgen]
    pub fn get_floppy_data(&self, drive: u8) -> Result<Vec<u8>, JsValue> {
        let drive_number = match drive {
            0 => DriveNumber::floppy_a(),
            1 => DriveNumber::floppy_b(),
            _ => {
                return Err(JsValue::from_str(
                    "Invalid floppy drive number (use 0 or 1)",
                ));
            }
        };

        let bios = self.computer.bios();
        let disk = bios
            .shared
            .drive_manager
            .get_floppy_disk(drive_number)
            .ok_or_else(|| JsValue::from_str("No disk in drive"))?;

        let geometry = disk.geometry();
        let total_sectors = geometry.total_sectors();
        let total_size = geometry.total_size;

        let mut data = Vec::with_capacity(total_size);

        for sector in 0..total_sectors {
            let sector_data = disk.read_sector_lba(sector).map_err(|e| {
                JsValue::from_str(&format!("Failed to read sector {}: {}", sector, e))
            })?;
            data.extend_from_slice(&sector_data);
        }

        log::info!(
            "Downloaded floppy {}: {} bytes",
            drive_number.to_letter(),
            data.len()
        );
        Ok(data)
    }

    /// Get hard drive disk data as a byte array for download.
    ///
    /// # Arguments
    /// * `drive_index` - Hard drive index (0 = C:, 1 = D:, etc.)
    ///
    /// # Returns
    /// Complete disk image as Vec<u8>
    #[wasm_bindgen]
    pub fn get_hard_drive_data(&self, drive_index: u8) -> Result<Vec<u8>, JsValue> {
        let drive_number = DriveNumber::from_standard(0x80 + drive_index);

        let bios = self.computer.bios();
        let disk = bios
            .shared
            .drive_manager
            .get_hard_drive_disk(drive_number)
            .ok_or_else(|| JsValue::from_str("No disk in drive"))?;

        let geometry = disk.geometry();
        let total_sectors = geometry.total_sectors();
        let total_size = geometry.total_size;

        let mut data = Vec::with_capacity(total_size);

        for sector in 0..total_sectors {
            let sector_data = disk.read_sector_lba(sector).map_err(|e| {
                JsValue::from_str(&format!("Failed to read sector {}: {}", sector, e))
            })?;
            data.extend_from_slice(&sector_data);
        }

        log::info!(
            "Downloaded hard drive {}: {} bytes",
            drive_number.to_letter(),
            data.len()
        );
        Ok(data)
    }

    /// List directory contents.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:, 0x80 = C:, 0x81 = D:, etc.)
    /// * `path` - Directory path (e.g., "/" or "/SUBDIR")
    ///
    /// # Returns
    /// JSON array of file entries with name, size, isDirectory, date, time, attributes
    #[wasm_bindgen]
    pub fn list_directory(&mut self, drive: u8, path: String) -> Result<JsValue, JsValue> {
        use serde::Serialize;

        #[derive(Serialize)]
        struct FileEntry {
            name: String,
            size: u32,
            #[serde(rename = "isDirectory")]
            is_directory: bool,
            date: String,
            time: String,
            attributes: u8,
        }

        // Helper to unpack DOS date (bits: YYYYYYYMMMMDDDDD)
        fn unpack_dos_date(packed: u16) -> String {
            let year = 1980 + ((packed >> 9) & 0x7F);
            let month = (packed >> 5) & 0x0F;
            let day = packed & 0x1F;
            format!("{:04}-{:02}-{:02}", year, month, day)
        }

        // Helper to unpack DOS time (bits: HHHHHMMMMMMSS SSS)
        fn unpack_dos_time(packed: u16) -> String {
            let hour = (packed >> 11) & 0x1F;
            let minute = (packed >> 5) & 0x3F;
            let second = (packed & 0x1F) * 2;
            format!("{:02}:{:02}:{:02}", hour, minute, second)
        }

        let drive_number = DriveNumber::from_standard(drive);

        // Build DOS path (e.g., "C:\*.*" or "C:\SUBDIR\*.*")
        let drive_letter = drive_number.to_letter();
        let dos_path = if path == "/" || path.is_empty() {
            format!("{}:\\*.*", drive_letter)
        } else {
            let clean_path = path.trim_start_matches('/').replace('/', "\\");
            format!("{}:\\{}\\*.*", drive_letter, clean_path)
        };

        let bios = self.computer.bios_mut();

        // Use find_first to start the search (attributes: 0x16 = directories + hidden + system)
        let (handle, find_data) = bios
            .find_first(&dos_path, 0x16)
            .map_err(|e| JsValue::from_str(&format!("Failed to list directory: {}", e)))?;

        let mut entries = Vec::new();

        // Add first entry
        entries.push(FileEntry {
            name: find_data.filename.clone(),
            size: find_data.size,
            is_directory: find_data.attributes & 0x10 != 0,
            date: unpack_dos_date(find_data.date),
            time: unpack_dos_time(find_data.time),
            attributes: find_data.attributes,
        });

        // Continue with find_next
        while let Ok(find_data) = bios.find_next(handle) {
            entries.push(FileEntry {
                name: find_data.filename.clone(),
                size: find_data.size,
                is_directory: find_data.attributes & 0x10 != 0,
                date: unpack_dos_date(find_data.date),
                time: unpack_dos_time(find_data.time),
                attributes: find_data.attributes,
            });
        }

        // Convert to JsValue using serde-wasm-bindgen
        serde_wasm_bindgen::to_value(&entries).map_err(|e| {
            JsValue::from_str(&format!("Failed to serialize directory listing: {}", e))
        })
    }

    /// Read a file from disk.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:, 0x80 = C:, 0x81 = D:, etc.)
    /// * `path` - File path (e.g., "/README.TXT" or "/SUBDIR/FILE.DAT")
    ///
    /// # Returns
    /// File contents as Vec<u8>
    #[wasm_bindgen]
    pub fn read_file_from_disk(&mut self, drive: u8, path: String) -> Result<Vec<u8>, JsValue> {
        let drive_number = DriveNumber::from_standard(drive);

        // Build DOS path (e.g., "C:\README.TXT")
        let drive_letter = drive_number.to_letter();
        let clean_path = path.trim_start_matches('/').replace('/', "\\");
        let dos_path = format!("{}:\\{}", drive_letter, clean_path);

        let bios = self.computer.bios_mut();

        // Open file for reading
        let handle = bios
            .file_open(&dos_path, FileAccess::ReadOnly)
            .map_err(|e| JsValue::from_str(&format!("Failed to open file: {}", e)))?;

        // Read file in chunks
        let mut data = Vec::new();
        let chunk_size = 32768; // 32KB chunks

        loop {
            let chunk = bios.file_read(handle, chunk_size).map_err(|e| {
                bios.file_close(handle).ok();
                JsValue::from_str(&format!("Failed to read file: {}", e))
            })?;

            if chunk.is_empty() {
                break;
            }

            data.extend_from_slice(&chunk);
        }

        // Close file
        bios.file_close(handle)
            .map_err(|e| JsValue::from_str(&format!("Failed to close file: {}", e)))?;

        log::info!("Read file {}: {} bytes", dos_path, data.len());
        Ok(data)
    }

    /// Write a file to disk.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:, 0x80 = C:, 0x81 = D:, etc.)
    /// * `path` - File path (e.g., "/README.TXT" or "/SUBDIR/FILE.DAT")
    /// * `data` - File contents as Vec<u8>
    #[wasm_bindgen]
    pub fn write_file_to_disk(
        &mut self,
        drive: u8,
        path: String,
        data: Vec<u8>,
    ) -> Result<(), JsValue> {
        let drive_number = DriveNumber::from_standard(drive);

        // Build DOS path (e.g., "C:\README.TXT")
        let drive_letter = drive_number.to_letter();
        let clean_path = path.trim_start_matches('/').replace('/', "\\");
        let dos_path = format!("{}:\\{}", drive_letter, clean_path);

        // Create parent directories if needed
        if let Some(parent_idx) = dos_path.rfind('\\') {
            let parent_path = &dos_path[..parent_idx];
            if parent_path.len() > 2 {
                // More than just "C:"
                let bios = self.computer.bios_mut();
                // Try to create the directory (ignore error if it already exists)
                bios.dir_create(parent_path).ok();
            }
        }

        let bios = self.computer.bios_mut();

        // Create file (0x00 = normal file attributes)
        let handle = bios
            .file_create(&dos_path, 0x00)
            .map_err(|e| JsValue::from_str(&format!("Failed to create file: {}", e)))?;

        // Write file in chunks
        let chunk_size = 32768; // 32KB chunks
        let mut offset = 0;

        while offset < data.len() {
            let end = (offset + chunk_size).min(data.len());
            let chunk = &data[offset..end];

            bios.file_write(handle, chunk).map_err(|e| {
                bios.file_close(handle).ok();
                JsValue::from_str(&format!("Failed to write file: {}", e))
            })?;

            offset = end;
        }

        // Close file
        bios.file_close(handle)
            .map_err(|e| JsValue::from_str(&format!("Failed to close file: {}", e)))?;

        log::info!("Wrote file {}: {} bytes", dos_path, data.len());
        Ok(())
    }

    /// Create a directory on disk.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:, 0x80 = C:, 0x81 = D:, etc.)
    /// * `path` - Directory path (e.g., "/NEWDIR" or "/PARENT/CHILD")
    #[wasm_bindgen]
    pub fn create_directory_on_disk(&mut self, drive: u8, path: String) -> Result<(), JsValue> {
        let drive_number = DriveNumber::from_standard(drive);

        // Build DOS path (e.g., "C:\NEWDIR")
        let drive_letter = drive_number.to_letter();
        let clean_path = path.trim_start_matches('/').replace('/', "\\");
        let dos_path = format!("{}:\\{}", drive_letter, clean_path);

        let bios = self.computer.bios_mut();

        bios.dir_create(&dos_path)
            .map_err(|e| JsValue::from_str(&format!("Failed to create directory: {}", e)))?;

        log::info!("Created directory {}", dos_path);
        Ok(())
    }

    /// Delete a file or directory from disk.
    ///
    /// # Arguments
    /// * `drive` - Drive number (0 = A:, 1 = B:, 0x80 = C:, 0x81 = D:, etc.)
    /// * `path` - File or directory path (e.g., "/README.TXT" or "/OLDDIR")
    #[wasm_bindgen]
    pub fn delete_from_disk(&mut self, drive: u8, path: String) -> Result<(), JsValue> {
        let drive_number = DriveNumber::from_standard(drive);

        // Build DOS path (e.g., "C:\README.TXT")
        let drive_letter = drive_number.to_letter();
        let clean_path = path.trim_start_matches('/').replace('/', "\\");
        let dos_path = format!("{}:\\{}", drive_letter, clean_path);

        let bios = self.computer.bios_mut();

        // Note: File deletion is not yet implemented in the emulator
        // For now, we only support directory deletion
        bios.dir_remove(&dos_path)
            .map_err(|e| JsValue::from_str(&format!("Failed to delete directory: {}. Note: File deletion is not yet supported, only directory deletion.", e)))?;

        log::info!("Deleted directory {}", dos_path);
        Ok(())
    }
}
