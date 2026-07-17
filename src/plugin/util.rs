// Shared source helpers: HTTP client, humanized numbers, text bars
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! Shared helpers for sources: HTTP client, humanized numbers, text bars.

use std::time::Duration;

pub fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .cookie_store(true)
        .danger_accept_invalid_certs(true)
        .build()
        .expect("build http client")
}

pub fn human_bytes(n: f64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = n.max(0.0);
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if v >= 100.0 || i == 0 {
        format!("{v:.0} {}", UNITS[i])
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

pub fn human_rate(bytes_per_sec: f64) -> String {
    format!("{}/s", human_bytes(bytes_per_sec))
}

/// "1h 12m" from seconds; "∞" for qBittorrent's forever sentinel.
pub fn human_eta(secs: i64) -> String {
    if secs <= 0 || secs >= 8_640_000 {
        return "∞".into();
    }
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h {m:02}m")
    } else if m > 0 {
        format!("{m}m {s:02}s")
    } else {
        format!("{s}s")
    }
}

/// A little text progress bar: `███░░░░░░░`
pub fn bar(ratio: f64, width: usize) -> String {
    let r = ratio.clamp(0.0, 1.0);
    let filled = (r * width as f64).round() as usize;
    format!(
        "{}{}",
        "█".repeat(filled.min(width)),
        "░".repeat(width - filled.min(width))
    )
}

/// Char-safe truncation with an ellipsis.
pub fn trunc(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

pub fn f64_of(v: &serde_json::Value) -> f64 {
    match v {
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        serde_json::Value::String(s) => s.parse().unwrap_or(0.0),
        _ => 0.0,
    }
}

pub fn str_of(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_humanize_across_units() {
        assert_eq!(human_bytes(512.0), "512 B");
        assert_eq!(human_bytes(2048.0), "2.0 KB");
        assert_eq!(human_bytes(6.216e9), "5.8 GB");
        assert_eq!(human_bytes(0.0), "0 B");
    }

    #[test]
    fn eta_formats_and_caps() {
        assert_eq!(human_eta(45), "45s");
        assert_eq!(human_eta(125), "2m 05s");
        assert_eq!(human_eta(4520), "1h 15m");
        assert_eq!(human_eta(8_640_000), "∞");
        assert_eq!(human_eta(-1), "∞");
    }

    #[test]
    fn bar_clamps_and_fills() {
        assert_eq!(bar(0.0, 4), "░░░░");
        assert_eq!(bar(0.5, 4), "██░░");
        assert_eq!(bar(1.0, 4), "████");
        assert_eq!(bar(7.5, 4), "████"); // over-range clamps
        assert_eq!(bar(-1.0, 4), "░░░░");
    }

    #[test]
    fn trunc_is_char_safe() {
        assert_eq!(trunc("changeover", 20), "changeover");
        assert_eq!(trunc("the space between frames", 10), "the space…");
        // multi-byte chars must not split
        assert_eq!(trunc("🎬🎬🎬🎬", 3), "🎬🎬…");
    }

    #[test]
    fn json_scalars_coerce() {
        use serde_json::json;
        assert_eq!(f64_of(&json!(42)), 42.0);
        assert_eq!(f64_of(&json!("3.5")), 3.5);
        assert_eq!(f64_of(&json!(null)), 0.0);
        assert_eq!(str_of(&json!("hi")), "hi");
        assert_eq!(str_of(&json!(null)), "");
    }
}
