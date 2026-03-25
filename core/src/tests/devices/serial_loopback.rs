use std::sync::{Arc, RwLock};

use crate::{devices::serial_loopback::SerialLoopback, tests::run_test};

fn make_loopback_computer() -> (
    crate::computer::Computer,
    Arc<RwLock<crate::video::VideoBuffer>>,
) {
    let (mut computer, video_buffer) = make_computer!();
    computer.set_com_port_device(1, Some(Arc::new(RwLock::new(SerialLoopback::new()))));
    (computer, video_buffer)
}

/// Basic TX→RX round-trip: bytes written to THR must be readable from RBR.
#[test_log::test]
pub(crate) fn tx_rx() {
    run_test(
        "devices/serial_loopback/tx_rx",
        make_loopback_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// MSR register walk: write patterns to MSR, verify bits 3:0 read back once
/// then are cleared on the second read (cleared-on-read delta bits).
#[test_log::test]
pub(crate) fn msr_register_walk() {
    run_test(
        "devices/serial_loopback/msr_register_walk",
        make_loopback_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}

/// Modem status lines: verify that the physical loopback wiring
/// (RTS→CTS, DTR→DSR+RI+DCD) is reflected correctly in MSR, including
/// delta bits set on line transitions and cleared after reading.
#[test_log::test]
pub(crate) fn modem_status_lines() {
    run_test(
        "devices/serial_loopback/modem_status_lines",
        make_loopback_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}
