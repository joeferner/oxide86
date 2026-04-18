use std::{
    io,
    sync::{Arc, Mutex, RwLock},
};

use crate::{devices::printer::Printer, tests::run_test};

/// A `Write` sink backed by a shared `Arc<Mutex<Vec<u8>>>` so the test can
/// read printer output after `computer.run()` without owning the `Printer`.
struct SharedVecWriter(Arc<Mutex<Vec<u8>>>);

impl io::Write for SharedVecWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Send "Hello, Printer!\r\n" to LPT1 via direct I/O and verify the raw bytes
/// reach the writer in order.
#[test_log::test]
pub(crate) fn printer_hello() {
    let output: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));

    let (mut computer, video_buffer) = make_computer!();
    computer.set_lpt_device(
        1,
        Some(Arc::new(RwLock::new(Printer::new(
            Box::new(SharedVecWriter(Arc::clone(&output))) as Box<dyn std::io::Write + Send + Sync>,
        )))),
    );

    run_test(
        "devices/printer/printer_hello",
        (computer, video_buffer),
        |comp, _| {
            comp.run();
            assert_eq!(*output.lock().unwrap(), b"Hello, Printer!\r\n");
        },
    );
}
