#[cfg(test)]
mod tests {
    use anyhow::Context;
    use std::io::Read;
    use std::sync::Arc;
    use std::{cell::RefCell, fs::File};

    use crate::{
        computer::Computer,
        cpu::Cpu,
        memory::Memory,
        memory_bus::MemoryBus,
        video::{VideoBuffer, VideoCard, video_buffer},
    };

    const TEST_SEGMENT: u16 = 0x1000;
    const TEST_OFFSET: u16 = 0x0100;

    fn create_computer() -> (Computer, Arc<VideoBuffer>) {
        let video_buffer = Arc::new(VideoBuffer::new());
        let video_card = RefCell::new(VideoCard::new(video_buffer.clone()));
        let memory_bus = MemoryBus::new(Memory::new(2048 * 1024), video_card);
        let cpu = Cpu::new();
        let computer = Computer::new(cpu, memory_bus);
        (computer, video_buffer)
    }

    fn load_data(name: &str) -> (Vec<u8>, Vec<u8>) {
        let program_data = {
            let filename = format!("{name}.com");
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
            let filename = format!("{name}.png");
            let f = image::open(&filename)
                .context(format!("failed to open: {filename}"))
                .unwrap();
            f.to_rgba8().into_raw()
        };

        (program_data, expected_image_data)
    }

    #[test]
    pub fn hello_world_video_memory() {
        let (program_data, expected_screen) = load_data("hello_world_video_memory");

        let (mut computer, video_buffer) = create_computer();
        computer
            .load_program(&program_data, TEST_SEGMENT, TEST_OFFSET)
            .unwrap();
        computer.run();
    }
}
