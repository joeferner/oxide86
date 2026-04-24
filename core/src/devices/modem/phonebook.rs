use std::collections::HashMap;

use anyhow::{Context, Result};

/// Maps short dial strings (e.g. `"555"`) to `"host:port"` endpoints.
///
/// Resolution priority for a dial number:
/// 1. Exact phonebook lookup.
/// 2. `+host:port` prefix — strip the `+` and use the rest literally.
/// 3. `a.b.c.d/port` slash notation — convert `/` to `:`.
/// 4. `None` — no match.
#[derive(Clone, Default)]
pub struct ModemPhonebook {
    entries: HashMap<String, String>,
}

impl ModemPhonebook {
    pub fn from_file(path: &std::path::Path) -> Result<Self> {
        let json = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read phonebook: {}", path.display()))?;
        Self::from_json(&json)
    }

    pub fn from_json(json: &str) -> Result<Self> {
        let entries: HashMap<String, String> = serde_json::from_str(json)
            .context("phonebook must be a JSON object mapping dial strings to host:port")?;
        Ok(Self { entries })
    }

    /// Look up a dial string in the phonebook. Returns the raw `host:port`
    /// string if found, `None` otherwise.
    pub fn lookup(&self, number: &str) -> Option<&str> {
        self.entries.get(number).map(String::as_str)
    }

    /// Resolve a dial number to a `host:port` string.
    ///
    /// Falls back to literal-address syntax when the number is not in the
    /// phonebook:
    /// - `+host:port`       → strip the `+` prefix
    /// - `a.b.c.d/port`    → replace `/` with `:`
    pub fn resolve(&self, number: &str) -> Option<String> {
        if let Some(addr) = self.lookup(number) {
            return Some(addr.to_owned());
        }
        if let Some(addr) = number.strip_prefix('+') {
            return Some(addr.to_owned());
        }
        if let Some(slash) = number.find('/') {
            let (host, port) = number.split_at(slash);
            return Some(format!("{}:{}", host, &port[1..]));
        }
        None
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

impl TryFrom<&str> for ModemPhonebook {
    type Error = anyhow::Error;
    fn try_from(json: &str) -> Result<Self> {
        Self::from_json(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn book() -> ModemPhonebook {
        ModemPhonebook::from_json(r#"{"0":"127.0.0.1:2323","555":"bbs.example.com:23"}"#).unwrap()
    }

    #[test]
    fn lookup_hit() {
        let b = book();
        assert_eq!(b.lookup("0"), Some("127.0.0.1:2323"));
        assert_eq!(b.lookup("555"), Some("bbs.example.com:23"));
    }

    #[test]
    fn lookup_miss() {
        assert_eq!(book().lookup("999"), None);
    }

    #[test]
    fn resolve_phonebook() {
        assert_eq!(book().resolve("0"), Some("127.0.0.1:2323".to_owned()));
        assert_eq!(book().resolve("555"), Some("bbs.example.com:23".to_owned()));
    }

    #[test]
    fn resolve_plus_prefix() {
        assert_eq!(
            book().resolve("+192.168.1.1:23"),
            Some("192.168.1.1:23".to_owned())
        );
    }

    #[test]
    fn resolve_slash_notation() {
        assert_eq!(
            book().resolve("192.168.1.1/513"),
            Some("192.168.1.1:513".to_owned())
        );
    }

    #[test]
    fn resolve_miss() {
        assert_eq!(book().resolve("999"), None);
    }

    #[test]
    fn invalid_json() {
        assert!(ModemPhonebook::from_json("not json").is_err());
        assert!(ModemPhonebook::from_json(r#"["not","an","object"]"#).is_err());
    }

    #[test]
    fn empty_phonebook() {
        let b = ModemPhonebook::from_json("{}").unwrap();
        assert!(b.is_empty());
        assert_eq!(b.resolve("+host:23"), Some("host:23".to_owned()));
    }
}
