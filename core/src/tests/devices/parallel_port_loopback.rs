use std::sync::{Arc, RwLock};

use crate::{devices::parallel_port_loopback::ParallelLoopback, tests::run_test};

fn make_loopback_computer() -> (
    crate::computer::Computer,
    Arc<RwLock<crate::video::VideoBuffer>>,
) {
    let (mut computer, video_buffer) = make_computer!();
    computer.set_lpt_device(1, Some(Arc::new(RwLock::new(ParallelLoopback::new()))));
    (computer, video_buffer)
}

/// LPT1 loopback: data register readback (0x00–0xFF) and control/status
/// loopback check matching the CheckIt diagnostic sequence.
#[test_log::test]
pub(crate) fn lpt_loopback() {
    run_test(
        "devices/parallel_port_loopback/lpt_loopback",
        make_loopback_computer(),
        |computer, _video_buffer| {
            computer.run();
        },
    );
}
