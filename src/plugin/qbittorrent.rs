// qBittorrent source: cookie login, transfer rates, torrent list and controls
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! qBittorrent: the torrent freight bay. Cookie login, v4/v5 compatible
//! (tries the v5 `stop`/`start` endpoints and falls back to `pause`/`resume`).

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::config::SourceCfg;

use super::util::{bar, f64_of, human_eta, human_rate, str_of, trunc};
use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct Qbit {
    base: String,
    user: String,
    pass: String,
    client: reqwest::Client,
    logged_in: bool,
}

impl Qbit {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            user: cfg.username.clone().unwrap_or_else(|| "admin".into()),
            pass: cfg.password.clone().unwrap_or_default(),
            client: super::util::client(),
            logged_in: false,
        })
    }

    async fn login(&mut self) -> Result<()> {
        let resp = self
            .client
            .post(format!("{}/api/v2/auth/login", self.base))
            .form(&[
                ("username", self.user.as_str()),
                ("password", self.pass.as_str()),
            ])
            .send()
            .await?;
        // Success is a 2xx (some builds send 200 + "Ok.", others 204 + empty
        // body) — but a wrong password can also be 200 + "Fails.".
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::ensure!(
            status.is_success() && !body.contains("Fails"),
            "qbittorrent login failed ({status}): {}",
            if body.is_empty() {
                "no body"
            } else {
                body.trim()
            }
        );
        self.logged_in = true;
        Ok(())
    }

    async fn get(&mut self, path: &str) -> Result<reqwest::Response> {
        if !self.logged_in {
            self.login().await?;
        }
        let url = format!("{}/api/v2/{path}", self.base);
        let resp = self.client.get(&url).send().await?;
        if resp.status() == reqwest::StatusCode::FORBIDDEN {
            self.login().await?;
            return Ok(self.client.get(&url).send().await?.error_for_status()?);
        }
        Ok(resp.error_for_status()?)
    }

    /// POST a form to `first`, falling back to `second` on 404 (v5 vs v4 API).
    async fn post_either(
        &mut self,
        first: &str,
        second: &str,
        form: &[(&str, &str)],
    ) -> Result<()> {
        if !self.logged_in {
            self.login().await?;
        }
        for (i, path) in [first, second].iter().enumerate() {
            let resp = self
                .client
                .post(format!("{}/api/v2/{path}", self.base))
                .form(form)
                .send()
                .await?;
            if resp.status() == reqwest::StatusCode::NOT_FOUND && i == 0 {
                continue;
            }
            resp.error_for_status()?;
            return Ok(());
        }
        anyhow::bail!("both endpoints missing: {first}, {second}")
    }
}

fn state_glyph(state: &str) -> (&'static str, Tone, u8) {
    // Third field is a sort rank: lower shows first.
    match state {
        "downloading" | "forcedDL" | "metaDL" => ("▼", Tone::Good, 0),
        "stalledDL" | "queuedDL" | "allocating" | "checkingDL" => ("…", Tone::Muted, 2),
        "uploading" | "forcedUP" => ("▲", Tone::Info, 1),
        "stalledUP" | "queuedUP" | "checkingUP" => ("✓", Tone::Muted, 4),
        "pausedDL" | "stoppedDL" => ("⏸", Tone::Warn, 3),
        "pausedUP" | "stoppedUP" => ("✓", Tone::Muted, 5),
        "error" | "missingFiles" => ("✗", Tone::Bad, 0),
        _ => ("·", Tone::Muted, 6),
    }
}

#[async_trait]
impl Source for Qbit {
    async fn poll(&mut self) -> Result<Panel> {
        let transfer: Value = self.get("transfer/info").await?.json().await?;
        let mut torrents: Vec<Value> = self.get("torrents/info?limit=500").await?.json().await?;
        // speedLimitsMode returns "0"/"1" as plain text.
        let alt = match self.get("transfer/speedLimitsMode").await {
            Ok(r) => r.text().await.unwrap_or_default().trim() == "1",
            Err(_) => false,
        };

        let total = torrents.len();
        torrents.sort_by_key(|t| {
            let (_, _, rank) = state_glyph(&str_of(&t["state"]));
            // Active transfers first within the same rank.
            let speed = (f64_of(&t["dlspeed"]) + f64_of(&t["upspeed"])) as i64;
            (rank, -speed)
        });

        let rows = torrents
            .iter()
            .take(14)
            .map(|t| {
                let state = str_of(&t["state"]);
                let (icon, tone, _) = state_glyph(&state);
                let prog = f64_of(&t["progress"]);
                let dl = f64_of(&t["dlspeed"]);
                let mut cells = vec![
                    cell(icon, tone),
                    cell(trunc(&str_of(&t["name"]), 42), Tone::Default),
                    cell(format!("{} {:.0}%", bar(prog, 8), prog * 100.0), Tone::Info),
                ];
                if dl > 0.0 {
                    cells.push(cell(human_rate(dl), Tone::Good));
                    cells.push(cell(human_eta(f64_of(&t["eta"]) as i64), Tone::Muted));
                } else {
                    cells.push(cell(state, Tone::Muted));
                }
                RowItem {
                    key: str_of(&t["hash"]),
                    cells,
                    actions: vec![
                        action("pause", "pause torrent", false),
                        action("resume", "resume torrent", false),
                        action("delete", "delete torrent (keep files)", true),
                    ],
                }
            })
            .collect();

        Ok(Panel {
            badge: Some(format!(
                "▼ {} ▲ {} · {total} torrents",
                human_rate(f64_of(&transfer["dl_info_speed"])),
                human_rate(f64_of(&transfer["up_info_speed"])),
            )),
            rows,
            footer: alt.then(|| "⚠ alternative speed limits are ON".into()),
            panel_actions: vec![
                action("alt", "toggle alternative speed limits", false),
                action("pause_all", "pause ALL torrents", true),
                action("resume_all", "resume all torrents", false),
            ],
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String> {
        match action_id {
            "pause" => {
                self.post_either("torrents/stop", "torrents/pause", &[("hashes", row_key)])
                    .await?;
                Ok("torrent paused".into())
            }
            "resume" => {
                self.post_either("torrents/start", "torrents/resume", &[("hashes", row_key)])
                    .await?;
                Ok("torrent resumed".into())
            }
            "delete" => {
                self.post_either(
                    "torrents/delete",
                    "torrents/delete",
                    &[("hashes", row_key), ("deleteFiles", "false")],
                )
                .await?;
                Ok("torrent deleted (files kept)".into())
            }
            "pause_all" => {
                self.post_either("torrents/stop", "torrents/pause", &[("hashes", "all")])
                    .await?;
                Ok("all torrents paused".into())
            }
            "resume_all" => {
                self.post_either("torrents/start", "torrents/resume", &[("hashes", "all")])
                    .await?;
                Ok("all torrents resumed".into())
            }
            "alt" => {
                self.post_either(
                    "transfer/toggleSpeedLimitsMode",
                    "transfer/toggleSpeedLimitsMode",
                    &[],
                )
                .await?;
                Ok("alternative speed limits toggled".into())
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}
