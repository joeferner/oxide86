# Joystick Test Program

Tests the IBM Game Control Adapter (port 0x201) by reading and displaying joystick axis timer states and button states in real-time.

## Building

```bash
nasm -f bin joystick_test.asm -o joystick_test.com
```

## Running

### Native GUI (with gamepad support)
```bash
# Enable joystick A slot (auto-detects first connected gamepad)
cargo run -p oxide86-native-gui -- --joystick-a joystick_test.com

# Enable both joystick slots (A and B)
cargo run -p oxide86-native-gui -- --joystick-a --joystick-b joystick_test.com
```

The GUI version uses gilrs to auto-detect connected gamepads:
- First connected gamepad → Joystick A (requires `--joystick-a`)
- Second connected gamepad → Joystick B (requires `--joystick-b`)

### Native CLI (with gamepad support)
```bash
# Enable joystick A slot
cargo run -p oxide86-native-cli -- --joystick-a joystick_test.com

# Enable both joystick slots
cargo run -p oxide86-native-cli -- --joystick-a --joystick-b joystick_test.com
```

CLI also supports gamepad auto-detection via gilrs.

### WASM (browser Gamepad API)

The WASM version requires JavaScript to poll `navigator.getGamepads()` and call the exposed methods:
```javascript
computer.handle_gamepad_axis(slot, axis, value);
computer.handle_gamepad_button(slot, button, pressed);
computer.gamepad_connected(slot, connected);
```

See the React app for an example implementation.

## Expected Output

The program displays:
- **Port value**: Raw hex byte from port 0x201
- **Axis Timers**: For each joystick (A/B) and axis (X/Y), shows "Running" or "TimedOut"
  - "Running" = Timer still counting (bit = 1)
  - "TimedOut" = Timer finished (bit = 0)
  - Timer values indicate joystick position (longer time = more deflection)
- **Buttons**: For each joystick (A/B) and button (1/2), shows "Pressed" or "Released"
  - Button bits are inverted: 0 = pressed, 1 = released

### Without Gamepad Connected
If no gamepad is detected or joystick flags are not passed:
- All axis timers will show "TimedOut" immediately (bits 0-3 = 0)
- All buttons will show "Released" (bits 4-7 = 1)
- Port value will be 0xF0

### With Gamepad Connected
With a gamepad connected and joystick flags enabled:
- Axis timers reflect stick positions
- Button states change when you press gamepad buttons (South = Button 1, East = Button 2)

## Hardware Details

### Port 0x201 Bit Layout
```
Bit 0 — Joystick A X-axis timer (1=running, 0=timed out)
Bit 1 — Joystick A Y-axis timer
Bit 2 — Joystick B X-axis timer
Bit 3 — Joystick B Y-axis timer
Bit 4 — Joystick A Button 1 (0=pressed, 1=released)
Bit 5 — Joystick A Button 2
Bit 6 — Joystick B Button 1
Bit 7 — Joystick B Button 2
```

### Axis Reading
Programs read joystick position by:
1. Writing any value to port 0x201 to fire the RC one-shots
2. Repeatedly reading port 0x201 and counting cycles until axis bit drops to 0
3. The cycle count represents the axis position

At 4.77 MHz:
- Axis range: ~115–3860 cycles (0%–100% deflection)
- Center position: ~2000 cycles

### Gamepad Button Mapping
- **South button** (A on Xbox, Cross on PlayStation) → Button 1
- **East button** (B on Xbox, Circle on PlayStation) → Button 2

## Exit

Press any key to exit the test program.
