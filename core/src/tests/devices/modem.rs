use std::sync::{Arc, RwLock};

use crate::{
    devices::modem::SerialModem,
    tests::{create_computer, run_test},
};

#[test_log::test]
pub(crate) fn at_basic() {
    run_test(
        "devices/modem/at_basic",
        create_computer(),
        |computer, _video_buffer| {
            let modem = Arc::new(RwLock::new(SerialModem::new()));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn at_dial_reject() {
    run_test(
        "devices/modem/at_dial_reject",
        create_computer(),
        |computer, _video_buffer| {
            let modem = Arc::new(RwLock::new(SerialModem::new()));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn at_hangup() {
    run_test(
        "devices/modem/at_hangup",
        create_computer(),
        |computer, _video_buffer| {
            let modem = Arc::new(RwLock::new(SerialModem::new()));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}
