# INT 15h AH=4Fh Keyboard Intercept Implementation

## Problem

DOS programs running under the **FDC8 multitasker** kernel receive no keyboard input.
Investigation showed the game polls `@17D5:50AC` (FDC8's active-task keyboard buffer flag),
which is only set by FDC8's INT 09h handler at `0395:0045` when a specific coroutine table
entry (`@03BC:04C6`) is non-zero — but that slot is always `0x0040`, so the jnz-0x01ba path
never fires.

**Hypothesis**: FDC8 uses the IBM AT BIOS keyboard intercept (INT 15h AH=4Fh) to route
keystrokes to the active task's buffer, bypassing the coroutine mechanism.

## Background: INT 15h AH=4Fh

AT-class BIOS INT 09h handler calls INT 15h AH=4Fh **before** buffering each keystroke:

- **Input**: AH=4Fh, AL=scan code, CF=1
- **Output**: CF=0 → BIOS buffers the key normally; CF=1 → key was intercepted, skip buffering

TSRs and multitaskers install custom INT 15h handlers to intercept this call and route
keystrokes to their own buffers. This is the standard AT mechanism (IBM AT BIOS Technical
Reference, 1984).

## Implementation

### Key design constraint

The emulator's F000 BIOS handlers are invoked via the **CALL FAR F000:00XX path** in
`computer.rs::step()`. Custom INT 09h handlers (like FDC8's) chain to BIOS using
`PUSHF + CALL FAR F000:0009`. The CPU then lands at F000:0009, which the F000 path
intercepts and dispatches to `handle_bios_interrupt_direct(0x09, ...)`.

The challenge: we need to invoke the custom INT 15h handler **from within** this CALL FAR path,
asynchronously (the real CPU would just call it normally before returning from INT 09h). We
cannot call `execute_int_with_io(0x15)` from within the F000 handler because that modifies
CS:IP and the F000 path in computer.rs would override it.

**Solution**: Continuation trampoline at `F000:0xFF`.

### Stack layout and flow

When INT 09h chains to BIOS (F000:0009) and a custom INT 15h is installed:

1. The F000 path detects `int_num == 0x09` and `IVT[15].segment != 0xF000`
2. It pops the `ret_offset` / `ret_segment` (CALL FAR return address from `PUSHF + CALL FAR`)
3. It pops `saved_flags` (the PUSHF'd flags)
4. It builds two stack frames on top of each other:

```
[SP+0] = 0x00FF        ← INT 15h IRET IP  → F000:0xFF continuation
[SP+2] = 0xF000        ← INT 15h IRET CS
[SP+4] = int15_flags   ← FLAGS with CF=1 (handler can leave or clear)
[SP+6] = ret_offset    ← caller's return IP
[SP+8] = ret_segment   ← caller's return CS
[SP+10]= saved_flags   ← caller's FLAGS
```

5. Sets AH=4Fh, AL=scan_code, CF=1, IF=0, jumps to IVT[15]
6. When the INT 15h handler does IRET, it returns to F000:0xFF
7. F000:0xFF (`handle_int09_post_intercept`) reads CF from current flags:
   - CF=0: buffer the key in BDA (INT 09h normal path)
   - CF=1: key was intercepted; skip BDA buffering
8. F000:0xFF then pops `ret_offset / ret_segment / saved_flags` and does a far return to the original caller

### Files changed

**`core/src/cpu/bios/int09.rs`**
- Simplified to pure BDA buffering (no INT 15h awareness here)
- Extracted `add_key_to_bda_buffer()` as `pub(super)` helper so F000:0xFF can call it

**`core/src/cpu/bios/int15.rs`**
- Added `0x4F => self.int15_keyboard_intercept()` to dispatch
- Default BIOS implementation: always returns CF=0 (proceed to buffer)

**`core/src/cpu/bios/mod.rs`**
- Added `PendingKeyboardIntercept` struct:
  ```rust
  pub struct PendingKeyboardIntercept {
      pub scan_code: u8,
      pub ascii_code: u8,
      pub already_buffered: bool,
  }
  ```
- Added `pending_int15_4f: Option<PendingKeyboardIntercept>` to `Bios`
- Added `0xFF` case to `handle_bios_interrupt_impl` → calls `handle_int09_post_intercept`
- `handle_int09_post_intercept` takes the pending data, reads CF, and buffers or skips

**`core/src/computer.rs`** (F000 path in `step()`)
- Before calling `handle_bios_interrupt_direct`:
  - Checks `int_num == 0x09` AND `IVT[15].segment != 0xF000`
  - If true: constructs async chain stack, stores `pending_int15_4f`, redirects CPU to IVT[15]
  - Returns early (skips `handle_bios_interrupt_direct`)
- Gated strictly on `int_num == 0x09` to avoid triggering for any other BIOS call

### Critical bug that was fixed

The first iteration placed the `pending_int15_4f` check **after** `handle_bios_interrupt_direct`
and for **any** int_num. This caused an infinite loop:

1. INT 09h fires → `pending_int15_4f` set → chain to IVT[15]
2. IVT[15] is our own BIOS handler at F000:0015
3. F000:0015 runs `handle_bios_interrupt_direct(0x15)` ... and the post-check fires AGAIN because `pending_int15_4f` is still set
4. Endless loop producing: `"INT 15h AH=4Fh: keyboard intercept scan=0x1E, CF=0 (proceed to buffer)"`

**Fix**: Move the check to BEFORE `handle_bios_interrupt_direct`, strictly gated on `int_num == 0x09`. This way:
- `int_num == 0x09` AND custom INT 15h → async chain, return early (BIOS INT 09h never runs directly)
- `int_num == 0x09` AND default BIOS INT 15h → skip chain, call `handle_bios_interrupt_direct(0x09)` normally
- Any other `int_num` → `handle_bios_interrupt_direct` called normally, no chain logic

## Verification

Check exec log for `"INT 09h->15h AH=4Fh"` entries when running under FDC8.
If FDC8 has a custom INT 15h handler, this confirms the route is working.
Then verify `@17D5:50AC` gets set after a keypress.

## Status

- Build: **passing** (pre-commit clean as of 2026-02-16)
- Keyboard under normal DOS: **working** (verified no regression)
- FDC8 keyboard routing via INT 15h: **implemented**, pending test with FDC8
