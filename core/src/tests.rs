#[cfg(test)]
mod tests {
    use anyhow::Context;
    use std::fs::File;
    use std::io::Read;
    use std::path::Path;
    use std::sync::{Arc, RwLock};

    use crate::computer::ComputerConfig;
    use crate::cpu::CpuType;
    use crate::devices::rtc::tests::MockClock;
    use crate::video::video_buffer::RenderResult;
    use crate::{
        computer::Computer,
        video::{VideoBuffer, VideoCard},
    };

    const TEST_SEGMENT: u16 = 0x1000;
    const TEST_OFFSET: u16 = 0x0100;

    fn create_computer() -> (Computer, Arc<RwLock<VideoBuffer>>) {
        let video_buffer = Arc::new(RwLock::new(VideoBuffer::new()));
        let mut computer = Computer::new(ComputerConfig {
            clock: Box::new(MockClock::new()),
            clock_speed: 8_000_000,
            cpu_type: CpuType::I8086,
            memory_size: 2048 * 1024,
        });
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

    mod cpu {
        use super::*;

        #[test_log::test]
        pub fn op8086() {
            run_test("cpu/op8086");
        }

        #[test_log::test]
        pub fn irq_chain() {
            run_test("cpu/irq_chain");
        }
    }

    mod keyboard {
        use super::*;

        #[test_log::test]
        pub fn check_keystroke_int16() {
            run_test_with_interaction("keyboard/check_keystroke_int16", |computer| {
                computer.push_key_press(0x18 /* 'o' */);
                computer.run();
            });
        }
    }

    mod pit {
        use super::*;

        #[test_log::test]
        pub fn check_timer_tick() {
            run_test("pit/check_timer_tick");
        }
    }

    mod uart {
        use std::collections::VecDeque;

        use crate::devices::uart::ComPortDevice;

        use super::*;

        struct HelloWorldTestComDevice {
            async_count: u32,
            from_computer: String,
            to_computer: VecDeque<u8>,
        }

        impl ComPortDevice for HelloWorldTestComDevice {
            fn read(&mut self) -> Option<u8> {
                if self.async_count > 3
                    && self.from_computer == "hello"
                    && let Some(out) = self.to_computer.pop_front()
                {
                    self.async_count = 0;
                    Some(out)
                } else {
                    self.async_count += 1;
                    None
                }
            }

            fn write(&mut self, value: u8) -> bool {
                if self.async_count > 3 {
                    self.async_count = 0;
                    self.from_computer.push(value as char);
                    true
                } else {
                    self.async_count += 1;
                    false
                }
            }
        }

        #[test_log::test]
        pub fn uart_hello_world() {
            run_test_with_interaction("uart/uart_hello_world", |computer| {
                let test_device = Arc::new(RwLock::new(HelloWorldTestComDevice {
                    async_count: 0,
                    from_computer: "".to_owned(),
                    to_computer: VecDeque::from(['o' as u8, 'k' as u8]),
                }));

                computer.set_com_port_device(1, Some(test_device.clone()));
                computer.run();

                let found = &test_device.read().unwrap().from_computer;
                assert_eq!("hello", found);
            });
        }
    }
}
