use anyhow::Context;
use core::panic;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::video::video_buffer::RenderResult;
use crate::{computer::Computer, video::VideoBuffer};

const TEST_SEGMENT: u16 = 0x1000;
const TEST_OFFSET: u16 = 0x0100;

#[macro_use]
mod macros {
    macro_rules! make_computer {
        ($($key:ident: $val:expr),* $(,)?) => {{
            let video_buffer = std::sync::Arc::new(std::sync::RwLock::new($crate::video::VideoBuffer::new()));
            #[allow(unused_mut)]
            let mut config = $crate::computer::ComputerConfig {
                clock: Box::new($crate::devices::rtc::tests::MockClock::new()),
                clock_speed: 8_000_000,
                cpu_type: $crate::cpu::CpuType::I8086,
                memory_size: 2048 * 1024,
                hard_disks: vec![],
                video_card_type: crate::video::VideoCardType::CGA,
                video_buffer: video_buffer.clone(),
                pc_speaker: Box::new($crate::devices::pc_speaker::NullPcSpeaker::new()),
                math_coprocessor: false,
            };
            $(config.$key = $val;)*
            let computer = $crate::computer::Computer::new(config);
            (computer, video_buffer)
        }};
    }
}

mod cpu;
mod devices;
mod video;

fn create_computer() -> (Computer, Arc<RwLock<VideoBuffer>>) {
    make_computer!()
}

fn load_program_data(name: &str) -> Vec<u8> {
    let filename = format!("src/test_data/{name}.com");
    let mut f = File::open(&filename)
        .context(format!("failed to open: {filename}"))
        .unwrap();
    let mut buffer = Vec::new();
    f.read_to_end(&mut buffer)
        .context(format!("failed to read: {filename}"))
        .unwrap();
    buffer
}

pub(super) fn assert_screen(name: &str, video_buffer: Arc<RwLock<VideoBuffer>>) {
    let expected_image_data = {
        let filename = format!("src/test_data/{name}.png");
        if Path::new(&filename).exists() {
            let f = image::open(&filename)
                .context(format!("failed to open: {filename}"))
                .unwrap();
            let data = f.to_rgba8().into_raw();
            RenderResult {
                data,
                width: f.width(),
                height: f.height(),
            }
        } else {
            panic!("could not find screen file: {filename}");
        }
    };

    let buffer = video_buffer.read().unwrap();
    let rendered_data = buffer.render();
    if rendered_data != expected_image_data {
        let filename = format!("src/test_data/{name}_actual.png");
        image::save_buffer(
            &filename,
            &rendered_data.data,
            rendered_data.width,
            rendered_data.height,
            image::ColorType::Rgba8,
        )
        .expect(&format!("failed to save {filename}"));
        panic!("frame mismatch {filename}");
    }
}

fn run_assert_screen_key_press_run_test(
    name: &str,
    computer: (Computer, Arc<RwLock<VideoBuffer>>),
) {
    run_test(name, computer, |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
        computer.push_key_press(0x1C /* Enter */);
        computer.run();
    });
}

fn run_test(
    name: &str,
    (mut computer, video_buffer): (Computer, Arc<RwLock<VideoBuffer>>),
    f: impl Fn(&mut Computer, Arc<RwLock<VideoBuffer>>),
) {
    let program_data = load_program_data(name);
    computer
        .load_program(&program_data, TEST_SEGMENT, TEST_OFFSET)
        .unwrap();
    f(&mut computer, video_buffer);
    assert_eq!(Some(0), computer.get_exit_code());
}

#[test_log::test]
pub(crate) fn hello_world_video_memory() {
    let name = "hello_world_video_memory";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}

#[test_log::test]
pub(crate) fn hello_world_int21_write_string() {
    let name = "hello_world_int21_write_string";
    run_test(name, create_computer(), |computer, video_buffer| {
        computer.run();
        assert_screen(name, video_buffer);
    });
}
