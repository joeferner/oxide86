/// Standard I/O implementation of Bios for native platform
use emu86_core::Bios;
use std::io::{self, Read, Write};

pub struct StdioBios;

impl Bios for StdioBios {
    fn read_char(&mut self) -> Option<u8> {
        let mut buffer = [0u8; 1];
        match io::stdin().read_exact(&mut buffer) {
            Ok(_) => Some(buffer[0]),
            Err(_) => None,
        }
    }

    fn write_char(&mut self, ch: u8) {
        print!("{}", ch as char);
        let _ = io::stdout().flush();
    }

    fn write_str(&mut self, s: &str) {
        print!("{}", s);
        let _ = io::stdout().flush();
    }
}
