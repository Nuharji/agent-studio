//! The usage limits of the Claude plan (the 5-hour window, the week …), as Claude Code's `/usage` shows them.
//! Read with Claude Code's own sign-in (macOS: the keychain entry "Claude Code-credentials", else
//! ~/.claude/.credentials.json) from Anthropic's usage endpoint; the token is only passed on to `curl` (via stdin,
//! not on the command line) and never stored. The endpoint is not documented — what it doesn't send is left out.

use serde::Serialize;
use serde_json::Value;
use std::io::Write;
use std::process::Stdio;

const URL: &str = "https://api.anthropic.com/api/oauth/usage";

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Limit {
    /// "five_hour", "seven_day", "seven_day_opus" …
    pub key: String,
    /// Used, in percent (0–100).
    pub percent: f64,
    /// When the window starts again (ISO time), if sent.
    pub resets_at: String,
}

#[derive(Serialize, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub limits: Vec<Limit>,
    /// "" when fine, else "noLogin" (no sign-in found, e.g. an API key), "expired" or the error text.
    pub error: String,
}

/// Reads the endpoint's answer: every top-level object with a "utilization" is a limit (five_hour first).
pub fn parse(j: &Value) -> Vec<Limit> {
    let mut out: Vec<Limit> = j.as_object().into_iter().flatten().filter_map(|(k, v)| {
        let percent = v.get("utilization")?.as_f64()?;
        let resets_at = v.get("resets_at").and_then(Value::as_str).unwrap_or("").to_string();
        Some(Limit { key: k.clone(), percent: percent.clamp(0.0, 100.0), resets_at })
    }).collect();
    let rank = |k: &str| ["five_hour", "seven_day", "seven_day_opus", "seven_day_sonnet"].iter().position(|x| *x == k).unwrap_or(9);
    out.sort_by(|a, b| rank(&a.key).cmp(&rank(&b.key)).then(a.key.cmp(&b.key)));
    out
}

/// The OAuth token and when it runs out (ms), from Claude Code's stored sign-in.
fn token() -> Option<(String, f64)> {
    let mut raw = String::new();
    #[cfg(target_os = "macos")]
    if let Ok(o) = crate::runner::command("security").args(["find-generic-password", "-s", "Claude Code-credentials", "-w"]).output() {
        if o.status.success() { raw = String::from_utf8_lossy(&o.stdout).trim().to_string(); }
    }
    if raw.is_empty() {
        let dir = std::env::var_os("CLAUDE_CONFIG_DIR").map(std::path::PathBuf::from).unwrap_or_else(|| crate::runner::home().join(".claude"));
        raw = std::fs::read_to_string(dir.join(".credentials.json")).ok()?;
    }
    let j: Value = serde_json::from_str(&raw).ok()?;
    let o = j.get("claudeAiOauth")?;
    let t = o.get("accessToken")?.as_str()?.to_string();
    Some((t, o.get("expiresAt").and_then(Value::as_f64).unwrap_or(0.0)))
}

pub fn fetch() -> Usage {
    let err = |e: &str| Usage { limits: vec![], error: e.to_string() };
    let Some((tok, exp)) = token() else { return err("noLogin") };
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as f64).unwrap_or(0.0);
    if exp > 0.0 && exp < now { return err("expired"); }
    if tok.contains(['"', '\n', '\r', '\\']) { return err("noLogin"); }
    let child = crate::runner::command("curl")
        .args(["-sS", "--max-time", "15", "-K", "-", URL])
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let Ok(mut child) = child else { return err("curl") };
    if let Some(mut i) = child.stdin.take() {
        let _ = write!(i, "header = \"Authorization: Bearer {tok}\"\nheader = \"anthropic-beta: oauth-2025-04-20\"\nheader = \"Accept: application/json\"\n");
    }
    let Ok(o) = child.wait_with_output() else { return err("curl") };
    if !o.status.success() { return err(String::from_utf8_lossy(&o.stderr).trim()); }
    let Ok(j) = serde_json::from_slice::<Value>(&o.stdout) else { return err("answer") };
    if let Some(e) = j.get("error") {
        let t = e.get("type").and_then(Value::as_str).unwrap_or("");
        return err(if t.contains("authentication") || t.contains("permission") { "expired" } else { e.get("message").and_then(Value::as_str).unwrap_or("answer") });
    }
    let limits = parse(&j);
    if limits.is_empty() { return err("answer"); }
    Usage { limits, error: String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_limits() {
        let j: Value = serde_json::from_str(r#"{"seven_day":{"utilization":41.0,"resets_at":"2026-11-09T08:00:00Z"},"five_hour":{"utilization":12.5,"resets_at":"2026-11-05T15:00:00Z"},
            "seven_day_opus":null,"seven_day_oauth_apps":null,"extra_usage":{"is_enabled":false},"seven_day_sonnet":{"utilization":130,"resets_at":null}}"#).unwrap();
        let l = parse(&j);
        assert_eq!(l.iter().map(|x| x.key.as_str()).collect::<Vec<_>>(), ["five_hour", "seven_day", "seven_day_sonnet"]);
        assert_eq!(l[0], Limit { key: "five_hour".into(), percent: 12.5, resets_at: "2026-11-05T15:00:00Z".into() });
        assert_eq!(l[2].percent, 100.0);
        assert_eq!(l[2].resets_at, "");
        assert!(parse(&serde_json::json!({"error": {"type": "x"}})).is_empty());
    }
}
