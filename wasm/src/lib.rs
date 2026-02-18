//! WebAssembly bindings for emu86 8086 emulator.
//!
//! This crate provides browser-based implementations of the emulator's
//! platform-independent traits (KeyboardInput, MouseInput, VideoController, SpeakerOutput)
//! and exposes a JavaScript API for controlling the emulator from web applications.

use emu86_core::{
    BackedDisk, Computer, DiskController, DiskGeometry, DriveNumber, JoystickInput, KeyPress,
    KeyboardInput, MemoryDiskBackend, MouseInput, MouseState, NullSpeaker, PartitionedDisk,
    SECTOR_SIZE, cpu::bios::FileAccess, create_formatted_disk, parse_mbr,
};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, Window};

mod clock;
mod web_joystick;
mod web_keyboard;
mod web_mouse;
mod web_speaker;
mod web_video;

use web_joystick::WebJoystick;
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

/// Wrapper around WebJoystick that shares ownership via Rc<RefCell<>>
/// This allows both the Computer and Emu86Computer to access the joystick
struct SharedJoystick(Rc<RefCell<WebJoystick>>);

impl JoystickInput for SharedJoystick {
    fn get_axis(&self, joystick: u8, axis: u8) -> f32 {
        self.0.borrow().get_axis(joystick, axis)
    }

    fn get_button(&self, joystick: u8, button: u8) -> bool {
        self.0.borrow().get_button(joystick, button)
    }

    fn is_connected(&self, joystick: u8) -> bool {
        self.0.borrow().is_connected(joystick)
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

/// Configuration for creating a new emulator instance.
///
/// Pass a plain JS object: `{ canvas_id, cpu_type?, memory_kb?, clock_mhz?, video_card?, com1_device?, com2_device? }`.
/// Only `canvas_id` is required; all other fields fall back to defaults.
#[derive(serde::Deserialize, tsify::Tsify, Default)]
#[tsify(from_wasm_abi)]
pub struct ComputerConfig {
    /// ID of the canvas element to render to (required)
    pub canvas_id: String,
    /// CPU type: "8086", "286", "386", or "486" (default: "8086")
    #[serde(default)]
    pub cpu_type: Option<String>,
    /// Memory size in KB (default: 640)
    #[serde(default)]
    pub memory_kb: Option<u32>,
    /// Target clock speed in MHz; 0.0 = unlimited (default: 4.77)
    #[serde(default)]
    pub clock_mhz: Option<f64>,
    /// Video card type: "cga", "ega", or "vga" (default: "ega")
    #[serde(default)]
    pub video_card: Option<String>,
    /// COM1 device: "mouse", "logger", or "null" (default: "mouse")
    #[serde(default)]
    pub com1_device: Option<String>,
    /// COM2 device: "mouse", "logger", or "null" (default: "null")
    #[serde(default)]
    pub com2_device: Option<String>,
    /// Enable PC speaker / audio output (default: true)
    #[serde(default)]
    pub audio_enabled: Option<bool>,
}

/// WASM wrapper for the Computer emulator
#[wasm_bindgen]
pub struct Emu86Computer {
    computer: Computer<WebVideo>,
    mouse: Rc<RefCell<WebMouse>>,
    joystick: Rc<RefCell<WebJoystick>>,
    // Performance tracking
    perf_last_update_time: f64,
    perf_last_cycle_count: u64,
    perf_current_mhz: f64,
    perf_update_interval_ms: f64,
    // Configuration
    target_mhz: f64,
}

#[wasm_bindgen]
impl Emu86Computer {
    /// Create a new emulator instance with custom configuration.
    ///
    /// # Arguments
    /// * `config` - Configuration object with canvas_id and optional settings
    #[wasm_bindgen(constructor)]
    pub fn new(config: ComputerConfig) -> Result<Emu86Computer, JsValue> {
        let canvas_id = config.canvas_id.as_str();
        let cpu_type_str = config.cpu_type.unwrap_or_else(|| "8086".to_string());
        let cpu_type = cpu_type_str.as_str();
        let memory_kb = config.memory_kb.unwrap_or(640);
        let clock_mhz = config.clock_mhz.unwrap_or(4.77);
        let video_card_str = config.video_card.unwrap_or_else(|| "ega".to_string());
        let video_card = video_card_str.as_str();
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

        // Create keyboard, mouse, and joystick
        let keyboard = Rc::new(RefCell::new(WebKeyboard::new()?));
        let mouse = Rc::new(RefCell::new(WebMouse::new(canvas_width, canvas_height)?));
        let joystick = Rc::new(RefCell::new(WebJoystick::new()));

        // Create wrappers for the Computer
        let keyboard_wrapper = Box::new(SharedKeyboard(keyboard));
        let mouse_wrapper = Box::new(SharedMouse(mouse.clone()));
        let joystick_wrapper = Box::new(SharedJoystick(joystick.clone()));

        let video = WebVideo::new(canvas)?;

        // Try to initialize Web Audio API, fall back to NullSpeaker if disabled or unavailable
        let audio_enabled = config.audio_enabled.unwrap_or(true);
        let speaker: Box<dyn emu86_core::SpeakerOutput> = if !audio_enabled {
            log::info!("PC speaker disabled (audio_enabled: false)");
            Box::new(NullSpeaker)
        } else {
            match WebSpeaker::new() {
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
            }
        };

        let resolved_cpu_type = match cpu_type {
            "286" => emu86_core::CpuType::I80286,
            "386" => emu86_core::CpuType::I80386,
            "486" => emu86_core::CpuType::I80486,
            _ => emu86_core::CpuType::I8086,
        };

        let resolved_video_card = emu86_core::VideoCardType::parse(video_card).unwrap_or_default();

        // Clamp memory: 256 KB minimum, 64 MB maximum (extended memory requires 286+)
        let memory_kb = memory_kb.clamp(256, 65536);

        log::info!(
            "Initializing emulator: CPU={}, memory={}KB, clock={} MHz, video={}",
            cpu_type,
            memory_kb,
            if clock_mhz == 0.0 {
                "unlimited".to_string()
            } else {
                format!("{}", clock_mhz)
            },
            video_card,
        );

        let clock = Box::new(clock::WasmClock);
        let mut computer = Computer::new(
            keyboard_wrapper,
            mouse_wrapper,
            joystick_wrapper,
            clock,
            video,
            speaker,
            emu86_core::ComputerConfig {
                cpu_type: resolved_cpu_type,
                memory_kb,
                video_card_type: resolved_video_card,
            },
        );

        // Configure COM ports based on configuration
        let com1_device_str = config.com1_device.unwrap_or_else(|| "mouse".to_string());
        let com2_device_str = config.com2_device.unwrap_or_else(|| "null".to_string());

        // Attach devices to COM1
        if com1_device_str == "mouse" {
            use emu86_core::SerialMouse;
            let mouse_clone =
                Box::new(SharedMouse(mouse.clone())) as Box<dyn emu86_core::MouseInput>;
            computer.set_com1_device(Box::new(SerialMouse::new(mouse_clone)));
            log::info!("Serial mouse attached to COM1");
        } else if com1_device_str == "logger" {
            use emu86_core::SerialLogger;
            computer.set_com1_device(Box::new(SerialLogger::new(0)));
            log::info!("Serial logger attached to COM1");
        }

        // Attach devices to COM2
        if com2_device_str == "mouse" {
            use emu86_core::SerialMouse;
            let mouse_clone =
                Box::new(SharedMouse(mouse.clone())) as Box<dyn emu86_core::MouseInput>;
            computer.set_com2_device(Box::new(SerialMouse::new(mouse_clone)));
            log::info!("Serial mouse attached to COM2");
        } else if com2_device_str == "logger" {
            use emu86_core::SerialLogger;
            computer.set_com2_device(Box::new(SerialLogger::new(1)));
            log::info!("Serial logger attached to COM2");
        }

        // Force initial video render to show blank screen
        computer.force_video_redraw();

        Ok(Self {
            computer,
            mouse,
            joystick,
            perf_last_update_time: 0.0,
            perf_last_cycle_count: 0,
            perf_current_mhz: 0.0,
            perf_update_interval_ms: 200.0,
            target_mhz: clock_mhz,
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

        // MemoryDiskBackend now uses Rc<RefCell<>> internally, so cloning shares the data
        let backend = MemoryDiskBackend::new(data);
        let disk =
            BackedDisk::new(backend.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?;

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
            let raw_disk =
                BackedDisk::new(backend.clone()).map_err(|e| JsValue::from_str(&e.to_string()))?;

            // Create partitioned disk for DOS filesystem operations
            // Both views share the same underlying data via Rc<RefCell<>>
            let partition_disk =
                BackedDisk::new(backend).map_err(|e| JsValue::from_str(&e.to_string()))?;
            let partitioned = PartitionedDisk::new(
                partition_disk,
                partition.start_sector,
                partition.sector_count,
            );

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

        // Update BDA hard drive count so BIOS knows the drive exists
        self.computer.update_bda_hard_drive_count();

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

        // target_mhz * 1000 = cycles per ms
        let cycles = (ms * self.target_mhz * 1000.0) as u64;
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

    /// Get the target clock rate in MHz.
    #[wasm_bindgen]
    pub fn get_target_mhz(&self) -> f64 {
        self.target_mhz
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
    /// * `pressed` - true for keydown, false for keyup
    #[wasm_bindgen]
    pub fn handle_key_event(
        &mut self,
        code: String,
        key: String,
        shift: bool,
        ctrl: bool,
        alt: bool,
        pressed: bool,
    ) {
        // Convert the keyboard event to a KeyPress and queue it for IRQ processing
        // Don't add to WebKeyboard buffer - let INT 09h handle it
        if let Some(mut key_press) = web_keyboard::event_to_keypress(&code, &key, shift, ctrl, alt)
        {
            // For key release, set bit 7 of scan code and clear ASCII code
            if !pressed {
                key_press.scan_code |= 0x80;
                key_press.ascii_code = 0;
            }
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

        // Try to delete as a file first
        if let Ok(()) = bios.file_delete(&dos_path) {
            log::info!("Deleted file {}", dos_path);
            return Ok(());
        }

        // If file deletion fails, try directory deletion
        bios.dir_remove(&dos_path)
            .map_err(|e| JsValue::from_str(&format!("Failed to delete: {}", e)))?;

        log::info!("Deleted directory {}", dos_path);
        Ok(())
    }

    /// Update joystick axis value (called from JavaScript gamepad polling)
    ///
    /// # Arguments
    /// * `joystick` - Joystick slot (0 = A, 1 = B)
    /// * `axis` - Axis number (0 = X, 1 = Y)
    /// * `value` - Normalized value 0.0-1.0 (center = 0.5)
    ///
    /// JavaScript should normalize gamepad axes from -1..1 to 0..1:
    /// ```javascript
    /// const x = (gamepad.axes[0] + 1) / 2;
    /// computer.handle_gamepad_axis(0, 0, x);
    /// ```
    pub fn handle_gamepad_axis(&mut self, joystick: u8, axis: u8, value: f32) {
        self.joystick
            .borrow_mut()
            .handle_gamepad_axis(joystick, axis, value);
    }

    /// Update joystick button state (called from JavaScript gamepad polling)
    ///
    /// # Arguments
    /// * `joystick` - Joystick slot (0 = A, 1 = B)
    /// * `button` - Button number (0 = button 1, 1 = button 2)
    /// * `pressed` - true if button is pressed, false if released
    ///
    /// JavaScript example:
    /// ```javascript
    /// computer.handle_gamepad_button(0, 0, gamepad.buttons[0].pressed);
    /// ```
    pub fn handle_gamepad_button(&mut self, joystick: u8, button: u8, pressed: bool) {
        self.joystick
            .borrow_mut()
            .handle_gamepad_button(joystick, button, pressed);
    }

    /// Set joystick connection state (called from JavaScript)
    ///
    /// # Arguments
    /// * `joystick` - Joystick slot (0 = A, 1 = B)
    /// * `connected` - true if gamepad is connected, false if disconnected
    ///
    /// JavaScript should call this when gamepads connect/disconnect:
    /// ```javascript
    /// window.addEventListener('gamepadconnected', (e) => {
    ///     computer.gamepad_connected(e.gamepad.index, true);
    /// });
    /// ```
    pub fn gamepad_connected(&mut self, joystick: u8, connected: bool) {
        self.joystick
            .borrow_mut()
            .gamepad_connected(joystick, connected);
    }
}

/// Create a blank FAT-formatted floppy disk image.
///
/// # Arguments
/// * `size_kb` - Floppy size in KB: 1440, 720, 360, or 160
/// * `label` - Optional volume label (up to 11 characters)
///
/// Returns the disk image as a Uint8Array.
#[wasm_bindgen]
pub fn create_floppy_image(size_kb: u32, label: Option<String>) -> Result<Vec<u8>, JsValue> {
    let geometry = match size_kb {
        1440 => DiskGeometry::FLOPPY_1440K,
        720 => DiskGeometry::FLOPPY_720K,
        360 => DiskGeometry::FLOPPY_360K,
        160 => DiskGeometry::FLOPPY_160K,
        _ => {
            return Err(JsValue::from_str(
                "Unsupported floppy size (use 1440, 720, 360, or 160)",
            ));
        }
    };
    create_formatted_disk(geometry, label.as_deref()).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// Create a blank FAT-formatted hard drive image with an MBR partition table.
///
/// # Arguments
/// * `size_mb` - Drive size in MB (minimum 2)
/// * `label` - Optional volume label (up to 11 characters)
///
/// Returns the disk image as a Uint8Array.
#[wasm_bindgen]
pub fn create_hdd_image(size_mb: u32, label: Option<String>) -> Result<Vec<u8>, JsValue> {
    if size_mb < 2 {
        return Err(JsValue::from_str("HDD size must be at least 2MB"));
    }
    let total_sectors = (size_mb as usize * 1024 * 1024) / SECTOR_SIZE;
    let geometry = DiskGeometry::hard_drive(total_sectors);
    create_formatted_disk(geometry, label.as_deref()).map_err(|e| JsValue::from_str(&e.to_string()))
}
