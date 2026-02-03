//! WebAssembly bindings for emu86 8086 emulator.
//!
//! This crate provides browser-based implementations of the emulator's
//! platform-independent traits (KeyboardInput, MouseInput, VideoController, SpeakerOutput)
//! and exposes a JavaScript API for controlling the emulator from web applications.

use emu86_core::{BackedDisk, Computer, DiskGeometry, DriveNumber, MemoryDiskBackend, NullSpeaker};
use wasm_bindgen::prelude::*;
use web_sys::{Document, HtmlCanvasElement, Window};

mod web_keyboard;
mod web_mouse;
mod web_video;

use web_keyboard::WebKeyboard;
use web_mouse::WebMouse;
use web_video::WebVideo;

/// Initialize WASM module (call this first from JavaScript)
#[wasm_bindgen(start)]
pub fn init() {
    // Set panic hook for better error messages in browser console
    console_error_panic_hook::set_once();

    // Initialize logging to browser console
    wasm_logger::init(wasm_logger::Config::default());

    log::info!("emu86 WASM module initialized");
}

/// WASM wrapper for the Computer emulator
#[wasm_bindgen]
pub struct Emu86Computer {
    computer: Computer<WebVideo>,
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
        let document: Document = window
            .document()
            .ok_or_else(|| JsValue::from_str("No document object"))?;

        let canvas = document
            .get_element_by_id(canvas_id)
            .ok_or_else(|| JsValue::from_str(&format!("Canvas {} not found", canvas_id)))?
            .dyn_into::<HtmlCanvasElement>()?;

        let keyboard = Box::new(WebKeyboard::new(&document)?);
        let mouse = Box::new(WebMouse::new(&canvas)?);
        let video = WebVideo::new(canvas)?;
        let speaker = Box::new(NullSpeaker);

        let computer = Computer::new(keyboard, mouse, video, speaker);

        Ok(Self { computer })
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
}
