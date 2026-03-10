# SvardOS Boot Investigation

## Symptom

SvardOS appears stuck waiting for a key press. No prompt message is visible and
pressing keys has no effect.

## Root Cause: The `option_key` Polling Loop

The stuck code is the DR-DOS/SvardOS BIOS `option_key` routine from
`drbio/biosinit.asm`, executing at **9675:52CA–52EA**. It silently polls for an
F5 or F8 keypress for up to 36 timer ticks (~2 seconds of emulated time) during
early BIOS initialisation. No prompt is ever printed — this is by design.

The full call chain:
```
9675:66A6  call 0x67af       (INT 0x2F AX=4A11h: extended memory check)
9675:5D33  call 0x52b1       (boot option check)
9675:52B1  call 0x52ca       (option_key)
9675:52CA               ← stuck loop starts here
```

Source from `biosinit.asm` (annotated):
```asm
; xor ax,ax
; int 1Ah          ; get BDA tick counter → DX = low word
; mov cx,dx        ; CX = initial tick (e.g. 0xF9F1)
option_key10:
;   push cx
;   mov ah,0x01
;   int 16h         ; AH=01 peek keyboard (non-blocking)
;   pop cx
;   jnz option_key30   ; exit if key found
;   test [flag],0x01
;   jnz option_key20   ; exit if flag set
;   push cx
;   xor ax,ax
;   int 1Ah         ; get BDA tick counter → DX = current
;   pop cx
;   sub dx,cx       ; elapsed = current - initial
;   cmp dx,36       ; 0x24 ticks timeout (~2 seconds)
;   jb option_key10    ; loop while elapsed < 36
option_key20:
;   xor ax,ax       ; timeout — ZF=1, no interesting key
;   ret
option_key30:
;   ... check if key is F5/F8, otherwise treat as timeout
```

## What the Exec Log Shows

- **`int 0x1a AH=00`** always returns `DX=0xF9F1` (BDA timer counter low word
  never changes across observed iterations).
- **`int 0x16 AH=01`** always returns `ZF=1` (no key in BDA buffer).
- No timer IRQ (INT 0x08) appears between loop iterations in the exec log.

## Timer Counter Analysis

The BDA timer counter (at physical address `0x046C`) was set to `0x0012F9EE` by
an earlier `int 0x1a AH=01` call at log line 16458. By the time the stuck loop
starts, the counter has advanced to `0xF9F1` — exactly **3 timer ticks** later.
This confirms the timer IS working: it fired three times before the stuck loop.

The observed log only captures ~6 iterations of the stuck loop (a ~2 ms window).
During this time the timer has not yet fired again. For the timer to advance one
tick inside the loop requires approximately:

```
cycles_per_irq  ≈ (8_000_000 × 65_536) / 1_193_182 ≈ 439_299
cycles/iteration ≈ 14 instructions × ~10 cycles each  ≈ 140
iterations needed ≈ 439_299 / 140 ≈ 3_138
```

So the timer fires roughly every 3,138 loop iterations. After **36 ticks ×
3,138 iterations = ~113,000 iterations** the timeout expires. At 8 MHz emulated
speed this takes ~2 seconds of wall time — working as intended.

With **exec_logging enabled** however, each instruction adds ~1 ms of real time
(file I/O). 113,000 iterations × 14 instructions × ~1 ms ≈ **26 minutes**. This
is why the emulator appears frozen when exec_logging is on.

## Why the Timer Never Fires: IF=0 Throughout the Loop

**The timer IRQ cannot fire because the interrupt flag (IF) is permanently 0
from log line 20775 onward — long before the `option_key` loop is ever reached.**

Hardware timer IRQs require `IF=1`. With `IF=0`, no IRQ can be delivered, the
BDA tick counter at `0x046C` is never incremented, and the `option_key` timeout
loop runs forever.

### Root Cause: `Verify386` in `drbio/biosinit.asm`

The function `Verify386` (line 2037) is a CPU-type detection routine called from
`relocated_init` (line 319) with interrupts **enabled** (`sti` was done at line
305). It clears IF as an unavoidable side effect on any 286+ CPU, and the caller
never restores IF with `sti` afterward.

```asm
Verify386:
    push sp             ; 8086: pushes SP-2 → ax≠sp → jne → exits WITHOUT popf
    pop  ax             ; 286+: pushes original SP → ax=sp → falls through
    cmp  ax, sp
    jne  Verify386fail  ; ← 8086 exits here, IF unchanged
    mov  ax, 3000h      ; try to set IOPL bits (12-13)
    push ax
    popf                ; ← clears IF on ALL paths (bit 9 = 0 in 0x3000)
    pushf
    pop  bx
    and  ax, bx         ; IOPL bits stuck? (386 yes, 286 no)
    jz   Verify386fail  ; 286: exits here, IF already 0
    ret                 ; 386: exits here, IF already 0
Verify386fail:
    stc
    ret
```

Both 286 and 386 exit paths go through `popf 0x3000`. Since bit 9 (IF) is 0 in
`0x3000`, IF is cleared on every 286+ exit path. On an **8086/8088**, the
`push sp` trick causes `jne Verify386fail` before the `popf`, so IF is never
touched — this is why the bug was never noticed on the original DR-DOS target
hardware.

### Traced Call Path (log lines)

```
log 20657:  IRET returns to 8991:4D94 with IF=1   (inside rploader)
              ... ~100 instructions with IF=1 ...
log 20752:  9675:50E9  call 0x5C31                 (wrapper for Verify386)
log 20774:  9675:5C60  popf                        ← POPF: IF 1 -> 0 (FLAGS=3000)
log 20781:  9675:5C69  ret                         returns from Verify386
log 20782:  9675:50EF  mov ax, [0x8213]            no STI follows
              ... 50,000+ instructions with IF=0 ...
log 92104:  9675:67B6  int 0x2f                    still IF=0
              ... option_key loop runs with IF=0 forever ...
```

### Why the Log Shows "Timer Fired 3 Times"

The 3 timer ticks that advanced `BDA[0x46C]` to `0xF9F1` occurred *before*
log line 20775 — when IF was still 1. After `Verify386` clears IF, no further
timer ticks are delivered for the rest of the logged boot.

### `Verify386` is called twice

`Verify386` is called from two places:

1. **`biosinit.asm:319`** — `relocated_init`, called once at boot. A `sti` at
   `biosinit.asm:456` (after `dos_init`) restores IF=1 after this first call.

2. **`config.asm:196`** — `cpu_init`, called from `config()` at
   `config.asm:149`. **This is the fatal call.** `cpu_init` clears IF via
   `Verify386` and never restores it. `config()` immediately proceeds to call
   `get_boot_options` (the `option_key` loop) with IF permanently 0.

```asm
; config.asm
config:
    call  country_init
    ...
    call  cpu_init          ; ← Verify386 clears IF here, no sti after
    ...
    call  config_process
    call  get_boot_options  ; ← option_key loop runs with IF=0
```

```asm
; config.asm
cpu_init:
    call  Verify386         ; ← clears IF on 286+
    jc    cpu_init10
    les   bx, func52_ptr
    mov   es:F52_CPU_TYPE[bx], 1
cpu_init10:
    stc                     ; no sti anywhere here
    ret
```

### Fix Options

| Option | Effect |
|--------|--------|
| Run emulator as **`CpuType::I8086`** | `push sp` uses post-decrement (8086 behavior), `Verify386` takes `jne` branch, `popf` never executes, IF stays 1 — **confirmed working** |
| Run emulator as **`CpuType::I80286`** or higher | Both `Verify386` calls clear IF; the second (`cpu_init`) is never followed by `sti` before `get_boot_options` — **hangs in emulator, works on real 286 hardware** |

SvardOS works correctly on real 286 hardware. The hang is therefore **not a
SvardOS bug** — it exposes a behavioural gap in the emulator's INT 0x1A
implementation. See the next section.

## Emulator Behavioural Gap: INT 0x1A Not Delivering Timer Ticks with IF=0

### What Real Hardware Does

On a real 286, `Verify386` clears IF. When `option_key` subsequently calls
`int 0x1a AH=00h` to read the tick counter:

1. The `int 0x1a` instruction pushes FLAGS (IF=0), CS, IP; clears IF; jumps to
   the BIOS ROM handler.
2. The ROM handler immediately executes `sti` — an actual x86 instruction.
   IF becomes 1.
3. The handler reads BDA[0x46C] via additional x86 instructions.
4. **Between those x86 instructions**, with IF=1, any pending timer IRQ fires
   normally: the CPU delivers INT 08h → increments BDA[0x46C] → `iret` →
   handler continues.
5. Handler returns the updated counter. `iret` restores IF=0 for the caller.

So even though the `option_key` loop runs with IF=0, the timer counter still
advances — one tick can be delivered on each INT 0x1A call that happens to
coincide with a PIT edge.

### What the Emulator Does

The emulator handles INT 0x1A as a single Rust function call
(`handle_int1a_time_services` in `core/src/cpu/bios/int1a_time_services.rs`):

```rust
pub(in crate::cpu) fn handle_int1a_time_services(&mut self, bus: &mut Bus) {
    // Tries to model the BIOS sti, but...
    self.set_flag(cpu_flag::INTERRUPT, true);
    // BDA is read in the same Rust call — take_irq() never runs between these.
    let function = (self.ax >> 8) as u8;
    match function {
        0x00 => self.int1a_get_system_time(bus),  // reads stale BDA[0x46C]
        ...
    }
}
```

`take_irq()` is only called at the top of `cpu::step()`, not inside BIOS
handlers. Setting IF=1 mid-handler has no effect because no instruction
boundary exists between the `set_flag` and the BDA read. After the handler
returns, `patch_flags_and_iret` restores the caller's IF=0 from the saved
stack frame. Result: BDA[0x46C] never advances and the `option_key` loop runs
forever.

### Proposed Fix

Add a `take_timer_irq` method to `Pic` that consumes only the pending timer
IRQ (without disturbing other pending IRQs). Call it from
`handle_int1a_time_services` after `set_flag(IF, true)`:

```rust
// In Pic:
pub(crate) fn take_timer_irq(&mut self, cycle_count: u32) -> bool {
    let bit = 1u8 << PIT_IRQ_LINE;
    let masked = self.mask & bit != 0;
    let in_service = self.in_service & bit != 0;
    if !masked && !in_service && self.pit.borrow_mut().take_pending_timer_irq(cycle_count) {
        self.in_service |= bit;
        return true;
    }
    false
}

// In handle_int1a_time_services, after set_flag(IF, true):
if bus.pic_mut().take_timer_irq(bus.cycle_count()) {
    bda_increment_timer_counter(bus);
    bus.io_write_u8(PIC_IO_PORT_COMMAND, PIC_COMMAND_EOI);
}
```

This is consistent with real hardware: the PIT's cycle-count check ensures a
tick is only delivered if enough emulated time has actually elapsed (same
rate limit as on real hardware). The fix does not unconditionally advance the
counter — it only delivers a tick when the PIT says one is due.

## Why the PIC/PIT Appears Not to Fire (in the Log)

With IF=0 established at log line 20775, no hardware IRQ can be delivered
regardless of PIC mask or PIT programming. The PIC mask is 0 (no OUT to port
0x21 was observed), so interrupts are not masked at the PIC level — they are
simply gated by the CPU's own IF flag.

## Keyboard Input

The `option_key` routine only acts on **F5** (scan 0x3F) or **F8** (scan 0x42).
Pressing any other key causes the routine to fall through to `option_key20`
(timeout path, ZF=1). This gives the impression that keyboard input is ignored.

The keyboard delivery path is:
1. GUI `push_key_press(scan_code)` → `keyboard_controller.key_press()` → `pending_key = true`
2. Next `cpu.step()`: `take_irq()` fires keyboard IRQ → `handle_int09` reads
   port 0x60, adds key to BDA buffer, sends EOI.
3. Next `int 0x16 AH=01` call → BDA buffer not empty → `ZF=0` → loop exits
   (for **any** key).
4. But `option_key30` then filters: only F5/F8 return a non-zero AX; all other
   keys fall to `option_key20` which returns `AX=0, ZF=1`.

So pressing Enter, Space, or any non-F5/F8 key **will** exit the polling loop
but the caller treats the result the same as a timeout (no special boot mode).
SvardOS then continues normal initialisation.

## Segments Involved

| Segment | Description |
|---------|-------------|
| `9675`  | SvardOS BIOS (`option_key` and surrounding boot init) |
| `8AEC`  | SvardOS kernel (device driver loading, config parsing) |
| `8991`  | SvardOS BIOS time/date helpers |
| `8656`  | SvardOS INT 0x2F multiplexer |
| `00EB`  | INT 0x2F chain entry (EMM386/DPMI check) |

## Possible Remaining Issues

1. **After option_key exits**: The exec log ends during the loop; we don't yet
   know what SvardOS does next. If it hits an unimplemented BIOS call or
   instruction, it could hang again.

2. **Missing interrupt handlers**: If SvardOS installs custom INT 0x08 or
   INT 0x09 handlers (replacing the emulator's BIOS stubs in the IVT), those
   handlers would run as x86 code and must correctly update BDA memory. The exec
   log did not capture this (no inter-iteration x86 interruptions observed),
   suggesting IVT[0x08] and IVT[0x09] still point to `BIOS_CODE_SEGMENT` during
   the logged portion.

3. **CPU type**: `Verify386` clears IF permanently on any 286+ CPU type. Running
   as `CpuType::I8086` avoids this (see above). If 286+ is needed, investigate
   whether SvardOS requires a `sti` patch after `relocated_init`.

## Recommendations

1. **Switch to `CpuType::I8086`** for SvardOS to avoid the `Verify386` IF bug.
   This is the most direct fix and matches the original DR-DOS target hardware.
2. **Run without exec_logging** to test normal speed behaviour after the fix.
   The 2-second `option_key` wait is by design.
3. **Press F5 or F8** during the 2-second window to change SvardOS boot
   options, or just wait.
4. **After the wait**, observe what SvardOS does next and capture fresh exec
   logs from that point if it hangs again.
5. **Check unimplemented BIOS calls**: look for `log::warn!("Unhandled …")` in
   the next log after the wait expires.
6. **Verify IVT integrity**: a real DR-DOS BIOS will install its own INT 0x08
   (timer) and INT 0x09 (keyboard) handlers. If the emulator's IVT is
   overwritten but the new handlers don't increment BDA[0x46C], the timer will
   stop advancing. Adding a log line when IVT entries are written would help
   detect this.
