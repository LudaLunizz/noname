//! Total tokens used through the local FreeLLMAPI pool, for the middle of the footer.
//!
//! Off by default (`free-tokens` in the config folder: `on` / `off`). The number comes from the
//! `free-usage --json` command, which reads the pool's request log read-only; noname never opens
//! the log itself and sends nothing over the network.

use std::process::{Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use crate::sys;

/// Seconds between two reads of the log while the option is on.
pub const EVERY: Duration = Duration::from_secs(30);

pub fn enabled() -> bool {
    std::fs::read_to_string(sys::cfgfile("free-tokens")).map(|s| s.trim() == "on").unwrap_or(false)
}

pub fn set_enabled(on: bool) {
    let f = sys::cfgfile("free-tokens");
    if let Some(d) = std::path::Path::new(&f).parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&f, if on { "on" } else { "off" });
}

/// All-time tokens from the JSON that `free-usage --json` prints.
pub fn parse(json: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    v.get("periods")?.get("all time")?.get("tokens_total")?.as_u64()
}

/// `2.87M`, `481.7k`, `950`.
pub fn short(n: u64) -> String {
    match n {
        0..=999 => n.to_string(),
        1_000..=999_999 => format!("{:.1}k", n as f64 / 1e3),
        1_000_000..=999_999_999 => format!("{:.2}M", n as f64 / 1e6),
        _ => format!("{:.2}B", n as f64 / 1e9),
    }
}

fn read() -> Option<u64> {
    let local = format!("{}/.local/bin/free-usage", sys::home());
    let cmd = if std::path::Path::new(&local).exists() { local } else { "free-usage".to_string() };
    let out = Command::new(cmd).arg("--json").stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse(&String::from_utf8_lossy(&out.stdout))
}

/// One read on a background thread; `None` on the channel means the command is missing or failed.
pub fn fetch() -> Receiver<Option<u64>> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let _ = tx.send(read());
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_time_total() {
        let j = r#"{"now":"x","periods":{"today":{"tokens_total":5},"all time":{"tokens_total":2870620}}}"#;
        assert_eq!(parse(j), Some(2_870_620));
    }

    #[test]
    fn rejects_other_shapes() {
        assert_eq!(parse("not json"), None);
        assert_eq!(parse(r#"{"periods":{}}"#), None);
        assert_eq!(parse(r#"{"periods":{"all time":{"tokens_total":"x"}}}"#), None);
    }

    #[test]
    fn shortens_numbers() {
        assert_eq!(short(950), "950");
        assert_eq!(short(481_673), "481.7k");
        assert_eq!(short(2_870_620), "2.87M");
        assert_eq!(short(3_100_000_000), "3.10B");
    }
}
