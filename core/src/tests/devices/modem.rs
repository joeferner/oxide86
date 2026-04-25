use std::sync::{Arc, RwLock};

use crate::{
    devices::modem::SerialModem,
    tests::{create_computer, run_test},
};

#[cfg(not(target_arch = "wasm32"))]
mod mock_dialer {
    use std::collections::VecDeque;
    use std::io::{Read as _, Write as _};
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use crate::devices::modem::transport::{ModemDialer, ModemTransport, TransportEvent};

    pub struct MockTransport {
        writer: Arc<Mutex<std::net::TcpStream>>,
        incoming: Arc<Mutex<VecDeque<u8>>>,
        disconnected: Arc<AtomicBool>,
        event_delivered: bool,
        disconnect_event_delivered: bool,
    }

    impl ModemTransport for MockTransport {
        fn poll_incoming(&mut self, out: &mut VecDeque<u8>) {
            if let Ok(mut q) = self.incoming.try_lock() {
                out.extend(q.drain(..));
            }
        }
        fn send_byte(&mut self, byte: u8) {
            if let Ok(mut s) = self.writer.lock() {
                let _ = s.write_all(&[byte]);
            }
        }
        fn take_event(&mut self) -> Option<TransportEvent> {
            if !self.event_delivered {
                self.event_delivered = true;
                return Some(TransportEvent::Connected);
            }
            if !self.disconnect_event_delivered && self.disconnected.load(Ordering::Acquire) {
                self.disconnect_event_delivered = true;
                return Some(TransportEvent::RemoteDisconnected);
            }
            None
        }
        fn start_escape_guard(&mut self, _guard_ms: u64) {}
        fn cancel_escape_guard(&mut self) {}
        fn escape_time_elapsed(&self) -> bool {
            false
        }
    }

    pub struct MockDialer {
        pub addr: String,
    }

    impl ModemDialer for MockDialer {
        fn dial(&self, _addr: &str) -> Box<dyn ModemTransport> {
            let stream = std::net::TcpStream::connect(&self.addr).expect("MockDialer connect");
            let _ = stream.set_nodelay(true);
            let writer = Arc::new(Mutex::new(stream.try_clone().unwrap()));
            let incoming = Arc::new(Mutex::new(VecDeque::new()));
            let disconnected = Arc::new(AtomicBool::new(false));
            let incoming_t = incoming.clone();
            let disconnected_t = disconnected.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 256];
                let mut s = stream;
                loop {
                    match s.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => incoming_t.lock().unwrap().extend(&buf[..n]),
                    }
                }
                disconnected_t.store(true, Ordering::Release);
            });
            Box::new(MockTransport {
                writer,
                incoming,
                disconnected,
                event_delivered: false,
                disconnect_event_delivered: false,
            })
        }
    }
}

#[test_log::test]
pub(crate) fn at_basic() {
    run_test(
        "devices/modem/at_basic",
        create_computer(),
        |computer, _video_buffer| {
            let modem = Arc::new(RwLock::new(SerialModem::new()));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn at_dial_reject() {
    run_test(
        "devices/modem/at_dial_reject",
        create_computer(),
        |computer, _video_buffer| {
            let modem = Arc::new(RwLock::new(SerialModem::new()));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}

#[test_log::test]
pub(crate) fn at_hangup() {
    run_test(
        "devices/modem/at_hangup",
        create_computer(),
        |computer, _video_buffer| {
            let modem = Arc::new(RwLock::new(SerialModem::new()));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test_log::test]
pub(crate) fn tcp_echo() {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    use crate::devices::modem::phonebook::ModemPhonebook;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_nodelay(true);
            let mut buf = [0u8; 64];
            loop {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if stream.write_all(&buf[..n]).is_err() {
                            break;
                        }
                    }
                }
            }
        }
    });

    let phonebook = ModemPhonebook::from_json(&format!(r#"{{"0":"127.0.0.1:{}"}}"#, port)).unwrap();

    run_test(
        "devices/modem/tcp_echo",
        create_computer(),
        |computer, _video_buffer| {
            let dialer = mock_dialer::MockDialer {
                addr: format!("127.0.0.1:{}", port),
            };
            let modem = Arc::new(RwLock::new(SerialModem::with_phonebook_and_dialer(
                Some(phonebook.clone()),
                Some(Box::new(dialer)),
            )));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test_log::test]
pub(crate) fn tcp_disconnect() {
    use std::io::{Read as _, Write as _};
    use std::net::TcpListener;

    use crate::devices::modem::phonebook::ModemPhonebook;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    // Echo server: read exactly 3 bytes, echo them back, then close
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_nodelay(true);
            let mut buf = [0u8; 3];
            if stream.read_exact(&mut buf).is_ok() {
                let _ = stream.write_all(&buf);
            }
            // Drop stream → TCP close → modem gets NO CARRIER
        }
    });

    let phonebook = ModemPhonebook::from_json(&format!(r#"{{"0":"127.0.0.1:{}"}}"#, port)).unwrap();

    run_test(
        "devices/modem/tcp_disconnect",
        create_computer(),
        |computer, _video_buffer| {
            let dialer = mock_dialer::MockDialer {
                addr: format!("127.0.0.1:{}", port),
            };
            let modem = Arc::new(RwLock::new(SerialModem::with_phonebook_and_dialer(
                Some(phonebook.clone()),
                Some(Box::new(dialer)),
            )));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test_log::test]
pub(crate) fn modem_msr() {
    use std::io::Read as _;
    use std::net::TcpListener;

    use crate::devices::modem::phonebook::ModemPhonebook;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    // Hold the connection alive until the modem drops it at test end
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_nodelay(true);
            let mut buf = [0u8; 64];
            while stream.read(&mut buf).is_ok_and(|n| n > 0) {}
        }
    });

    let phonebook = ModemPhonebook::from_json(&format!(r#"{{"0":"127.0.0.1:{}"}}"#, port)).unwrap();

    run_test(
        "devices/modem/modem_msr",
        create_computer(),
        |computer, _video_buffer| {
            let dialer = mock_dialer::MockDialer {
                addr: format!("127.0.0.1:{}", port),
            };
            let modem = Arc::new(RwLock::new(SerialModem::with_phonebook_and_dialer(
                Some(phonebook.clone()),
                Some(Box::new(dialer)),
            )));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test_log::test]
pub(crate) fn tcp_escape() {
    use std::io::Read as _;
    use std::net::TcpListener;

    use crate::devices::modem::phonebook::ModemPhonebook;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    // Hold the connection alive until EOF (modem drops it at test end)
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let _ = stream.set_nodelay(true);
            let mut buf = [0u8; 64];
            while stream.read(&mut buf).is_ok_and(|n| n > 0) {}
        }
    });

    let phonebook = ModemPhonebook::from_json(&format!(r#"{{"0":"127.0.0.1:{}"}}"#, port)).unwrap();

    run_test(
        "devices/modem/tcp_escape",
        create_computer(),
        |computer, _video_buffer| {
            let dialer = mock_dialer::MockDialer {
                addr: format!("127.0.0.1:{}", port),
            };
            let modem = Arc::new(RwLock::new(SerialModem::with_phonebook_and_dialer(
                Some(phonebook.clone()),
                Some(Box::new(dialer)),
            )));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}
