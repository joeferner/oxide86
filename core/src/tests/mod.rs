use anyhow::Context;
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
            let video_card = $crate::video::VideoCard::new(crate::video::VideoCardType::VGA, video_buffer.clone());
            #[allow(unused_mut)]
            let mut config = $crate::computer::ComputerConfig {
                clock: Box::new($crate::devices::rtc::tests::MockClock::new()),
                clock_speed: 8_000_000,
                cpu_type: $crate::cpu::CpuType::I8086,
                memory_size: 2048 * 1024,
                hard_disks: vec![],
                video_card: std::rc::Rc::new(std::cell::RefCell::new(video_card)),
            };
            $(config.$key = $val;)*
            let computer = $crate::computer::Computer::new(config);
            (computer, video_buffer)
        }};
    }
}

mod cpu;
mod keyboard;
mod mock_com_device;
mod pit;
mod uart;

fn create_computer() -> (Computer, Arc<RwLock<VideoBuffer>>) {
    make_computer!()
}

fn load_data(name: &str) -> (Vec<u8>, Option<RenderResult>) {
    let program_data = {
        let filename = format!("src/test_data/{name}.com");
        let mut f = File::open(&filename)
            .context(format!("failed to open: {filename}"))
            .unwrap();
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)
            .context(format!("failed to read: {filename}"))
            .unwrap();
        buffer
    };

    let expected_image_data = {
        let filename = format!("src/test_data/{name}.png");
        if Path::new(&filename).exists() {
            let f = image::open(&filename)
                .context(format!("failed to open: {filename}"))
                .unwrap();
            let data = f.to_rgba8().into_raw();
            Some(RenderResult {
                data,
                width: f.width(),
                height: f.height(),
            })
        } else {
            None
        }
    };

    (program_data, expected_image_data)
}

fn assert_screen(name: &str, expected_screen: RenderResult, buffer: Arc<RwLock<VideoBuffer>>) {
    let buffer = buffer.read().unwrap();
    let render = buffer.render();
    if render != expected_screen {
        let filename = format!("src/test_data/{name}_actual.png");
        image::save_buffer(
            &filename,
            &render.data,
            render.width,
            render.height,
            image::ColorType::Rgba8,
        )
        .expect(&format!("failed to save {filename}"));
        panic!("frame mismatch");
    }
}

fn run_test_configured(
    name: &str,
    (mut computer, video_buffer): (Computer, Arc<RwLock<VideoBuffer>>),
    f: fn(&mut Computer),
) {
    let (program_data, expected_screen) = load_data(name);
    computer
        .load_program(&program_data, TEST_SEGMENT, TEST_OFFSET)
        .unwrap();
    f(&mut computer);
    if let Some(expected_screen) = expected_screen {
        assert_screen(name, expected_screen, video_buffer);
    }
    assert_eq!(Some(0), computer.get_exit_code());
}

fn run_test_with_interaction(name: &str, f: fn(&mut Computer)) {
    run_test_configured(name, create_computer(), f);
}

fn run_test(name: &str) {
    run_test_with_interaction(name, |computer| {
        computer.run();
    });
}

#[test_log::test]
pub(crate) fn hello_world_video_memory() {
    run_test("hello_world_video_memory");
}

#[test_log::test]
pub(crate) fn hello_world_int21_write_string() {
    run_test("hello_world_int21_write_string");
}
