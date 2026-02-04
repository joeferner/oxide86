# INT 16h AH=00h Blocking Behavior Implementation Plan

## Problem
INT 16h AH=00h (Read Character) is a blocking BIOS function that should wait for keyboard input. Currently:
- **Terminal (CLI)**: Fixed to block internally in `read_key()`
- **GUI**: Returns `None` immediately, causing DOS programs to spin-loop
- **WASM**: Returns `None` immediately, causing DOS programs to spin-loop

The spin-loop approach works but wastes CPU cycles and creates poor user experience.

## Desired Behavior
When INT 16h AH=00h is called and no key is available:
- **Terminal**: Block the thread until a key arrives (already implemented)
- **GUI**: Pause emulation, return control to event loop, resume when key arrives
- **WASM**: Pause emulation, return control to browser, resume when key arrives

## Implementation Strategy

### Option 1: CPU Wait State (Recommended)
Add a wait state to the CPU that pauses instruction execution until a condition is met.

**Architecture:**
```rust
// In core/src/cpu/mod.rs
pub enum CpuWaitState {
    Running,
    WaitingForKeyboard,
    // Future: WaitingForTimer, WaitingForDisk, etc.
}

pub struct Cpu {
    // ... existing fields
    wait_state: CpuWaitState,
}
```

**Changes needed:**

1. **Add wait state to CPU** (`core/src/cpu/mod.rs`):
   - Add `wait_state: CpuWaitState` field
   - Add `pub fn is_waiting_for_keyboard(&self) -> bool`
   - Add `pub fn resume_from_wait(&mut self)`

2. **Update INT 16h handler** (`core/src/cpu/bios/int16.rs`):
   ```rust
   fn int16_read_char(&mut self, memory: &mut Memory, io: &mut super::Bios) {
       // Check buffer first (existing logic)
       if head != tail {
           // Return buffered key (existing logic)
       } else {
           // Try non-blocking read
           if let Some(key) = io.read_key() {
               // Return key (existing logic)
           } else {
               // No key available - enter wait state
               log::debug!("INT 16h AH=00h: No key available, entering wait state");
               self.wait_state = CpuWaitState::WaitingForKeyboard;
               // Don't modify AX - we'll retry the interrupt when resumed
           }
       }
   }
   ```

3. **Update Computer execution loop** (`core/src/computer.rs`):
   ```rust
   pub fn run_single_instruction(&mut self) -> Result<bool, String> {
       // Check if CPU is waiting
       if self.cpu.is_waiting_for_keyboard() {
           // Check if a key is available now
           if self.bios.check_key().is_some() {
               log::debug!("Key available, resuming from wait state");
               self.cpu.resume_from_wait();
               // Fall through to execute instruction
           } else {
               // Still waiting - return without executing
               return Ok(false); // false = keep running but didn't execute
           }
       }

       // Execute instruction (existing logic)
   }
   ```

4. **Update native implementations** (`native/src/main.rs`, `native-gui/src/main.rs`):
   - Main loop already calls `run_single_instruction()` repeatedly
   - When it returns with CPU in wait state, can yield to OS/event loop
   - GUI: Process events, check keyboard buffer, resume if key available

5. **Update WASM implementation** (`wasm/src/lib.rs`):
   - Similar to GUI - when in wait state, return from tick function
   - JavaScript will continue processing events
   - Next tick checks if key arrived and resumes if so

### Option 2: Interrupt Re-execution
Instead of wait state, re-execute the INT 16h instruction when a key becomes available.

**Pros:**
- Simpler - no new CPU state
- INT 16h handler doesn't need special logic

**Cons:**
- Need to rewind IP to re-execute INT instruction
- More complex to implement correctly
- May interfere with interrupt handling

### Option 3: Hybrid Approach
- Terminal: Block in `read_key()` (current implementation)
- GUI/WASM: Use wait state approach (Option 1)
- KeyboardInput trait method indicates if blocking is supported

**Pros:**
- Best of both worlds
- Terminal gets true blocking with no CPU overhead
- GUI/WASM get efficient pause/resume

**Cons:**
- More complex implementation
- Two different code paths

## Recommendation
Implement **Option 1** (CPU Wait State) because:
- Clean, extensible architecture (can add other wait states later)
- Works across all platforms
- Low overhead - no spin-loop wasting CPU
- Maintains proper BIOS semantics

## Implementation Steps
1. Add `CpuWaitState` enum and field to CPU
2. Update INT 16h AH=00h to set wait state when no key available
3. Update `Computer::run_single_instruction()` to check wait state
4. Update native/GUI main loops to handle wait state efficiently
5. Update WASM tick function to handle wait state
6. Test on all platforms (terminal, GUI, WASM)

## Testing
- DOS programs that wait for input (e.g., `PAUSE`, `CHOICE`, `EDIT`)
- Verify no spin-loop in GUI/WASM (check CPU usage)
- Verify F12 still works in terminal
- Verify rapid key presses don't lose characters
