
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
