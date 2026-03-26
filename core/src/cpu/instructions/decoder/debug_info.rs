use super::{Operand, Reg16};

/// Extract a port number from an IN/OUT port operand and look up its name.
/// Handles both immediate ports (`imm8`) and DX-indirect ports (`Reg16(DX, value)`).
pub fn port_comment(port_op: Option<&Operand>) -> Option<String> {
    let port = match port_op? {
        Operand::Imm8(p) => *p as u16,
        Operand::Reg16(Reg16::DX, v) => *v,
        _ => return None,
    };
    io_port_name(port).map(|s| s.to_string())
}

pub fn io_port_name(port: u16) -> Option<&'static str> {
    match port {
        // PIC
        0x0020 => Some("PIC1 Command"),
        0x0021 => Some("PIC1 Mask"),
        0x00A0 => Some("PIC2 Command"),
        0x00A1 => Some("PIC2 Mask"),
        // PIT
        0x0040 => Some("PIT Channel 0"),
        0x0041 => Some("PIT Channel 1"),
        0x0042 => Some("PIT Channel 2"),
        0x0043 => Some("PIT Control"),
        // System Control Port B
        0x0061 => Some("System Control Port B"),
        // Keyboard Controller
        0x0060 => Some("KBC Data"),
        0x0064 => Some("KBC Status/Command"),
        // RTC
        0x0070 => Some("RTC Register Select"),
        0x0071 => Some("RTC Data"),
        // CGA/VGA CRTC
        0x03B4 => Some("MDA CRTC Address"),
        0x03B5 => Some("MDA CRTC Data"),
        0x03C0 => Some("VGA AC Address/Data"),
        0x03C1 => Some("VGA AC Data Read"),
        0x03C7 => Some("VGA DAC Read Index"),
        0x03C8 => Some("VGA DAC Write Index"),
        0x03C9 => Some("VGA DAC Data"),
        0x03D4 => Some("CGA CRTC Address"),
        0x03D5 => Some("CGA CRTC Data"),
        0x03D9 => Some("CGA Color Select"),
        0x03DA => Some("CGA Status"),
        // FDC
        0x03F2 => Some("FDC DOR"),
        0x03F4 => Some("FDC Status"),
        0x03F5 => Some("FDC Data"),
        0x03F7 => Some("FDC DIR"),
        // HDC
        0x01F0 => Some("HDC Data"),
        0x01F1 => Some("HDC Error/Features"),
        0x01F2 => Some("HDC Sector Count"),
        0x01F3 => Some("HDC Sector Number"),
        0x01F4 => Some("HDC Cylinder Low"),
        0x01F5 => Some("HDC Cylinder High"),
        0x01F6 => Some("HDC Drive/Head"),
        0x01F7 => Some("HDC Command/Status"),
        0x03F6 => Some("HDC Device Control"),
        // UART (COM ports)
        0x03F8 => Some("COM1 Data"),
        0x03F9 => Some("COM1 IER/DLM"),
        0x03FA => Some("COM1 IIR/FCR"),
        0x03FB => Some("COM1 LCR"),
        0x03FC => Some("COM1 MCR"),
        0x03FD => Some("COM1 LSR"),
        0x03FE => Some("COM1 MSR"),
        0x02F8 => Some("COM2 Data"),
        0x02F9 => Some("COM2 IER/DLM"),
        0x02FA => Some("COM2 IIR/FCR"),
        0x02FB => Some("COM2 LCR"),
        0x02FC => Some("COM2 MCR"),
        0x02FD => Some("COM2 LSR"),
        0x02FE => Some("COM2 MSR"),
        0x03E8 => Some("COM3 Data"),
        0x02E8 => Some("COM4 Data"),
        _ => None,
    }
}

pub fn int_description(int_num: u8, ah: u8) -> Option<(&'static str, bool)> {
    match int_num {
        0x08 => Some(("timer interrupt", false)),
        0x09 => Some(("keyboard interrupt", false)),
        0x10 => {
            let desc = match ah {
                0x00 => "set video mode",
                0x01 => "set cursor shape",
                0x02 => "set cursor position",
                0x03 => "get cursor position",
                0x05 => "select active page",
                0x06 => "scroll up",
                0x07 => "scroll down",
                0x08 => "read char/attr",
                0x09 => "write char/attr",
                0x0A => "write char",
                0x0B => "set color palette",
                0x0E => "teletype output",
                0x0F => "get video mode",
                0x10 => "palette registers",
                0x11 => "character generator",
                0x12 => "alternate function select",
                0x15 => "return physical display params",
                0x1A => "display combination code",
                _ => return None,
            };
            Some((desc, true))
        }
        0x11 => Some(("get equipment list", false)),
        0x12 => Some(("get memory size", false)),
        0x13 => {
            let desc = match ah {
                0x00 => "reset disk",
                0x01 => "get disk status",
                0x02 => "read sectors",
                0x03 => "write sectors",
                0x04 => "verify sectors",
                0x08 => "get drive params",
                0x15 => "get disk type",
                0x16 => "detect disk change",
                0x18 => "set DASD type",
                _ => return None,
            };
            Some((desc, true))
        }
        0x14 => {
            let desc = match ah {
                0x00 => "initialize serial port",
                0x01 => "write char",
                0x02 => "read char",
                0x03 => "get status",
                _ => return None,
            };
            Some((desc, true))
        }
        0x15 => {
            let desc = match ah {
                0x10 => "TopView multi-DOS",
                0x41 => "wait external event",
                0x4F => "keyboard intercept",
                0x88 => "get extended memory",
                0x91 => "device interrupt complete",
                0xC0 => "get system config",
                0xC1 => "get EBDA segment",
                0xC2 => "PS/2 mouse services",
                _ => return None,
            };
            Some((desc, true))
        }
        0x16 => {
            let desc = match ah {
                0x00 => "read char",
                0x01 => "check keystroke",
                0x02 => "get shift flags",
                0x55 => "word TSR check",
                0x92 => "get keyboard capabilities",
                0xA2 => "122-key capability check",
                _ => return None,
            };
            Some((desc, true))
        }
        0x17 => {
            let desc = match ah {
                0x01 => "initialize printer",
                _ => return None,
            };
            Some((desc, true))
        }
        0x1A => {
            let desc = match ah {
                0x00 => "get system time",
                0x01 => "set system time",
                0x02 => "read RTC time",
                0x03 => "set RTC time",
                0x04 => "read RTC date",
                0x05 => "set RTC date",
                0x06 => "set RTC alarm",
                0x07 => "cancel RTC alarm",
                _ => return None,
            };
            Some((desc, true))
        }
        0x21 => {
            let desc = match ah {
                0x02 => "write char",
                0x09 => "write string",
                0x4C => "exit",
                _ => return None,
            };
            Some((desc, true))
        }
        0x4A => Some(("user alarm interrupt", false)),
        0x74 => Some(("PS/2 mouse interrupt", false)),
        _ => None,
    }
}
