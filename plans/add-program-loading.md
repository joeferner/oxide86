# Add Program Loading to native-gui and wasm

## Overview
Add capability to load and run programs (like .COM files) similar to how native-cli works, in addition to the existing boot-from-disk functionality.

## Changes Needed

### 1. native-gui (native-gui/src/main.rs)

**CLI Arguments:**
- Add `program: Option<String>` - Path to program binary
- Add `segment: String` with default "0x0000" - Starting segment
- Add `offset: String` with default "0x0100" - Starting offset (.COM file default)
- Make `boot` not required, use `required_unless_present = "program"` on `boot` and vice versa

**Implementation:**
- In `create_computer()` function, after handling boot mode:
  - Check if `!cli.boot && cli.program.is_some()`
  - Read program file with `std::fs::read`
  - Parse segment/offset with `parse_hex_or_dec`
  - Call `computer.load_program(&program_data, segment, offset)`
  - Log loading info

### 2. wasm (wasm/src/lib.rs)

**New Method:**
- Add `pub fn load_program(&mut self, data: Vec<u8>, segment: u16, offset: u16) -> Result<(), JsValue>`
- Take program data as Vec<u8> from JavaScript
- Call `self.computer.load_program(&data, segment, offset)`
- Log loading info
- Return Result for error handling

**JavaScript Integration:**
- Expose method via #[wasm_bindgen]
- Allow web UI to upload .COM files and load them

## Testing
- native-gui: Test with simple .COM file
- wasm: Test with JavaScript calling load_program()

## Notes
- Both boot and program loading should be mutually exclusive (use one or the other)
- Default segment:offset of 0x0000:0x0100 matches .COM file standard
- Error handling for file read failures and invalid addresses
