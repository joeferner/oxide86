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
  - ✅ Fires INT 0x08 (timer hardware interrupt)
  - ✅ Fires INT 0x1C (user timer tick)

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

~~### 1. Update Computer::increment_cycles~~ ✅ DONE
~~### 2. Add Timer Interrupt Queue~~ ✅ DONE
~~### 3. Initialize IVT Entries~~ ✅ DONE (IVT already initializes all 256 vectors)

## Critical Files to Modify
1. ~~`core/src/cpu/bios/int08.rs` - CREATE~~ ✅ DONE
2. ~~`core/src/cpu/bios/int1c.rs` - CREATE~~ ✅ DONE
3. ~~`core/src/cpu/bios/mod.rs` - UPDATE (add handlers, module refs)~~ ✅ DONE
4. ~~`core/src/computer.rs` - UPDATE (fire timer IRQ, move BDA update)~~ ✅ DONE
5. ~~`core/src/memory.rs` - UPDATE (IVT initialization)~~ ✅ DONE (already complete)

## Implementation Details

### INT 0x08 Handler (✅ IMPLEMENTED in `core/src/cpu/bios/int08.rs`)
The handler:
- Increments BDA timer counter at 0x0040:0x006C
- Checks for midnight rollover (sets flag at 0x0040:0x0070)
- Chains to INT 0x1C via `chain_to_interrupt()` method which properly handles both BIOS and user-installed handlers

### INT 0x1C Handler (✅ IMPLEMENTED in `core/src/cpu/bios/int1c.rs`)
Default BIOS handler is a no-op. Programs can install custom handlers via IVT modification.

### fire_timer_irq() (✅ IMPLEMENTED in `core/src/computer.rs`)
- Checks IF flag before firing (queues preserved if interrupts disabled)
- Wakes CPU from HLT state
- Pushes FLAGS, CS, IP onto stack
- Clears IF and TF flags
- Jumps to INT 0x08 handler

### increment_cycles (✅ IMPLEMENTED in `core/src/computer.rs`)
- Updates PIT counters and speaker
- Queues pending_timer_irqs when tick threshold reached
- BDA update moved to INT 0x08 handler

### Process pending timer IRQs in step() (✅ IMPLEMENTED in `core/src/computer.rs`)
Timer IRQs processed after keyboard/serial IRQs with lowest priority.

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

## Notes
- PIT Channel 0 already runs at correct frequency (18.2 Hz via cycles_per_tick)
- Speaker/sound hardware already fully functional
- Timer interrupt delivery mechanism now complete
- This matches real PC hardware behavior where IRQ0 fires INT 0x08
