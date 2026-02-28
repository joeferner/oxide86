#[cfg(test)]
mod tests {
    use anyhow::Context;
    use std::fs::File;
    use std::io::Read;
    use std::sync::Arc;

    use crate::Devices;
    use crate::cpu::CpuType;
    use crate::io_bus::IoBus;
    use crate::video::video_buffer::RenderResult;
    use crate::{
        computer::Computer,
        cpu::Cpu,
        memory::Memory,
        memory_bus::MemoryBus,
        video::{VideoBuffer, VideoCard},
    };

    const TEST_SEGMENT: u16 = 0x1000;
    const TEST_OFFSET: u16 = 0x0100;

    fn create_computer() -> (Computer, Arc<VideoBuffer>) {
        let video_buffer = Arc::new(VideoBuffer::new());
        let mut devices = Devices::new();
        devices.push(VideoCard::new(video_buffer.clone()));
        let memory_bus = MemoryBus::new(Memory::new(2048 * 1024), devices.clone());
        let io_bus = IoBus::new(devices);
        let cpu = Cpu::new(CpuType::I8086);
        let computer = Computer::new(cpu, memory_bus, io_bus);
        (computer, video_buffer)
    }

    fn load_data(name: &str) -> (Vec<u8>, RenderResult) {
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
            let f = image::open(&filename)
                .context(format!("failed to open: {filename}"))
                .unwrap();
            let data = f.to_rgba8().into_raw();
            RenderResult {
                data,
                width: f.width(),
                height: f.height(),
            }
        };

        (program_data, expected_image_data)
    }

    fn assert_screen(name: &str, expected_screen: RenderResult, video_buffer: Arc<VideoBuffer>) {
        video_buffer.emu_try_flip();
        if let Some(frame) = video_buffer.ui_get_data() {
            let render = frame.render();
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
        } else {
            panic!("could not get frame");
        }
    }

    fn run_test(name: &str) {
        let (program_data, expected_screen) = load_data(name);

        let (mut computer, video_buffer) = create_computer();
        computer
            .load_program(&program_data, TEST_SEGMENT, TEST_OFFSET)
            .unwrap();
        computer.run();

        assert_screen(name, expected_screen, video_buffer);
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
}
