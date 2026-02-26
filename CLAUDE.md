# Oxide86 - x86 Emulator

Intel x86 CPU emulator in Rust with native and WebAssembly support.

## Coding Rules

- Code in core must support both native and wasm
- Don't worry about backwards compatibility
- Run ./scripts/pre-commit.sh when done
- Instead of running cargo build or clippy run ./scripts/pre-commit.sh instead
- logs are written to oxide86.log
- when logging unimplemented features use log::warn!

## Resources
- [8086 User Manual](https://edge.edx.org/c4x/BITSPilani/EEE231/asset/8086_family_Users_Manual_1_.pdf)
- [x86 Reference](https://www.felixcloutier.com/x86/)
- [8086 Opcodes](http://www.mlsite.net/8086/)
