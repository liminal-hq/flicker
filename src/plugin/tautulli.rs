// Tautulli source: active Plex streams, bandwidth sparkline, terminate action
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! Tautulli: who is watching what, right now.

use std::collections::VecDeque;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde_json::Value;

use crate::config::SourceCfg;

use super::util::{bar, f64_of, str_of, trunc};
use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct Tautulli {
    base: String,
    key: String,
    client: reqwest::Client,
    /// Rolling total-bandwidth history (kbps) for the sparkline.
    hist: VecDeque<u64>,
}

impl Tautulli {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            key: cfg.api_key()?,
            client: super::util::client(),
            hist: VecDeque::new(),
        })
    }

    async fn cmd(&self, cmd: &str, extra: &str) -> Result<Value> {
        let url = format!("{}/api/v2?apikey={}&cmd={cmd}{extra}", self.base, self.key);
        let v: Value = self.client.get(&url).send().await?.json().await?;
        anyhow::ensure!(
            v["response"]["result"] == "success",
            "tautulli: {}",
            str_of(&v["response"]["message"])
        );
        Ok(v["response"]["data"].clone())
    }
}

#[async_trait]
impl Source for Tautulli {
    async fn poll(&mut self) -> Result<Panel> {
        let data = self.cmd("get_activity", "").await?;
        let sessions = data["sessions"].as_array().cloned().unwrap_or_default();
        let kbps = f64_of(&data["total_bandwidth"]);
        self.hist.push_back(kbps as u64);
        while self.hist.len() > 120 {
            self.hist.pop_front();
        }

        let rows = sessions
            .iter()
            .map(|s| {
                let state = str_of(&s["state"]);
                let (icon, itone) = match state.as_str() {
                    "playing" => ("▶", Tone::Good),
                    "paused" => ("⏸", Tone::Warn),
                    _ => ("◌", Tone::Info),
                };
                let title = if s["media_type"] == "episode" {
                    format!(
                        "{} · S{:02}E{:02} {}",
                        str_of(&s["grandparent_title"]),
                        f64_of(&s["parent_media_index"]) as u32,
                        f64_of(&s["media_index"]) as u32,
                        str_of(&s["title"]),
                    )
                } else {
                    format!("{} ({})", str_of(&s["title"]), str_of(&s["year"]))
                };
                let prog = f64_of(&s["progress_percent"]) / 100.0;
                let decision = str_of(&s["transcode_decision"]);
                let dtone = if decision == "transcode" {
                    Tone::Warn
                } else {
                    Tone::Good
                };
                let mbps = f64_of(&s["bandwidth"]) / 1000.0;
                RowItem {
                    key: str_of(&s["session_key"]),
                    cells: vec![
                        cell(icon, itone),
                        cell(str_of(&s["user"]), Tone::Accent2),
                        cell(trunc(&title, 46), Tone::Default),
                        cell(
                            format!("{} {:.0}%", bar(prog, 10), prog * 100.0),
                            Tone::Info,
                        ),
                        cell(decision, dtone),
                        cell(format!("{mbps:.1} Mbps"), Tone::Muted),
                        cell(trunc(&str_of(&s["player"]), 18), Tone::Muted),
                    ],
                    actions: vec![action("terminate", "terminate this stream", true)],
                }
            })
            .collect::<Vec<_>>();

        let n = rows.len();
        Ok(Panel {
            badge: Some(format!("{n} on air · {:.1} Mbps", kbps / 1000.0)),
            spark: Some(("bandwidth".into(), self.hist.iter().copied().collect())),
            footer: if n == 0 {
                Some("the house is dark — nobody is watching".into())
            } else {
                None
            },
            rows,
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String> {
        match action_id {
            "terminate" => {
                self.cmd(
                    "terminate_session",
                    &format!(
                        "&session_key={row_key}&message=The projectionist needs this reel back. Sorry!"
                    ),
                )
                .await
                .context("terminate_session")?;
                Ok(format!("stream {row_key} terminated"))
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}
