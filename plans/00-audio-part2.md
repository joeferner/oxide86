# PC Speaker/Timer Interrupt Fix for QBASIC PLAY Command

## Problem Summary
QBASIC's PLAY command causes a soft lock because the emulator doesn't fire INT 0x08 (timer interrupt) or INT 0x1C (user timer tick). QBASIC installs an INT 0x1C handler that updates memory locations [0x24ba] and [0x24bc], but these locations never change because the interrupts aren't being fired, causing an infinite polling loop.

## Root Cause Analysis
From the log analysis:
- QBASIC is polling memory at [0x24ba] and [0x24bc], waiting for value to change from 0x0006 to 0x0002
- These are in QBASIC's program data segment (around segment 0x024B)
- QBASIC's PLAY routine expects INT 0x1C to fire periodically to update sound state
- The emulator currently:
  - ✅ Updates BDA timer counter at 0x6C every 18.2 Hz
  - ✅ Has full PIT (8253/8254) emulation for PC speaker
  - ✅ Has I/O port 0x61 (system control port) implemented
  - ✅ Has working speaker output via rodio
  - ❌ Does NOT fire INT 0x08 (timer hardware interrupt)
  - ❌ Does NOT fire INT 0x1C (user timer tick)

## PC Timer Interrupt Architecture
Standard PC BIOS behavior:
1. **IRQ0** fires 18.2 times per second (every ~55ms)
2. **IRQ0** triggers **INT 0x08** (timer hardware interrupt)
3. **INT 0x08** BIOS handler:
   - Increments BDA timer counter at 0x0040:0x006C
   - Calls **INT 0x1C** (user timer tick hook)
   - Returns with IRET
4. **INT 0x1C** default handler: just IRET (no-op)
5. **Programs install custom INT 0x1C handlers** for periodic tasks (music, animation, etc.)

## Implementation Plan

### 1. Update Computer::increment_cycles
**File:** `core/src/computer.rs` (lines 605-642)

Modify increment_cycles to:
- Keep PIT and speaker update logic
- Remove BDA timer counter update (move to INT 0x08 handler)
- Add timer interrupt firing when tick threshold reached
- Queue timer interrupt like keyboard/serial IRQs

### 2. Add Timer Interrupt Queue
**File:** `core/src/computer.rs`

Add to Computer struct:
- `pending_timer_irqs: u32` counter for queued timer ticks

Add method:
- `fire_timer_irq()` similar to `fire_keyboard_irq()` and `fire_serial_irq()`

### 3. Initialize IVT Entries
**File:** `core/src/memory.rs`

Add IVT initialization for:
- INT 0x08 vector at 0x0000:0x0020 (4 bytes: offset, segment)
- INT 0x1C vector at 0x0000:0x0070 (4 bytes: offset, segment)
- Point both to stub handlers in BIOS ROM area (F000:xxxx)

## Critical Files to Modify
1. ~~`core/src/cpu/bios/int08.rs` - CREATE~~ ✅ DONE
2. ~~`core/src/cpu/bios/int1c.rs` - CREATE~~ ✅ DONE
3. ~~`core/src/cpu/bios/mod.rs` - UPDATE (add handlers, module refs)~~ ✅ DONE
4. `core/src/computer.rs` - UPDATE (fire timer IRQ, move BDA update)
5. `core/src/memory.rs` - UPDATE (IVT initialization)

## Implementation Details

### INT 0x08 Handler (✅ IMPLEMENTED in `core/src/cpu/bios/int08.rs`)
The handler:
- Increments BDA timer counter at 0x0040:0x006C
- Checks for midnight rollover (sets flag at 0x0040:0x0070)
- Chains to INT 0x1C via `chain_to_interrupt()` method which properly handles both BIOS and user-installed handlers

### INT 0x1C Handler (✅ IMPLEMENTED in `core/src/cpu/bios/int1c.rs`)
Default BIOS handler is a no-op. Programs can install custom handlers via IVT modification.

### fire_timer_irq() Pattern (TO IMPLEMENT)
```rust
// Follow existing fire_keyboard_irq() pattern:
1. Read IVT entry for INT 0x08
2. Push FLAGS, CS, IP
3. Clear IF and TF flags
4. Jump to handler CS:IP

### Modified increment_cycles
```rust
fn increment_cycles(&mut self, cycles: u64) {
    self.cycle_count += cycles;
    self.total_cycles += cycles;

    self.io_device.update_pit(cycles);
    self.update_speaker();

    // Fire timer interrupt when tick threshold reached
    while self.cycle_count >= self.cycles_per_tick {
        self.cycle_count -= self.cycles_per_tick;
        self.pending_timer_irqs += 1;
    }
}
```

### Process pending timer IRQs in step()
In `Computer::step()` after keyboard/serial IRQ processing:
```rust
if self.pending_timer_irqs > 0 {
    self.pending_timer_irqs -= 1;
    self.fire_timer_irq();
    return;
}
```

## Testing Plan

### 1. Basic Timer Test
Create test that:
- Installs custom INT 0x1C handler that increments a counter
- Runs for several ticks
- Verifies counter incremented

### 2. QBASIC PLAY Test
Run the original QBASIC program with PLAY command:
- Should no longer soft lock
- Memory at [0x24ba] and [0x24bc] should update
- Music should play (or at least not hang)

### 3. BDA Timer Counter Test
Verify BDA counter at 0x0040:0x006C:
- Still increments at 18.2 Hz
- Midnight rollover still works
- INT 1Ah services still work

## Edge Cases to Handle

1. **Nested interrupts**: If INT 0x1C takes too long and another timer tick occurs
   - Queue pending_timer_irqs counter handles this

2. **Interrupt enable flag**: Only fire if IF flag is set
   - Check in fire_timer_irq() before firing
   - Or queue and fire when IF becomes set again

3. **Halt state**: Timer interrupt should wake CPU from HLT
   - fire_timer_irq() should clear cpu.halted flag

4. **Multiple pending ticks**: Use counter not bool
   - Prevents lost ticks if multiple accumulate

## Notes
- PIT Channel 0 already runs at correct frequency (18.2 Hz via cycles_per_tick)
- Speaker/sound hardware already fully functional
- Only missing piece is interrupt delivery mechanism
- This matches real PC hardware behavior where IRQ0 fires INT 0x08
