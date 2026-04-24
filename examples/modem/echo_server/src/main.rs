use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::thread;

fn handle(mut stream: TcpStream) {
    let peer = stream.peer_addr().ok();
    println!("[+] connected: {peer:?}");
    let _ = stream.set_nodelay(true);
    let mut buf = [0u8; 4096];
    loop {
        match stream.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                print!("[>] {} bytes: ", n);
                for &b in &buf[..n] {
                    if b.is_ascii_graphic() || b == b' ' {
                        print!("{}", b as char);
                    } else {
                        print!("\\x{b:02x}");
                    }
                }
                println!();
                if stream.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
        }
    }
    println!("[-] disconnected: {peer:?}");
}

fn main() {
    let addr = "0.0.0.0:2323";
    let listener = TcpListener::bind(addr).expect("bind failed");
    println!("echo server listening on {addr}");
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                thread::spawn(move || handle(s));
            }
            Err(e) => eprintln!("accept error: {e}"),
        }
    }
}
