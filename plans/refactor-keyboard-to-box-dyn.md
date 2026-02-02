# Refactor KeyboardInput from Generic to Box<dyn>

## Goal
Change `Bios<K: KeyboardInput>` to `Bios` with `keyboard: Box<dyn KeyboardInput>` to simplify the codebase.

## Changes Required

### 1. Core Library

**core/src/keyboard.rs**
- No changes needed - trait definition stays the same

**core/src/cpu/bios/mod.rs**
- Change `Bios<K: KeyboardInput>` to `Bios`
- Change `keyboard: K` to `keyboard: Box<dyn KeyboardInput>`
- Remove generic parameter from all `impl<K: KeyboardInput> Bios<K>` blocks
- Update `new()` to take `Box<dyn KeyboardInput>`

**core/src/computer.rs**
- Change `Computer<K: KeyboardInput, V: VideoController>` to `Computer<V: VideoController>`
- Update `new()` to take `Box<dyn KeyboardInput>`
- Remove K from bios() return type

**core/src/cpu/mod.rs**
- Remove K generic from `execute_int_with_io` and `execute_with_io`
- Change `Bios<K>` to `Bios`

### 2. BIOS Interrupt Handlers
All files in core/src/cpu/bios/:
- int08.rs, int13.rs, int14.rs, int15.rs, int16.rs, int17.rs
- int1a.rs, int1c.rs, int20.rs, int21.rs, int25.rs, int2f.rs
- int33.rs, int35_3f.rs

For each: Remove `<K: KeyboardInput>` from `impl Bios<K>` blocks.

### 3. Platform-Specific

**native/src/main.rs**
- Box the keyboard: `Box::new(TerminalKeyboard::new())`

**native-gui/src/main.rs**
- Box the keyboard: `Box::new(GuiKeyboard::new())`

## Implementation Order
1. Update core/src/cpu/bios/mod.rs (Bios struct and impl)
2. Update all interrupt handlers (remove generic)
3. Update core/src/cpu/mod.rs (CPU execute methods)
4. Update core/src/computer.rs (Computer struct)
5. Update native/src/main.rs
6. Update native-gui/src/main.rs
7. Run pre-commit.sh to verify
