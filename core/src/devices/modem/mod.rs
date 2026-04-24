pub(crate) mod at_parser;
pub mod phonebook;

use std::collections::VecDeque;

use at_parser::AtCommand;
use phonebook::ModemPhonebook;

use crate::devices::uart::{ComPortDevice, ModemControlLines};

#[allow(dead_code)]
enum ModemState {
    CommandMode,
    DataMode,
    Dialing,
    Connected,
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
            s_registers: [0u8; S_REGISTER_COUNT],
            dcd: false,
            phonebook,
        }
    }

    fn reset_defaults(&mut self) {
        self.echo = true;
        self.verbose = true;
        self.quiet = false;
        self.s_registers = [0u8; S_REGISTER_COUNT];
        self.dcd = false;
        self.state = ModemState::CommandMode;
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
        let command = at_parser::parse(&raw);
        self.execute(command);
    }

    fn execute(&mut self, command: AtCommand) {
        match command {
            AtCommand::At => {
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Reset | AtCommand::FactoryReset => {
                self.reset_defaults();
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Echo(on) => {
                self.echo = on;
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Verbose(on) => {
                self.verbose = on;
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Quiet(on) => {
                self.quiet = on;
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Dial(number) => {
                let addr = self.phonebook.as_ref().and_then(|pb| pb.resolve(&number));
                match addr {
                    Some(endpoint) => {
                        log::warn!(
                            "modem: ATDT{} → {} (TCP not yet implemented)",
                            number,
                            endpoint
                        );
                        self.send_result(ResultCode::NoDialtone);
                    }
                    None => {
                        log::warn!("modem: ATDT{} — no phonebook entry", number);
                        self.send_result(ResultCode::NoDialtone);
                    }
                }
            }
            AtCommand::HangUp => {
                let was_connected = matches!(self.state, ModemState::Connected);
                self.dcd = false;
                self.state = ModemState::CommandMode;
                if was_connected {
                    self.send_result(ResultCode::NoCarrier);
                } else {
                    self.send_result(ResultCode::Ok);
                }
            }
            AtCommand::OffHook => {
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Answer => {
                self.send_result(ResultCode::Error);
            }
            AtCommand::SRegisterSet { reg, val } => {
                if (reg as usize) < S_REGISTER_COUNT {
                    self.s_registers[reg as usize] = val;
                }
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Info => {
                self.queue_bytes(b"oxide86 Virtual Modem\r\n");
                self.send_result(ResultCode::Ok);
            }
            AtCommand::SRegisterQuery(reg) => {
                let val = if (reg as usize) < S_REGISTER_COUNT {
                    self.s_registers[reg as usize]
                } else {
                    0
                };
                let s = format!("{}\r\n", val);
                self.queue_bytes(s.as_bytes());
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Escape => {
                // In CommandMode already — no-op; phase 3 handles DataMode escape
                self.state = ModemState::CommandMode;
                self.send_result(ResultCode::Ok);
            }
            AtCommand::Unknown(raw) => {
                log::warn!("modem: unrecognised AT command: {}", raw.trim());
                self.send_result(ResultCode::Error);
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
        let byte = self.rx_queue.pop_front();
        self.irq_pending = !self.rx_queue.is_empty();
        byte
    }

    fn write(&mut self, value: u8) -> bool {
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
        // DSR (bit 5) and CTS (bit 4) always high; DCD (bit 7) when connected
        0x30 | if self.dcd { 0x80 } else { 0 }
    }
}
