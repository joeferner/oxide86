use std::sync::{Arc, RwLock};

use crate::{
    devices::modem::SerialModem,
    tests::{create_computer, run_test},
};

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
            let modem = Arc::new(RwLock::new(SerialModem::with_phonebook(Some(
                phonebook.clone(),
            ))));
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
            let modem = Arc::new(RwLock::new(SerialModem::with_phonebook(Some(
                phonebook.clone(),
            ))));
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
            let modem = Arc::new(RwLock::new(SerialModem::with_phonebook(Some(
                phonebook.clone(),
            ))));
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
            let modem = Arc::new(RwLock::new(SerialModem::with_phonebook(Some(
                phonebook.clone(),
            ))));
            computer.set_com_port_device(1, Some(modem));
            computer.run();
        },
    );
}
