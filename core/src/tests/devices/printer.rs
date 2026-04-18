use std::sync::{Arc, RwLock};

use crate::{devices::printer::Printer, tests::run_test};

fn make_printer_computer() -> (
    crate::computer::Computer,
    Arc<RwLock<crate::video::VideoBuffer>>,
) {
    let (mut computer, video_buffer) = make_computer!();
    computer.set_lpt_device(1, Some(Arc::new(RwLock::new(Printer::new()))));
    (computer, video_buffer)
}

/// Send "Hello, Printer!\r\n" to LPT1 via direct I/O and verify the raw bytes
/// are captured by `take_lpt_output`.
#[test_log::test]
pub(crate) fn printer_hello() {
    run_test(
        "devices/printer/printer_hello",
        make_printer_computer(),
        |computer, _video_buffer| {
            computer.run();
            let output = computer.take_lpt_output(1);
            assert_eq!(output, b"Hello, Printer!\r\n");
        },
    );
}
