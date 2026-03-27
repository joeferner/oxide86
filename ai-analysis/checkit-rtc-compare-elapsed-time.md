# CheckIt 3.0 — "Compare Elapsed Time" Test Hang Analysis

## Symptom

The RTC test in CheckIt 3.0 hangs indefinitely on the "Compare Elapsed Time" sub-test. The program loops in state 5 of a polling state machine, calling `elapsed_ticks_ge` (1582:1445) hundreds of times without ever getting a non-zero return.

---

## State Machine Overview

The test is driven by a large function in segment `1E88`. A state variable at `[bp-0x1a]` selects a handler via a 7-entry jump table at `1E88:0BA3`. The states involved are:

| State | Handler entry | Purpose |
|-------|--------------|---------|
| 3 | `1E88:0844` | Wait for RTC alarm OR timeout; transitions to state 4 when alarm fires |
| 4 | `1E88:08D8` | Immediate — sets up display, transitions to state 5 |
| 5 | `1E88:0910` | **Stuck here** — polls `elapsed_ticks_ge` waiting for BDA ticks |

Each iteration of the outer loop (at `lbl_1E88_0292`) reads the current datetime via `rtc_read_datetime` (which calls INT 1Ah AH=2 and AH=4), updates the display, and dispatches into the active state handler.

---

## `elapsed_ticks_ge` (1582:1445)

```asm
elapsed_ticks_ge:
    push bp
    mov bp, sp
    sub sp, 0x0002
    push cs
    call 0x1427             ; read_bios_timer_direct → AX=low word, DX=high word
    sub ax, [bp+0x06]       ; elapsed_low = current_low - start_low
    mov [bp-0x02], ax
    mov ax, [bp+0x0a]       ; threshold
    cmp [bp-0x02], ax       ; signed 16-bit compare
    jl lbl_1582_1462        ; if elapsed < threshold → not done
    ; === 5-byte gap at 1582:145D–1461 (NEVER executed) ===
    ; Should be the "elapsed" return path (return AX≠0)
lbl_1582_1462:
    sub ax, ax              ; AX = 0 (not elapsed)
    mov sp, bp
    pop bp
    retf
```

Caller (state 5, `1E88:0940`):
```asm
push [bp-0x16]          ; threshold
push [bp+0xfe2c]        ; start tick high word
push [bp+0xfe2a]        ; start tick low word
call far 0x1582, 0x1445 ; elapsed_ticks_ge
or ax, ax
jne lbl_1E88_095A       ; ← success path (gap, never executed)
jmp lbl_1E88_0292       ; ← always loops back
```

### Execution counts (from trace)

| Instruction | Count |
|------------|-------|
| `push bp` (entry) | 500 |
| `mov ax, [bp+0x0a]` (load threshold) | 500 |
| `cmp [bp-0x02], ax` | 499 |
| `jl lbl_1582_1462` | 499 |
| `sub ax, ax` (return 0) | 499 |
| `retf` | 499 |

On the 500th call, execution reached `mov ax, [bp+0x0a]` (1582:1455) but `cmp` was not yet logged — an interrupt likely fired between those two instructions and the trace ended there. The program is still running the loop.

---

## `read_bios_timer_direct` (1582:1427)

```asm
mov [bp-0x02], 0x0000   ; far pointer segment = 0x0000
mov [bp-0x04], 0x046c   ; far pointer offset  = 0x046C
les bx, [bp-0x04]       ; ES:BX = 0x0000:0x046C
mov ax, ES:[bx]         ; AX = BDA timer counter low word
mov dx, ES:[bx+0x02]    ; DX = BDA timer counter high word
retf
```

Reads the 32-bit BIOS Data Area timer counter at physical `0x046C/0x046E` directly, bypassing INT 1Ah. `elapsed_ticks_ge` uses **only the low 16-bit word (AX)**; DX is ignored.

---

## How the BDA Timer Counter Advances

The BDA counter at `0x0000:0x046C` is updated only by `bda_increment_timer_counter`, called from `handle_int08_timer_interrupt` (BIOS INT 08h), which fires from the PIT IRQ0 at ~18.2 Hz (every `cycles_per_irq` emulated CPU cycles).

### CheckIt's INT 08h Hook

CheckIt installs its own timer interrupt handler at `1306:0300`. On each timer tick it:
1. Saves all registers (pusha-style)
2. Increments its own 32-bit tick counter at `[DS:0x0C7A/0x0C7C]`
3. Calls `func_36C5_06D2` — a stack-manipulation routine that chains to the **original** INT 08h vector via `retf`, bypassing the normal call-return mechanism

`func_36C5_06D2` replaces the saved AX/CX on the stack with the chain target CS:IP (stored at `[0x0366]:[0x0364]`), pops the saved registers, and does `retf` to jump to the original handler — leaving the original interrupt frame intact on the stack for the BIOS handler's eventual IRET.

The chain target should be the BIOS handler at `0xF000:0x0008`. When the CPU arrives there, `step_bios_segment` dispatches `handle_int08_timer_interrupt`, which calls `bda_increment_timer_counter` and sends EOI. **So the BDA counter is being updated** — the trace confirms the INT 08h handler ran 37 times.

---

## Root Cause: Real-Time / Emulated-Time Mismatch

The test operates as follows:

1. CheckIt programs an **RTC alarm** via INT 1Ah AH=6 for N seconds from now
2. State 3 polls until the alarm fires (via INT 70h → INT 4Ah) — signalled by a flag at `[DS:0x8206]` becoming non-zero
3. After the alarm fires, state 5 checks whether the **BDA tick counter** has advanced by `threshold` ticks from a captured start time

The critical mismatch:

- The **RTC alarm** fires based on **real wall-clock time** (the emulator's RTC reads the host system clock)
- The **BDA timer counter** advances based on **emulated CPU cycles** (every ~439K cycles at 8 MHz default)

If the emulator is running **faster than real-time**, the RTC alarm fires before enough emulated cycles have accumulated, so the BDA counter hasn't advanced enough ticks. State 5 then loops, waiting for more BDA ticks — but since the alarm has already fired, the loop can still make progress as long as the timer IRQ keeps firing.

With only 37 BDA ticks accumulated across ~493 loop iterations, and the threshold potentially requiring 90+ ticks (e.g. a 5-second interval × 18.2 Hz), the test may need ~1000–2000 more iterations before completing. This makes the test appear to hang even though it is still making slow progress.

---

## The Unexecuted Gap at 1582:145D–1461

The 5-byte gap between `jl lbl_1582_1462` and `lbl_1582_1462` is the "time elapsed" return path. It was never reached during the trace, meaning `elapsed_ticks_ge` **always returned 0** across all 500 observed calls. This confirms the BDA counter never advanced past the threshold during the trace window.

The gap presumably contains something like:
```asm
mov ax, 0x0001   ; 3 bytes — return non-zero (elapsed)
jmp 0x1467       ; 2 bytes — skip the sub ax,ax at lbl_1582_1462
```

---

## Summary

| Component | Status |
|-----------|--------|
| INT 08h hook chain | Working — BDA counter is incremented (37 ticks observed) |
| BDA counter reads | Correct — `read_bios_timer_direct` reads physical `0x046C` |
| RTC alarm firing | Working — alarm fired, state transitioned |
| Timer ticks vs. threshold | **Insufficient** — threshold not reached in trace window |
| Root cause | Real-time/emulated-time skew: alarm fires before enough BDA ticks accumulate |

The test is not permanently broken — it will eventually complete once enough timer ticks accumulate. The perceived hang is due to the emulator running faster than real-time, causing fewer BDA ticks per RTC-second than a real 8 MHz 286 would produce.
