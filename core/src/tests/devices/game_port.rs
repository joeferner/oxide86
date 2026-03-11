use crate::tests::{create_computer, run_test};

#[test_log::test]
pub(crate) fn game_port_test() {
    run_test(
        "devices/game_port/game_port_test",
        create_computer(),
        |computer, _video_buffer| {
            // X1=0: times out immediately after one-shot write (cycles_needed=0).
            // Y1=200: needs ~17600 cycles at 8 MHz — well above a single IN latency.
            computer.joystick_mut().set_axes(0, 200, 128, 128);
            // Button 1 pressed (bit 4 = 0), button 2 released (bit 5 = 1).
            computer
                .joystick_mut()
                .set_buttons(true, false, false, false);
            computer.run();
        },
    );
}
