
# Getting Started

## MS-DOS 5.0

1. Download MS-DOS 5.0
1. Create a hard drive `dd if=/dev/zero of=examples/hdd.img bs=1M count=32`
1. Run `RUST_LOG=info cargo run -p emu86-native -- --boot --floppy-a examples/msdos-5.0/Disk01.img --hdd examples/hdd.img`

## Running

In both the CLI and GUI pressing F12 will exit exclusive mode.

`
RUST_LOG=info cargo run -p emu86-native -- --boot --hdd examples/hdd.img --boot-drive 0x80
RUST_LOG=info cargo run -p emu86-native-gui -- --boot --hdd examples/hdd.img --boot-drive 0x80 --com1 mouse
`

# Creating a floppy with files

```
# Create a blank 1.44MB image
mkfs.msdos -C mouse.img 1440

# Copy the file into the image
mcopy -i mouse.img -s * ::
```

# Compatibility

:white_check_mark: - Tested
:hourglass: - Partially Working
:x: - Does not work

- OS
  - :white_check_mark: MS-DOS 2.11
  - :white_check_mark: MS-DOS 3.31
  - :white_check_mark: MS-DOS 4.01
  - :white_check_mark: MS-DOS 5.0

# History

| Date | Title | Description |
|------|-------|-------------|
| 1978-06-08 | Intel 8086 Released | The Intel 8086 microprocessor was developed as a 16-bit extension of the 8080 microprocessor and served as the foundation for the x86 architecture. |
| 1979-07-01 | Intel 8088 Released | A variant of the 8086 with an 8-bit external data bus. This processor powered the original IBM PC released in 1981. |
| 1981-08 | IBM CGA Released | The IBM Color Graphics Adapter was the first color display standard for IBM PCs, offering 16-color, 4-color, or 2-color modes at resolutions up to 640x200 pixels. |
| 1982 | Intel 80186 Released | The Intel 80186 (i186) 16-bit microprocessor was introduced as an enhanced version of the 8086 with integrated peripherals. |
| 1982-02-01 | Intel 80286 Released | The Intel 80286 (286) was a 16-bit CPU featuring memory management and protected mode, enabling multitasking and extended memory access. |
| 1984-10 | IBM EGA Released | The Enhanced Graphics Adapter was designed as a higher-resolution successor to CGA, supporting 640x350 pixels with 16 colors from a palette of 64. |
| 1985-10 | Intel 386 Released | The Intel 386 (i386) processor was the first 32-bit x86 chip, introducing 32-bit registers and a flat memory model. |
| 1987-04-02 | IBM VGA Released | The Video Graphics Array standard introduced 640x480 resolution with 16 colors (or 320x200 with 256 colors), becoming the de facto standard for PC graphics. |
| 1987-08 | AdLib Card Released | The AdLib Music Synthesizer Card brought FM synthesis audio to IBM PCs using the Yamaha YM3812 chip. |
| 1989-11 | Sound Blaster Released | Creative Labs launched the Sound Blaster, which combined AdLib compatibility with digital audio playback and recording capabilities. |

# Miscellaneous

## Shorter logs

`cat emu86.log | grep -v ' naga::' | grep -v 'Port 0x0064' | grep -v 'Serial I/O' | grep -v 'IVT Write' | grep -v 'emu86_core' > emu86.log.new; mv emu86.log.new emu86.log`
