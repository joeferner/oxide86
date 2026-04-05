# Plan: 286 Protected Mode Support

## Context

The emulator currently operates in real mode only. The 286 CPU type exists (`CpuType::I80286`) and gates some instructions, but there is no protected mode machinery — no descriptor tables, no privilege levels, no mode-aware segment resolution. The goal is to add 286 protected mode support in small, testable increments, structured so 386 protected mode can build on top later.

## Key Design Decisions

- **Segment resolution is the central abstraction to change.** Today `physical_address()` in `bus.rs` does `(seg << 4) + offset`. In protected mode, segment register values are *selectors* that index into the GDT/LDT to get a base address and limit. The CPU must translate selectors → descriptors → base+limit before calling into the bus.
- **Descriptor tables live in emulated memory**, accessed via `bus.memory_read_*`. The CPU stores only the GDTR/IDTR/LDTR/TR register values (base + limit).
- **A new `protected_mode` module** under `core/src/cpu/` will hold descriptor parsing, segment loading, and privilege checking — keeping `mod.rs` and instruction files focused.
- **Each step has its own test .asm file** that runs as a `.com` program, sets up minimal GDT/IDT, enters protected mode, exercises the feature, then halts. (The 286 cannot return to real mode without a CPU reset — clearing PE is a 386+ feature.)

## Critical Files

| File | Role |
|---|---|
| `core/src/cpu/mod.rs` | CPU struct — add CR0/MSW, GDTR, IDTR, LDTR, TR, CPL fields |
| `core/src/cpu/cpu_type.rs` | Uncomment `supports_protected_mode()` |
| `core/src/cpu/protected_mode.rs` | **New** — descriptor structs, segment loading, privilege checks |
| `core/src/cpu/instructions/data_transfer.rs` | LGDT/LIDT/LLDT/SGDT/SIDT/SLDT, LMSW, MOV to/from CR0, segment loads |
| `core/src/cpu/instructions/control_flow.rs` | Protected-mode interrupt dispatch (IDT), IRET, far CALL/JMP through gates |
| `core/src/bus.rs` | `physical_address` may need a protected-mode-aware variant or the CPU resolves before calling bus |
| `core/src/tests/cpu/mod.rs` | New test entries |
| `core/src/test_data/cpu/` | New .asm test files |

## Implementation Steps

### ✅ Step 1: CPU State — CR0/MSW and System Registers

Add to `Cpu` struct:
- `cr0: u16` (286 MSW is the low 16 bits of CR0; only PE, MP, EM, TS bits matter)
- `gdtr_base: u32`, `gdtr_limit: u16` — Global Descriptor Table Register
- `idtr_base: u32`, `idtr_limit: u16` — Interrupt Descriptor Table Register
- `ldtr: u16` — Local Descriptor Table selector
- `tr: u16` — Task Register selector
- `cpl: u8` — Current Privilege Level (0–3)
- Helper: `fn in_protected_mode(&self) -> bool { self.cr0 & 1 != 0 }`

Update `SMSW` to return `self.cr0 & 0xFFFF` instead of `0x0000`.
Update `LMSW` to set the low 4 bits of CR0 (PE can be set but not cleared via LMSW).

**Test:** Write `pm_smsw_lmsw.asm` — SMSW to verify PE=0 initially, LMSW to set PE, SMSW to read it back and verify PE=1. (On 286, PE cannot be cleared once set — only a CPU reset clears it.)

---

### ✅ Step 2: LGDT / LIDT / SGDT / SIDT Instructions

Implement the remaining `0F 01` sub-opcodes:
- `/0` SGDT — store 6 bytes (limit:16, base:24 on 286) to memory
- `/1` SIDT — same for IDTR
- `/2` LGDT — load 6 bytes from memory into GDTR
- `/3` LIDT — load 6 bytes from memory into IDTR

On 286, the base is 24-bit (3 bytes), not 32-bit. The 6-byte format is: limit (2 bytes), base (3 bytes), then 1 reserved byte.

**Test:** Write `pm_lgdt_sgdt.asm` — build a GDT in memory, LGDT it, SGDT to a buffer, compare values.

---

### ✅ Step 3: Descriptor Table Structures and Segment Loading

Create `core/src/cpu/protected_mode.rs`:
- `SegmentDescriptor` struct: base (24-bit on 286), limit (16-bit), access byte (present, DPL, type, S, etc.)
- `fn load_descriptor(bus, table_base, table_limit, selector) -> Result<SegmentDescriptor, Fault>`
- `fn check_segment_access(descriptor, selector, cpl) -> Result<(), Fault>` — privilege checks

In the CPU, when loading a segment register (MOV sreg, r/m16; POP sreg; LDS; LES; far JMP/CALL; IRET; INT):
- If in protected mode: treat value as selector, load descriptor, validate, cache base+limit
- If in real mode: existing behavior (value is paragraph base)

Add cached descriptor fields for each segment register (base, limit, access rights) to avoid re-reading descriptors on every memory access. These are the "hidden" parts of segment registers.

**Test:** Write `pm_segment_load.asm` — enter protected mode, load DS with a valid selector, read memory through it, verify the descriptor's base is applied.

---

### ✅ Step 4: Protected-Mode Physical Address Resolution

Change how the CPU resolves segment:offset to physical addresses in protected mode:
- Instead of `(segment << 4) + offset`, use `cached_base[segreg] + offset`
- Check `offset <= cached_limit[segreg]` and raise #GP if out of bounds
- This affects `fetch_byte`, `push`, `pop`, all memory reads/writes in instructions

The key change: `bus.physical_address(segment, offset)` either needs a protected-mode variant or the CPU computes the physical address itself using cached descriptor bases and passes it directly to `bus.memory_read/write`.

Approach: Add a `resolve_address(&self, segreg, offset) -> usize` method on `Cpu` that returns the physical address (using cached bases in protected mode, `seg<<4` in real mode). Gradually migrate callers.

**Test:** Write `pm_memory_access.asm` — set up segments with different bases, write to one, read through another at the corresponding physical address, verify.

---

### ✅ Step 5: Protected-Mode Interrupt Dispatch (IDT)

When `in_protected_mode()`, `dispatch_interrupt` must:
1. Read the IDT entry (8 bytes per entry on 286) at `idtr_base + int_num * 8`
2. Parse the interrupt gate descriptor (selector + offset + type)
3. Validate: present, correct type (interrupt gate or trap gate), DPL check
4. Push FLAGS, CS, IP onto stack (same as real mode)
5. Load CS:IP from the gate descriptor's selector:offset
6. For interrupt gates: clear IF. For trap gates: leave IF unchanged.

**Test:** Write `pm_idt.asm` — set up a minimal IDT with one interrupt gate, trigger INT in protected mode, verify handler runs and returns via IRET.

---

### Step 6: Protected-Mode Exception Handling (#GP, #NP, #SS, #TS)

Add proper CPU exceptions for protected mode faults:
- `#GP` (13) — general protection fault (bad selector, privilege violation, limit violation)
- `#NP` (11) — segment not present
- `#SS` (12) — stack segment fault  
- `#TS` (10) — invalid TSS

These are dispatched through the IDT like regular interrupts, but push an error code onto the stack.

**Test:** Write `pm_exceptions.asm` — intentionally load an invalid selector, verify #GP fires and the handler receives the correct error code.

---

### Step 7: LLDT / SLDT / LTR / STR Instructions

Implement:
- `LLDT r/m16` (0F 00 /2) — load LDT register from selector
- `SLDT r/m16` (0F 00 /0) — store LDT register
- `LTR r/m16` (0F 00 /3) — load Task Register
- `STR r/m16` (0F 00 /1) — store Task Register

LDT loading: read the LDT descriptor from the GDT, cache its base and limit.

**Test:** Write `pm_lldt.asm` — set up a GDT entry for an LDT, LLDT it, create a segment in the LDT, load that segment, access memory through it.

---

### Step 8: Far CALL/JMP and RET Through Call Gates (286)

In protected mode, far JMP and far CALL check the selector:
- If it points to a code segment: direct transfer (with privilege checks)
- If it points to a call gate: indirect transfer through the gate (which can change privilege levels)

Far RET must pop CS:IP (and possibly SS:SP for inter-privilege returns) and validate selectors.

**Test:** Write `pm_call_gate.asm` — set up a call gate in GDT, far CALL through it, verify control reaches the target and far RET returns correctly.

---

### Step 9: Privilege Level Transitions (Ring 0 ↔ Ring 3)

Full CPL/RPL/DPL checking:
- Code segment loads check CPL vs DPL (conforming vs non-conforming)
- Data segment loads check max(CPL, RPL) vs DPL  
- Stack switches on privilege transitions (TSS provides ring 0/1/2 SS:SP)
- IRET from ring 0 to ring 3 must restore SS:SP from stack

**Test:** Write `pm_rings.asm` — set up ring 0 and ring 3 code/data/stack segments, transition from ring 0 to ring 3 via IRET, return to ring 0 via call gate, verify.

---

### Step 10: Real Mode → Protected Mode Transition and Reset Path

On a real 286, protected mode is a one-way trip — there is no instruction to clear PE. The only way back to real mode is a CPU reset (typically triggered by the keyboard controller's reset line or a triple fault), after which the BIOS checks a shutdown byte in CMOS (address 0x0F) to determine whether to resume from a known address rather than cold-booting.

Implement:
- **Entry to PM:** set PE bit via LMSW, immediately far JMP to flush prefetch and load CS with a valid selector
- **Reset path:** support the keyboard controller reset (port 0x64, command 0xFE) and/or triple fault triggering a CPU reset. On reset, check CMOS shutdown byte to resume at the address stored in the BIOS data area (40:67h), which is how real 286 systems returned to real mode.
- Verify that real-mode programs still work identically when CPU type is 286 and PE=0.

**Test:** Write `pm_mode_switch.asm` — enter protected mode, do some work, trigger a reset via keyboard controller, verify the CPU resumes in real mode at the expected address.

---

## Verification

After each step:
1. Run `cargo test --all` to verify no regressions
2. The new `.asm` test should pass (exit code 0)
3. Run `./scripts/pre-commit.sh` after the final step

End-to-end: a program that enters protected mode, sets up GDT/IDT/LDT, switches to ring 3, triggers an interrupt, returns to ring 0 via a call gate, and exits back to real mode — all passing.

## Future: 386 Protected Mode

This plan is structured so 386 support extends naturally:
- `SegmentDescriptor` gains 32-bit base/limit fields (already present in 386 descriptors)
- `cr0` becomes `u32`, add `cr2`/`cr3`/`cr4`
- Paging layer sits between segment resolution and physical bus access
- 32-bit operand/address size prefixes affect gate sizes and stack frame layouts
- TSS structure grows to 32-bit version
