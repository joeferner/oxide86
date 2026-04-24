#[derive(Debug, PartialEq)]
pub enum AtCommand {
    /// AT — no-op
    At,
    /// ATZ / ATZ<n> — software reset (profile number ignored)
    Reset,
    /// AT&F / AT&F<n> — factory defaults
    FactoryReset,
    /// ATE0 / ATE1 — echo off / on
    Echo(bool),
    /// ATV0 / ATV1 — numeric / verbose result codes
    Verbose(bool),
    /// ATQ0 / ATQ1 — result codes on / off
    Quiet(bool),
    /// ATDT / ATDP / ATD <number> — dial
    Dial(String),
    /// ATH / ATH0 — hang up
    HangUp,
    /// ATH1 — go off-hook (no-op until phase 3)
    OffHook,
    /// ATA — answer incoming call
    Answer,
    /// ATS<reg>=<val> — set S-register
    SRegisterSet { reg: u8, val: u8 },
    /// ATI — modem identity string
    Info,
    /// ATS<reg>? — query S-register value
    SRegisterQuery(u8),
    /// AT+++ — escape from data mode back to command mode
    Escape,
    /// Accepted with no effect: &C, &D, &K, &Q, &W, &Y (and variants with digits)
    Ignore,
    /// Anything not recognised; carries the original input for logging
    Unknown(String),
}

/// Parse a complete AT command string (must include the `AT` prefix).
/// Returns a vec of commands — Telix and similar terminal software chains multiple
/// commands in a single `AT...` string (e.g. `AT&C1&D2S7=60`).
pub fn parse(input: &str) -> Vec<AtCommand> {
    let trimmed = input.trim();
    let upper = trimmed.to_ascii_uppercase();

    let rest = match upper.strip_prefix("AT") {
        Some(r) => r,
        None => return vec![AtCommand::Unknown(trimmed.to_owned())],
    };

    if rest.is_empty() {
        return vec![AtCommand::At];
    }

    parse_chain(rest, trimmed)
}

fn consume_optional_digit(chars: &[char], pos: &mut usize) -> Option<u8> {
    if *pos < chars.len() && chars[*pos].is_ascii_digit() {
        let d = chars[*pos].to_digit(10).unwrap() as u8;
        *pos += 1;
        Some(d)
    } else {
        None
    }
}

fn consume_number(chars: &[char], pos: &mut usize) -> Option<u8> {
    let start = *pos;
    while *pos < chars.len() && chars[*pos].is_ascii_digit() {
        *pos += 1;
    }
    if *pos == start {
        return None;
    }
    let s: String = chars[start..*pos].iter().collect();
    s.parse::<u8>().ok()
}

fn parse_chain(rest: &str, original: &str) -> Vec<AtCommand> {
    let chars: Vec<char> = rest.chars().collect();
    let mut pos = 0;
    let mut commands = Vec::new();

    while pos < chars.len() {
        // skip spaces
        while pos < chars.len() && chars[pos] == ' ' {
            pos += 1;
        }
        if pos >= chars.len() {
            break;
        }

        let ch = chars[pos].to_ascii_uppercase();

        match ch {
            '&' => {
                pos += 1;
                if pos >= chars.len() {
                    commands.push(AtCommand::Unknown(original.to_owned()));
                    break;
                }
                let letter = chars[pos].to_ascii_uppercase();
                pos += 1;
                let _n = consume_optional_digit(&chars, &mut pos);
                let cmd = match letter {
                    'C' | 'D' | 'K' | 'Q' | 'W' | 'Y' => AtCommand::Ignore,
                    'F' => AtCommand::FactoryReset,
                    _ => {
                        commands.push(AtCommand::Unknown(original.to_owned()));
                        break;
                    }
                };
                commands.push(cmd);
            }
            'S' => {
                pos += 1;
                let Some(reg) = consume_number(&chars, &mut pos) else {
                    commands.push(AtCommand::Unknown(original.to_owned()));
                    break;
                };
                if pos < chars.len() && chars[pos] == '=' {
                    pos += 1;
                    let Some(val) = consume_number(&chars, &mut pos) else {
                        commands.push(AtCommand::Unknown(original.to_owned()));
                        break;
                    };
                    commands.push(AtCommand::SRegisterSet { reg, val });
                } else if pos < chars.len() && chars[pos] == '?' {
                    pos += 1;
                    commands.push(AtCommand::SRegisterQuery(reg));
                } else {
                    commands.push(AtCommand::Unknown(original.to_owned()));
                    break;
                }
            }
            'D' => {
                pos += 1;
                // skip optional 'T' or 'P' tone/pulse prefix
                if pos < chars.len() && matches!(chars[pos].to_ascii_uppercase(), 'T' | 'P') {
                    pos += 1;
                }
                let number: String = chars[pos..].iter().collect();
                commands.push(AtCommand::Dial(number));
                break; // dial consumes the rest of the string
            }
            '+' => {
                if chars[pos..].starts_with(&['+', '+', '+']) {
                    pos += 3;
                    commands.push(AtCommand::Escape);
                } else {
                    commands.push(AtCommand::Unknown(original.to_owned()));
                    break;
                }
            }
            '?' => {
                pos += 1;
                commands.push(AtCommand::SRegisterQuery(0));
            }
            _ => {
                // single-letter command + optional digit
                pos += 1;
                let n = consume_optional_digit(&chars, &mut pos);
                let cmd = match ch {
                    'Z' => AtCommand::Reset,
                    'E' => AtCommand::Echo(n.unwrap_or(1) != 0),
                    'V' => AtCommand::Verbose(n.unwrap_or(1) != 0),
                    'Q' => AtCommand::Quiet(n.unwrap_or(0) != 0),
                    'H' => {
                        if n == Some(1) {
                            AtCommand::OffHook
                        } else {
                            AtCommand::HangUp
                        }
                    }
                    'A' => AtCommand::Answer,
                    'I' => AtCommand::Info,
                    _ => {
                        commands.push(AtCommand::Unknown(original.to_owned()));
                        break;
                    }
                };
                commands.push(cmd);
            }
        }
    }

    if commands.is_empty() {
        commands.push(AtCommand::At);
    }

    commands
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_no_op() {
        assert_eq!(parse("AT"), vec![AtCommand::At]);
        assert_eq!(parse("at"), vec![AtCommand::At]);
        assert_eq!(parse("  AT  "), vec![AtCommand::At]);
    }

    #[test]
    fn at_reset() {
        assert_eq!(parse("ATZ"), vec![AtCommand::Reset]);
        assert_eq!(parse("atz"), vec![AtCommand::Reset]);
        assert_eq!(parse("ATZ0"), vec![AtCommand::Reset]);
        assert_eq!(parse("ATZ1"), vec![AtCommand::Reset]);
    }

    #[test]
    fn at_factory_reset() {
        assert_eq!(parse("AT&F"), vec![AtCommand::FactoryReset]);
        assert_eq!(parse("AT&F0"), vec![AtCommand::FactoryReset]);
    }

    #[test]
    fn at_echo() {
        assert_eq!(parse("ATE0"), vec![AtCommand::Echo(false)]);
        assert_eq!(parse("ATE1"), vec![AtCommand::Echo(true)]);
    }

    #[test]
    fn at_verbose() {
        assert_eq!(parse("ATV0"), vec![AtCommand::Verbose(false)]);
        assert_eq!(parse("ATV1"), vec![AtCommand::Verbose(true)]);
    }

    #[test]
    fn at_quiet() {
        assert_eq!(parse("ATQ0"), vec![AtCommand::Quiet(false)]);
        assert_eq!(parse("ATQ1"), vec![AtCommand::Quiet(true)]);
    }

    #[test]
    fn at_dial() {
        assert_eq!(parse("ATDT555"), vec![AtCommand::Dial("555".to_owned())]);
        assert_eq!(parse("ATDP555"), vec![AtCommand::Dial("555".to_owned())]);
        assert_eq!(parse("ATD555"), vec![AtCommand::Dial("555".to_owned())]);
        assert_eq!(parse("ATDT0"), vec![AtCommand::Dial("0".to_owned())]);
        assert_eq!(
            parse("ATDT+192.168.1.1:23"),
            vec![AtCommand::Dial("+192.168.1.1:23".to_owned())]
        );
    }

    #[test]
    fn at_hangup() {
        assert_eq!(parse("ATH"), vec![AtCommand::HangUp]);
        assert_eq!(parse("ATH0"), vec![AtCommand::HangUp]);
        assert_eq!(parse("ATH1"), vec![AtCommand::OffHook]);
    }

    #[test]
    fn at_answer() {
        assert_eq!(parse("ATA"), vec![AtCommand::Answer]);
    }

    #[test]
    fn at_s_register() {
        assert_eq!(
            parse("ATS0=2"),
            vec![AtCommand::SRegisterSet { reg: 0, val: 2 }]
        );
        assert_eq!(
            parse("ATS12=50"),
            vec![AtCommand::SRegisterSet { reg: 12, val: 50 }]
        );
    }

    #[test]
    fn at_info() {
        assert_eq!(parse("ATI"), vec![AtCommand::Info]);
    }

    #[test]
    fn at_s_register_query() {
        assert_eq!(parse("ATS0?"), vec![AtCommand::SRegisterQuery(0)]);
        assert_eq!(parse("ATS12?"), vec![AtCommand::SRegisterQuery(12)]);
        assert_eq!(parse("AT?"), vec![AtCommand::SRegisterQuery(0)]);
    }

    #[test]
    fn at_escape() {
        assert_eq!(parse("AT+++"), vec![AtCommand::Escape]);
    }

    #[test]
    fn at_ignore() {
        assert_eq!(parse("AT&C1"), vec![AtCommand::Ignore]);
        assert_eq!(parse("AT&D2"), vec![AtCommand::Ignore]);
        assert_eq!(parse("AT&K3"), vec![AtCommand::Ignore]);
        assert_eq!(parse("AT&W0"), vec![AtCommand::Ignore]);
        assert_eq!(parse("AT&Y0"), vec![AtCommand::Ignore]);
        assert_eq!(parse("AT&Q5"), vec![AtCommand::Ignore]);
    }

    #[test]
    fn at_unknown() {
        assert!(matches!(parse("ATXYZ").as_slice(), [AtCommand::Unknown(_)]));
        assert!(matches!(
            parse("NOTANCOMMAND").as_slice(),
            [AtCommand::Unknown(_)]
        ));
    }

    #[test]
    fn at_compound() {
        // Telix init string
        assert_eq!(
            parse("AT&C1&D2&K3S7=60S11=55"),
            vec![
                AtCommand::Ignore,
                AtCommand::Ignore,
                AtCommand::Ignore,
                AtCommand::SRegisterSet { reg: 7, val: 60 },
                AtCommand::SRegisterSet { reg: 11, val: 55 },
            ]
        );
        // write NVRAM / set default profile
        assert_eq!(parse("AT&W0"), vec![AtCommand::Ignore]);
        assert_eq!(parse("AT&Y0"), vec![AtCommand::Ignore]);
        // reset to stored profile
        assert_eq!(parse("ATZ0"), vec![AtCommand::Reset]);
        // async mode + S-register, with space separator
        assert_eq!(
            parse("AT&Q5 S36=7"),
            vec![
                AtCommand::Ignore,
                AtCommand::SRegisterSet { reg: 36, val: 7 },
            ]
        );
    }
}
