pub(crate) mod at_parser;
pub mod phonebook;
pub mod transport;

use std::collections::VecDeque;

use at_parser::AtCommand;
use phonebook::ModemPhonebook;
use transport::{ModemDialer, ModemTransport, TransportEvent};

use crate::devices::uart::{ComPortDevice, ModemControlLines};

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
    transport: Option<Box<dyn ModemTransport>>,
    dialer: Option<Box<dyn ModemDialer>>,
}

impl SerialModem {
    pub fn new() -> Self {
        Self::with_phonebook_and_dialer(None, None)
    }

    pub fn with_phonebook(phonebook: Option<ModemPhonebook>) -> Self {
        Self::with_phonebook_and_dialer(phonebook, None)
    }

    pub fn with_phonebook_and_dialer(
        phonebook: Option<ModemPhonebook>,
        dialer: Option<Box<dyn ModemDialer>>,
    ) -> Self {
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
            transport: None,
            dialer,
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
        self.transport = None;
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
            if let Some(t) = &mut self.transport {
                t.cancel_escape_guard();
            }
            self.state = ModemState::CommandMode;
            self.send_result(ResultCode::Ok);
        } else {
            self.escape_guard_reads = Some(new_count);
        }
    }

    fn poll_transport(&mut self) {
        if self.transport.is_none() {
            return;
        }

        // Drain incoming bytes unless escape guard is pending (guest expects OK next).
        if self.escape_guard_reads.is_none() {
            if let Some(t) = &mut self.transport {
                t.poll_incoming(&mut self.rx_queue);
            }
            if !self.rx_queue.is_empty() {
                self.irq_pending = true;
            }
        }

        let event = self.transport.as_mut().and_then(|t| t.take_event());
        match event {
            Some(TransportEvent::Connected) => {
                self.state = ModemState::Connected;
                self.dcd = true;
                self.send_result(ResultCode::Connect);
            }
            Some(TransportEvent::ConnectFailed) => {
                self.state = ModemState::CommandMode;
                self.transport = None;
                self.send_result(ResultCode::NoCarrier);
            }
            Some(TransportEvent::RemoteDisconnected) => {
                self.send_result(ResultCode::NoCarrier);
                self.dcd = false;
                self.escape_count = 0;
                self.escape_guard_reads = None;
                self.state = ModemState::CommandMode;
                self.transport = None;
            }
            None => {}
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
                        let transport = self.dialer.as_ref().map(|d| d.dial(&endpoint));
                        if let Some(transport) = transport {
                            self.transport = Some(transport);
                            self.state = ModemState::Dialing;
                        } else {
                            log::warn!("modem: ATDT{} → {} (TCP not supported)", number, endpoint);
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
                let was_connected =
                    self.dcd || matches!(self.state, ModemState::Connected | ModemState::Dialing);
                self.dcd = false;
                self.escape_count = 0;
                self.state = ModemState::CommandMode;
                self.transport = None;
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
        self.poll_transport();
        self.poll_escape_guard();
        let byte = self.rx_queue.pop_front();
        self.irq_pending = !self.rx_queue.is_empty();
        byte
    }

    fn write(&mut self, value: u8) -> bool {
        if self.transport.is_some() {
            if matches!(self.state, ModemState::Connected) {
                if value == b'+' {
                    self.escape_count += 1;
                    if self.escape_count >= 3 && self.escape_guard_reads.is_none() {
                        // Third '+' received — start guard countdown (read-poll and wall-clock).
                        self.escape_guard_reads = Some(0);
                        let guard_ms = self.s_registers[12] as u64 * 20;
                        if let Some(t) = &mut self.transport {
                            t.start_escape_guard(guard_ms);
                        }
                    }
                    // Don't forward '+' to TCP until escape is confirmed or cancelled.
                    return true;
                } else {
                    let guard_by_reads = self
                        .escape_guard_reads
                        .is_some_and(|n| n >= self.s_registers[12] as u32);
                    let guard_by_time = self
                        .transport
                        .as_ref()
                        .map(|t| t.escape_time_elapsed())
                        .unwrap_or(false);
                    if self.escape_count >= 3 && (guard_by_reads || guard_by_time) {
                        // Guard expired — confirm escape and fall through to command-mode handling.
                        self.escape_guard_reads = None;
                        self.escape_count = 0;
                        if let Some(t) = &mut self.transport {
                            t.cancel_escape_guard();
                        }
                        self.state = ModemState::CommandMode;
                        self.send_result(ResultCode::Ok);
                        // fall through: handle `value` as a command-mode character below
                    } else {
                        // Guard not expired — cancel escape and forward buffered '+' plus this byte.
                        let pending = self.escape_count;
                        self.escape_count = 0;
                        self.escape_guard_reads = None;
                        if let Some(t) = &mut self.transport {
                            t.cancel_escape_guard();
                            for _ in 0..pending {
                                t.send_byte(b'+');
                            }
                            t.send_byte(value);
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
        // Pump transport so irq_pending reflects any data that arrived since the last read().
        // Without this, interrupt-driven software (e.g. PCPlus) deadlocks: it waits for the
        // RX IRQ, but poll_transport() is normally only called from read(), which is only called
        // after the IRQ fires.
        self.poll_transport();
        let pending = self.irq_pending;
        self.irq_pending = false;
        pending
    }

    fn modem_control_changed(&mut self, _lines: ModemControlLines) {}

    fn modem_status(&mut self) -> u8 {
        // Pump transport so DCD/CTS reflect the actual connection state on every MSR read.
        self.poll_transport();
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
