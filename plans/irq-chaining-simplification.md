# Plan: Simplify and Improve IRQ Chaining Architecture

## Context

The current IRQ chaining implementation has evolved to solve a critical "softlock" issue where timer IRQs would queue but never fire when the Interrupt Flag (IF) is stuck at 0. While the solution works, it has become complex and fragile:

**Key Problems:**
1. **Fragile stack manipulation** (lines 713-749 in computer.rs) - assumes exact chain depth with 4 sequential pops
2. **Confusing `skip_int08_chain` parameter** - non-obvious when to use true vs false, easy to misuse
3. **Memory interception complexity** - BDA timer counter is read-intercepted, requiring sync between two `pending_timer_irqs` fields
4. **Code duplication** - normal path (fire_timer_irq) vs inline path (handle_bios_interrupt_direct) have similar but different logic
5. **Inline INT 1Ch limitation** - custom handlers don't execute during inline processing (breaks QBasic PLAY during disk ops)

This plan simplifies the architecture while maintaining all existing behavior: timer accuracy, stall prevention, and support for custom interrupt handlers.

## Solution: Simplified Chaining Architecture

Replace implicit stack manipulation and dual code paths with explicit chain state tracking and a unified IRQ firing mechanism.

### Core Changes

#### 1. Add Explicit Chain Context to CPU

**File: [core/src/cpu/mod.rs](core/src/cpu/mod.rs)**

Add chain state tracking:
```rust
pub struct Cpu {
    // ... existing fields ...

    /// IRQ chain context - tracks nested interrupt chaining
    /// None = normal execution
    /// Some(ChainContext) = currently processing a chained interrupt
    irq_chain_context: Option<IrqChainContext>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IrqChainContext {
    /// The original interrupt that started the chain (e.g., 0x08)
    original_int: u8,
    /// Stack pointer before chain started (for validation)
    original_sp: u16,
    /// Return address after chain completes
    return_cs: u16,
    return_ip: u16,
    /// Flags to restore after chain
    return_flags: u16,
}
```

Add chain management methods:
```rust
impl Cpu {
    /// Begin an IRQ chain (e.g., INT 08h -> INT 1Ch)
    pub(crate) fn begin_irq_chain(&mut self, from_int: u8, to_int: u8, memory: &Memory);

    /// Check if CPU is in an IRQ chain
    pub(crate) fn is_in_irq_chain(&self) -> bool;

    /// Complete an IRQ chain (called when chained handler does IRET)
    pub(crate) fn complete_irq_chain(&mut self, memory: &Memory) -> Option<IrqChainContext>;
}
```

**Benefits:**
- Explicit state instead of implicit stack assumptions
- Stack validation catches corruption early
- Self-documenting via clear method names

#### 2. Eliminate Memory Interception

**File: [core/src/memory.rs](core/src/memory.rs)**

Remove timer counter interception from `read_u8()` - it currently intercepts BDA timer reads to add pending_timer_irqs.

**File: [core/src/computer.rs](core/src/computer.rs)**

Add direct BDA timer management:
```rust
pub struct Computer<V: VideoController> {
    // ... existing fields ...

    /// Pending timer IRQs (moved from Memory, no longer duplicated)
    pending_timer_irqs: u32,

    /// Base BDA timer counter - independent of pending IRQs
    bda_timer_base: u32,
}

impl<V: VideoController> Computer<V> {
    /// Write BDA timer counter directly to memory
    fn write_bda_timer(&mut self, tick_count: u32);

    /// Read BDA timer counter (includes pending IRQs)
    fn read_bda_timer(&self) -> u32;

    /// Sync BDA timer with accurate count (before time reads)
    fn sync_bda_timer(&mut self);
}
```

**Benefits:**
- Single source of truth for timer counter
- No more sync between Computer.pending_timer_irqs and Memory.pending_timer_irqs
- Explicit sync before time reads (instead of hidden interception)
- Simpler Memory implementation

#### 3. Unified IRQ Firing Mechanism

**File: [core/src/computer.rs](core/src/computer.rs)**

Replace dual paths (fire_timer_irq + inline processing) with single mechanism:
```rust
/// Unified timer IRQ firing - handles both normal and inline cases
/// Returns true if IRQ was processed
fn process_timer_irq(&mut self) -> bool;
```

Logic:
- If custom INT 08h: use full interrupt with stack frame
- If BIOS INT 08h: update BDA counter directly
  - If custom INT 1Ch: call `cpu.begin_irq_chain(0x08, 0x1C, ...)`
  - If BIOS INT 1Ch: done (no-op handler)

**Benefits:**
- No more skip_int08_chain parameter (eliminates confusion)
- Single code path (easier maintenance)
- Context-aware behavior via `is_in_irq_chain()`

#### 4. Simplified F000 Handling

**File: [core/src/computer.rs](core/src/computer.rs), lines 625-749**

Replace fragile 4-pop sequence with chain detection:
```rust
if current_cs == 0xF000 {
    let int_num = (current_ip & 0xFF) as u8;

    // Pop return address from CALL FAR
    let ret_offset = self.cpu.pop(&self.memory);
    let ret_segment = self.cpu.pop(&self.memory);

    // Call BIOS handler (no skip_int08_chain needed!)
    self.cpu.handle_bios_interrupt(&mut self.memory, &mut self.bios, &mut self.video);

    // Check if handler started an IRQ chain
    if self.cpu.is_in_irq_chain() {
        // Chain in progress - let it complete naturally via IRET
        return;
    }

    // Normal return - pop FLAGS and return to caller
    let saved_flags = self.cpu.pop(&self.memory);
    self.cpu.flags = (self.cpu.flags & 0xF8FF) | (saved_flags & 0x0700);
    self.cpu.ip = ret_offset;
    self.cpu.cs = ret_segment;

    // Process pending timer IRQs inline if IF=1
    while self.cpu.get_flag(cpu_flag::INTERRUPT) && self.pending_timer_irqs > 0 {
        self.process_timer_irq();
        self.pending_timer_irqs -= 1;
    }

    return;
}
```

**Benefits:**
- No more fragile stack manipulation (4 pops based on assumptions)
- Chain detection is explicit and testable
- Stack layout is validated by IrqChainContext
- Inline processing uses same mechanism as normal path

#### 5. Enhanced IRET Handling

**File: [core/src/cpu/instructions.rs](core/src/cpu/instructions.rs)**

Modify IRET to detect chain completion:
```rust
pub(super) fn execute_iret(&mut self, memory: &Memory) {
    // Standard IRET: pop IP, CS, FLAGS
    self.ip = self.pop(memory);
    self.cs = self.pop(memory);
    self.flags = self.pop(memory);

    // Check if this completes an IRQ chain
    if let Some(context) = self.complete_irq_chain(memory) {
        log::debug!("IRQ Chain Complete: INT 0x{:02X}", context.original_int);

        // Validate stack pointer (catches corruption early)
        if self.sp != context.original_sp {
            log::warn!("Stack mismatch after IRQ chain");
        }
    }
}
```

**Benefits:**
- Automatic chain cleanup on IRET
- Stack validation catches bugs early
- Clear logging for debugging

#### 6. Remove skip_int08_chain Parameter

**Files:**
- [core/src/cpu/bios/int08.rs](core/src/cpu/bios/int08.rs)
- [core/src/cpu/bios/mod.rs](core/src/cpu/bios/mod.rs)

Remove `skip_int08_chain` parameter from:
- `handle_int08()`
- `handle_bios_interrupt_direct()`

Replace with context-aware behavior:
- If custom INT 1Ch detected: use `begin_irq_chain()`
- If BIOS INT 1Ch: no chaining needed

**Benefits:**
- Simpler API
- No more parameter confusion
- Intent is clear from method names

## Implementation Steps

### Phase 1: Add New Structures (Non-Breaking)
1. Add `IrqChainContext` struct to [core/src/cpu/mod.rs](core/src/cpu/mod.rs)
2. Add `irq_chain_context: Option<IrqChainContext>` field to CPU
3. Initialize to `None` in CPU::new()
4. Add methods: `begin_irq_chain()`, `complete_irq_chain()`, `is_in_irq_chain()`
5. Add `bda_timer_base: u32` field to Computer
6. Add methods: `write_bda_timer()`, `read_bda_timer()`, `sync_bda_timer()`
7. Add `process_timer_irq()` method to Computer

### Phase 2: Switch to New Path (Atomic)
1. Update F000 handling in [core/src/computer.rs](core/src/computer.rs) step() to use chain detection
2. Update step() to call `process_timer_irq()` instead of `fire_timer_irq()`
3. Update IRET in [core/src/cpu/instructions.rs](core/src/cpu/instructions.rs) to call `complete_irq_chain()`
4. Update [core/src/cpu/bios/int08.rs](core/src/cpu/bios/int08.rs) to use `begin_irq_chain()` instead of `chain_to_interrupt()`
5. Remove `skip_int08_chain` parameter from all call sites

### Phase 3: Cleanup (Safe)
1. Remove `Memory.pending_timer_irqs` field from [core/src/memory.rs](core/src/memory.rs)
2. Remove timer interception logic from `Memory::read_u8()`
3. Remove `set_pending_timer_irqs()` method from Memory
4. Remove `increment_bda_timer()` method from Memory
5. Remove old `fire_timer_irq()` if no longer needed
6. Remove `chain_to_interrupt()` method (replaced by `begin_irq_chain()`)

### Phase 4: Testing
Test with programs that stress IRQ handling:
1. **CheckIt 2.1** - tests timer accuracy during disk operations
2. **QBasic with PLAY** - tests custom INT 1Ch during inline processing (currently broken)
3. **DOS programs with custom INT 08h handlers** - verify full interrupt mechanism
4. **Direct BDA timer reads** - verify accuracy matches before/after

## Benefits

### Maintainability
- ✅ Clear separation of concerns
- ✅ Explicit state tracking (no hidden assumptions)
- ✅ Self-documenting via IrqChainContext
- ✅ Easier to test (can mock chain scenarios)

### Robustness
- ✅ Stack validation catches corruption early
- ✅ No fragile multi-pop sequences
- ✅ Context tracks expected state
- ✅ Fails fast on unexpected behavior

### Fixes Issues
- ✅ Issue 1: Fragile stack manipulation → replaced with explicit context
- ✅ Issue 2: Confusing skip_int08_chain → parameter eliminated
- ✅ Issue 3: Memory interception → replaced with direct BDA management
- ✅ Issue 4: Code duplication → unified process_timer_irq()
- ✅ Issue 5: Inline INT 1Ch limitation → works via begin_irq_chain()
- ✅ Issue 6: Double pending_timer_irqs → single field in Computer

### Performance
- ✅ No overhead (same number of operations)
- ✅ Memory read_u8() is simpler (no interception)
- ✅ Inline processing still works

### Compatibility
- ✅ All existing behavior preserved
- ✅ Custom handlers work correctly
- ✅ Timer accuracy maintained
- ✅ Stall prevention unchanged

## Critical Files

1. [core/src/cpu/mod.rs](core/src/cpu/mod.rs) - Add IrqChainContext, chain management methods
2. [core/src/computer.rs](core/src/computer.rs) - Add bda_timer_base, process_timer_irq(), simplify F000 handling
3. [core/src/cpu/bios/int08.rs](core/src/cpu/bios/int08.rs) - Remove skip_chain parameter, use begin_irq_chain()
4. [core/src/cpu/bios/mod.rs](core/src/cpu/bios/mod.rs) - Remove skip_int08_chain parameter
5. [core/src/memory.rs](core/src/memory.rs) - Remove pending_timer_irqs field, remove timer interception
6. [core/src/cpu/instructions.rs](core/src/cpu/instructions.rs) - Update IRET to call complete_irq_chain()

## Verification

After implementation, verify:
1. Run CLI: `cargo run -p emu86-native-cli -- test-programs/checkit.com`
2. Verify timer accuracy remains within ±1 tick during disk operations
3. Test QBasic PLAY during disk I/O (currently broken, should be fixed)
4. Check logs for "IRQ Chain" debug messages during custom INT 1Ch execution
5. Run pre-commit: `./scripts/pre-commit.sh`
6. No regressions in existing functionality
