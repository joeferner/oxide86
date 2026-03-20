use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

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

impl DebugShared {
    pub fn new() -> Self {
        Self {
            breakpoints: Mutex::new(HashSet::new()),
            has_breakpoints: AtomicBool::new(false),
            write_watchpoints: Mutex::new(HashSet::new()),
            has_write_watchpoints: AtomicBool::new(false),
            watchpoint_hit: Mutex::new(None),
            pause_requested: AtomicBool::new(false),
            paused: AtomicBool::new(false),
            snapshot: Mutex::new(None),
            pending_command: Mutex::new(None),
            pending_response: Mutex::new(None),
            cond_paused: Condvar::new(),
            cond_command: Condvar::new(),
        }
    }

    pub fn add_breakpoint(&self, cs: u16, ip: u16) {
        let mut bps = self.breakpoints.lock().unwrap();
        bps.insert((cs, ip));
        self.has_breakpoints.store(true, Ordering::Relaxed);
    }

    pub fn remove_breakpoint(&self, cs: u16, ip: u16) {
        let mut bps = self.breakpoints.lock().unwrap();
        bps.remove(&(cs, ip));
        if bps.is_empty() {
            self.has_breakpoints.store(false, Ordering::Relaxed);
        }
    }

    pub fn list_breakpoints(&self) -> Vec<(u16, u16)> {
        self.breakpoints.lock().unwrap().iter().copied().collect()
    }

    pub fn add_write_watchpoint(&self, addr: u32) {
        let mut wps = self.write_watchpoints.lock().unwrap();
        wps.insert(addr);
        self.has_write_watchpoints.store(true, Ordering::Relaxed);
    }

    pub fn remove_write_watchpoint(&self, addr: u32) {
        let mut wps = self.write_watchpoints.lock().unwrap();
        wps.remove(&addr);
        if wps.is_empty() {
            self.has_write_watchpoints.store(false, Ordering::Relaxed);
        }
    }

    pub fn list_write_watchpoints(&self) -> Vec<u32> {
        self.write_watchpoints
            .lock()
            .unwrap()
            .iter()
            .copied()
            .collect()
    }

    /// Send a command to the emulator and wait for its response.
    pub fn send_command(&self, cmd: DebugCommand) -> DebugResponse {
        {
            let mut lock = self.pending_command.lock().unwrap();
            *lock = Some(cmd);
        }
        self.cond_command.notify_all();

        let mut lock = self.pending_response.lock().unwrap();
        loop {
            if let Some(resp) = lock.take() {
                return resp;
            }
            lock = self.cond_paused.wait(lock).unwrap();
        }
    }
}

#[derive(Clone)]
pub struct DebugSnapshot {
    pub cs: u16,
    pub ip: u16,
    pub ax: u16,
    pub bx: u16,
    pub cx: u16,
    pub dx: u16,
    pub si: u16,
    pub di: u16,
    pub sp: u16,
    pub bp: u16,
    pub ds: u16,
    pub es: u16,
    pub ss: u16,
    pub fs: u16,
    pub gs: u16,
    pub flags: u16,
}

pub enum DebugCommand {
    Continue,
    Step(u32),
    ReadMemory { addr: u32, len: u32 },
    SendKey(u8),
    AddWriteWatchpoint(u32),
    RemoveWriteWatchpoint(u32),
}

pub enum DebugResponse {
    Ok,
    Memory(Vec<u8>),
}
