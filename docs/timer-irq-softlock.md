# Timer IRQ Soft Lock Issue

## Problem

CheckIt 2.1 hangs indefinitely during "Determine System Components" phase. The program appears stuck in a timing loop waiting for the system timer to advance.

## Symptoms

- BDA timer counter at `0040:006C` never increments
- Program polls INT 0x1A (get system time) in a tight loop
- No progress regardless of how long the emulator runs

## Root Cause

The Interrupt Flag (IF) was stuck at 0, preventing INT 0x08 timer hardware interrupts from firing. The timer tick mechanism in oxide86 works as follows:

1. CPU executes instructions, accumulating cycles
2. `increment_cycles()` queues timer IRQs when cycle threshold reached (~262088 cycles = 1 tick at 18.2 Hz)
3. `step()` checks for pending timer IRQs and calls `fire_timer_irq()`
4. `fire_timer_irq()` only fires if IF=1; otherwise the IRQ stays queued
5. INT 0x08 handler increments BDA timer counter

The issue was that programs like CheckIt perform disk operations or time queries that:
1. Call INT (which clears IF as part of the interrupt mechanism)
2. The BIOS handler executes and returns via IRET
3. IRET restores FLAGS from the stack, but the saved FLAGS often had IF=0
4. Timer IRQs accumulate but never fire because IF never becomes 1 at the right moment

## Solution

### 1. AT-class BIOS Behavior (STI in Handlers)

Real AT-class BIOS implementations enable interrupts (STI) during disk and time operations. This allows timer IRQs to fire even during extended disk I/O:

**`core/src/cpu/bios/int13.rs`:**
```rust
pub(super) fn handle_int13(&mut self, memory: &mut Memory, io: &mut super::Bios) {
    // Enable interrupts during disk operations (AT-class BIOS behavior)
    self.set_flag(cpu_flag::INTERRUPT, true);
    // ... rest of handler
}
```

**`core/src/cpu/bios/int1a.rs`:**
```rust
pub(super) fn handle_int1a(&mut self, memory: &mut Memory, io: &mut super::Bios) {
    // Enable interrupts during time services (allows timer IRQs to fire)
    self.set_flag(cpu_flag::INTERRUPT, true);
    // ... rest of handler
}
```

### 2. Inline Timer IRQ Processing

The STI alone wasn't sufficient because IRET immediately restores IF=0 from the saved flags before we can process queued timer IRQs. The fix processes pending timer IRQs inline during F000 BIOS returns while IF=1:

**`core/src/computer.rs`** (in the F000 segment handling):
```rust
// Before restoring flags, check if BIOS handler enabled interrupts (STI)
// and there are pending timer IRQs. If so, handle them now while IF=1.
let if_currently_enabled = self.cpu.get_flag(crate::cpu::cpu_flag::INTERRUPT);
while if_currently_enabled && self.pending_timer_irqs > 0 {
    // Directly call the BIOS INT 0x08 handler to update BDA timer counter
    // This bypasses the full interrupt machinery (no stack frame manipulation)
    self.cpu.handle_bios_interrupt_direct(
        0x08,
        &mut self.memory,
        &mut self.bios,
        &mut self.video,
    );
    self.pending_timer_irqs -= 1;
}
```

This calls `handle_bios_interrupt_direct()` instead of `fire_timer_irq()` because:
- We're already in the middle of handling an F000 call
- `fire_timer_irq()` would push a stack frame pointing back to F000, causing infinite loops
- `handle_bios_interrupt_direct()` just updates the BDA timer counter directly

## Key Insight

The timing window between STI and IRET is normally too short for timer IRQs to fire naturally. Real hardware handles this through the PIC (Programmable Interrupt Controller) latching pending interrupts, but our emulation processes IRQs synchronously at instruction boundaries. By checking for pending timer IRQs immediately after the BIOS handler enables interrupts (but before IRET restores IF=0), we simulate the effect of hardware interrupt latching.

## Files Modified

- `core/src/cpu/bios/int13.rs` - Added STI at handler start
- `core/src/cpu/bios/int1a.rs` - Added STI at handler start
- `core/src/computer.rs` - Added inline timer IRQ processing during F000 returns

## Testing

CheckIt 2.1 now progresses through "Determine System Components" without hanging. The BDA timer counter increments correctly at ~18.2 Hz.
