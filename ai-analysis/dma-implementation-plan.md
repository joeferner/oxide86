# True DMA Implementation Plan

## Problem

The DMA controller (`core/src/devices/dma.rs`) is a register-only stub. It accepts all
8237A register reads/writes but never performs actual transfers. Programs like CheckIt
poll channel 0's `current_address`/`current_count` registers expecting them to change as
a transfer progresses, but they always read back the same value, causing the DMA test to
spin forever and fail.

## Current Architecture

### What exists

- **DmaController** — two `Dma8237` structs (DMA1 channels 0-3, DMA2 channels 4-7)
  with full register state: base/current address, base/current count, mode, page, mask,
  flip-flop, status.
- **Bus** — owns `Memory` and a `Vec<DeviceRef>`. Every device method receives
  `cycle_count: u32`, a monotonically increasing counter incremented after each CPU
  instruction. Exposes `memory_read_u8(addr)` / `memory_write_u8(addr, val)` for
  physical memory access.
- **FDC** — has a full command state machine (Idle → Command → Execution → Result).
  During Execution phase it buffers sector data. Currently used only in PIO mode: the
  BIOS int 13h handler reads bytes one at a time from port 0x3F5 and writes them to
  memory.
- **PIT** — already uses the cycle-count mechanism to fire timer IRQs every
  `cycles_per_irq` cycles; proves the pattern works.

### What's missing

1. ~~No DMA transfer engine — registers are written but nothing moves bytes.~~ ✅ Done — `tick()` advances counters every 4 CPU cycles.
2. No DREQ/DACK signaling between FDC and DMA controller.
3. ~~No terminal-count (TC) handling — status register TC bits are never set.~~ ✅ Done — TC sets status bit; auto-init reloads; non-auto-init masks channel.
4. ~~No cycle-based timing for transfers.~~ ✅ Done — `CPU_CYCLES_PER_DMA_CYCLE = 4`, wired into `Bus::increment_cycle_count`.
5. BIOS int 13h uses PIO exclusively.

## Design

### Core idea

The DMA controller needs a `tick(cycle_count, bus_memory)` method called from
`Bus::increment_cycle_count()`. On each tick it checks whether any unmasked channel has
a pending DREQ and, if enough cycles have elapsed, transfers one or more bytes between
the device and memory, decrementing `current_count` and advancing `current_address`.

### 8237A transfer modes to support

| Mode | Behavior |
|---|---|
| **Single** | Transfer one byte per DREQ, release bus between bytes |
| **Block** | Transfer entire block once DREQ is asserted |
| **Demand** | Transfer until DREQ is deasserted or TC |

For CheckIt compatibility, **single mode** is the priority (channel 0 memory refresh
test). For floppy, **single mode on channel 2** is standard.

### Address calculation

Physical address = `(page << 16) | current_address` (20-bit, wraps at 64K boundary
within the page — this is real 8237A behavior, addresses wrap within the 64K page).

### Transfer direction (from mode register bits 2-3)

| Bits | Direction |
|---|---|
| 01 | Write (device → memory) |
| 10 | Read (memory → device) |
| 00 | Verify (no actual transfer, but address/count still advance) |

**Verify mode** is what CheckIt uses for channel 0 — it just expects the counters to
move without any data transfer.

## Implementation Steps

### ✅ Phase 1: DMA transfer engine (counters advance) — COMPLETE

**Files:** `core/src/devices/dma.rs`, `core/src/bus.rs`

1. ✅ **Added `dreq: u8` and `last_tick_cycle: u32` to `Dma8237`.** Channel 0 DREQ permanently set (memory refresh).

2. ~~**Add `DmaTransfer` result type**~~ — skipped; tick advances counters in-place, no transfer ops needed for verify-only phase.

3. ✅ **Implemented `Dma8237::tick(cycle_count)`:**
   - Calculates elapsed cycles, converts to DMA cycles at 4:1 ratio
   - For each unmasked channel with active DREQ: advances address, decrements count
   - TC sets status bit; auto-init reloads base registers; non-auto-init masks channel

4. ✅ **`DmaController::tick()` delegates to `dma1` and `dma2`.**

5. ✅ **Wired into Bus:**
   - `Bus` holds `dma: Rc<RefCell<DmaController>>` as a named field
   - `Bus::increment_cycle_count()` calls `self.dma.borrow_mut().tick(self.cycle_count)`
   - **Note:** Bus reset order bug fixed — devices reset before `bios_reset`, so BIOS DMA init isn't wiped

### ✅ Phase 2: DREQ/DACK signaling — COMPLETE

**Files:** `core/src/devices/dma.rs`, `core/src/bus.rs`, `core/src/lib.rs`

1. ✅ **Added `Bus::dma_request(channel: u8)` / `Bus::dma_release(channel: u8)`:**
   - Calls `DmaController::set_dreq(global_channel, asserted)` which routes to DMA1/DMA2

2. ✅ **Added DACK notification via Device trait (Option B):**
   ```rust
   fn dma_read_u8(&mut self) -> Option<u8> { None }
   fn dma_write_u8(&mut self, _val: u8) -> bool { false }
   ```
   All existing devices get these as default no-ops.

3. ✅ **Channel-to-device mapping:**
   - `dma_devices: [Option<DeviceRef>; 8]` added to Bus
   - FDC wired to channel 2 in `Bus::new`

4. ✅ **`DmaTransfer` struct + tick returns ops:**
   - `Dma8237::tick()` emits one `DmaTransfer` per DMA cycle for non-verify channels
   - `DmaController::tick()` returns combined ops from both controllers
   - `Bus::execute_dma_transfer()` calls `dma_read_u8` / `dma_write_u8` on the registered device;
     silently skips if no device is registered (channel 0 memory refresh case)

### ✅ Phase 3: Channel 0 memory refresh (CheckIt fix) — COMPLETE

**Files:** `core/src/devices/dma.rs`, `core/src/cpu/bios/mod.rs`

1. ✅ **BIOS programs channel 0** in `bios_dma_reset()` (called from `bios_reset`):
   - Master clear, then base address = 0x0000, count = 0xFFFF
   - Mode = 0x58 (single, increment, auto-init, read, ch 0)
   - Unmasks channel 0

2. ✅ **Channel 0 DREQ permanently active** — `dreq = 0x01` in `Dma8237::default()`; survives master clear and software reset.

3. ✅ **Auto-init working** — TC reloads base_address/base_count, channel stays active indefinitely.

4. ✅ **Transfer rate ~4 CPU cycles/DMA cycle** — `CPU_CYCLES_PER_DMA_CYCLE = 4`.

### ✅ Phase 4: FDC DMA transfers (floppy read/write via DMA) — COMPLETE

**Files:** `core/src/devices/floppy_disk_controller.rs`, `core/src/bus.rs`

1. ✅ **FDC changes:**
   - DMA vs PIO detected from command byte: `cmd & 0xE0 == 0` → DMA, otherwise PIO (BIOS sends 0x46 with MFM bit set)
   - Added `DmaExecution { data, index, result }` phase to `FdcPhase` enum
   - On DMA command: FDC enters `DmaExecution`, sets `pending_dreq = Some(true)` to assert DREQ on ch2
   - `dma_read_u8()` returns next byte from sector buffer; on last byte transitions to Result phase and sets `pending_dreq = Some(false)` to deassert DREQ
   - MSR during `DmaExecution` returns `FDC_MSR_CB` only (no RQM/DIO/NDM) — DMA exec phase

2. ✅ **DREQ drain pattern:**
   - `pending_dreq: Option<bool>` + `take_dreq_request()` on FDC (mirrors keyboard A20 drain)
   - Bus drains DREQ after each `io_write_u8` and after each `execute_dma_transfer`
   - No Bus reference needed in FDC; no circular dependencies

3. ✅ **Fallback:** PIO path preserved — BIOS int 13h continues to work unchanged (uses MFM bit in command byte, reads data via port 0x3F5 in Execution phase).

4. ✅ **Test:** `devices/dma/floppy_read_dma` — programs DMA ch2 directly, sends READ DATA (0x06, DMA mode), verifies 512 zeroed bytes landed in buffer. All 158 tests pass.

### ✅ Phase 5: IRQ on terminal count — COMPLETE

**Files:** `core/src/devices/floppy_disk_controller.rs`, `core/src/devices/pic.rs`, `core/src/bus.rs`

- ✅ FDC raises IRQ 6 (INT 0x0E) on command completion: READ DATA (DMA and PIO), WRITE DATA, VERIFY, RECALIBRATE.
  - `pending_irq: bool` field set at each completion point; `take_pending_irq()` consumed by PIC.
  - SENSE INTERRUPT STATUS does NOT re-raise IRQ (it's the acknowledgment).
- ✅ PIC polls FDC in `take_irq()` on IRQ line 6; delivers INT 0x0E when unmasked and not in-service.
- ✅ Bus calls `notify_irq_pending()` when DREQ is deasserted (DMA done), fast-tracking the PIC scan.
- All 158 tests pass.

## Testing Strategy

1. ✅ **CheckIt DMA test** — passes. Channel 0 counters advance as expected.
2. ✅ **Floppy read/write tests** — existing `int13::floppy_read()` and
   `int13::floppy_write()` tests continue to pass (PIO path preserved).
3. ✅ **`devices/dma/counter_advances`** — ASM test programs channel 0, polls count until it changes. Passes.
4. ✅ **`devices/dma/dma_verify_mode`** — channel 1 in verify mode with count=0xFFFF; buffer pre-filled 0xCC; verifies all bytes untouched after DMA runs.
5. ✅ **`devices/dma/dma_auto_init`** — channel 1 with count=0 and auto-init; after many TC events, mask register bit 1 must be clear (channel still running).

## Implementation Order

| Priority | Phase | Why |
|---|---|---|
| 1 | ✅ Phase 1 + Phase 3 | Fixes CheckIt immediately — just needs counters to advance |
| 2 | ✅ Phase 2 | Infrastructure for device↔DMA communication |
| 3 | ✅ Phase 4 | Real floppy DMA — needed for programs that bypass BIOS |
| 4 | ✅ Phase 5 | Polish — most programs don't depend on DMA TC interrupts |

## Key Files

| File | Role |
|---|---|
| `core/src/devices/dma.rs` | DMA controller — main implementation target |
| `core/src/bus.rs` | Bus — needs tick integration and DREQ/DACK API |
| `core/src/lib.rs` | Device trait — may need `dma_read_u8`/`dma_write_u8` |
| `core/src/devices/floppy_disk_controller.rs` | FDC — needs DMA mode support |
| `core/src/cpu/bios/int13_disk_services.rs` | BIOS — switch from PIO to DMA |
| `core/src/devices/pit.rs` | Reference for cycle-based timing pattern |

## Risks

- **Address wrapping:** 8237A wraps within a 64K page boundary. Getting this wrong
  could corrupt memory in unrelated areas. Must mask `current_address` to 16 bits.
- **Transfer rate:** Too fast and programs may miss intermediate counter values; too
  slow and transfers take unrealistically long. Start with ~4 CPU cycles per DMA byte
  and adjust.
- **Borrow conflicts:** `Bus::increment_cycle_count()` will need to borrow DMA mutably
  while also accessing memory. May need to split the tick into "calculate ops" (borrow
  DMA) then "execute ops" (borrow memory) to avoid double-borrow of Bus internals.
- **Existing PIO path:** Must not break existing floppy tests while transitioning.
  Keep PIO as fallback until DMA path is proven.
