pub(crate) mod at_parser;
pub mod phonebook;

use std::collections::VecDeque;

use at_parser::AtCommand;
use phonebook::ModemPhonebook;

use crate::devices::uart::{ComPortDevice, ModemControlLines};

#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[allow(dead_code)]
enum ModemState {
    CommandMode,
    DataMode,
    Dialing,
    Connected,
}

enum ExecuteResult {
    Continue,
    Terminal,
    Error,
}

#[allow(dead_code)]
enum ResultCode {
    Ok,
    Error,
    Connect,
    Ring,
    NoCarrier,
    NoDialtone,
    Busy,
    NoAnswer,
}

impl ResultCode {
    fn verbose_str(&self) -> &'static str {
        match self {
            ResultCode::Ok => "OK",
            ResultCode::Error => "ERROR",
            ResultCode::Connect => "CONNECT",
            ResultCode::Ring => "RING",
            ResultCode::NoCarrier => "NO CARRIER",
            ResultCode::NoDialtone => "NO DIALTONE",
            ResultCode::Busy => "BUSY",
            ResultCode::NoAnswer => "NO ANSWER",
        }
    }

    fn numeric(&self) -> u8 {
        match self {
            ResultCode::Ok => 0,
            ResultCode::Connect => 1,
            ResultCode::Ring => 2,
            ResultCode::NoCarrier => 3,
            ResultCode::Error => 4,
            ResultCode::NoDialtone => 6,
            ResultCode::Busy => 7,
            ResultCode::NoAnswer => 8,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct TcpBridge {
    incoming: Arc<Mutex<VecDeque<u8>>>,
    outgoing: mpsc::Sender<u8>,
    connected: Arc<AtomicBool>,
    connection_done: Arc<AtomicBool>,
    /// Set by the reader thread when the remote end closes the connection.
    disconnected: Arc<AtomicBool>,
}

const S_REGISTER_COUNT: usize = 32;

pub struct SerialModem {
    state: ModemState,
    cmd_buf: String,
    rx_queue: VecDeque<u8>,
    irq_pending: bool,
    echo: bool,
    verbose: bool,
    quiet: bool,
    s_registers: [u8; S_REGISTER_COUNT],
    dcd: bool,
    phonebook: Option<ModemPhonebook>,
    /// Counts consecutive '+' bytes received in Connected state.
    escape_count: u8,
    /// After the 3rd '+', counts read() polls before confirming escape (S12 guard-after).
    /// `None` = no escape pending; `Some(n)` = n polls done so far.
    escape_guard_reads: Option<u32>,
    #[cfg(not(target_arch = "wasm32"))]
    /// Wall-clock timestamp of the 3rd '+'. Fallback guard for terminal software (e.g. Telix)
    /// that waits via a timer rather than by reading the COM port between +++ and ATH.
    escape_guard_time: Option<std::time::Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    tcp_bridge: Option<TcpBridge>,
}

impl SerialModem {
    pub fn new() -> Self {
        Self::with_phonebook(None)
    }

    pub fn with_phonebook(phonebook: Option<ModemPhonebook>) -> Self {
        Self {
            state: ModemState::CommandMode,
            cmd_buf: String::new(),
            rx_queue: VecDeque::new(),
            irq_pending: false,
            echo: true,
            verbose: true,
            quiet: false,
            s_registers: Self::default_s_registers(),
            dcd: false,
            phonebook,
            escape_count: 0,
            escape_guard_reads: None,
            #[cfg(not(target_arch = "wasm32"))]
            escape_guard_time: None,
            #[cfg(not(target_arch = "wasm32"))]
            tcp_bridge: None,
        }
    }

    fn default_s_registers() -> [u8; S_REGISTER_COUNT] {
        let mut regs = [0u8; S_REGISTER_COUNT];
        regs[12] = 50; // S12: guard-after poll count before confirming +++ escape
        regs
    }

    fn reset_defaults(&mut self) {
        self.echo = true;
        self.verbose = true;
        self.quiet = false;
        self.s_registers = Self::default_s_registers();
        self.dcd = false;
        self.escape_count = 0;
        self.escape_guard_reads = None;
        self.state = ModemState::CommandMode;
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.escape_guard_time = None;
            self.tcp_bridge = None;
        }
    }

    fn queue_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.rx_queue.push_back(b);
        }
        if !self.rx_queue.is_empty() {
            self.irq_pending = true;
        }
    }

    fn send_result(&mut self, code: ResultCode) {
        if self.quiet {
            return;
        }
        if self.verbose {
            let text = code.verbose_str();
            let mut buf = Vec::with_capacity(text.len() + 2);
            buf.extend_from_slice(text.as_bytes());
            buf.push(0x0D);
            buf.push(0x0A);
            self.queue_bytes(&buf);
        } else {
            let n = code.numeric();
            let s = format!("{}\r\n", n);
            self.queue_bytes(s.as_bytes());
        }
    }

    fn process_command(&mut self) {
        let raw = std::mem::take(&mut self.cmd_buf);
        log::debug!("modem: AT command: {:?}", raw.trim());
        let commands = at_parser::parse(&raw);
        for command in commands {
            match self.execute(command) {
                ExecuteResult::Continue => {}
                ExecuteResult::Terminal => return,
                ExecuteResult::Error => {
                    self.send_result(ResultCode::Error);
                    return;
                }
            }
        }
        self.send_result(ResultCode::Ok);
    }

    /// S12 guard-after: called on every read() poll while +++ escape is pending.
    /// After s_registers[12] polls the escape is confirmed: switch to command mode and send OK.
    fn poll_escape_guard(&mut self) {
        let Some(count) = self.escape_guard_reads else {
            return;
        };
        let new_count = count + 1;
        if new_count >= self.s_registers[12] as u32 {
            self.escape_guard_reads = None;
            self.escape_count = 0;
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.escape_guard_time = None;
            }
            self.state = ModemState::CommandMode;
            self.send_result(ResultCode::Ok);
        } else {
            self.escape_guard_reads = Some(new_count);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn poll_tcp(&mut self) {
        let Some(bridge) = &self.tcp_bridge else {
            return;
        };

        // While escape guard is pending the guest expects OK next, not TCP data.
        if self.escape_guard_reads.is_none() {
            if let Ok(mut q) = bridge.incoming.try_lock() {
                while let Some(byte) = q.pop_front() {
                    self.rx_queue.push_back(byte);
                }
            }
            if !self.rx_queue.is_empty() {
                self.irq_pending = true;
            }
        }

        match self.state {
            ModemState::Dialing if bridge.connection_done.load(Ordering::Acquire) => {
                if bridge.connected.load(Ordering::Acquire) {
                    self.state = ModemState::Connected;
                    self.dcd = true;
                } else {
                    self.state = ModemState::CommandMode;
                    self.tcp_bridge = None;
                }
            }
            ModemState::Connected if bridge.disconnected.load(Ordering::Acquire) => {
                // Remote closed the TCP connection — queue NO CARRIER and go idle
                self.send_result(ResultCode::NoCarrier);
                self.dcd = false;
                self.escape_count = 0;
                self.escape_guard_reads = None;
                self.state = ModemState::CommandMode;
                self.tcp_bridge = None;
            }
            _ => {}
        }
    }

    fn execute(&mut self, command: AtCommand) -> ExecuteResult {
        match command {
            AtCommand::At | AtCommand::Ignore => ExecuteResult::Continue,
            AtCommand::Reset | AtCommand::FactoryReset => {
                self.reset_defaults();
                ExecuteResult::Continue
            }
            AtCommand::Echo(on) => {
                self.echo = on;
                ExecuteResult::Continue
            }
            AtCommand::Verbose(on) => {
                self.verbose = on;
                ExecuteResult::Continue
            }
            AtCommand::Quiet(on) => {
                self.quiet = on;
                ExecuteResult::Continue
            }
            AtCommand::Dial(number) => {
                let addr = self.phonebook.as_ref().and_then(|pb| pb.resolve(&number));
                match addr {
                    Some(endpoint) => {
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            let incoming = Arc::new(Mutex::new(VecDeque::new()));
                            let connected = Arc::new(AtomicBool::new(false));
                            let connection_done = Arc::new(AtomicBool::new(false));
                            let disconnected = Arc::new(AtomicBool::new(false));
                            let (outgoing_tx, outgoing_rx) = mpsc::channel::<u8>();
                            let incoming_t = incoming.clone();
                            let connected_t = connected.clone();
                            let connection_done_t = connection_done.clone();
                            let disconnected_t = disconnected.clone();
                            let verbose = self.verbose;
                            let quiet = self.quiet;
                            std::thread::spawn(move || {
                                connect_tcp(TcpArgs {
                                    addr: endpoint,
                                    incoming: incoming_t,
                                    outgoing_rx,
                                    connected: connected_t,
                                    connection_done: connection_done_t,
                                    disconnected: disconnected_t,
                                    verbose,
                                    quiet,
                                });
                            });
                            self.tcp_bridge = Some(TcpBridge {
                                incoming,
                                outgoing: outgoing_tx,
                                connected,
                                connection_done,
                                disconnected,
                            });
                            self.state = ModemState::Dialing;
                        }
                        #[cfg(target_arch = "wasm32")]
                        {
                            log::warn!(
                                "modem: ATDT{} → {} (TCP not supported on WASM)",
                                number,
                                endpoint
                            );
                            self.send_result(ResultCode::NoDialtone);
                        }
                    }
                    None => {
                        log::warn!("modem: ATDT{} — no phonebook entry", number);
                        self.send_result(ResultCode::NoDialtone);
                    }
                }
                ExecuteResult::Terminal
            }
            AtCommand::HangUp => {
                // was_connected covers: Connected, Dialing, and CommandMode-with-DCD (post-+++)
                let was_connected =
                    self.dcd || matches!(self.state, ModemState::Connected | ModemState::Dialing);
                self.dcd = false;
                self.escape_count = 0;
                self.state = ModemState::CommandMode;
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.tcp_bridge = None;
                }
                if was_connected {
                    self.send_result(ResultCode::NoCarrier);
                } else {
                    self.send_result(ResultCode::Ok);
                }
                ExecuteResult::Terminal
            }
            AtCommand::OffHook => {
                self.send_result(ResultCode::Ok);
                ExecuteResult::Terminal
            }
            AtCommand::Answer => ExecuteResult::Error,
            AtCommand::SRegisterSet { reg, val } => {
                if (reg as usize) < S_REGISTER_COUNT {
                    self.s_registers[reg as usize] = val;
                }
                ExecuteResult::Continue
            }
            AtCommand::Info => {
                self.queue_bytes(b"oxide86 Virtual Modem\r\n");
                ExecuteResult::Continue
            }
            AtCommand::SRegisterQuery(reg) => {
                let val = if (reg as usize) < S_REGISTER_COUNT {
                    self.s_registers[reg as usize]
                } else {
                    0
                };
                let s = format!("{}\r\n", val);
                self.queue_bytes(s.as_bytes());
                ExecuteResult::Continue
            }
            AtCommand::Escape => {
                // AT+++ in command mode: already idle, just acknowledge
                self.state = ModemState::CommandMode;
                ExecuteResult::Continue
            }
            AtCommand::Unknown(raw) => {
                log::warn!("modem: unrecognised AT command: {}", raw.trim());
                ExecuteResult::Error
            }
        }
    }
}

impl Default for SerialModem {
    fn default() -> Self {
        Self::new()
    }
}

impl ComPortDevice for SerialModem {
    fn reset(&mut self) {
        self.cmd_buf.clear();
        self.rx_queue.clear();
        self.irq_pending = false;
        self.reset_defaults();
    }

    fn read(&mut self) -> Option<u8> {
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_tcp();
        self.poll_escape_guard();
        let byte = self.rx_queue.pop_front();
        self.irq_pending = !self.rx_queue.is_empty();
        byte
    }

    fn write(&mut self, value: u8) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if matches!(self.state, ModemState::Connected) {
                if value == b'+' {
                    self.escape_count += 1;
                    if self.escape_count >= 3 && self.escape_guard_reads.is_none() {
                        // Third '+' received — start guard countdown (read-poll and wall-clock).
                        self.escape_guard_reads = Some(0);
                        self.escape_guard_time = Some(std::time::Instant::now());
                    }
                    // Don't forward '+' to TCP until escape is confirmed or cancelled.
                    return true;
                } else {
                    // Non-'+' byte arrived while escape may be pending.
                    let guard_by_reads = self
                        .escape_guard_reads
                        .is_some_and(|n| n >= self.s_registers[12] as u32);
                    let guard_by_time = self
                        .escape_guard_time
                        .map(|t| t.elapsed().as_millis() >= self.s_registers[12] as u128 * 20)
                        .unwrap_or(false);
                    if self.escape_count >= 3 && (guard_by_reads || guard_by_time) {
                        // Guard expired — confirm escape and fall through to command-mode handling.
                        self.escape_guard_reads = None;
                        self.escape_guard_time = None;
                        self.escape_count = 0;
                        self.state = ModemState::CommandMode;
                        self.send_result(ResultCode::Ok);
                        // fall through: handle `value` as a command-mode character below
                    } else {
                        // Guard not expired — cancel escape and forward buffered '+' plus this byte.
                        let pending = self.escape_count;
                        self.escape_count = 0;
                        self.escape_guard_reads = None;
                        self.escape_guard_time = None;
                        if let Some(bridge) = &self.tcp_bridge {
                            for _ in 0..pending {
                                let _ = bridge.outgoing.send(b'+');
                            }
                            let _ = bridge.outgoing.send(value);
                        }
                        return true;
                    }
                }
            }
            if matches!(self.state, ModemState::Dialing) {
                return true;
            }
        }
        match value {
            0x0D => {
                if self.echo {
                    self.queue_bytes(&[0x0D, 0x0A]);
                }
                self.process_command();
            }
            0x08 => {
                if self.echo {
                    self.queue_bytes(&[0x08]);
                }
                self.cmd_buf.pop();
            }
            _ => {
                if self.echo {
                    self.queue_bytes(&[value]);
                }
                if value.is_ascii() {
                    self.cmd_buf.push(value as char);
                }
            }
        }
        true
    }

    fn take_irq(&mut self) -> bool {
        let pending = self.irq_pending;
        self.irq_pending = false;
        pending
    }

    fn modem_control_changed(&mut self, _lines: ModemControlLines) {}

    fn modem_status(&mut self) -> u8 {
        // CTS (bit 4): low while dialing, high otherwise
        // DSR (bit 5): always high
        // DCD (bit 7): high when TCP is connected
        let cts = if matches!(self.state, ModemState::Dialing) {
            0
        } else {
            0x10
        };
        cts | 0x20 | if self.dcd { 0x80 } else { 0 }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct TcpArgs {
    addr: String,
    incoming: Arc<Mutex<VecDeque<u8>>>,
    outgoing_rx: mpsc::Receiver<u8>,
    connected: Arc<AtomicBool>,
    connection_done: Arc<AtomicBool>,
    disconnected: Arc<AtomicBool>,
    verbose: bool,
    quiet: bool,
}

#[cfg(not(target_arch = "wasm32"))]
fn connect_tcp(args: TcpArgs) {
    let TcpArgs {
        addr,
        incoming,
        outgoing_rx,
        connected,
        connection_done,
        disconnected,
        verbose,
        quiet,
    } = args;
    use std::io::{Read as _, Write as _};

    match std::net::TcpStream::connect(&addr) {
        Ok(stream) => {
            let _ = stream.set_nodelay(true);
            connected.store(true, Ordering::Release);
            connection_done.store(true, Ordering::Release);
            if !quiet {
                let msg: &[u8] = if verbose { b"CONNECT\r\n" } else { b"1\r\n" };
                incoming.lock().unwrap().extend(msg.iter().copied());
            }
            let mut reader = stream.try_clone().expect("clone TCP stream");
            let incoming_for_reader = incoming;
            let disconnected_for_reader = disconnected;
            std::thread::spawn(move || {
                let mut buf = [0u8; 256];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            incoming_for_reader
                                .lock()
                                .unwrap()
                                .extend(buf[..n].iter().copied());
                        }
                    }
                }
                disconnected_for_reader.store(true, Ordering::Release);
            });
            let mut writer = stream;
            while let Ok(byte) = outgoing_rx.recv() {
                if writer.write_all(&[byte]).is_err() {
                    break;
                }
            }
            // Sender dropped (ATH / bridge torn down) or write failed — shut down the socket
            // so the reader thread's read() unblocks and the remote end sees EOF.
            let _ = writer.shutdown(std::net::Shutdown::Both);
        }
        Err(e) => {
            log::warn!("modem: TCP connect to {} failed: {}", addr, e);
            connection_done.store(true, Ordering::Release);
            if !quiet {
                let msg: &[u8] = if verbose { b"NO CARRIER\r\n" } else { b"3\r\n" };
                incoming.lock().unwrap().extend(msg.iter().copied());
            }
        }
    }
}
