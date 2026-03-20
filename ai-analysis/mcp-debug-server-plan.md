# MCP Debug Server Implementation Plan

Interactive debugging for the oxide86 emulator via a Model Context Protocol server.
The server is opt-in via `--debug-mcp <port>` and has near-zero overhead when not enabled.

---

## Goals

- Allow interactive debugging of a running emulator session (set breakpoints, inspect registers/memory, step/continue)
- Zero overhead on the hot path when not enabled
- MCP server lives on its own thread; the emulator stays single-threaded
- CLI option: `--debug-mcp <port>` (e.g. `--debug-mcp 7777`)

---

## New Crate: `oxide86-debugger`

A new crate in the workspace that both `native-common` and potentially `oxide86-core` can depend on.
It contains the shared debug state and the MCP server logic.

```
oxide86/
├── oxide86-debugger/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs          — DebugShared, DebugSnapshot types
│       └── server.rs       — MCP TCP server, tool dispatch
```

`core` does **not** depend on `oxide86-debugger`. Instead, `native-common` creates a `DebugShared`
and passes it into `Computer`. This keeps the core crate wasm-safe.

---

## Shared Debug State (`oxide86-debugger/src/lib.rs`)

```rust
pub struct DebugShared {
    /// Set of (CS, IP) breakpoint pairs. Checked each instruction only if non-empty.
    pub breakpoints: Mutex<HashSet<(u16, u16)>>,

    /// Fast flag: any breakpoints exist? Checked with Relaxed load on every step.
    pub has_breakpoints: AtomicBool,

    /// Set of physical addresses that trigger a pause on write.
    pub write_watchpoints: Mutex<HashSet<u32>>,

    /// Fast flag: any write watchpoints exist?
    pub has_write_watchpoints: AtomicBool,

    /// Set when a write watchpoint fires: (physical_addr, value_written, cs, ip).
    pub watchpoint_hit: Mutex<Option<(u32, u8, u16, u16)>>,

    /// Set by the MCP `pause` tool to request a halt on the next step.
    pub pause_requested: AtomicBool,

    /// True while emulator is paused (breakpoint hit or explicit pause request).
    pub paused: AtomicBool,

    /// CPU/Bus snapshot captured when a breakpoint is hit.
    pub snapshot: Mutex<Option<DebugSnapshot>>,

    /// MCP server places a command here; emulator reads and executes it.
    pub pending_command: Mutex<Option<DebugCommand>>,

    /// Emulator places its response here after executing a command.
    pub pending_response: Mutex<Option<DebugResponse>>,

    /// Signalled when emulator transitions to paused (MCP server waits here).
    pub cond_paused: Condvar,

    /// Signalled when MCP server sends a new command (emulator waits here).
    pub cond_command: Condvar,
}

pub struct DebugSnapshot {
    pub cs: u16, pub ip: u16,
    pub ax: u16, pub bx: u16, pub cx: u16, pub dx: u16,
    pub si: u16, pub di: u16, pub sp: u16, pub bp: u16,
    pub ds: u16, pub es: u16, pub ss: u16, pub fs: u16, pub gs: u16,
    pub flags: u16,
}

pub enum DebugCommand {
    Continue,
    Step(u32),           // Step N instructions
    ReadMemory { addr: u32, len: u32 },
    SendKey(u8),         // Inject a PC scan code via Computer::push_key_press
    AddWriteWatchpoint(u32),     // Add physical address watchpoint
    RemoveWriteWatchpoint(u32),  // Remove physical address watchpoint
}

pub enum DebugResponse {
    Ok,
    Memory(Vec<u8>),
}
```

---

## Changes to `core` — `Computer` / `Cpu`

### `Computer` gets an optional `Arc<DebugShared>`

```rust
pub struct Computer {
    cpu: Cpu,
    bus: Bus,
    // ... existing fields ...
    debug: Option<Arc<DebugShared>>,  // None → zero overhead
}

impl Computer {
    pub fn set_debug(&mut self, debug: Arc<DebugShared>) {
        self.debug = Some(debug);
    }
}
```

### Hook into `Computer::step()`

The check happens in `Computer::step()` rather than inside `Cpu::step()` so the core CPU code
stays clean and has no dependency on the debugger types.

```rust
pub fn step(&mut self) {
    // Fast path: single Option::is_none() check — one pointer comparison, branch-predicted away
    if let Some(ref dbg) = self.debug {
        self.debug_check(dbg);
    }

    self.process_key_presses();
    self.cpu.step(&mut self.bus);
}

fn debug_check(&mut self, dbg: &Arc<DebugShared>) {
    // If already paused, service commands until Continue/Step is received
    if dbg.paused.load(Ordering::Relaxed) {
        self.service_debug_commands(dbg);
        return;
    }

    // Explicit pause requested by MCP `pause` tool
    if dbg.pause_requested.load(Ordering::Relaxed) {
        dbg.pause_requested.store(false, Ordering::Relaxed);
        self.do_pause(dbg);
        return;
    }

    // Check breakpoints only when any exist (AtomicBool load — still very cheap)
    if dbg.has_breakpoints.load(Ordering::Relaxed) {
        let cs = self.cpu.cs();
        let ip = self.cpu.ip();
        let bps = dbg.breakpoints.lock().unwrap();
        if bps.contains(&(cs, ip)) {
            drop(bps);
            self.do_pause(dbg);
        }
    }
}

fn do_pause(&mut self, dbg: &Arc<DebugShared>) {
    // Capture snapshot
    *dbg.snapshot.lock().unwrap() = Some(self.cpu.snapshot());

    // Signal MCP server that we are now paused
    dbg.paused.store(true, Ordering::SeqCst);
    dbg.cond_paused.notify_all();

    // Block until Continue or Step
    self.service_debug_commands(dbg);
}

fn service_debug_commands(&mut self, dbg: &Arc<DebugShared>) {
    loop {
        let cmd = {
            let mut lock = dbg.pending_command.lock().unwrap();
            while lock.is_none() {
                lock = dbg.cond_command.wait(lock).unwrap();
            }
            lock.take().unwrap()
        };

        match cmd {
            DebugCommand::Continue => {
                dbg.paused.store(false, Ordering::SeqCst);
                *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Ok);
                dbg.cond_paused.notify_all(); // wake MCP if it's watching
                break;
            }
            DebugCommand::Step(n) => {
                // Execute n instructions then re-pause
                for _ in 0..n {
                    self.cpu.step(&mut self.bus);
                }
                *dbg.snapshot.lock().unwrap() = Some(self.cpu.snapshot());
                *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Ok);
                dbg.cond_paused.notify_all();
                // Stay paused — loop again waiting for next command
            }
            DebugCommand::ReadMemory { addr, len } => {
                let bytes: Vec<u8> = (addr..addr + len)
                    .map(|a| self.bus.memory_read_u8(a as usize))
                    .collect();
                *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Memory(bytes));
                dbg.cond_paused.notify_all();
                // Stay paused — wait for next command
            }
            DebugCommand::SendKey(scan_code) => {
                // Works whether paused or running; push_key_press queues the scan code
                // so the emulator processes it on the next step after Continue/Step.
                self.push_key_press(scan_code);
                *dbg.pending_response.lock().unwrap() = Some(DebugResponse::Ok);
                dbg.cond_paused.notify_all();
                // Stay paused — wait for next command
            }
        }
    }
}
```

`Cpu` needs a small addition — public accessors for `cs()`, `ip()`, and `snapshot()`:

```rust
pub(crate) fn cs(&self) -> u16 { self.cs }
pub(crate) fn ip(&self) -> u16 { self.ip }
pub(crate) fn snapshot(&self) -> DebugSnapshot { ... }
```

These are already internally accessible; just need `pub(crate)` visibility for `Computer` to call them.

---

## Write Watchpoints — Bus Hook

Write watchpoints fire on any `Bus::memory_write_u8` call. Because `Bus` is owned by `Computer`,
the `Arc<DebugShared>` is passed down to `Bus` alongside the existing `debug` field.

```rust
// In Bus::memory_write_u8 (fast path unchanged when has_write_watchpoints is false)
pub fn memory_write_u8(&mut self, addr: usize, val: u8) {
    // ... existing device / memory write logic ...

    if let Some(ref dbg) = self.debug {
        if dbg.has_write_watchpoints.load(Ordering::Relaxed) {
            let wps = dbg.write_watchpoints.lock().unwrap();
            if wps.contains(&(addr as u32)) {
                drop(wps);
                // Record what fired so the snapshot is enriched
                *dbg.watchpoint_hit.lock().unwrap() =
                    Some((addr as u32, val, /*cs/ip filled in by Computer::do_pause*/0, 0));
                dbg.pause_requested.store(true, Ordering::SeqCst);
            }
        }
    }
}
```

`Computer::do_pause` fills in the `cs`/`ip` fields of `watchpoint_hit` from `self.cpu` before
calling `cond_paused.notify_all()`. The MCP `get_registers` response includes the watchpoint info
when present so the client knows why execution stopped.

`AddWriteWatchpoint` / `RemoveWriteWatchpoint` commands update `write_watchpoints` and flip
`has_write_watchpoints` accordingly; these are handled directly by the MCP server thread without
needing the emulator to be paused (Mutex is sufficient).

---

## MCP Server (`oxide86-debugger/src/server.rs`)

The MCP server runs on its own `std::thread`, bound to the configured TCP port.
It speaks the [MCP protocol](https://modelcontextprotocol.io/docs/concepts/architecture) over a
persistent TCP connection (stdio transport is also fine if preferred).

### Startup (called from `native-common`)

```rust
pub fn start_mcp_server(port: u16, debug: Arc<DebugShared>) {
    std::thread::spawn(move || {
        let listener = TcpListener::bind(("127.0.0.1", port)).expect("MCP port in use");
        log::info!("MCP debug server listening on port {port}");
        for stream in listener.incoming() {
            let debug = Arc::clone(&debug);
            std::thread::spawn(move || handle_connection(stream.unwrap(), debug));
        }
    });
}
```

### MCP Tools Exposed

| Tool | Arguments | Description |
|---|---|---|
| `get_registers` | — | Returns all registers + flags from latest snapshot |
| `read_memory` | `addr: u32`, `len: u32` | Reads `len` bytes from flat physical address |
| `set_breakpoint` | `seg: u16`, `off: u16` | Adds (seg, off) to the breakpoint set |
| `clear_breakpoint` | `seg: u16`, `off: u16` | Removes a breakpoint |
| `list_breakpoints` | — | Returns all current breakpoints |
| `pause` | — | Requests the emulator to pause after the current instruction (sets `pause_requested`), then waits on `cond_paused` |
| `continue` | — | Resumes execution from a paused state |
| `step` | `n: u32` (default 1) | Executes N instructions while paused |
| `run_until_int` | `ah: u8` | Sets a soft breakpoint on the next INT 21h with AH == ah |
| `get_status` | — | Returns `running` or `paused_at CS:IP` |
| `send_key` | `scan_code: u8` | Injects a PC scan code into the keyboard buffer (calls `Computer::push_key_press`) |
| `set_write_watchpoint` | `addr: u32` | Adds a physical address to the write watchpoint set; emulator pauses on next write to that address |
| `clear_write_watchpoint` | `addr: u32` | Removes a write watchpoint |
| `list_write_watchpoints` | — | Returns all current write watchpoints |

### JSON-RPC message shape (MCP standard)

```json
// Request
{ "jsonrpc": "2.0", "id": 1, "method": "tools/call",
  "params": { "name": "set_breakpoint", "arguments": { "seg": "0x1234", "off": "0x0042" } } }

// Response
{ "jsonrpc": "2.0", "id": 1, "result": { "content": [{ "type": "text", "text": "Breakpoint set at 1234:0042" }] } }
```

### Dependencies to add to `oxide86-debugger/Cargo.toml`

```toml
[dependencies]
log = { workspace = true }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

No async runtime needed — each connection gets its own thread, and the MCP tool calls are synchronous
request/response that block until the emulator acks.

---

## CLI Changes (`native-common/src/cli.rs`)

```rust
/// Debugging
#[arg(long = "debug-mcp", value_name = "PORT")]
pub debug_mcp_port: Option<u16>,
```

### `create_computer` in `native-common/src/lib.rs`

```rust
pub fn create_computer(cli: &CommonCli, ...) -> Result<...> {
    // ... existing setup ...

    if let Some(port) = cli.debug_mcp_port {
        let debug = Arc::new(DebugShared::new());
        computer.set_debug(Arc::clone(&debug));
        oxide86_debugger::start_mcp_server(port, debug);
        log::info!("MCP debug server started on port {port}");
    }

    Ok((computer, ...))
}
```

---

## Performance Impact Analysis

| Condition | Per-instruction overhead |
|---|---|
| `--debug-mcp` not passed | Single `Option::is_none()` pointer comparison — ~0 cycles |
| Debug enabled, no breakpoints | Two `AtomicBool::load(Relaxed)` — ~2-4 cycles |
| Debug enabled, breakpoints set | AtomicBool + `Mutex::try_lock` + HashSet lookup — ~20-50 cycles |
| Paused at breakpoint | Emulator thread blocked on condvar — 0 CPU usage |

At 8 MHz emulated speed the batch loop runs ~8000 instructions/ms. Two relaxed atomic loads add
negligible overhead even in the breakpoint-armed case.

---

## File Change Summary

| File | Change |
|---|---|
| `oxide86-debugger/` | New crate (lib.rs, server.rs, Cargo.toml) |
| `Cargo.toml` (workspace) | Add `oxide86-debugger` member; add `serde_json` workspace dep |
| `core/Cargo.toml` | No changes — core stays wasm-clean |
| `core/src/computer.rs` | Add `debug: Option<Arc<DebugShared>>`, hook in `step()` |
| `core/src/cpu/mod.rs` | Add `pub(crate)` accessors: `cs()`, `ip()`, `snapshot()` |
| `native-common/Cargo.toml` | Add `oxide86-debugger` dep |
| `native-common/src/cli.rs` | Add `--debug-mcp <PORT>` arg to `CommonCli` |
| `native-common/src/lib.rs` | Wire up `DebugShared` + `start_mcp_server` in `create_computer` |

---

## Implementation Order

1. Create `oxide86-debugger` crate with `DebugShared`, `DebugSnapshot`, `DebugCommand`, `DebugResponse` types
2. Add `pub(crate)` CPU accessors + `Computer::set_debug()` + `Computer::debug_check()`
3. Add `--debug-mcp` CLI arg and wire it up in `create_computer`
4. Implement MCP server (JSON-RPC listener, tool dispatch)
5. Test: run emulator with `--debug-mcp 7777`, connect with an MCP client, set a breakpoint, verify pause/resume

---

## Open Questions

- **Transport: TCP** — bind to `127.0.0.1:<port>`. stdio transport is not used; it would require
  spawning the emulator as a subprocess and is not suitable for an already-running process.
- **`run_until_int` implementation**: Could be a temporary single-use breakpoint inserted at the INT
  dispatch site, or a separate `AtomicU8` checked only in the BIOS interrupt path. The latter is
  cleaner and avoids polluting the regular breakpoint set.
- **GUI frontend**: `native-gui` uses `create_computer` from `native-common`, so it would get
  `--debug-mcp` for free once wired up there. No extra work needed.

---

## README.md — VSCode Claude Code Extension Setup

Add the following section to the project `README.md` so users know how to connect the Claude Code
VSCode extension to a running emulator session.

---

### Debugging with Claude Code (VSCode Extension)

The emulator can expose a live debug interface over MCP, letting Claude inspect registers, memory,
and breakpoints while the emulator is running.

**1. Start the emulator with the MCP debug server**

```bash
cargo run -p oxide86-cli -- --debug-mcp 7777 mygame.exe
```

**2. Register the MCP server in VSCode**

Add the server via the Claude Code CLI (run this once per port you want to use):

```bash
claude mcp add --transport tcp oxide86 127.0.0.1:7777
```

Or edit `~/.claude/mcp.json` (global) / `.claude/mcp.json` (project) directly:

```json
{
  "mcpServers": {
    "oxide86": {
      "type": "tcp",
      "host": "127.0.0.1",
      "port": 7777
    }
  }
}
```

Then type `/mcp` in the Claude Code chat panel and use **Reconnect** to pick up the new server.

**3. Use the tools in Claude**

Once connected, Claude can call the debugger tools directly:

| What you can ask Claude | Tool used |
|---|---|
| "What are the current register values?" | `get_registers` |
| "Set a breakpoint at 160F:0042" | `set_breakpoint` |
| "Show me 64 bytes at physical address 0x4064E" | `read_memory` |
| "Pause the emulator" | `pause` |
| "Step 10 instructions" | `step` |
| "Watch for writes to address 0x4064E" | `set_write_watchpoint` |
| "Resume execution" | `continue` |

**Notes**

- The MCP server only binds to `127.0.0.1` — it is not accessible over the network.
- The server accepts one connection at a time; reconnect if the session drops.
- Starting a new emulator run requires re-registering or reusing the same port.
