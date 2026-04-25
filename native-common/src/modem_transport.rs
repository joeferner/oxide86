use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use oxide86_core::devices::modem::transport::{ModemDialer, ModemTransport, TransportEvent};

// ---------------------------------------------------------------------------
// Telnet IAC filter (moved from core/src/devices/modem/telnet.rs)
// ---------------------------------------------------------------------------

const IAC: u8 = 0xFF;
const WILL: u8 = 0xFB;
const WONT: u8 = 0xFC;
const DO: u8 = 0xFD;
const DONT: u8 = 0xFE;
const SB: u8 = 0xFA;
const SE: u8 = 0xF0;
const OPT_ECHO: u8 = 0x01;
const OPT_SGA: u8 = 0x03;
const OPT_NAWS: u8 = 0x1F;

enum TelnetState {
    Normal,
    Iac,
    Cmd(u8),
    Sb,
    SbIac,
}

struct TelnetFilter {
    state: TelnetState,
    responses: VecDeque<u8>,
}

impl TelnetFilter {
    fn new() -> Self {
        Self {
            state: TelnetState::Normal,
            responses: VecDeque::new(),
        }
    }

    fn process(&mut self, byte: u8) -> Option<u8> {
        match self.state {
            TelnetState::Normal => {
                if byte == IAC {
                    self.state = TelnetState::Iac;
                    None
                } else {
                    Some(byte)
                }
            }
            TelnetState::Iac => match byte {
                IAC => {
                    self.state = TelnetState::Normal;
                    Some(IAC)
                }
                WILL | WONT | DO | DONT => {
                    self.state = TelnetState::Cmd(byte);
                    None
                }
                SB => {
                    self.state = TelnetState::Sb;
                    None
                }
                _ => {
                    self.state = TelnetState::Normal;
                    None
                }
            },
            TelnetState::Cmd(cmd) => {
                self.state = TelnetState::Normal;
                self.handle_option(cmd, byte);
                None
            }
            TelnetState::Sb => {
                if byte == IAC {
                    self.state = TelnetState::SbIac;
                }
                None
            }
            TelnetState::SbIac => {
                if byte == SE {
                    self.state = TelnetState::Normal;
                } else {
                    self.state = TelnetState::Sb;
                }
                None
            }
        }
    }

    fn handle_option(&mut self, cmd: u8, opt: u8) {
        match cmd {
            DO => match opt {
                OPT_NAWS => {
                    self.responses.extend([IAC, WILL, OPT_NAWS]);
                    self.responses
                        .extend([IAC, SB, OPT_NAWS, 0, 80, 0, 25, IAC, SE]);
                }
                _ => {
                    self.responses.extend([IAC, WONT, opt]);
                }
            },
            WILL => match opt {
                OPT_ECHO | OPT_SGA => {
                    self.responses.extend([IAC, DO, opt]);
                }
                _ => {
                    self.responses.extend([IAC, DONT, opt]);
                }
            },
            _ => {}
        }
    }

    fn flush_responses<W: std::io::Write>(&mut self, writer: &mut W) {
        if !self.responses.is_empty() {
            let data: Vec<u8> = self.responses.drain(..).collect();
            let _ = writer.write_all(&data);
        }
    }
}

// ---------------------------------------------------------------------------
// NativeTcpTransport
// ---------------------------------------------------------------------------

pub struct NativeTcpTransport {
    incoming: Arc<Mutex<VecDeque<u8>>>,
    /// Wrapped in Mutex to satisfy `Sync` (mpsc::Sender is !Sync).
    outgoing: Mutex<mpsc::Sender<u8>>,
    connected: Arc<AtomicBool>,
    connection_done: Arc<AtomicBool>,
    disconnected: Arc<AtomicBool>,
    connect_event_delivered: bool,
    disconnect_event_delivered: bool,
    escape_guard_time: Option<std::time::Instant>,
    escape_guard_ms: u64,
}

impl ModemTransport for NativeTcpTransport {
    fn poll_incoming(&mut self, out: &mut VecDeque<u8>) {
        if let Ok(mut q) = self.incoming.try_lock() {
            out.extend(q.drain(..));
        }
    }

    fn send_byte(&mut self, byte: u8) {
        if let Ok(tx) = self.outgoing.lock() {
            let _ = tx.send(byte);
        }
    }

    fn take_event(&mut self) -> Option<TransportEvent> {
        if !self.connect_event_delivered && self.connection_done.load(Ordering::Acquire) {
            self.connect_event_delivered = true;
            return Some(if self.connected.load(Ordering::Acquire) {
                TransportEvent::Connected
            } else {
                TransportEvent::ConnectFailed
            });
        }
        if !self.disconnect_event_delivered && self.disconnected.load(Ordering::Acquire) {
            self.disconnect_event_delivered = true;
            return Some(TransportEvent::RemoteDisconnected);
        }
        None
    }

    fn start_escape_guard(&mut self, guard_ms: u64) {
        self.escape_guard_time = Some(std::time::Instant::now());
        self.escape_guard_ms = guard_ms;
    }

    fn cancel_escape_guard(&mut self) {
        self.escape_guard_time = None;
    }

    fn escape_time_elapsed(&self) -> bool {
        self.escape_guard_time
            .map(|t| t.elapsed().as_millis() >= self.escape_guard_ms as u128)
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// NativeDialer
// ---------------------------------------------------------------------------

pub struct NativeDialer;

impl NativeDialer {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NativeDialer {
    fn default() -> Self {
        Self::new()
    }
}

impl ModemDialer for NativeDialer {
    fn dial(&self, addr: &str) -> Box<dyn ModemTransport> {
        let incoming = Arc::new(Mutex::new(VecDeque::new()));
        let connected = Arc::new(AtomicBool::new(false));
        let connection_done = Arc::new(AtomicBool::new(false));
        let disconnected = Arc::new(AtomicBool::new(false));
        let (outgoing_tx, outgoing_rx) = mpsc::channel::<u8>();

        let incoming_t = incoming.clone();
        let connected_t = connected.clone();
        let connection_done_t = connection_done.clone();
        let disconnected_t = disconnected.clone();
        let addr = addr.to_owned();

        std::thread::spawn(move || {
            connect_tcp_thread(
                addr,
                incoming_t,
                outgoing_rx,
                connected_t,
                connection_done_t,
                disconnected_t,
            );
        });

        Box::new(NativeTcpTransport {
            incoming,
            outgoing: Mutex::new(outgoing_tx),
            connected,
            connection_done,
            disconnected,
            connect_event_delivered: false,
            disconnect_event_delivered: false,
            escape_guard_time: None,
            escape_guard_ms: 0,
        })
    }
}

// ---------------------------------------------------------------------------
// TCP connection thread
// ---------------------------------------------------------------------------

fn connect_tcp_thread(
    addr: String,
    incoming: Arc<Mutex<VecDeque<u8>>>,
    outgoing_rx: mpsc::Receiver<u8>,
    connected: Arc<AtomicBool>,
    connection_done: Arc<AtomicBool>,
    disconnected: Arc<AtomicBool>,
) {
    match std::net::TcpStream::connect(&addr) {
        Ok(stream) => {
            let _ = stream.set_nodelay(true);
            connected.store(true, Ordering::Release);
            connection_done.store(true, Ordering::Release);

            let mut reader = stream.try_clone().expect("clone TCP stream");
            let incoming_for_reader = incoming;
            let disconnected_for_reader = disconnected;
            std::thread::spawn(move || {
                let mut buf = [0u8; 256];
                let mut telnet = TelnetFilter::new();
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            let mut data = Vec::with_capacity(n);
                            for &byte in &buf[..n] {
                                if let Some(b) = telnet.process(byte) {
                                    data.push(b);
                                }
                            }
                            telnet.flush_responses(&mut reader);
                            if !data.is_empty() {
                                incoming_for_reader
                                    .lock()
                                    .unwrap()
                                    .extend(data.iter().copied());
                            }
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
            let _ = writer.shutdown(std::net::Shutdown::Both);
        }
        Err(e) => {
            log::warn!("modem: TCP connect to {} failed: {}", addr, e);
            connection_done.store(true, Ordering::Release);
        }
    }
}
