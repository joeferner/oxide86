use std::sync::{Arc, RwLock};

use crate::devices::serial_mouse::SerialMouse;
use crate::tests::run_test_with_interaction;

/// Tests that a PS/2 mouse motion packet is delivered to the guest callback.
///
/// Flow:
/// 1. The assembly initializes the PS/2 mouse via INT 15h AH=C2h (init, enable,
///    set handler), then polls a `mouse_ready` flag.
/// 2. The Rust test waits for the aux port to be enabled, then injects
///    push_ps2_mouse_event(10, 5, 0).
/// 3. IRQ12 fires, the BIOS INT 74h handler reads the packet and FAR CALLs the
///    registered handler, which stores dx/dy and sets `mouse_ready`.
/// 4. The assembly verifies dx=10, dy=5, no buttons and exits with code 0.
#[test_log::test]
pub(crate) fn check_mouse_motion_ps2() {
    run_test_with_interaction("mouse/check_mouse_motion_ps2", |computer| {
        let mut motion_pushed = false;
        loop {
            if computer.get_exit_code().is_some() {
                break;
            }
            if !motion_pushed && computer.is_ps2_mouse_ready() {
                computer.push_ps2_mouse_event(10, 5, 0);
                motion_pushed = true;
            }
            computer.step();
        }
    });
}

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
    run_test_with_interaction("mouse/check_mouse_motion_com1", |computer| {
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
