use std::sync::{Arc, RwLock};

use crate::devices::serial_mouse::SerialMouse;
use crate::tests::run_test_with_interaction;

/// Tests that a serial mouse motion packet is delivered to the guest program.
///
/// Flow:
/// 1. Attach a SerialMouse to COM1.
/// 2. The assembly initializes COM1 via INT 14h AH=00h, which raises DTR and
///    causes the mouse to send the 'M' identification byte.
/// 3. The Rust test detects initialization and injects push_motion(10, 5).
/// 4. The assembly reads 'M', then verifies the 3-byte packet (dx=10, dy=5,
///    no buttons: 0x40 0x0A 0x05).
#[test_log::test]
pub(crate) fn check_mouse_motion() {
    run_test_with_interaction("mouse/check_mouse_motion", |computer| {
        let mouse = Arc::new(RwLock::new(SerialMouse::new()));
        computer.set_com_port_device(1, Some(mouse.clone()));

        let mut motion_pushed = false;
        loop {
            if computer.get_exit_code().is_some() {
                break;
            }
            if !motion_pushed && mouse.read().unwrap().is_initialized() {
                mouse.write().unwrap().push_motion(10, 5);
                motion_pushed = true;
            }
            computer.step();
        }
    });
}
