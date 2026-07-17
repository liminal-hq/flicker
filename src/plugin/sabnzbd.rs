// SABnzbd source: the other usenet truck — queue, rates, per-slot controls
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::config::SourceCfg;

use super::util::{f64_of, pct_bar, str_of, trunc};
use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct Sabnzbd {
    base: String,
    key: String,
    client: reqwest::Client,
}

impl Sabnzbd {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            key: cfg.api_key()?,
            client: super::util::client(),
        })
    }

    async fn api(&self, args: &str) -> Result<Value> {
        let url = format!("{}/api?output=json&apikey={}&{args}", self.base, self.key);
        // The key rides in the URL, so scrub it from any error we surface.
        let v: Value = async {
            anyhow::Ok(
                self.client
                    .get(&url)
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?,
            )
        }
        .await
        .map_err(|e| super::util::redact(e, &self.key))?;
        if v["status"] == false {
            anyhow::bail!("sabnzbd: {}", str_of(&v["error"]));
        }
        Ok(v)
    }
}

#[async_trait]
impl Source for Sabnzbd {
    async fn poll(&mut self) -> Result<Panel> {
        let v = self.api("mode=queue").await?;
        let q = &v["queue"];
        let kbps = f64_of(&q["kbpersec"]);
        let paused = q["paused"] == true;

        let rows = q["slots"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .take(12)
            .map(|s| {
                let status = str_of(&s["status"]);
                let tone = match status.as_str() {
                    "Downloading" => Tone::Good,
                    "Paused" => Tone::Warn,
                    "Queued" => Tone::Muted,
                    _ => Tone::Info,
                };
                let prog = f64_of(&s["percentage"]) / 100.0;
                RowItem {
                    key: str_of(&s["nzo_id"]),
                    cells: vec![
                        cell(trunc(&str_of(&s["filename"]), 42), Tone::Default),
                        cell(pct_bar(prog, 8), Tone::Info),
                        cell(status.to_lowercase(), tone),
                        cell(str_of(&s["timeleft"]), Tone::Muted),
                    ],
                    actions: vec![
                        action("slot_pause", "pause this download", false),
                        action("slot_resume", "resume this download", false),
                        action("slot_delete", "delete from queue", true),
                    ],
                }
            })
            .collect::<Vec<_>>();

        let badge = if paused {
            format!("⏸ paused · {} left", str_of(&q["sizeleft"]))
        } else {
            format!(
                "{:.1} MB/s · {} left",
                kbps / 1024.0,
                str_of(&q["sizeleft"])
            )
        };
        Ok(Panel {
            badge: Some(badge),
            footer: Some(format!(
                "{} free on download disk",
                str_of(&q["diskspace1_norm"])
            )),
            rows,
            panel_actions: vec![
                action("pause_all", "pause the whole queue", false),
                action("resume_all", "resume the queue", false),
            ],
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String> {
        match action_id {
            "pause_all" => {
                self.api("mode=pause").await?;
                Ok("queue paused".into())
            }
            "resume_all" => {
                self.api("mode=resume").await?;
                Ok("queue resumed".into())
            }
            "slot_pause" | "slot_resume" | "slot_delete" => {
                let name = match action_id {
                    "slot_pause" => "pause",
                    "slot_resume" => "resume",
                    _ => "delete",
                };
                self.api(&format!("mode=queue&name={name}&value={row_key}"))
                    .await?;
                Ok(format!("download {name}d"))
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}
