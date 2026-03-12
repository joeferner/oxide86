# Performance Analysis — 2026-03-11

Source: `perf script` output from `perf.data` captured during a running emulator session.

## Hot Spots Summary

After discarding startup noise (X11/Vulkan/wgpu initialisation), the steady-state emulator
samples break down as follows:

| Function | Samples | ~% of emulator time |
|---|---|---|
| `oxide86_core::devices::nuked_opl3::process_slot` | 1619 | ~39% |
| `oxide86_core::cpu::Cpu::step` | 533 | ~13% |
| `oxide86_core::devices::adlib::Adlib::advance_to_cycle` | 467 | ~11% |
| `oxide86_core::devices::pic::Pic::take_irq` | 367 | ~9% |
| `oxide86_core::devices::nuked_opl3::generate_resampled` | 323 | ~8% |
| `oxide86_core::video::video_buffer::VideoBuffer::render_into` | 308 | ~7% |
| `oxide86_core::devices::uart::Uart::take_pending_irq` | 274 | ~7% |
| `oxide86_core::cpu::instructions::shift_rotate::*` | 337 | ~8% |

**OPL3 synthesis (nuked_opl3) accounts for roughly 58% of emulator CPU time.**

---

## Root Cause Analysis

### 1. `advance_to_cycle` called every instruction (biggest problem)

`Bus::increment_cycle_count` (`bus.rs:115-119`) calls `SoundCard::advance_to_cycle` on
**every single instruction**, regardless of whether a new audio sample is actually due.

At 44100 Hz audio and 4.77 MHz CPU, one sample is only needed every ~108 CPU cycles.
Most instructions execute in 4–20 cycles, so the vast majority of `advance_to_cycle` calls
compute `n_out = 0` and generate nothing — yet they still run the timer arithmetic and cycle
accumulator math every time.

When `n_out > 0`, `generate_resampled` is called, which in turn calls `process_slot` for
all 18 OPL3 operator slots. This is expensive, but it's supposed to happen rarely. The
problem is the per-instruction call overhead for all the calls where nothing is produced.

### 2. PIC polls every device every instruction

`Cpu::step` (`cpu/mod.rs:199`) calls `bus.pic_mut().take_irq()` on every instruction.
`Pic::take_irq` does:
- `pit.borrow_mut()` → `take_pending_timer_irq`
- `keyboard_controller.borrow_mut()` → `take_pending_key`
- `uart.borrow()` → `take_pending_irq` × 4 (COM1–4)
- `keyboard_controller.borrow_mut()` → `take_pending_mouse`

That is 6+ RefCell/RwLock borrow operations per instruction, most returning false/None.
The UART alone accounts for 274 samples despite almost never having data ready.

### 3. Video rendered unconditionally

`VideoBuffer::render_into` (308 samples) appears to be called every frame regardless of
whether VRAM has changed. In text-heavy or idle scenes this work is entirely wasted.

---

## Recommended Optimisations

### Priority 1 — Gate `advance_to_cycle` on a cycle threshold (~50% speedup potential)

Add a `next_sample_cycle: u32` field to `Adlib` that tracks the cycle count at which the
next audio sample will be due. Update `Bus::increment_cycle_count` to check this before
calling through:

```rust
// bus.rs — increment_cycle_count
pub(crate) fn increment_cycle_count(&mut self, cycles: u32) {
    self.cycle_count = self.cycle_count.wrapping_add(cycles);
    if let Some(sc) = &self.sound_card {
        if self.cycle_count >= sc.borrow().next_sample_cycle() {
            sc.borrow_mut().advance_to_cycle(self.cycle_count);
        }
    }
}
```

Inside `Adlib::advance_to_cycle`, after computing `n_out`, update `next_sample_cycle` to
`cycle_count + (cpu_freq - cycle_acc) / ADLIB_SAMPLE_RATE` (the number of CPU cycles until
the accumulator rolls over again).

This would reduce `advance_to_cycle` calls by ~100× and cut OPL overhead to only the
samples that are actually produced.

Note: the `SoundCard` trait's `advance_to_cycle` method would need a companion
`next_sample_cycle() -> u32` method, or the gating logic can live on `Bus` with a
separate `next_sound_cycle: u32` field updated by the sound card after each flush.

### Priority 2 — PIC dirty-IRQ flag (~15% speedup potential)

Add a shared `pending_irq: AtomicBool` (or a plain `bool` behind the existing `RefCell`)
that is set to `true` whenever a device raises an interrupt, and cleared by `take_irq`
when it consumes one.

```rust
// Pic::take_irq fast path
pub(crate) fn take_irq(&mut self, cycle_count: u32) -> Option<u8> {
    if !self.any_pending {
        return None;
    }
    // ... existing logic ...
}
```

Devices (PIT, keyboard controller, UART, mouse) set `any_pending = true` when they have
data/timer fire to deliver, and the PIC clears it after consuming. This collapses the
6+ borrow operations per instruction to a single flag check in the common (no IRQ) case.

Alternatively — and simpler — only poll keyboard and UART IRQs every 100 instructions.
They do not require sub-microsecond precision. Keep the PIT checked every instruction
since timer accuracy is visible to software.

### Priority 3 — Video dirty tracking (~7% speedup, larger in text/idle modes)

Add a `dirty: bool` flag to `VideoBuffer` (or a dirty-region bitset for graphics modes).
Set it on any VRAM write in `memory_write_u8`. In the render path, skip `render_into`
entirely when `dirty` is false.

```rust
// VideoBuffer
pub fn render_and_clear_dirty(&mut self, ...) -> Option<RenderResult> {
    if !self.dirty {
        return None;
    }
    self.dirty = false;
    Some(self.render_into(...))
}
```

### Priority 4 — Lower OPL internal sample rate (fidelity tradeoff)

`nuked_opl3` is initialised at 44100 Hz (`adlib.rs:58`). Running OPL synthesis at 22050 Hz
and resampling to 44100 for output would halve synthesis calls at the cost of slightly
reduced high-frequency accuracy (largely inaudible for OPL FM content). This is a
straightforward constant change to evaluate.

---

## Non-Issues

- The large startup samples (`ld-linux`, `libX11`, `libvulkan`, Vulkan/GL driver
  initialisation) are one-time costs and do not affect steady-state performance.
- `shift_rotate` instructions being prominent (337 samples) is normal — the emulated
  program is doing lots of bit manipulation. No action needed.
- `Bus::memory_read_u16` (54 samples) is low enough to ignore for now.
