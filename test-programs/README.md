
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

## Keyboard

### waitkey.asm
Tests INT 16h AH=00h blocking keyboard input behavior. Waits for keypresses and echoes them back to the console. Demonstrates blocking keyboard reads, scan code/ASCII code handling, and prints each key pressed. Press ESC to exit.

## Misc

### hello_program.asm
Simple "Hello World" test program for verifying program loading functionality. Uses INT 21h AH=09h to display a message and INT 21h AH=4Ch to exit. Minimal .COM file structure starting at CS:0100h.

## Video

### cga_graphics.asm
Comprehensive CGA graphics mode test program. Switches to mode 0x04 (320x200, 4 colors) and tests both text rendering and graphics drawing in graphics mode.

**Expected Output:**
- **Pixel rows 0-39:** Three colored boxes at top (cyan, magenta, white) at columns 0, 20, 40
- **Pixel rows 48-63:** White text "CGA Graphics Mode 0x04 Test" (character row 6)
- **Pixel rows 64-71:** Magenta text "Drawing test patterns..." (character row 8)
- **Pixel rows 100-139:** Three colored boxes at middle (cyan, pattern, pattern) at columns 0, 20, 40
- **Pixel rows 184-191:** Cyan text "Test complete! Press any key..." at column 2 (character row 23)

**Technical Details:**
Demonstrates INT 10h AH=0Eh teletype output in graphics mode (draws 8x8 characters pixel-by-pixel using CP437 font), direct video memory writes at 0xB8000, pixel encoding (2 bits per pixel, 4 pixels per byte), CGA interlaced memory layout (even scan lines at 0x0000-0x1F3F, odd lines at 0x2000-0x3F3F), palette selection via port 0x3D9 (palette 1 with intensity: cyan, magenta, white), and proper cursor positioning in 40-column character grid (40 chars × 25 rows). Waits for keypress then returns to text mode.

### color.bas
QBasic program that demonstrates text mode color capabilities. Displays all 16 foreground colors (0-15) with black background, then shows standard background colors (0-7) with white foreground. Illustrates COLOR command usage and text mode color attribute handling.
