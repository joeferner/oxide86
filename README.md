
# Getting Started

## MS-DOS 5.0

1. Download MS-DOS 5.0
1. Create a hard drive `dd if=/dev/zero of=examples/hdd.img bs=1M count=32`
1. Run `RUST_LOG=debug cargo run -p emu86-native -- --boot --floppy-a examples/msdos-5.0/Disk01.img --hdd examples/hdd.img`

# Creating a floppy with files

```
# Create a blank 1.44MB image
mkfs.msdos -C mouse.img 1440

# Copy the file into the image
mcopy -i mouse.img mouse.com ::/
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

1978-06-08 - The Intel 8086 microprocessor was released on June 8, 1978. It was developed by Intel to act as a 16-bit extension of the 8080 microprocessor and served as the foundation for the x86 architecture.
1979-07-01 - The Intel 8088 microprocessor was officially introduced on July 1, 1979. It was a variant of the 8086 with an 8-bit external data bus, widely known for powering the original IBM PC released in 1981.
1981-08 - The IBM Color Graphics Adapter (CGA) was released in August 1981. It was the first color display standard for IBM PCs, offering 16-color, 4-color, or 2-color modes at resolutions up to (640x200) pixels.
1982 - The Intel 80186 (i186) 16-bit microprocessor was introduced in 1982
1982-02 - The Intel 80286 microprocessor (286), a 16-bit CPU featuring memory management and protected mode, was officially released by Intel on February 1, 1982
1984-10 - IBM introduced the Enhanced Graphics Adapter (EGA) in October 1984. It was designed as a higher-resolution successor to CGA, supporting (640x350) pixels with 16 colors. 
1985-10 - The Intel 386 (i386) processor was introduced, making it the first 32-bit x86 chip
1987-04-02 - The Video Graphics Array (VGA) standard was officially released by IBM on April 2, 1987. It introduced a 640x480 resolution with 16 colors.
1987-08 - The original AdLib Music Synthesizer Card was released
1989-10 - The original Sound Blaster, developed by Creative Labs, was officially launched in November 1989