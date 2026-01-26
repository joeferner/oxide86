
# Getting Started

## MS-DOS 5.0

1. Download MS-DOS 4.01
1. Create a hard drive `dd if=/dev/zero of=examples/hdd.img bs=1M count=32`
1. Run `RUST_LOG=debug cargo run -p emu86-native -- --boot --floppy-a examples/msdos-4.01/Disk00.img --hdd examples/hdd.img`
1. `A> fdisk`