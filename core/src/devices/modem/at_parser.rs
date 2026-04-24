#[derive(Debug, PartialEq)]
pub enum AtCommand {
    /// AT — no-op
    At,
    /// ATZ — software reset to defaults
    Reset,
    /// AT&F — factory defaults
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
    /// Anything not recognised; carries the original input for logging
    Unknown(String),
}

/// Parse a complete AT command string (must include the `AT` prefix).
/// Input is trimmed but not case-folded by the caller — the parser handles
/// case-insensitivity internally.
pub fn parse(input: &str) -> AtCommand {
    let upper = input.trim().to_ascii_uppercase();

    let rest = match upper.strip_prefix("AT") {
        Some(r) => r,
        None => return AtCommand::Unknown(input.to_owned()),
    };

    match rest {
        "" => AtCommand::At,
        "Z" => AtCommand::Reset,
        "&F" => AtCommand::FactoryReset,
        "E0" => AtCommand::Echo(false),
        "E1" => AtCommand::Echo(true),
        "V0" => AtCommand::Verbose(false),
        "V1" => AtCommand::Verbose(true),
        "Q0" => AtCommand::Quiet(false),
        "Q1" => AtCommand::Quiet(true),
        "H" | "H0" => AtCommand::HangUp,
        "H1" => AtCommand::OffHook,
        "A" => AtCommand::Answer,
        "I" => AtCommand::Info,
        "+++" => AtCommand::Escape,
        "?" => AtCommand::SRegisterQuery(0),
        _ => parse_extended(rest, input),
    }
}

fn parse_extended(rest: &str, original: &str) -> AtCommand {
    // ATDT<num>, ATDP<num>, ATD<num>
    if let Some(num) = rest
        .strip_prefix("DT")
        .or_else(|| rest.strip_prefix("DP"))
        .or_else(|| rest.strip_prefix('D'))
    {
        return AtCommand::Dial(num.to_owned());
    }

    // ATS<reg>? and ATS<reg>=<val>
    if let Some(s_rest) = rest.strip_prefix('S') {
        if let Some(reg_str) = s_rest.strip_suffix('?')
            && let Ok(reg) = reg_str.parse::<u8>()
        {
            return AtCommand::SRegisterQuery(reg);
        }
        if let Some((reg_str, val_str)) = s_rest.split_once('=')
            && let (Ok(reg), Ok(val)) = (reg_str.parse::<u8>(), val_str.parse::<u8>())
        {
            return AtCommand::SRegisterSet { reg, val };
        }
    }

    AtCommand::Unknown(original.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_no_op() {
        assert_eq!(parse("AT"), AtCommand::At);
        assert_eq!(parse("at"), AtCommand::At);
        assert_eq!(parse("  AT  "), AtCommand::At);
    }

    #[test]
    fn at_reset() {
        assert_eq!(parse("ATZ"), AtCommand::Reset);
        assert_eq!(parse("atz"), AtCommand::Reset);
    }

    #[test]
    fn at_factory_reset() {
        assert_eq!(parse("AT&F"), AtCommand::FactoryReset);
    }

    #[test]
    fn at_echo() {
        assert_eq!(parse("ATE0"), AtCommand::Echo(false));
        assert_eq!(parse("ATE1"), AtCommand::Echo(true));
    }

    #[test]
    fn at_verbose() {
        assert_eq!(parse("ATV0"), AtCommand::Verbose(false));
        assert_eq!(parse("ATV1"), AtCommand::Verbose(true));
    }

    #[test]
    fn at_quiet() {
        assert_eq!(parse("ATQ0"), AtCommand::Quiet(false));
        assert_eq!(parse("ATQ1"), AtCommand::Quiet(true));
    }

    #[test]
    fn at_dial() {
        assert_eq!(parse("ATDT555"), AtCommand::Dial("555".to_owned()));
        assert_eq!(parse("ATDP555"), AtCommand::Dial("555".to_owned()));
        assert_eq!(parse("ATD555"), AtCommand::Dial("555".to_owned()));
        assert_eq!(parse("ATDT0"), AtCommand::Dial("0".to_owned()));
        assert_eq!(
            parse("ATDT+192.168.1.1:23"),
            AtCommand::Dial("+192.168.1.1:23".to_owned())
        );
    }

    #[test]
    fn at_hangup() {
        assert_eq!(parse("ATH"), AtCommand::HangUp);
        assert_eq!(parse("ATH0"), AtCommand::HangUp);
        assert_eq!(parse("ATH1"), AtCommand::OffHook);
    }

    #[test]
    fn at_answer() {
        assert_eq!(parse("ATA"), AtCommand::Answer);
    }

    #[test]
    fn at_s_register() {
        assert_eq!(parse("ATS0=2"), AtCommand::SRegisterSet { reg: 0, val: 2 });
        assert_eq!(
            parse("ATS12=50"),
            AtCommand::SRegisterSet { reg: 12, val: 50 }
        );
    }

    #[test]
    fn at_info() {
        assert_eq!(parse("ATI"), AtCommand::Info);
    }

    #[test]
    fn at_s_register_query() {
        assert_eq!(parse("ATS0?"), AtCommand::SRegisterQuery(0));
        assert_eq!(parse("ATS12?"), AtCommand::SRegisterQuery(12));
        assert_eq!(parse("AT?"), AtCommand::SRegisterQuery(0));
    }

    #[test]
    fn at_escape() {
        assert_eq!(parse("AT+++"), AtCommand::Escape);
    }

    #[test]
    fn at_unknown() {
        assert!(matches!(parse("ATXYZ"), AtCommand::Unknown(_)));
        assert!(matches!(parse("NOTANCOMMAND"), AtCommand::Unknown(_)));
    }
}
