#[cfg(test)]
mod tests {
    use anyhow::Context;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    use std::sync::{Arc, RwLock};

    use crate::KeyPress;
    use crate::cpu::CpuType;
    use crate::video::video_buffer::RenderResult;
    use crate::{
        computer::Computer,
        cpu::Cpu,
        memory::Memory,
        video::{VideoBuffer, VideoCard},
    };

    const TEST_SEGMENT: u16 = 0x1000;
    const TEST_OFFSET: u16 = 0x0100;

    fn create_computer() -> (Computer, Arc<RwLock<VideoBuffer>>) {
        let video_buffer = Arc::new(RwLock::new(VideoBuffer::new()));
        let cpu = Cpu::new(CpuType::I8086);
        let mut computer = Computer::new(cpu, Memory::new(2048 * 1024));
        computer.add_device(VideoCard::new(video_buffer.clone()));
        (computer, video_buffer)
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

    fn run_test_with_interaction(name: &str, f: fn(&mut Computer)) {
        let (program_data, expected_screen) = load_data(name);

        let (mut computer, video_buffer) = create_computer();
        computer
            .load_program(&program_data, TEST_SEGMENT, TEST_OFFSET)
            .unwrap();
        f(&mut computer);

        if let Some(expected_screen) = expected_screen {
            assert_screen(name, expected_screen, video_buffer);
        }
        assert_eq!(Some(0), computer.get_exit_code());
    }

    fn run_test(name: &str) {
        run_test_with_interaction(name, |computer| {
            computer.run();
        });
    }

    #[test_log::test]
    pub fn hello_world_video_memory() {
        run_test("hello_world_video_memory");
    }

    #[test_log::test]
    pub fn hello_world_int21_write_string() {
        run_test("hello_world_int21_write_string");
    }

    #[test_log::test]
    pub fn cpu_op8086() {
        run_test("cpu/op8086");
    }

    #[test_log::test]
    pub fn keyboard_check_keystroke_int16() {
        run_test_with_interaction("keyboard/check_keystroke_int16", |computer| {
            computer.push_key_press(0x18 /* 'o' */);
            computer.run();
        });
    }
}
