
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

### adlib_detection.asm
AdLib (OPL2/YM3812) sound card detection and two-note playback test. Uses the standard IBM AdLib detection sequence: resets timer registers, sets Timer 1 to 0xFF, starts it, waits ~400 µs, then reads the status port (0x388) — bits 7 and 5 must be set (0xC0) for a card to be present. If detected, configures OPL2 channel 0 (FM synthesis, modulator + carrier operators) and plays A4 (440 Hz) then D5 (~587 Hz) as half-second tones, then silences the channel. Outputs "AdLib OPL2 detected" or "AdLib not found" via INT 21h.

**Running:**
```bash
cargo run -p emu86-native-gui -- --sound-card adlib test-programs/audio/adlib_detection.com
cargo run -p emu86-native-cli -- --sound-card adlib test-programs/audio/adlib_detection.com
```

**Expected Output (with `--sound-card adlib`):**
```
AdLib OPL2 detected - playing two notes...
```
Two tones (A4 then D5) play through the audio output, approximately 0.5 seconds each.

**Expected Output (without AdLib):**
```
AdLib not found (status check failed)
```

## CD-ROM

### cdrom/cdrom_detect.asm
MSCDEX CD-ROM detection and ISO 9660 sector-read test. Uses INT 2Fh AX=1500h to check for MSCDEX (Microsoft CD-ROM Extension), prints the drive count and MSCDEX version, then reads sector 16 (the Primary Volume Descriptor) via INT 13h on drive 0xE0 and verifies the "CD001" ISO 9660 signature.

**Running:**
```bash
# Build first
nasm -f bin test-programs/cdrom/cdrom_detect.asm -o test-programs/cdrom/cdrom_detect.com

# Run with an ISO image
cargo run -p emu86-native-cli -- --cdrom my_disk.iso test-programs/cdrom/cdrom_detect.com
cargo run -p emu86-native-gui -- --cdrom my_disk.iso test-programs/cdrom/cdrom_detect.com
```

**Expected Output (with a valid ISO 9660 image):**
```
MSCDEX detected: 1 CD-ROM drive(s)
MSCDEX version: 2.00
Reading ISO 9660 PVD (sector 16)... OK - CD001 signature found
```

**Expected Output (without --cdrom):**
```
No CD-ROM / MSCDEX not found
```

**Technical Details:**
INT 2Fh AX=1500h (MSCDEX install check) returns AX=0xADAD and BX=drive count when the MSCDEX shim is active. INT 2Fh AX=150Ch returns the MSCDEX version in BX (BH=major, BL=minor). INT 13h AH=02h reads sectors from drive 0xE0 (first CD-ROM slot); since CD sectors are 2048 bytes but INT 13h uses 512-byte addressing, LBA sector 16 (ISO PVD) maps to INT 13h sector 65 (CHS cylinder=0, head=0, sector=65 using 1-based sector numbering).

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

### opcode-test/op8086.asm
Comprehensive test suite for validating CPU instruction implementation. Tests 55 different instruction categories with multiple test cases each, reporting results to COM1 serial logger. Covers approximately 70% of the 8086 instruction set.

**Running with serial logger:**
```bash
# Native CLI
cargo run -p emu86-native-cli -- test-programs/opcode-test/op8086.com --com1-device logger

# Native GUI
cargo run -p emu86-native-gui -- test-programs/opcode-test/op8086.com --com1-device logger
```

**Expected Output:**
```
[COM1] === emu86 Opcode Test Suite ===
[COM1] MOV: PASS
[COM1] ADD: PASS
[COM1] SUB: PASS
[COM1] INC: PASS
[COM1] DEC: PASS
[COM1] NEG: PASS
[COM1] CMP: PASS
[COM1] AND: PASS
[COM1] OR: PASS
[COM1] XOR: PASS
[COM1] NOT: PASS
[COM1] TEST: PASS
[COM1] SHL: PASS
[COM1] SHR: PASS
[COM1] ROL: PASS
[COM1] ROR: PASS
[COM1] MUL: PASS
[COM1] DIV: PASS
[COM1] PUSH/POP: PASS
[COM1] LODSB: PASS
[COM1] STOSB: PASS
[COM1] MOVSB: PASS
[COM1] CMPSB: PASS
[COM1] SCASB: PASS
[COM1] ADC/SBB: PASS
[COM1] IMUL: PASS
[COM1] IDIV: PASS
[COM1] RCL/RCR: PASS
[COM1] LOOPZ/LOOPNZ: PASS
[COM1] DAA/DAS: PASS
[COM1] AAA/AAS: PASS
[COM1] AAM/AAD: PASS
[COM1] JO/JNO: PASS
[COM1] JP/JNP: PASS
[COM1] LDS/LES: PASS
[COM1] RET imm: PASS
[COM1] XCHG: PASS
[COM1] LEA: PASS
[COM1] CBW/CWD: PASS
[COM1] LAHF/SAHF: PASS
[COM1] XLATB: PASS
[COM1] SAR: PASS
[COM1] LODSW: PASS
[COM1] STOSW: PASS
[COM1] MOVSW: PASS
[COM1] CMPSW: PASS
[COM1] SCASW: PASS
[COM1] Conditional Jumps: PASS
[COM1] JMP: PASS
[COM1] CALL/RET: PASS
[COM1] JCXZ/LOOP: PASS
[COM1] CLC/STC/CMC: PASS
[COM1] CLD/STD/CLI/STI: PASS
[COM1] RETF: PASS
[COM1] PUSHF/POPF: PASS
[COM1]
[COM1] --- Summary ---
[COM1] 55 passed, 0 failed
```

**Tested Instructions:**

*Basic Arithmetic:*
- **MOV**: Register-to-register data movement, immediate values
- **ADD**: Addition with carry flag, overflow detection
- **ADC**: Add with carry for multi-word arithmetic (32-bit addition)
- **SUB**: Subtraction with borrow flag, underflow detection
- **SBB**: Subtract with borrow for multi-word arithmetic (32-bit subtraction)
- **INC**: Increment operations, wrap-around behavior
- **DEC**: Decrement operations, wrap-around behavior
- **NEG**: Two's complement negation
- **CMP**: Comparison operations, flag setting (ZF, CF, SF)

*Logical Operations:*
- **AND**: Bitwise AND operations, masking patterns
- **OR**: Bitwise OR operations, bit setting patterns
- **XOR**: Bitwise XOR operations, self-zeroing, bit flipping
- **NOT**: Bitwise NOT (one's complement)
- **TEST**: Bitwise AND without storing result (flags only)

*Shift/Rotate Operations:*
- **SHL**: Shift left operations, carry flag propagation
- **SHR**: Shift right operations, carry flag propagation
- **ROL**: Rotate left (bit wrap without carry)
- **ROR**: Rotate right (bit wrap without carry)
- **RCL**: Rotate through carry left (9-bit/17-bit rotation)
- **RCR**: Rotate through carry right (9-bit/17-bit rotation)

*Multiply/Divide:*
- **MUL**: Unsigned multiply (8-bit and 16-bit)
- **IMUL**: Signed multiply with negative numbers
- **DIV**: Unsigned divide with quotient and remainder
- **IDIV**: Signed divide with negative numbers

*Stack Operations:*
- **PUSH/POP**: Stack operations, LIFO verification, SP tracking

*String Operations:*
- **LODSB**: Load string byte (DS:SI → AL, increment/decrement SI)
- **STOSB**: Store string byte (AL → ES:DI, increment/decrement DI)
- **MOVSB**: Move string byte (DS:SI → ES:DI)
- **CMPSB**: Compare string byte with REP prefixes
- **SCASB**: Scan string byte (find character with REPNE)

*Loop Instructions:*
- **LOOPZ/LOOPE**: Loop while zero flag set
- **LOOPNZ/LOOPNE**: Loop while zero flag clear

*BCD Arithmetic:*
- **DAA/DAS**: Decimal adjust for addition/subtraction (packed BCD)
- **AAA/AAS**: ASCII adjust for addition/subtraction (unpacked BCD)
- **AAM/AAD**: ASCII adjust for multiply/divide (base-10 conversion)

*Conditional Jumps:*
- **JO/JNO**: Jump on overflow/no overflow flag
- **JP/JNP**: Jump on parity/no parity (even/odd bit count)

*Segment Operations:*
- **LDS/LES**: Load far pointer to DS:SI or ES:DI from memory

*Advanced Stack:*
- **RET imm**: Return with immediate stack cleanup (stdcall convention)

*Data Transfer Extensions:*
- **XCHG**: Exchange register/memory values (includes NOP as XCHG AX,AX)
- **LEA**: Load effective address (calculate address without dereferencing)
- **CBW**: Convert byte to word (sign-extend AL to AX)
- **CWD**: Convert word to doubleword (sign-extend AX to DX:AX)
- **LAHF/SAHF**: Load/store flags register via AH
- **XLATB**: Translate byte via lookup table (AL = [BX + AL])

*Shift Extensions:*
- **SAR**: Shift arithmetic right (sign-extending shift)

*String Word Operations:*
- **LODSW**: Load string word (DS:SI → AX, SI±2)
- **STOSW**: Store string word (AX → ES:DI, DI±2)
- **MOVSW**: Move string word (DS:SI → ES:DI, SI±2, DI±2) with REP
- **CMPSW**: Compare string word with REPE/REPNE prefixes
- **SCASW**: Scan string word (find word with REPNE)

*Control Flow:*
- **Conditional Jumps**: JA, JAE, JB, JBE, JE/JZ, JNE/JNZ, JG, JGE, JL, JLE, JS, JNS (all flag-based branches)
- **JMP**: Unconditional jump (short and near)
- **CALL/RET**: Near call and return with nesting
- **JCXZ**: Jump if CX register is zero
- **LOOP**: Basic loop (decrement CX, jump if non-zero)

*Flag Operations:*
- **CLC/STC/CMC**: Clear/set/complement carry flag
- **CLD/STD**: Clear/set direction flag (affects string operations)
- **CLI/STI**: Clear/set interrupt flag (enable/disable interrupts)
- **PUSHF/POPF**: Push/pop flags register to/from stack

*Advanced Control:*
- **RETF**: Far return (restore CS:IP from stack)

**Technical Details:**
Each test includes multiple assertions checking both result values and CPU flags. Tests validate edge cases like carry propagation in multi-word arithmetic, sign extension in signed operations, proper flag behavior in conditional loops, BCD/ASCII decimal arithmetic, overflow detection, parity calculation, far pointer loading, stack cleanup on return, effective address calculation, register exchange, arithmetic vs logical shifts, word string operations with direction flag, all conditional jump conditions (unsigned, signed, equality, sign, carry, overflow, parity), flag manipulation instructions, and interrupt/direction flag control. Failed tests print specific error messages (e.g., "carry/borrow propagation incorrect", "signed multiply incorrect", "BCD decimal adjust incorrect", "exchange incorrect", "effective address incorrect", "conditional jump incorrect"). The test framework validates all critical 8086 instructions needed for real-world programs including MS-DOS applications, games, and commercial software requiring BCD arithmetic, stdcall conventions, string processing, table lookups, and complex control flow.

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

### mode_13h_vga_320x200x256.asm
VGA graphics mode 0x13 test program. Switches to 320x200, 256-color mode using the VGA linear framebuffer at A000:0000 (1 byte per pixel, offset = y × 320 + x). Displays all 256 colors as a 16×16 grid of color blocks, then shows mode info text at the bottom.

**Running (requires VGA card):**
```bash
cargo run -p emu86-native-gui -- --video-card vga test-programs/video/mode_13h_vga_320x200x256.com
```

**Expected Output:**
- **Pixel rows 0-191:** 16×16 grid of 20×12 pixel color swatches covering all 256 palette entries (arranged left-to-right, top-to-bottom by color index)
- **Character row 22:** White text "VGA Mode 13h - 256 Color Palette"
- **Character row 23:** Yellow text "320x200, 1 Byte Per Pixel (Linear)"

**Technical Details:**
Demonstrates VGA mode 0x13 (INT 10h AH=00h AL=13h) with direct linear framebuffer access via A000:0000. Unlike CGA (interlaced) or EGA (planar), mode 13h uses simple linear addressing: one byte per pixel, each byte is a color index (0-255) into the VGA DAC palette. Shows both direct memory writes for graphics and INT 10h AH=0Eh text output in graphics mode. The default VGA DAC palette maps indices 0-15 to standard EGA colors and 16-255 to other colors. Waits for keypress then returns to text mode.

### color.bas
QBasic program that demonstrates text mode color capabilities. Displays all 16 foreground colors (0-15) with black background, then shows standard background colors (0-7) with white foreground. Illustrates COLOR command usage and text mode color attribute handling.
