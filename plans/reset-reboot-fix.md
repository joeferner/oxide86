# Fix: Reset Button Should Re-Boot Computer

## Problem
In the WASM interface, clicking the Reset button only calls `cpu.reset()` which sets CPU registers to the BIOS ROM location (0xF000:0xFFF0). Since no BIOS ROM is loaded, this doesn't actually re-boot the computer. Users expect reset to restart the boot process.

## Current Behavior
- `Computer::reset()` only calls `cpu.reset()`
- CPU registers are reset but no boot sector is reloaded
- Computer appears frozen after reset

## Desired Behavior
- Reset should re-boot from the same drive that was originally booted from
- Video should clear and boot process should start fresh
- All state should be reset (CPU, memory, BIOS state)

## Solution

### Option 1: Track Boot Drive and Auto Re-Boot (Recommended)
1. Add `boot_drive: Option<DriveNumber>` field to `Computer` struct
2. Store drive number when `boot()` is called
3. Modify `reset()` to:
   - Call `cpu.reset()`
   - Clear video buffer
   - Re-call `boot(boot_drive)` if a drive was previously booted
4. Update WASM `reset()` method to automatically re-boot

Pros:
- Matches user expectation (reset = reboot)
- Simple to implement
- No JS changes needed beyond what exists

Cons:
- Need to handle boot failures gracefully

### Option 2: Separate Reset and Reboot Methods
1. Keep `reset()` as CPU-only reset
2. Add new `reboot()` method that re-boots from tracked drive
3. Update WASM binding to call `reboot()` instead of `reset()`

Pros:
- Clear separation of concerns
- More flexible

Cons:
- More complex API

## Implementation Plan (Option 1)

### Step 1: Modify Computer struct
File: `core/src/computer.rs`
- Add `boot_drive: Option<DriveNumber>` field to struct
- Initialize to `None` in `new()`

### Step 2: Track boot drive
File: `core/src/computer.rs`
- In `boot()` method, store drive: `self.boot_drive = Some(drive);`

### Step 3: Implement proper reset
File: `core/src/computer.rs`
- Modify `reset()` to:
  ```rust
  pub fn reset(&mut self) {
      self.cpu.reset();
      self.video.clear();
      if let Some(drive) = self.boot_drive {
          let _ = self.boot(drive); // Ignore errors on reset
      }
  }
  ```

### Step 4: Update WASM binding (if needed)
File: `wasm/src/lib.rs`
- The existing `reset()` method should now work as expected
- Consider updating to return Result and show error status

## Testing
1. Load a disk image
2. Boot from it
3. Click Reset button
4. Verify computer reboots and shows boot screen again
5. Test with both floppy and hard drive boots
