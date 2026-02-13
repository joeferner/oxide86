
# Getting Started

## MS-DOS 5.0

1. Download MS-DOS 5.0
1. Create a hard drive `dd if=/dev/zero of=examples/hdd.img bs=1M count=32`
1. Run `RUST_LOG=info cargo run -p emu86-native-cli -- --boot --floppy-a examples/msdos-5.0/Disk01.img --hdd examples/hdd.img`

## Running

In both the CLI and GUI pressing F12 will exit exclusive mode.

`
RUST_LOG=info cargo run -p emu86-native-cli -- --boot --hdd examples/hdd.img --boot-drive 0x80
RUST_LOG=info cargo run -p emu86-native-gui -- --boot --hdd examples/hdd.img --boot-drive 0x80 --com1 mouse
`

# Creating a floppy with files

```bash
# Create a blank 1.44MB image
emu86-disktools -- format --floppy-1440 test.img

# Copy the file into the image
emu86-disktools -- copy -i test.img my-files/* ::/
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

- Games
  - :white_check_mark: Zork 1
  - :white_check_mark: Alley Cat
  - :white_check_mark: Kings Quest 1
  - :white_check_mark: Flight Simulator 1

# History

| Date | Title | OS Requirement | Description |
| :--- | :--- | :--- | :--- |
| 1978-06-08 | **Intel 8086 Released** | | The foundation of x86 architecture. A 16-bit processor with a 1MB address space. |
| 1979-07-01 | **Intel 8088 Released** | | A cost-effective 8086 variant with an 8-bit external bus, used in the original IBM PC. |
| 1981-08 | IBM CGA Released | | The IBM Color Graphics Adapter was the first color display standard for IBM PCs, offering 16-color, 4-color, or 2-color modes at resolutions up to 640x200 pixels. |
| 1981-08-12 | **MS-DOS 1.0** | 8088/8086 | Released alongside the IBM PC. The definitive OS for the early 16-bit generation. |
| 1982 | Intel 80186 Released | | The Intel 80186 (i186) 16-bit microprocessor was introduced as an enhanced version of the 8086 with integrated peripherals. |
| 1982-01 | **Lotus 1-2-3** | MS-DOS 1.1+ | The "killer app" for the IBM PC. Its performance on the **8088** drove business adoption. |
| 1982-02-01 | **Intel 80286 Released** | | Introduced "Protected Mode" and support for up to 16MB of RAM. |
| 1982-11 | **MS Flight Simulator 1.0** | MS-DOS 1.0+ | A legendary benchmark; if an **8088** clone could run this, it was "100% compatible." |
| 1983-03 | **MS-DOS 2.0** | 8088/8086 | Major rewrite for the PC-XT. Introduced subdirectories (folders) and hard drive support. |
| 1984-08 | **Kings Quest I** | 8088/8086 | King's Quest: Quest for the Crown (1984) is a pioneering graphical adventure game designed by Roberta Williams where players control Sir Graham |
| 1984-08 | **MS-DOS 3.0** | 80286 | Released with the PC/AT. Added support for 1.2MB high-density floppies and 32MB partitions. |
| 1984-10 | IBM EGA Released | | The Enhanced Graphics Adapter was designed as a higher-resolution successor to CGA, supporting 640x350 pixels with 16 colors from a palette of 64. |
| 1984-10 | **Alley Cat** | MS-DOS 1.1+ | A PC classic designed for the **8088**. Famous for its 4-color CGA palette and tight gameplay. |
| 1985-10-17 | **Intel 386 Released** | | First 32-bit x86 CPU. Introduced the flat memory model and hardware multitasking. |
| 1985-11-20 | **Windows 1.0** | MS-DOS 2.0+ | Microsoft’s first GUI. Required 256KB RAM; ran on **8088** but struggled without a **286**. |
| 1987-04 | **MS-DOS 3.3** | 80286/386 | The most stable early version. Added support for multiple 32MB partitions and 1.44MB floppies. |
| 1987-04-02 | **IBM VGA Released** | | Video Graphics Array; introduced the 256-color mode that defined **386/486** gaming. |
| 1987-08 | AdLib Card Released | | The AdLib Music Synthesizer Card brought FM synthesis audio to IBM PCs using the Yamaha YM3812 chip. |
| 1988-07 | **MS-DOS 4.0** | 80286/386 | Introduced the "DOS Shell" visual interface and support for hard drive partitions larger than 32MB. |
| 1988-12 | **Battle Chess** | MS-DOS 2.1+ | One of the first major titles to utilize the high-color VGA standard on **286/386** systems. |
| 1989-04-10 | **Intel 486 Released** | | Featured an integrated FPU (Math Co-processor) and L1 cache, doubling **386** performance. |
| 1989-11 | Sound Blaster Released | | Creative Labs launched the Sound Blaster, which combined AdLib compatibility with digital audio playback and recording capabilities. |
| 1990-05-22 | **Windows 3.0** | MS-DOS 3.1+ | The first massive Windows success; utilized **386** Enhanced Mode for multitasking. |
| 1990-12-14 | **Commander Keen 1-3** | MS-DOS 3.0+ | Revolutionized PC gaming with "Adaptive Tile Refresh," allowing smooth scrolling on **8088/286** PCs. |
| 1991-06 | **MS-DOS 5.0** | 80286/386/486 | Major update; allowed loading drivers into "High Memory," freeing up "Conventional Memory" for games. |
| 1991-09-17 | **Linux Kernel 0.01** | BIOS/386 | Linus Torvalds created Linux specifically to exploit the task-switching features of the **386**. |
| 1992-05-05 | **Wolfenstein 3D** | MS-DOS 3.0+ | The grandfather of FPS games; required a **286** but practically demanded a **386**. |
| 1993-03 | **MS-DOS 6.0 / 6.2** | 80386/486 | Added **DoubleSpace** disk compression and **MemMaker** for automatic RAM optimization. |
| 1993-03-22 | **Intel Pentium Released** | | The "P5" architecture. Superscalar design that could execute two instructions per clock. |
| 1993-12-10 | **DOOM** | MS-DOS 5.0+ | A cultural phenomenon. Pushed the **486** to its absolute limit with pseudo-3D rendering. |
| 1994-06 | **MS-DOS 6.22** | 80386/486 | The final standalone retail version. Replaced DoubleSpace with **DriveSpace** due to legal issues. |
| 1995-08-24 | **Windows 95** | MS-DOS 7.0 | Defined the end of the early era. Required a **386DX** but was the swan song for the **486**. |

# Miscellaneous

## Shorter logs

`cat emu86.log | grep -v ' naga::' | grep -v 'Port 0x0064' | grep -v 'Serial I/O' | grep -v 'IVT Write' | grep -v 'emu86_core' > emu86.log.new; mv emu86.log.new emu86.log`
