// Uptime Kuma source: monitor states scraped from the /metrics endpoint
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! Kuma v1 has no REST API, but it does export Prometheus text metrics.
//! `monitor_status` is 1 up / 0 down / 2 pending / 3 maintenance;
//! `monitor_response_time` fills in the note. Auth (if enabled) is HTTP basic
//! with an API key as the password — set `api_key` in the source config.

use std::collections::BTreeMap;

use anyhow::Result;
use async_trait::async_trait;

use crate::config::SourceCfg;

use super::util::trunc;
use super::{cell, Panel, RowItem, Source, Tone};

pub struct UptimeKuma {
    base: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl UptimeKuma {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            api_key: cfg.api_key.clone(),
            client: super::util::client(),
        })
    }
}

#[derive(Debug, Default, Clone)]
pub struct Monitor {
    pub status: i64,
    pub response_ms: Option<f64>,
}

/// Pull `monitor_name="…"` out of a Prometheus label set.
fn label_name(labels: &str) -> Option<String> {
    let start = labels.find("monitor_name=\"")? + "monitor_name=\"".len();
    let end = labels[start..].find('"')? + start;
    Some(labels[start..end].to_string())
}

/// Parse Kuma's metrics text into name → monitor state.
pub fn parse_metrics(text: &str) -> BTreeMap<String, Monitor> {
    let mut out: BTreeMap<String, Monitor> = BTreeMap::new();
    for line in text.lines() {
        let Some((head, value)) = line.rsplit_once(' ') else {
            continue;
        };
        let (metric, labels) = match head.split_once('{') {
            Some((m, l)) => (m, l.trim_end_matches('}')),
            None => continue,
        };
        let Some(name) = label_name(labels) else {
            continue;
        };
        match metric {
            "monitor_status" => {
                out.entry(name).or_default().status = value.parse().unwrap_or(0);
            }
            "monitor_response_time" => {
                out.entry(name).or_default().response_ms = value.parse().ok();
            }
            _ => {}
        }
    }
    out
}

#[async_trait]
impl Source for UptimeKuma {
    async fn poll(&mut self) -> Result<Panel> {
        let mut req = self.client.get(format!("{}/metrics", self.base));
        if let Some(key) = &self.api_key {
            req = req.basic_auth("", Some(key));
        }
        let text = req.send().await?.error_for_status()?.text().await?;
        let monitors = parse_metrics(&text);

        let total = monitors.len();
        let up = monitors.values().filter(|m| m.status == 1).count();

        let mut entries: Vec<(&String, &Monitor)> = monitors.iter().collect();
        // Trouble floats to the top: down, pending, maintenance, then up.
        entries.sort_by_key(|(_, m)| match m.status {
            0 => 0,
            2 => 1,
            3 => 2,
            _ => 3,
        });

        let rows = entries
            .iter()
            .take(14)
            .map(|(name, m)| {
                let (icon, label, tone) = match m.status {
                    1 => ("●", "up", Tone::Good),
                    0 => ("○", "down", Tone::Bad),
                    2 => ("◌", "pending", Tone::Warn),
                    _ => ("◑", "maintenance", Tone::Info),
                };
                let note = m
                    .response_ms
                    .map(|ms| format!("{ms:.0} ms"))
                    .unwrap_or_default();
                RowItem {
                    key: String::new(),
                    cells: vec![
                        cell(icon, tone),
                        cell(trunc(name, 32), Tone::Default),
                        cell(label, tone),
                        cell(note, Tone::Muted),
                    ],
                    actions: vec![],
                }
            })
            .collect();

        Ok(Panel {
            badge: Some(format!("{up}/{total} up")),
            rows,
            footer: (total > 14).then(|| format!("showing 14 of {total}")),
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, _row_key: &str) -> Result<String> {
        anyhow::bail!("uptime-kuma has no actions (tried {action_id})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"# HELP monitor_status Monitor Status (1 = UP, 0 = DOWN, 2 = PENDING, 3 = MAINTENANCE)
monitor_status{monitor_name="plex",monitor_type="http",monitor_url="http://x"} 1
monitor_status{monitor_name="sonarr",monitor_type="http",monitor_url="http://y"} 0
monitor_status{monitor_name="modem",monitor_type="ping",monitor_hostname="1.1"} 2
monitor_response_time{monitor_name="plex",monitor_type="http",monitor_url="http://x"} 42
"#;

    #[test]
    fn parses_status_and_response_time() {
        let m = parse_metrics(SAMPLE);
        assert_eq!(m.len(), 3);
        assert_eq!(m["plex"].status, 1);
        assert_eq!(m["plex"].response_ms, Some(42.0));
        assert_eq!(m["sonarr"].status, 0);
        assert_eq!(m["modem"].status, 2);
        assert_eq!(m["sonarr"].response_ms, None);
    }

    #[test]
    fn ignores_lines_without_monitor_name() {
        let m = parse_metrics("# comment\nprocess_cpu_seconds_total 1.5\nup 1\n");
        assert!(m.is_empty());
    }
}
