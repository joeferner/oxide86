
# Running

## .asm

1. Build

    ```bash
    nasm -f bin hello_program.asm -o hello_program.com
    ```

1. Run

    ```bash
    cargo run -p emu86-native-cli -- hello_program.com
    cargo run -p emu86-native-gui -- hello_program.com
    ```

## .bas

1. Create a disk image

    ```bash
    # Create a blank 1.44MB image
    mkfs.msdos -C audio.img 1440

    # Copy the file into the image
    mcopy -i audi.img -s audio.bas ::
    ```

1. Start the emulator running DOS
1. Load the img into a floppy drive
1. Open QBasic
1. Load program
1. Run

# Program Descriptions

## Audio

### audio.bas
QBasic program that plays a musical melody using the PLAY command. Demonstrates the BASIC PLAY statement for generating musical notes through the PC speaker.

### beep.asm
Assembly program that directly controls the PC speaker using the Intel 8253/8254 Programmable Interval Timer (PIT). Configures Channel 2 in Mode 3 (square wave), sets frequency to ~1000 Hz (count value 1193), enables the speaker via port 0x61, waits approximately 1 second, then disables the speaker and exits.

### simple_beep.asm
Simplified version of beep.asm that produces a 1000 Hz tone for approximately 1 second. Tests basic PC speaker functionality through PIT Channel 2 configuration and port 0x61 control.

## Joystick

### joystick/joystick_test.asm
Tests both joysticks (A and B) on the IBM Game Control Adapter (port 0x201) in real-time. Fires RC one-shots and counts how many reads each axis timer stays high — the count is a proxy for stick deflection (0 = timed out immediately / no joystick, ~15 = center, ~30 = full deflection). Also shows button states for both joysticks with inverted logic (0=pressed, 1=released).

**Running with gamepad support:**
```bash
# Enable joystick A slot (requires connected gamepad)
cargo run -p emu86-native-gui -- --joystick-a test-programs/joystick/joystick_test.com

# Enable both joystick slots
cargo run -p emu86-native-gui -- --joystick-a --joystick-b test-programs/joystick/joystick_test.com
```

**Expected Output:**
- **Joystick A / B X and Y axis counts** (4-digit zero-padded): 0000 with no gamepad; ~0015 at center; ~0030 at full deflection
- **Button 1 / 2 states**: `Released` with no gamepad; `Pressed ` when South (A/Cross) or East (B/Circle) held
- **Raw port 0x201 hex**: `F0` with no gamepad (all timers expired, all buttons released)

**Technical Details:**
Fires RC timer one-shots by writing to port 0x201, then polls in a tight loop (up to 500 iterations) counting reads where each axis bit (bits 0-3) stays high. Stops early when all four timer bits go low. Button bits (4-7) are read after timers expire: 0=pressed, 1=released. Gamepad mapping via gilrs: first connected gamepad → Joystick A, second → Joystick B. South (A/Cross) → Button 1, East (B/Circle) → Button 2. Requires `--joystick-a` and/or `--joystick-b` flags. Press any key to exit. See [joystick/README.md](joystick/README.md) for full protocol details.

## Keyboard

### waitkey.asm
Tests INT 16h AH=00h blocking keyboard input behavior. Waits for keypresses and echoes them back to the console. Demonstrates blocking keyboard reads, scan code/ASCII code handling, and prints each key pressed. Press ESC to exit.

## Misc

### hello_program.asm
Simple "Hello World" test program for verifying program loading functionality. Uses INT 21h AH=09h to display a message and INT 21h AH=4Ch to exit. Minimal .COM file structure starting at CS:0100h.

## Opcode Test

### opcode-test/opctest.asm
Systematic test suite for validating CPU instruction implementation. Tests 10 common opcodes with multiple test cases each, reporting results to COM1 serial logger.

**Running with serial logger:**
```bash
# Native CLI
cargo run -p emu86-native-cli -- test-programs/opcode-test/opctest.com --com1-device logger

# Native GUI
cargo run -p emu86-native-gui -- test-programs/opcode-test/opctest.com --com1-device logger
```

**Expected Output:**
```
[COM1] === emu86 Opcode Test Suite ===
[COM1] MOV: PASS
[COM1] ADD: PASS
[COM1] SUB: PASS
[COM1] AND: PASS
[COM1] OR: PASS
[COM1] XOR: PASS
[COM1] SHL: PASS
[COM1] INC: PASS
[COM1] CMP: PASS
[COM1] PUSH/POP: PASS
[COM1]
[COM1] --- Summary ---
[COM1] 10 passed, 0 failed
```

**Tested Instructions:**
- **MOV**: Basic register-to-register data movement, immediate values
- **ADD**: Addition with and without carry flag, overflow detection
- **SUB**: Subtraction with and without borrow, underflow detection
- **AND**: Bitwise AND operations, masking patterns
- **OR**: Bitwise OR operations, bit setting patterns
- **XOR**: Bitwise XOR operations, self-zeroing, bit flipping
- **SHL**: Shift left operations, carry flag propagation
- **INC**: Increment operations, wrap-around behavior
- **CMP**: Comparison operations, flag setting (ZF, CF, SF)
- **PUSH/POP**: Stack operations, LIFO verification, SP tracking

**Technical Details:**
Each test includes multiple assertions checking both result values and CPU flags. Failed tests print specific error messages (e.g., "result or carry incorrect"). The framework is designed to be easily extended with additional instruction tests. Future additions could include: DEC, MUL, DIV, ROL, ROR, RCL, RCR, SHR, SAR, NOT, NEG, TEST, string operations (MOVS, CMPS, SCAS, LODS, STOS with REP), segment operations (LES, LDS), and BCD arithmetic (DAA, DAS, AAA, AAS, AAM, AAD).

## Serial

### serial/serial_logger_test.asm
Tests serial port logger functionality by writing debug output to COM1. Initializes COM1 at 9600 baud (8N1), then sends multiple test messages to verify the serial logger captures and displays them correctly.

**Running with serial logger:**
```bash
# Native CLI
cargo run -p emu86-native-cli -- test-programs/serial/serial_logger_test.com --com1-device logger

# Native GUI
cargo run -p emu86-native-gui -- test-programs/serial/serial_logger_test.com --com1-device logger
```

**Expected Log Output:**
```
[COM1] [TEST] Serial Logger Test Program
[COM1] [TEST] Test 1: Simple message
[COM1] [TEST] Test 2: Multiple lines
[COM1] [TEST]   - Line 2
[COM1] [TEST]   - Line 3
[COM1] [TEST] Test 3: Number output: 42
[COM1] 1234
[COM1] 65535
[COM1] [TEST] Test 4: Mixed content - CPU initialized, memory OK, ready to run
[COM1] [TEST] All tests completed successfully!
```

**Technical Details:**
Demonstrates INT 14h serial port services (AH=00h initialize, AH=01h write character), serial port initialization with baud rate and parameters (9600,N,1,8), null-terminated string output, CR+LF line endings (\r\n), and decimal number conversion/output. The program writes to the BIOS serial port interface which gets logged by the attached SerialLogger device. Output appears at INFO log level with `[COM1]` prefix to distinguish from other log messages.

## Video

### mode_04h_cga_320x200x4.asm
Comprehensive CGA graphics mode test program. Switches to mode 0x04 (320x200, 4 colors) and tests both text rendering and graphics drawing in graphics mode.

**Expected Output:**
- **Pixel rows 0-39:** Three colored boxes at top (cyan, magenta, white) at columns 0, 20, 40
- **Pixel rows 48-63:** White text "CGA Graphics Mode 0x04 Test" (character row 6)
- **Pixel rows 64-71:** Magenta text "Drawing test patterns..." (character row 8)
- **Pixel rows 100-139:** Three colored boxes at middle (cyan, pattern, pattern) at columns 0, 20, 40
- **Pixel rows 184-191:** Cyan text "Test complete! Press any key..." at column 2 (character row 23)

**Technical Details:**
Demonstrates INT 10h AH=0Eh teletype output in graphics mode (draws 8x8 characters pixel-by-pixel using CP437 font), direct video memory writes at 0xB8000, pixel encoding (2 bits per pixel, 4 pixels per byte), CGA interlaced memory layout (even scan lines at 0x0000-0x1F3F, odd lines at 0x2000-0x3F3F), palette selection via port 0x3D9 (palette 1 with intensity: cyan, magenta, white), and proper cursor positioning in 40-column character grid (40 chars × 25 rows). Waits for keypress then returns to text mode.

### mode_04h_composite.asm
CGA composite mode test program. Starts in mode 0x04 (320x200, 4 colors), draws test patterns using 2bpp format, then enables composite mode by setting the hires bit (bit 4) in port 0x3D8. This creates a 640x200 composite artifact color mode as used by programs like Sierra AGI games (King's Quest I).

**Expected Output:**
- **Pixel rows 0-15:** 16 horizontal gradient bars, each showing a different nibble value (0x00 through 0xFF) rendered as composite colors
- **Pixel rows 100-139:** 8 vertical color bars using mixed nibble patterns (0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF)
- **Pixel rows 160-167:** Text "CGA Composite Mode Active!" (character row 20)
- **Pixel rows 184-191:** Text "Press any key to exit..." (character row 23)

**Technical Details:**
Demonstrates CGA composite mode activation via port 0x3D8 (changing from 0x0A to 0x1A to enable hires bit), nibble-to-palette rendering (each byte interpreted as 2 nibbles, each nibble 0-15 maps to a color), effective resolution of 160x200 pixels scaled to 640x400 display, and NTSC composite artifact coloring simulation. The program stays in mode 0x04 internally (CPU continues using 2bpp pixel format) but the renderer interprets the data differently when composite mode is active. Demonstrates how Sierra games and similar software achieved more colors through composite artifacts. Waits for keypress then returns to text mode.

### mode_06h_cga_640x200x2.asm
CGA graphics mode 0x06 test program. Switches to 640x200 monochrome mode (1 bit per pixel, 8 pixels per byte) and draws patterned boxes using direct video memory writes at 0xB800.

**Expected Output:**
- **Pixel rows 0-39:** Three patterned boxes at top (solid, dense checkerboard, alternating) at columns 0, 20, 40
- **Pixel rows 48-63:** White text "CGA Graphics Mode 0x06 Test" (character row 6)
- **Pixel rows 64-71:** White text "640x200, 2 Colors (1 bit/pixel)" (character row 8)
- **Pixel rows 100-139:** Three patterned boxes at middle (alternating, sparse, sparse checkerboard) at columns 0, 20, 40
- **Pixel rows 184-191:** White text "Test complete! Press any key..." at column 5 (character row 23)

**Technical Details:**
Demonstrates INT 10h AH=0Eh teletype output in 640x200 graphics mode, direct video memory writes, pixel encoding (1 bit per pixel, 8 pixels per byte), CGA interlaced memory layout, and 80-column character grid (80 chars × 25 rows). Waits for keypress then returns to text mode.

### mode_0dh_ega_320x200x16.asm
EGA graphics mode 0x0D test program. Switches to 320x200, 16-color mode using EGA planar memory at A000:0000. Displays all 16 EGA colors by programming the Sequencer Map Mask register (port 0x3C4/0x3C5) to select which bit planes receive writes.

**Expected Output:**
- **Pixel rows 0-39:** 8 colored boxes (colors 0-7): black, blue, green, cyan, red, magenta, brown, light gray — each 40 pixels wide spanning full screen width
- **Pixel rows 40-47:** Color number labels "0  1  2  3  4  5  6  7" (character row 5)
- **Pixel rows 56-63:** White text "EGA Mode 0x0D - 16 Colors" (character row 7)
- **Pixel rows 72-79:** Light cyan text "320x200, 4 Bit Planes" (character row 9)
- **Pixel rows 88-103:** Color name abbreviations for both rows (character rows 11, 13)
- **Pixel rows 120-159:** 8 colored boxes (colors 8-15): dark gray, light blue, light green, light cyan, light red, light magenta, yellow, white

**Technical Details:**
Demonstrates all 16 EGA colors via the planar memory model (4 bit planes at A000:0000, 40 bytes per row). Each color is drawn by clearing all planes then setting the Map Mask to the color value (each bit enables one plane). Shows EGA linear addressing (offset = row × 40 + column, no interlacing), INT 10h AH=0Eh teletype in 40-column grid, and labeled color swatches. Waits for keypress then returns to text mode.

### color.bas
QBasic program that demonstrates text mode color capabilities. Displays all 16 foreground colors (0-15) with black background, then shows standard background colors (0-7) with white foreground. Illustrates COLOR command usage and text mode color attribute handling.
