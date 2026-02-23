# PIC Interrupt-In-Service (ISR) Fix for Re-entrant Keyboard IRQ

## Problem

When a game's custom INT 09h handler executes `STI` early (as Sierra AGI / KQ2 does at
`10B2:5F6F`), a queued keyboard release event can fire **re-entrantly** before the press
handler has read port 0x60. This overwrites port 0x60 with the break code, so the press
handler reads stale data and never takes its key-specific code path.

### Concrete failure trace (KQ2, Keypad 5 = stop movement)

1. Both press (`0x4C`) and release (`0xCC`) are queued simultaneously (winit delivers them
   close together).
2. Press `0x4C` fires → `set_keyboard_data(0x4C, 0x00)` → port 0x60 = `0x4C`. Key is
   pre-buffered in BDA. Custom INT 09h handler at `10B2:5F6E` is called.
3. Handler executes `STI` at `10B2:5F6F` → IF = 1.
4. The queued release `0xCC` fires **immediately** (inside the press handler) →
   `set_keyboard_data(0xCC, 0x00)` → **port 0x60 overwritten to `0xCC`**.
5. The nested `0xCC` handler runs, chains to BIOS, BIOS discards it (break code), IRET.
6. The `0x4C` press handler resumes. Reads port 0x60 → `0xCC` (stale!). The check
   `cmp al, 0x4C ; jnz 0x5FA8` **fails** — the Keypad 5 special path is never taken.
7. Because the key was pre-buffered (step 2), `AX=0x4C00` eventually surfaces via INT 16h.
8. The text-input routine sees `AL=0x00` (ASCII byte of `0x4C00`) and writes char `0x00`
   (null, rendered as a space) to the command prompt.

On **real hardware** an 8259A PIC keeps IRQ 1 (keyboard) in its In-Service Register (ISR)
from the moment the IRQ fires until the handler writes EOI (`OUT 0x20, 0x20`). No second
keyboard IRQ can be delivered during this window, regardless of the IF flag. The press
handler always reads the correct `0x4C` from port 0x60.

---

## Root Cause

The emulator has no PIC In-Service Register. Once an interrupt fires and IF is re-enabled
by the handler (`STI`), a queued IRQ of the **same level** can fire immediately, causing
re-entrance and stale port 0x60 data.

---

## Fix: Simplified PIC ISR Tracking (IRQs 0 and 1)

Implement just enough 8259A PIC behaviour to prevent re-entrant keyboard (IRQ 1) and
timer (IRQ 0) interrupts:

- **ISR bit**: one bit per IRQ level. Set when the IRQ fires; cleared on EOI.
- **EOI command**: `OUT 0x20, 0x20` — non-specific EOI, clear highest-priority set ISR
  bit (sufficient for the master PIC in AT-class systems).
- **BIOS INT 09h sends EOI**: our BIOS handler at the end clears ISR bit 1 (mirrors real
  AT BIOS behaviour).

No full 8259A register set or IMR emulation is required for this fix.

---

## Implementation Steps

### 1. Add ISR field to `IoDevice` — `core/src/io/mod.rs`

Add a single `pic_isr: u8` byte. Bit 0 = IRQ 0 (timer), bit 1 = IRQ 1 (keyboard), etc.

```rust
pub struct IoDevice {
    // ... existing fields ...
    /// PIC In-Service Register: bit N = IRQ N currently being serviced.
    /// Prevents re-entrant delivery of the same IRQ level while its handler runs.
    /// Cleared by EOI command (OUT 0x20, 0x20).
    pic_isr: u8,
}
```

Add `IoDevice::new()` initialisation: `pic_isr: 0`.

Add helper methods:

```rust
impl IoDevice {
    /// Set ISR bit for the given IRQ level (0–7).
    pub fn set_irq_in_service(&mut self, irq: u8) {
        self.pic_isr |= 1 << irq;
    }

    /// Clear ISR bit for the given IRQ level (0–7). Called on EOI.
    pub fn clear_irq_in_service(&mut self, irq: u8) {
        self.pic_isr &= !(1 << irq);
    }

    /// Returns true if the given IRQ level is currently in service.
    pub fn is_irq_in_service(&self, irq: u8) -> bool {
        self.pic_isr & (1 << irq) != 0
    }
}
```

### 2. Handle EOI on port 0x20 — `core/src/io/mod.rs`

In `IoDevice::write_byte()`, add a case for port `0x20`:

```rust
0x20 => {
    // 8259A PIC command port
    if value == 0x20 {
        // Non-specific EOI: clear the highest-priority (lowest number) ISR bit
        for irq in 0..8 {
            if self.pic_isr & (1 << irq) != 0 {
                self.clear_irq_in_service(irq);
                log::trace!("PIC: EOI cleared ISR bit for IRQ {}", irq);
                break;
            }
        }
    }
    // Ignore other 8259A command writes (ICW/OCW) — not needed for our purposes
}
```

Also add a read handler for port `0x20` (programs that poll PIC status):

```rust
0x20 => 0xFF, // Return non-specific value; ISR reads via 0x0A/0x0B not needed here
```

### 3. Set ISR bit when keyboard IRQ fires — `core/src/computer.rs`

In `fire_keyboard_irq()`, before dispatching the interrupt:

**Add guard at the top** (after the IF check):

```rust
// Prevent re-entrant keyboard IRQ: real 8259A keeps IRQ 1 in-service
// until the handler sends EOI (OUT 0x20, 0x20).
if self.io_device.is_irq_in_service(1) {
    log::trace!("Keyboard IRQ suppressed: IRQ 1 already in service (ISR)");
    return false; // Leave key in queue; will fire after EOI clears ISR
}
```

**Set the ISR bit** just before pushing the INT frame:

```rust
self.io_device.set_irq_in_service(1);
log::trace!("PIC: IRQ 1 (keyboard) set in-service");
```

### 4. Clear ISR bit in BIOS INT 09h handler — `core/src/cpu/bios/int09.rs`

At the very end of `handle_int09()` (just before the function returns in all paths):

Real AT BIOS sends `OUT 0x20, 0x20` before IRET in the INT 09h handler. Mirror this:

```rust
// Send EOI to PIC — mirrors real AT BIOS INT 09h handler behaviour.
// Clears ISR bit 1 (keyboard) so the next keyboard IRQ can be delivered.
bus.io_device_mut().clear_irq_in_service(1);
```

This must be called in **all exit paths** of `handle_int09`:
- The early return for break codes (line ~35)
- The early return for pre-buffered keys (line ~45)
- After successfully buffering a key (end of function)

> **Note**: Programs with custom INT 09h handlers that do *not* chain to the BIOS must
> send their own EOI (`OUT 0x20, 0x20`). If they do, step 2 clears the ISR bit.
> If they chain to F000:0009, step 4 clears it. Both paths are covered.

### 5. Apply same ISR logic to timer IRQ (IRQ 0) — `core/src/computer.rs`

In `process_timer_irq()`, apply the same pattern for IRQ 0 to prevent timer re-entrance:

- Add guard: if `is_irq_in_service(0)`, skip firing.
- Set `set_irq_in_service(0)` when timer IRQ fires.
- In the BIOS INT 08h handler, call `clear_irq_in_service(0)` (or rely on the game
  writing EOI, which already flows through port 0x20 handler from step 2).

Note: the BIOS INT 08h path in `computer.rs` already has inline processing. Add EOI
clearing at the end of the BIOS-only path. Custom INT 08h handlers that chain to F000
will clear via port 0x20 if they send EOI, or via BIOS handler when it runs.

---

## Files Changed

| File | Change |
|------|--------|
| `core/src/io/mod.rs` | Add `pic_isr: u8` field; add port 0x20 EOI write handler; ISR helper methods |
| `core/src/computer.rs` | `fire_keyboard_irq`: ISR guard + set ISR bit; `process_timer_irq`: same for IRQ 0 |
| `core/src/cpu/bios/int09.rs` | Call `clear_irq_in_service(1)` on all exit paths |
| `core/src/cpu/bios/mod.rs` | Add `io_device_mut()` accessor if not already present for use in int09.rs |

---

## Expected Outcome

After the fix:

1. Press `0x4C` fires → ISR bit 1 set → port 0x60 = `0x4C`.
2. Handler executes `STI`. Queued `0xCC` **cannot fire** (ISR bit 1 set).
3. Handler reads port 0x60 → `0x4C` (correct). Takes Keypad 5 special path at
   `10B2:5F80`.
4. Handler chains to BIOS (or sends own EOI). ISR bit 1 cleared.
5. `0xCC` release fires on the next step(). Handler discards it normally. No
   space written to prompt.

---

## Risks / Edge Cases

- **Programs that never send EOI**: ISR bit stays set permanently, locking out further
  keyboard input. This would manifest as keyboard freeze. Should not happen with
  well-written DOS programs; can be diagnosed by watching the ISR bit in logs.
- **Custom INT 08h without EOI**: Same risk for timer. Monitor for timer lockout.
- **INT 15h AH=4Fh intercept path**: The keyboard intercept trampoline at F000:0xFF
  is called *after* the BIOS INT 09h handler returns. Ensure `clear_irq_in_service(1)`
  is not called before the INT 15h chain completes. Current structure in `computer.rs`
  fires INT 15h as a *separate* interrupt chain after INT 09h, so ISR bit 1 would already
  be cleared by the time INT 15h fires — that is correct, as INT 15h is a software INT,
  not a hardware IRQ.
- **Nested interrupts of different levels** (e.g. IRQ 0 fires inside IRQ 1 handler): Not
  prevented by this fix (correct — real hardware allows this). IRQ 0 has higher priority
  and its ISR bit would be cleared independently.
