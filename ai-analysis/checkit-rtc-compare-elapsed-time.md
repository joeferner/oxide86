# CheckIt 3.0 — "Compare Elapsed Time" Test Hang Analysis

## Symptom

The RTC test in CheckIt 3.0 hangs indefinitely on the "Compare Elapsed Time" sub-test. The program loops in state 5 of a polling state machine, calling `elapsed_ticks_ge` (1582:1445) thousands of times without ever getting a non-zero return.

---

## State Machine Overview

The test is driven by a large function in segment `1E88`. A state variable at `[bp-0x1a]` selects a handler via a 7-entry jump table at `1E88:0BA3`. The outer loop at `lbl_1E88_0292` calls `rtc_read_datetime` on every iteration and ran **4062 times** in the emulated-clock trace.

| State | Handler entry | Purpose |
|-------|--------------|---------|
| 3 | `1E88:0844` | Wait for RTC alarm OR 127-tick timeout; transitions to state 4 when alarm fires |
| 4 | `1E88:08D8` | Immediate — sets up display, transitions to state 5 |
| 5 | `1E88:0910` | **Stuck here** — polls `elapsed_ticks_ge` waiting for 218 BDA ticks from test start |

### Initialization (before outer loop)

At `1E88:027A`, before the outer loop starts, the code captures the current BDA timer counter into `[bp+0xfe2a/0xfe2c]` — this is the **start tick for state 5**. At `1E88:028D` it stores `0xDA` (218) into `[bp-0x16]` — the **tick threshold for state 5**.

State 5 therefore measures total elapsed BDA ticks from **test start**, not from when the alarm fires. The threshold of 218 ticks ≈ 12 seconds at 18.2 Hz.

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
push [bp-0x16]          ; threshold = 218
push [bp+0xfe2c]        ; start tick high word (captured at test init)
push [bp+0xfe2a]        ; start tick low word (captured at test init)
call far 0x1582, 0x1445 ; elapsed_ticks_ge
or ax, ax
jne lbl_1E88_095A       ; ← success path (gap, never executed)
jmp lbl_1E88_0292       ; ← always loops back
```

`elapsed_ticks_ge` is also called from state 3 (`1E88:0898`) with a different start tick (`[bp+0xfe2e]`, captured just before alarm set) and a threshold of 127 ticks — this is the state 3 alarm-wait timeout.

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

The chain target should be the BIOS handler at `0xF000:0x0008`. When the CPU arrives there, `step_bios_segment` dispatches `handle_int08_timer_interrupt`, which calls `bda_increment_timer_counter` and sends EOI.

### RTC Alarm — INT 4Ah Handler

CheckIt installs an INT 4Ah (user alarm) handler at `1E88:0FA2` via `func_36C5_0682` (at `1E88:07CE`). When the RTC alarm fires (INT 70h → INT 4Ah), this handler sets `[0x8206] = 1`. State 3 polls this flag each iteration at `1E88:0871`.

The alarm is programmed at `1E88:07E6` (call to `rtc_cancel_or_set_alarm`) using a datetime struct copied from `[0xcf64]`. The state 3 timeout start tick is captured at `1E88:077A`, immediately before the alarm is set.

---

## Root Cause: Real-Time / Emulated-Time Mismatch

The test design requires:

1. CheckIt programs an **RTC alarm** for N seconds from now
2. State 3 polls until the alarm fires (signalled by `[0x8206]` becoming non-zero), with a 127-tick timeout from alarm-set time
3. State 5 checks whether **218 BDA ticks** have elapsed since **test start** (not since alarm fire)

The 218-tick threshold represents the expected total test duration (~12 seconds). For this to pass immediately when the alarm fires, the alarm must be programmed for roughly 12 seconds minus the test setup overhead.

### Original behaviour (real-time clock)

- The **RTC alarm** fires based on **real wall-clock time**
- The **BDA timer counter** advances based on **emulated CPU cycles** (every ~439K cycles at 8 MHz)
- With execution logging enabled the emulator runs far faster than real-time, so the alarm fires almost immediately while the BDA counter has barely advanced

**Result:** INT 08h hook ran **37 times** total. State 3 exited almost immediately (alarm fired in ~60 iterations). State 5 entered with only ~37 BDA ticks elapsed, needing 218. Never converged within the trace window (500 `elapsed_ticks_ge` calls).

### With `EmulatedClock`

The RTC clock now derives time from emulated CPU cycles rather than wall-clock time — both the alarm and the BDA counter are driven by the same cycle base.

**Result:** INT 08h hook ran **184 times** total (5× improvement). State 3 ran **2179 iterations** before the alarm fired, accumulating ticks in emulated time. The state 3 elapsed timeout (127 ticks) ran 2178 times without triggering — the alarm always fired within the expected window.

State 5 ran **1878 iterations**. At trace end: **184 ticks accumulated, threshold 218** (84% complete, 34 ticks short). At the observed rate of ~0.045 ticks per outer-loop iteration, approximately **750 more state 5 iterations** are needed to cross the threshold.

---

## Trace Comparison

| Metric | Real-clock trace | EmulatedClock trace |
|--------|-----------------|---------------------|
| Outer loop iterations | ~493 | 4062 |
| INT 08h hook executions | 37 | 184 |
| State 3 iterations | ~60 | 2179 |
| State 3 timeout triggered | No | No |
| State 5 iterations | ~500 | 1878 |
| BDA ticks at trace end | 37 | 184 |
| Threshold | 218 | 218 |
| `elapsed_ticks_ge` calls (state 5) | ~500 | 1878 |
| `elapsed_ticks_ge` calls (state 3 timeout) | — | 2178 |
| Test converged | No | No (84% complete) |

---

## The Unexecuted Gap at 1582:145D–1461

The 5-byte gap between `jl lbl_1582_1462` and `lbl_1582_1462` is the "time elapsed" return path. It was never reached in either trace, meaning `elapsed_ticks_ge` **always returned 0** across all observed calls. This confirms the BDA counter never advanced past the threshold during either trace window.

The gap presumably contains something like:
```asm
mov ax, 0x0001   ; 3 bytes — return non-zero (elapsed)
jmp 0x1467       ; 2 bytes — skip the sub ax,ax at lbl_1582_1462
```

---

## Full Run Results (EmulatedClock, Test to Completion)

A subsequent full run (5,083 outer loop iterations) allowed the test to run to completion. New findings:

### PIT Reprogramming — 263 Hz BDA Ticks

CheckIt reprograms PIT channel 0 from the BIOS default (divisor 65,536 → 18.2 Hz) to divisor ~4,533. This causes INT 08h (and the BDA tick counter at `0x046C`) to fire at **~263 Hz** — 14.4× faster than real hardware. `bda_logs_analyze.py` confirmed 30,438 instructions/tick avg at steady state (ticks 21+), implied rate 263 Hz.

The PIT reprogramming was intentional — CheckIt wants fast polling. Both the RTC alarm and BDA counter are driven by the same CPU cycle base (via `EmulatedClock`), so the 263 Hz rate applies consistently to both. This is correct behavior.

### State 5 Eventually Passed

With the full run, state 5 (`elapsed_ticks_ge` check) ran **2,902 iterations** before the BDA elapsed tick count reached 218. At that point `jne 0x095A` at `1E88:0955` was **taken once** — the success path at `1E88:095A` (`state5_success_path`) executed.

The success path:
1. Called `func_1306_000A` and `func_1306_008B`
2. Performed an FPU comparison of actual elapsed time vs expected bounds — `jae 0x0A26` taken (pass path), gap at `0A23` never executed
3. Called `func_16E4_0698` to record the result
4. **`inc [0x8208]`** at `1E88:0A77` — incremented the Compare Elapsed Time pass counter from 0 to 1
5. Transitioned to state 6 (`mov [bp-0x1a], 0x0006` at `1E88:0B9B`)

### New Failure Point: Cumulative Score Check at 1E88:05E4

After state 6, the outer loop eventually reaches the cumulative result check:

```asm
1E88:05E4  cmp [bx+0x02b1], 0x000C   ; is total score == 12?
1E88:05EA  je  0x05ef                 ; ← NOT taken
1E88:05EC  jmp 0x06b8                 ; ← TAKEN (fail path)
; gap 1E88:05EF - 1E88:06B8 (201 bytes) = overall-pass path, never executed
```

`ES:[bx+0x02b1]` holds the cumulative result total across all 6 sub-tests, each clamped to 2 passes max. The required total is **12** (6 × 2). The `je` was **not taken** — the score was less than 12 — so the 201-byte pass path at `1E88:05EF–06B8` remains entirely unexecuted and the test fails.

### Root Cause of Score Shortfall

The Compare Elapsed Time sub-test (`[0x8208]`) was incremented **once** (score = 1 instead of 2). The test would need the outer loop to cycle through state 5 success **twice** to reach a score of 2 for this sub-test.

At 263 Hz BDA, reaching the 218-tick threshold from test start takes **2,902 outer loop iterations**. After that first success the loop transitions to state 6 and then back to state 0 for a second measurement cycle. By that point, with ~5,083 total outer loop iterations and ~2,902 consumed reaching the first threshold, there is insufficient budget to cross the 218-tick threshold a second time.

The 263 Hz BDA rate means CheckIt's 218-tick "12-second" timeout actually covers only ~0.83 seconds of emulated time (`218 / 263 Hz ≈ 0.83s`). The FPU comparison at `1E88:0A23/0A26` checks whether the actual elapsed time matches the expected ~12-second window — it passes only because the EmulatedClock correctly tracks emulated time in seconds, not raw ticks. But the outer loop iteration cost to accumulate 218 ticks at 30,438 instructions/tick is real and limits how many passes can complete.

---

## Trace Comparison

| Metric | Real-clock trace | EmulatedClock (early stop) | EmulatedClock (full run) |
|--------|-----------------|---------------------------|--------------------------|
| Outer loop iterations | ~493 | 4,062 | 5,083 |
| INT 08h hook executions | 37 | 184 | ~263+ |
| BDA tick rate | 18.2 Hz | 263 Hz | 263 Hz |
| State 3 iterations | ~60 | 2,179 | 2,179 |
| State 3 timeout triggered | No | No | No |
| State 5 iterations | ~500 | 1,878 | 2,902 |
| BDA ticks at state 5 exit | 37 | 184 (84%) | 218 ✓ |
| `lbl_1E88_095A` reached | No | No | Yes (once) |
| `inc [0x8208]` count | 0 | 0 | 1 |
| Cumulative score check | — | — | < 12 (fail) |
| Test converged | No | No | No — score 1/2 for sub-test |

---

## Summary

| Component | Real-clock | EmulatedClock |
|-----------|-----------|---------------|
| INT 08h hook chain | Working (37 ticks) | Working (263 Hz) |
| BDA counter reads | Correct | Correct |
| RTC alarm timing | **Wrong** — wall-clock time | Fixed — emulated cycles |
| State 3 alarm wait | Near-instant (wrong) | 2,179 iterations (correct) |
| State 3 timeout | Not triggered | Not triggered ✓ |
| State 5 threshold crossed | Never | Yes — after 2,902 iterations |
| FPU elapsed-time check | — | Passes ✓ |
| Compare Elapsed Time score | 0/2 | **1/2** (needs 2 passes) |
| Cumulative score | < 12 | < 12 (fail at `1E88:05E4`) |
| Overall pass path `05EF–06B8` | Never executed | Never executed |
| Root cause | Real/emulated-time skew | 263 Hz BDA leaves no budget for 2nd state-5 pass |
