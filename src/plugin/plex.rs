// Plex source: sessions straight from the horse's mouth, no Tautulli required
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::config::SourceCfg;

use super::util::{bar, f64_of, str_of, trunc};
use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct Plex {
    base: String,
    token: String,
    client: reqwest::Client,
}

impl Plex {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            token: cfg.api_key()?,
            client: super::util::client(),
        })
    }

    async fn get_raw(&self, path: &str, extra: &str) -> Result<reqwest::Response> {
        let url = format!("{}{path}?X-Plex-Token={}{extra}", self.base, self.token);
        Ok(self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await?
            .error_for_status()?)
    }

    async fn get(&self, path: &str, extra: &str) -> Result<Value> {
        Ok(self.get_raw(path, extra).await?.json().await?)
    }
}

#[async_trait]
impl Source for Plex {
    async fn poll(&mut self) -> Result<Panel> {
        let v = self.get("/status/sessions", "").await?;
        let sessions = v["MediaContainer"]["Metadata"]
            .as_array()
            .cloned()
            .unwrap_or_default();

        let rows = sessions
            .iter()
            .map(|s| {
                let state = str_of(&s["Player"]["state"]);
                let (icon, itone) = match state.as_str() {
                    "playing" => ("▶", Tone::Good),
                    "paused" => ("⏸", Tone::Warn),
                    _ => ("◌", Tone::Info),
                };
                let title = if s["type"] == "episode" {
                    format!(
                        "{} · S{:02}E{:02} {}",
                        str_of(&s["grandparentTitle"]),
                        f64_of(&s["parentIndex"]) as u32,
                        f64_of(&s["index"]) as u32,
                        str_of(&s["title"]),
                    )
                } else {
                    format!("{} ({})", str_of(&s["title"]), f64_of(&s["year"]) as u32)
                };
                let duration = f64_of(&s["duration"]).max(1.0);
                let prog = f64_of(&s["viewOffset"]) / duration;
                let transcoding = !s["TranscodeSession"].is_null();
                RowItem {
                    key: str_of(&s["Session"]["id"]),
                    cells: vec![
                        cell(icon, itone),
                        cell(str_of(&s["User"]["title"]), Tone::Accent2),
                        cell(trunc(&title, 46), Tone::Default),
                        cell(
                            format!("{} {:.0}%", bar(prog, 10), prog * 100.0),
                            Tone::Info,
                        ),
                        cell(
                            if transcoding {
                                "transcode"
                            } else {
                                "direct play"
                            },
                            if transcoding { Tone::Warn } else { Tone::Good },
                        ),
                        cell(trunc(&str_of(&s["Player"]["product"]), 18), Tone::Muted),
                    ],
                    actions: vec![action("terminate", "terminate this session", true)],
                }
            })
            .collect::<Vec<_>>();

        let n = rows.len();
        Ok(Panel {
            badge: Some(format!("{n} session{}", if n == 1 { "" } else { "s" })),
            footer: (n == 0).then(|| "the lamp is warm, the house is empty".into()),
            rows,
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String> {
        match action_id {
            "terminate" => {
                anyhow::ensure!(!row_key.is_empty(), "session has no id");
                // Success is an empty non-JSON body, so only check the status.
                self.get_raw(
                    "/status/sessions/terminate",
                    &format!("&sessionId={row_key}&reason=The projectionist needs this reel back."),
                )
                .await?;
                Ok("session terminated".into())
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}
