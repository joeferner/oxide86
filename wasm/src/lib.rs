//! WebAssembly bindings for emu86 8086 emulator.
//!
//! This crate provides browser-based implementations of the emulator's
//! platform-independent traits (KeyboardInput, MouseInput, VideoController, SpeakerOutput)
//! and exposes a JavaScript API for controlling the emulator from web applications.

use emu86_core::{
    BackedDisk, Computer, DiskGeometry, DriveNumber, KeyPress, KeyboardInput, MemoryDiskBackend,
    MouseInput, MouseState, NullSpeaker,
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

        let computer = Computer::new(keyboard_wrapper, mouse_wrapper, video, speaker);

        Ok(Self { computer, mouse })
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

        let backend = MemoryDiskBackend::new(data);
        let disk = BackedDisk::new(backend).map_err(|e| JsValue::from_str(&e.to_string()))?;

        let drive_number = self.computer.bios_mut().add_hard_drive(Box::new(disk));
        log::info!(
            "Added hard drive {}: ({} bytes)",
            drive_number.to_letter(),
            geometry.total_size
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
            .map_err(|e| JsValue::from_str(&e.to_string()))
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
    ///
    /// # Returns
    /// true if CPU is still running, false if halted
    #[wasm_bindgen]
    pub fn run_for_ms(&mut self, ms: f64) -> bool {
        // 8086 at 4.77 MHz: approximately 4770 cycles per ms
        let cycles = (ms * 4770.0) as u64;
        let mut remaining = cycles;

        while remaining > 0 {
            if self.computer.is_halted() {
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

    /// Handle mouse button event from JavaScript.
    ///
    /// # Arguments
    /// * `button` - Button number (0=left, 1=middle, 2=right)
    /// * `pressed` - true for mousedown, false for mouseup
    #[wasm_bindgen]
    pub fn handle_mouse_button(&mut self, button: u8, pressed: bool) {
        self.mouse.borrow_mut().inject_mouse_button(button, pressed);
    }
}
