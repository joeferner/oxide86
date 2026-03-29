use std::sync::{Arc, RwLock};

use js_sys::Uint8Array;
use oxide86_core::{
    computer::{Computer, ComputerConfig},
    cpu::CpuType,
    devices::{
        clock::{EmulatedClock, LocalDate, LocalTime},
        pc_speaker::NullPcSpeaker,
    },
    disk::{BackedDisk, Disk, DriveNumber, MemBackend},
    video::{VideoBuffer, VideoCardType},
};
use serde::{Deserialize, Serialize};
use tsify::Tsify;
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, ImageData};

#[derive(Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
pub struct WasmComputerConfig {
    pub cpu_type: String,
    pub has_fpu: bool,
    pub memory_kb: u32,
    pub clock_hz: u32,
    pub video_card: String,
    /// Full year, e.g. 1990
    pub start_year: u16,
    pub start_month: u8,
    pub start_day: u8,
    pub start_hour: u8,
    pub start_minute: u8,
    pub start_second: u8,
}

#[derive(Serialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct RunResult {
    pub halted: bool,
    pub exit_code: Option<u8>,
    pub cycles_executed: u32,
}

struct ComputerState {
    computer: Computer,
    video_buffer: Arc<RwLock<VideoBuffer>>,
}

impl ComputerState {
    fn create(config: &WasmComputerConfig, hdd_data: Option<&[u8]>) -> Result<Self, String> {
        let cpu_type = CpuType::parse(&config.cpu_type)
            .ok_or_else(|| format!("Invalid cpu_type: {}", config.cpu_type))?;

        let video_card_type = VideoCardType::parse(&config.video_card)
            .ok_or_else(|| format!("Invalid video_card: {}", config.video_card))?;

        let clock_hz = if config.clock_hz == 0 {
            4_772_727u32
        } else {
            config.clock_hz
        };

        let start_date = LocalDate {
            century: (config.start_year / 100) as u8,
            year: (config.start_year % 100) as u8,
            month: config.start_month,
            day: config.start_day,
        };
        let start_time = LocalTime {
            hours: config.start_hour,
            minutes: config.start_minute,
            seconds: config.start_second,
            milliseconds: 0,
        };
        let clock = Box::new(EmulatedClock::new(clock_hz as u64, start_date, start_time));

        let memory_size = (config.memory_kb as usize).max(64) * 1024;
        let video_buffer = Arc::new(RwLock::new(VideoBuffer::new()));

        let mut hard_disks: Vec<Box<dyn Disk>> = Vec::new();
        if let Some(data) = hdd_data {
            match BackedDisk::new(MemBackend::from_data(data.to_vec())) {
                Ok(disk) => hard_disks.push(Box::new(disk)),
                Err(e) => return Err(format!("Invalid HDD image: {e}")),
            }
        }

        let computer = Computer::new(ComputerConfig {
            cpu_type,
            clock_speed: clock_hz,
            memory_size,
            clock,
            hard_disks,
            video_card_type,
            video_buffer: Arc::clone(&video_buffer),
            pc_speaker: Box::new(NullPcSpeaker::new()),
            math_coprocessor: config.has_fpu,
        });

        Ok(Self {
            computer,
            video_buffer,
        })
    }
}

#[wasm_bindgen]
pub struct Oxide86Computer {
    config: WasmComputerConfig,
    state: Option<ComputerState>,
    last_error: Option<String>,
    last_cycle_count: u64,
    last_timestamp_ms: f64,
    floppy_a_data: Option<Vec<u8>>,
    floppy_b_data: Option<Vec<u8>>,
    hdd_data: Option<Vec<u8>>,
    boot_drive: Option<String>,
    frame_buf: Vec<u8>,
}

#[wasm_bindgen]
impl Oxide86Computer {
    #[wasm_bindgen(constructor)]
    pub fn new(config: WasmComputerConfig) -> Result<Self, JsValue> {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::new(log::Level::Debug));

        // Validate config eagerly so JS gets a clear error at construction time.
        CpuType::parse(&config.cpu_type)
            .ok_or_else(|| JsValue::from_str(&format!("Invalid cpu_type: {}", config.cpu_type)))?;
        VideoCardType::parse(&config.video_card).ok_or_else(|| {
            JsValue::from_str(&format!("Invalid video_card: {}", config.video_card))
        })?;

        Ok(Self {
            config,
            state: None,
            last_error: None,
            last_cycle_count: 0,
            last_timestamp_ms: js_sys::Date::now(),
            floppy_a_data: None,
            floppy_b_data: None,
            hdd_data: None,
            boot_drive: None,
            frame_buf: Vec::new(),
        })
    }

    pub fn power_on(
        &mut self,
        hdd_image: Option<Uint8Array>,
        floppy_a_image: Option<Uint8Array>,
        floppy_b_image: Option<Uint8Array>,
        boot_drive: Option<String>,
    ) {
        self.hdd_data = hdd_image.map(|a| a.to_vec());
        self.floppy_a_data = floppy_a_image.map(|a| a.to_vec());
        self.floppy_b_data = floppy_b_image.map(|a| a.to_vec());
        self.boot_drive = boot_drive;
        self.state = None;
        let hdd = self.hdd_data.clone();
        let floppy_a = self.floppy_a_data.clone();
        let floppy_b = self.floppy_b_data.clone();
        self.start_computer(hdd, floppy_a, floppy_b);
    }

    pub fn power_off(&mut self) {
        self.state = None;
    }

    pub fn reboot(&mut self) {
        let hdd = self.hdd_data.clone();
        let floppy_a = self.floppy_a_data.clone();
        let floppy_b = self.floppy_b_data.clone();
        self.state = None;
        self.start_computer(hdd, floppy_a, floppy_b);
    }

    pub fn run_for_cycles(&mut self, cycles: u32) -> RunResult {
        let state = match &mut self.state {
            Some(s) => s,
            None => {
                return RunResult {
                    halted: true,
                    exit_code: None,
                    cycles_executed: 0,
                };
            }
        };

        let start = state.computer.get_cycle_count();
        let target = start + cycles as u64;

        loop {
            if state.computer.is_terminal_halt() || state.computer.get_exit_code().is_some() {
                return RunResult {
                    halted: true,
                    exit_code: state.computer.get_exit_code(),
                    cycles_executed: (state.computer.get_cycle_count() - start) as u32,
                };
            }
            state.computer.step();
            if state.computer.get_cycle_count() >= target {
                break;
            }
            // Yield to JS when waiting for a keypress so the browser event loop
            // can deliver key events and the tab doesn't appear frozen.
            if state.computer.wait_for_key_press() {
                break;
            }
        }

        RunResult {
            halted: false,
            exit_code: None,
            cycles_executed: (state.computer.get_cycle_count() - start) as u32,
        }
    }

    /// Renders the current frame to the canvas context if the frame is dirty.
    /// The pixel buffer is reused across calls to avoid per-frame allocation.
    pub fn render_frame(&mut self, ctx: &CanvasRenderingContext2d) -> Result<(), JsValue> {
        // Clone the Arc so we can release the borrow on self.state before
        // mutating self.frame_buf.
        let video_buffer = match &self.state {
            Some(state) => Arc::clone(&state.video_buffer),
            None => return Ok(()),
        };

        if !video_buffer.read().unwrap().is_dirty() {
            return Ok(());
        }

        let (width, height) = {
            let mut vb = video_buffer.write().unwrap();
            let (w, h) = vb.render_resolution();
            let needed = w as usize * h as usize * 4;
            if self.frame_buf.len() != needed {
                self.frame_buf.resize(needed, 0);
            }
            vb.render_and_clear_dirty(&mut self.frame_buf);
            (w, h)
        };

        if let Some(canvas) = ctx.canvas()
            && (canvas.width() != width || canvas.height() != height)
        {
            canvas.set_width(width);
            canvas.set_height(height);
        }

        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            wasm_bindgen::Clamped(self.frame_buf.as_slice()),
            width,
            height,
        )?;
        ctx.put_image_data(&image_data, 0.0, 0.0)?;
        Ok(())
    }

    pub fn push_key_event(&mut self, scan_code: u8, is_down: bool) {
        if let Some(state) = &mut self.state {
            let code = if is_down { scan_code } else { scan_code | 0x80 };
            state.computer.push_key_press(code);
        }
    }

    pub fn push_mouse_event(&mut self, dx: i16, dy: i16, buttons: u8) {
        if let Some(state) = &mut self.state {
            let dx8 = dx.clamp(i8::MIN as i16, i8::MAX as i16) as i8;
            let dy8 = dy.clamp(i8::MIN as i16, i8::MAX as i16) as i8;
            state.computer.push_ps2_mouse_event(dx8, dy8, buttons);
        }
    }

    /// Insert a floppy image. `drive`: 0 = A:, 1 = B:
    pub fn insert_floppy(&mut self, drive: u8, image: Uint8Array) {
        let data = image.to_vec();
        let drive_num = if drive == 0 {
            DriveNumber::floppy_a()
        } else {
            DriveNumber::floppy_b()
        };

        if drive == 0 {
            self.floppy_a_data = Some(data.clone());
        } else {
            self.floppy_b_data = Some(data.clone());
        }

        if let Some(state) = &mut self.state {
            match BackedDisk::new(MemBackend::from_data(data)) {
                Ok(disk) => {
                    state
                        .computer
                        .set_floppy_disk(drive_num, Some(Box::new(disk)));
                }
                Err(e) => {
                    self.last_error = Some(format!("Invalid floppy image: {e}"));
                }
            }
        }
    }

    /// Eject floppy. `drive`: 0 = A:, 1 = B:
    pub fn eject_floppy(&mut self, drive: u8) {
        let drive_num = if drive == 0 {
            DriveNumber::floppy_a()
        } else {
            DriveNumber::floppy_b()
        };

        if drive == 0 {
            self.floppy_a_data = None;
        } else {
            self.floppy_b_data = None;
        }

        if let Some(state) = &mut self.state {
            state.computer.set_floppy_disk(drive_num, None);
        }
    }

    /// Returns effective MHz since the last call (call roughly every 500 ms).
    pub fn get_effective_mhz(&mut self) -> f64 {
        let now = js_sys::Date::now();
        let elapsed_ms = now - self.last_timestamp_ms;
        if elapsed_ms <= 0.0 {
            return 0.0;
        }
        let current = self
            .state
            .as_ref()
            .map_or(0, |s| s.computer.get_cycle_count());
        let delta = current.saturating_sub(self.last_cycle_count);
        self.last_cycle_count = current;
        self.last_timestamp_ms = now;
        (delta as f64 / 1_000_000.0) / (elapsed_ms / 1000.0)
    }

    /// Total cycles executed as f64 (avoids JS safe-integer overflow for large counts).
    pub fn get_cycle_count(&self) -> f64 {
        self.state
            .as_ref()
            .map_or(0, |s| s.computer.get_cycle_count()) as f64
    }

    /// Returns and clears the last error message, if any.
    pub fn get_last_error(&mut self) -> Option<String> {
        self.last_error.take()
    }
}

impl Oxide86Computer {
    fn start_computer(
        &mut self,
        hdd: Option<Vec<u8>>,
        floppy_a: Option<Vec<u8>>,
        floppy_b: Option<Vec<u8>>,
    ) {
        match ComputerState::create(&self.config, hdd.as_deref()) {
            Ok(mut state) => {
                for (data, drive_num, label) in [
                    (floppy_a.as_deref(), DriveNumber::floppy_a(), "floppy A"),
                    (floppy_b.as_deref(), DriveNumber::floppy_b(), "floppy B"),
                ] {
                    if let Some(data) = data {
                        match BackedDisk::new(MemBackend::from_data(data.to_vec())) {
                            Ok(disk) => {
                                state
                                    .computer
                                    .set_floppy_disk(drive_num, Some(Box::new(disk)));
                            }
                            Err(e) => {
                                self.last_error = Some(format!("Invalid {label} image: {e}"));
                                self.state = Some(state);
                                return;
                            }
                        }
                    }
                }

                let boot_drive = match self.boot_drive.as_deref() {
                    Some("floppy_a") if floppy_a.is_some() => Some(DriveNumber::floppy_a()),
                    Some("floppy_b") if floppy_b.is_some() => Some(DriveNumber::floppy_b()),
                    Some("hdd") if hdd.is_some() => Some(DriveNumber::from_hard_drive_index(0)),
                    _ => {
                        // Auto: floppy A first, then HDD
                        if floppy_a.is_some() {
                            Some(DriveNumber::floppy_a())
                        } else if hdd.is_some() {
                            Some(DriveNumber::from_hard_drive_index(0))
                        } else {
                            None
                        }
                    }
                };

                if let Some(drive) = boot_drive
                    && let Err(e) = state.computer.boot(drive)
                {
                    self.last_error = Some(format!("Boot failed: {e}"));
                }
                self.state = Some(state);
            }
            Err(e) => self.last_error = Some(e),
        }
    }
}
