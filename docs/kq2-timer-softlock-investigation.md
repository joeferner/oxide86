# KQ2 / Sierra AGI Timer Soft-Lock Investigation

**Date:** 2026-02-22  
**Game:** King's Quest 2 (Sierra AGI interpreter)  
**Symptom:** Animation stuck; program hangs at log line ~380375  
**Status:** Root cause identified - INT 1Ch not chained from BIOS INT 08h path

---

## Symptom

The program enters an infinite spin loop at `05A3:7EB6–7EC2`:

```asm
7EB6: mov al, [0x0013]   ; reads game variable = 0x02 (frame wait count)
7EB9: sub ah, ah         ; AX = 2
7EBB: mov di, ax         ; DI = 2
7EBD: mov ax, [0x177a]   ; reads animation frame counter = 0x0000
7EC0: cmp ax, di         ; 0x0000 < 0x0002 → branch always taken
7EC2: jb 0x7eb6          ; infinite loop - counter never reaches 2
```

- DS = `0x0F98` → `[0x0013]` = physical `0xF993` (a game variable, not BDA)
- `[0x177A]` = physical `0x110FA` (animation tick counter)
- The game waits until `[0x177A]` reaches the value in `[0x0013]` (2 frames)

The timer interrupt IS firing (visible at log line 380316) but the counter at `[0x177A]` **never gets incremented**.

---

## Program / System Architecture

The game (Sierra AGI / KQ2) uses a three-level timer chain:

1. **Custom INT 08h at `05A3:845A`** — installed by the AGI interpreter over the original handler
2. **Task manager at `038D:003C`** — a cooperative multitasking kernel called every 3 ticks by the custom INT 08h
3. **BIOS ROM at `F000:0008`** — called by the task manager to update the BDA tick counter

### Timer ISR flow (05A3:845A)

```
INT 08h fires
  → 05A3:845A: checks flag at [0x124E]
      if zero  (always): jump to 846C
      if nonzero: execute 3-byte block at 8469 (never reached)
  → 846C: dec [0x1845] (frame counter, 3-tick cycle)
      if zero: call far [0x1823] → 038D:003C (task manager), reset counter to 3
      if nonzero: IRET (exit without calling task manager)
```

### Task manager at 038D:003C

```
038D:003C: call 0x0147        ; enter task manager body
  0147: set up DS = 0x0F98
  014F: cs: mov bp, cs:[0x0010] ; load current task descriptor offset (= 0x0040)
  0156: xchg al, es:[bp+0x00] ; set reentrancy flag at 03B4:0040
  015E: cs: sub cs:[0x0010], 0x0008
  0164: es: mov es:[bp+0x02], sp ; save SP
  0168: es: mov es:[bp+0x04], ss ; save SS = 0x0F98
  016E: es: mov bp, es:[bp+0x06] ; load next task offset = 0x04C6
  0172: es: cmp es:[bp+0x00], ax ; compare: [03B4:04C6] vs ax (both 0x0040)
  0176: jnz 0x01ba              ; *** NEVER TAKEN *** (zero flag always set)
  ; falls through to 0178 (no context switch)
  ...
  018A: cs: call far cs:[bp+0x00] ; calls F000:0008 (BIOS timer ROM)
  018E: (resume after BIOS returns)
  01AC: IRET
```

---

## Key Finding: Single-Task Ring

The task manager maintains a circular linked list of task descriptors in segment `03B4`:

| Address (03B4:) | Value | Meaning |
|-----------------|-------|---------|
| 0040 | 0x00/0x01 | Reentrancy flag (0=free, 1=busy during ISR) |
| 0042 | 0x27FC | Saved SP |
| 0044 | 0x0F98 | Saved SS |
| 0046 | 0x04C6 | Next task descriptor offset |
| 04C6 | 0x0040 | Value at next-task slot = 0x0040 |

The ring contains **only one task**: descriptor at 0x0040 → next at 0x04C6 → next = 0x0040 (back to itself).

At `038D:0172`, the code checks:  
`es:[bp+0x00]` (= `03B4:04C6` = 0x0040) vs `ax` (= 0x0040)  
They are always equal → ZF=1 → `jnz 0x01ba` is never taken.

**No context switch ever occurs.** The second task (animation task) that should increment `[0x177A]` never runs.

---

## Why `[0x177A]` Never Increments

Through exhaustive log analysis, `[0x177A]` (physical `0x110FA`) is **only ever read, never written** in the 380,000-line execution log.

There are three potential paths that could increment it — all fail:

### Path 1: Context switch at 038D:01BA (most likely)
When the task ring contains a second task (animation task), the `jnz 0x01ba` would be taken. That code path saves the current task's registers, loads the next task's saved registers, and returns to the animation task. The animation task would then run and increment `[0x177A]`. **This never fires because the ring only has one entry.**

### Path 2: 3-byte block at 05A3:8469 (conditional on [0x124E])
The timer ISR has a 3-byte block at 8469 that only executes when `[0x124E] != 0`. Since `[0x124E]` is always 0, those bytes never execute. The role of those bytes is unclear (too short for most memory-write instructions addressing `[0x177A]` by absolute address), but they appear to be timer-related capability gating.

### Path 3: INT 1Ch chain (root cause)
On real hardware, the BIOS INT 08h handler (`F000:0008`) calls INT 1Ch after updating the BDA tick counter. If the game installed a custom INT 1Ch handler that increments `[0x177A]`, it would fire on every tick.

**In the emulator, when the task manager calls `F000:0008` directly (via `cs: call far cs:[bp+0x00]`), the emulator detects the BIOS ROM address and invokes `handle_int08()`.** This function only updates the BDA counter — it does **not** chain to INT 1Ch.

The INT 1Ch chaining only happens in `process_timer_irq()` when there is **no** custom INT 08h handler. Since KQ2 installed a custom INT 08h, `process_timer_irq()` dispatches to the custom handler and never checks INT 1Ch. The custom handler then calls the BIOS ROM directly, but that BIOS ROM path also skips INT 1Ch.

---

## Timeline of Events

| Log line | Event |
|----------|-------|
| 1–570 | Pre-exec-log: Game boots, INT 08h hooked to `038D:003C`, later rewrapped at `05A3:845A` |
| 571 | First logged timer IRQ (via `038D:003C`) |
| ~114,658 | Second logged timer ISR (via `05A3:845A`) — game doing EGA blitting |
| ~196,472 | Third timer ISR — game at `05A3:980F` (EGA movsb) |
| ~272,215 | Fourth timer ISR — game at `05A3:1FC9` (character parsing) |
| ~310,290 | Game calls wait function at `05A3:7EB1`, enters spin loop at `05A3:7EB6` |
| ~380,316 | Timer fires at spin loop — `[0x177A]` still 0 — stuck |
| 380,375+ | Soft lock confirmed — animation counter never increments |

The game made forward progress between the first four timer IRQs (EGA blitting, character work) but then entered the blocking wait and never escaped.

---

## INT 1Ch Chaining Gap (Root Cause)

```
process_timer_irq() {
    if custom INT 08h installed:
        → fire custom handler (05A3:845A)
        → custom handler → task manager → F000:0008
        → handle_int08() updates BDA only  ← NO INT 1Ch here
        ← return
        ← IRET from custom handler
        // INT 1Ch NEVER called
    else:
        → update BDA
        → if custom INT 1Ch: fire it   ← only reached without custom INT 08h
}
```

The fix should be: after `handle_int08()` runs (whether via the F000:0008 BIOS ROM path or the normal path), check whether INT 1Ch has a custom handler installed and fire it if so.

---

## Supporting Evidence

- `jnz 0x01ba` at `038D:0176` appears in the log exactly at lines 114658, 196472, and 272215 — but is **never taken** (the following instruction is always `038D:0178 push bp`, confirming fall-through)
- Zero instances of "INT 0x1C" or "INT 1C" in the entire 380,000-line log
- `[0F98:177A]` has zero write operations in the log (only reads via `mov ax, [0x177a]`)
- `[0x124E]` is always 0x0000; the 3-byte block at 8469 never executes
- `[0x1845]` cycles through a 3-tick countdown; the only logged value is 0x01 (just before decrement to 0 triggers the task manager call)

---

## Fix Required

In `core/src/computer.rs`, in the F000 BIOS ROM execution path for INT 08h (`handle_bios_interrupt_direct` or equivalent), after calling `handle_int08()`, check the INT 1Ch vector. If a custom handler is installed (i.e., the vector does not point to the default BIOS stub), chain to it via the same mechanism used in `process_timer_irq()` for the no-custom-INT08h path.

Alternatively, when `process_timer_irq()` dispatches to a custom INT 08h handler, it should also arrange to fire INT 1Ch after the custom handler completes its IRET, regardless of whether the custom handler itself chains to the BIOS ROM.

This matches AT-class BIOS behavior where INT 1Ch is always called as part of the INT 08h tick cycle.

---

## Files to Investigate

| File | Relevance |
|------|-----------|
| `core/src/computer.rs` | `process_timer_irq()`, F000 BIOS ROM path, INT 1Ch chaining logic |
| `core/src/cpu/bios/int08.rs` | `handle_int08()` — does not currently chain INT 1Ch |
| `core/src/cpu/bios/int1c.rs` | INT 1Ch handler (if it exists) |

---

## Notes

- This was described as a pre-existing bug, not a regression from recent commits
- The `cpu detection` commit (33e5a47) changed `PUSH SP` behavior for 286+ CPUs, but this is not the cause — the task manager uses `PUSH BP`, not `PUSH SP`
- The game is running with `--cpu 286` or similar (log shows "Emulating CPU type: 80286")
- FDC8 references in earlier investigation notes are unrelated; this is the Sierra AGI interpreter
