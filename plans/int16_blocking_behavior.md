# INT 16h AH=00h Blocking Behavior Implementation

## Problem Statement
INT 16h AH=00h (Read Character) is a blocking BIOS function that should wait for keyboard input.

**Initial Issues:**
- **Terminal (CLI)**: `read_key()` was returning `None` when F12 was pressed, causing "No key available (unexpected in blocking mode)" warning
- **GUI**: Returns `None` immediately when no key buffered, causing DOS programs to spin-loop
- **WASM**: Returns `None` immediately when no key buffered, causing DOS programs to spin-loop

The spin-loop approach wastes CPU cycles and creates poor user experience.

## Solution Overview
Implemented a two-part solution:
1. **Terminal**: Fixed `read_key()` to loop when F12 is intercepted (true blocking)
2. **GUI/WASM**: CPU wait state that pauses execution and retries INT 16h when key arrives

## Implementation Details

### Part 1: Terminal Keyboard Fix
**File**: `native/src/terminal_keyboard.rs`

**Problem**: When F12 was pressed, `read_key()` returned `None` to hide F12 from the emulated program, but INT 16h AH=00h is a blocking call.

**Solution**: Loop until a non-F12 key is pressed:
```rust
fn read_key(&mut self) -> Option<KeyPress> {
    // Check buffered keys first
    if let Some(key) = self.keyboard_buffer.pop_front() {
        return Some(key);
    }

    // Block and loop to handle F12 interception
    loop {
        let key = self.internal_read_key()?;
        if key.scan_code == SCAN_CODE_F12 {
            self.command_mode_requested = true;
            continue;  // Keep blocking for next key
        }
        return Some(key);
    }
}
```

### Part 2: CPU Wait State for GUI/WASM
**Files**:
- `core/src/cpu/mod.rs`
- `core/src/cpu/bios/int16.rs`
- `core/src/computer.rs`

#### 2.1 CPU Wait State Enum
```rust
pub enum CpuWaitState {
    Running,
    WaitingForKeyboardInt16,  // Waiting for INT 16h retry
}
```

The variant name explicitly indicates that INT 16h needs to be retried when resumed.

#### 2.2 INT 16h Handler Changes
When no key is available, instead of returning 0:
```rust
} else {
    // No key available - enter wait state
    log::debug!("INT 16h AH=00h: No key available, entering wait state");
    self.set_waiting_for_keyboard();
    // Don't modify AX - INT will be retried when resumed
}
```

**Critical Design Decision**: The INT instruction has already completed when the handler runs (IP advanced, stack pushed, etc.). We can't simply rewind IP because IRET will return to the wrong place. Instead, we mark that INT 16h needs to be retried and handle it in the execution loop.

#### 2.3 Execution Loop Changes
`Computer::step()` checks for wait state and retries INT 16h:
```rust
if self.cpu.is_waiting_for_keyboard() {
    if self.bios.check_key().is_some() {
        log::debug!("Key available, resuming from wait state and retrying INT 16h");
        if self.cpu.resume_from_wait() {  // Returns true for INT 16h retry
            // Directly call INT 16h handler again
            self.cpu.int16_read_char(&mut self.memory, &mut self.bios);
            return;
        }
    } else {
        // Still waiting - return without executing
        return;
    }
}
```

**Why Direct Handler Call Works**:
- The original INT 16h completed and returned (IRET executed)
- CPU is sitting at the instruction after the INT
- When we retry, we call the handler directly (not via INT instruction)
- Handler reads the key and sets AX
- Execution continues from the instruction after the INT
- From the program's perspective, INT 16h blocked and returned the key

#### 2.4 Helper Methods
```rust
// CPU methods
pub fn is_waiting_for_keyboard(&self) -> bool
pub fn set_waiting_for_keyboard(&mut self)
pub fn resume_from_wait(&mut self) -> bool  // Returns true if INT 16h retry needed

// Make handler accessible from Computer
pub(crate) fn int16_read_char(&mut self, memory: &mut Memory, io: &mut super::Bios)
```

## Platform Behavior

### Terminal (CLI)
- `read_key()` blocks internally and never returns `None` (except for fatal errors)
- Wait state is a safety net, rarely entered
- True blocking at the OS level - no CPU overhead

### GUI
- `read_key()` returns `None` immediately when no key buffered
- CPU enters `WaitingForKeyboardInt16` state
- `step()` returns early without executing instructions
- Main loop continues processing window events
- When key arrives, next `step()` retries INT 16h and resumes

### WASM
- Same as GUI
- `tick()` function returns when CPU is waiting
- JavaScript event loop continues
- Next `tick()` checks for keyboard input and resumes

## Benefits
✅ No more spin-loops wasting CPU in GUI/WASM
✅ Proper BIOS blocking semantics maintained
✅ Works correctly across all platforms
✅ Extensible architecture for future wait states (disk I/O, timer, etc.)
✅ Clean separation: blocking handled differently per platform based on capabilities

## Testing

Created `examples/waitkey.asm` to test INT 16h AH=00h blocking behavior:
- Waits for keypresses in a loop
- Echoes each key pressed
- ESC to exit
- Verifies that blocking works without spin-loop CPU waste

**Test Procedure**:
```bash
cd examples
nasm -f bin waitkey.asm -o waitkey.com
cargo run -p emu86-native -- waitkey.com --segment 0x100 --offset 0x0000
```

**Expected Behavior**:
- Terminal: Should block waiting for input, no CPU usage while waiting
- GUI: Should not spin-loop, responsive UI, low CPU while waiting
- WASM: Should not freeze browser, low CPU while waiting

## Files Modified

1. `native/src/terminal_keyboard.rs` - Loop on F12 interception
2. `core/src/cpu/mod.rs` - Add `CpuWaitState` enum and methods
3. `core/src/cpu/bios/int16.rs` - Enter wait state when no key, make handler public(crate)
4. `core/src/computer.rs` - Check wait state and retry INT 16h
5. `examples/waitkey.asm` - New test program

## Future Extensions

The `CpuWaitState` enum can be extended for other blocking operations:
- `WaitingForDiskInt13` - Retry INT 13h for async disk operations
- `WaitingForTimerInt15` - Block for INT 15h AH=86h wait calls
- `WaitingForSerialInt14` - Block for serial port input

Each variant name indicates which INT handler to retry when the condition is met.
