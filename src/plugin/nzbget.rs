// NZBGet source: usenet freight — rates, remaining, queue groups
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::SourceCfg;

use super::util::{f64_of, human_bytes, human_rate, pct_bar, str_of, trunc};
use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct Nzbget {
    base: String,
    user: String,
    pass: String,
    client: reqwest::Client,
}

impl Nzbget {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            user: cfg.username.clone().unwrap_or_else(|| "nzbget".into()),
            pass: cfg.password.clone().unwrap_or_default(),
            client: super::util::client(),
        })
    }

    async fn rpc(&self, method: &str, params: Value) -> Result<Value> {
        let v: Value = self
            .client
            .post(format!("{}/jsonrpc", self.base))
            .basic_auth(&self.user, Some(&self.pass))
            .json(&json!({ "method": method, "params": params, "id": 1 }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if !v["error"].is_null() {
            anyhow::bail!("nzbget rpc {method}: {}", str_of(&v["error"]["message"]));
        }
        Ok(v["result"].clone())
    }
}

#[async_trait]
impl Source for Nzbget {
    async fn poll(&mut self) -> Result<Panel> {
        let status = self.rpc("status", json!([])).await?;
        let groups = self.rpc("listgroups", json!([0])).await?;

        let rate = f64_of(&status["DownloadRate"]);
        let remaining_mb = f64_of(&status["RemainingSizeMB"]);
        let paused = status["DownloadPaused"] == true;

        let rows = groups
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .take(12)
            .map(|g| {
                let size = f64_of(&g["FileSizeMB"]);
                let left = f64_of(&g["RemainingSizeMB"]);
                let prog = if size > 0.0 { 1.0 - left / size } else { 0.0 };
                let gstatus = str_of(&g["Status"]);
                let tone = match gstatus.as_str() {
                    "DOWNLOADING" => Tone::Good,
                    "PAUSED" => Tone::Warn,
                    s if s.starts_with("PP_") => Tone::Info, // post-processing
                    _ => Tone::Muted,
                };
                RowItem {
                    key: f64_of(&g["NZBID"]).to_string(),
                    cells: vec![
                        cell(trunc(&str_of(&g["NZBName"]), 44), Tone::Default),
                        cell(pct_bar(prog, 8), Tone::Info),
                        cell(gstatus.to_lowercase(), tone),
                        cell(human_bytes(left * 1024.0 * 1024.0), Tone::Muted),
                    ],
                    actions: vec![
                        action("group_pause", "pause this download", false),
                        action("group_resume", "resume this download", false),
                    ],
                }
            })
            .collect::<Vec<_>>();

        let badge = if paused {
            format!(
                "⏸ paused · {} left",
                human_bytes(remaining_mb * 1024.0 * 1024.0)
            )
        } else {
            format!(
                "{} · {} left",
                human_rate(rate),
                human_bytes(remaining_mb * 1024.0 * 1024.0)
            )
        };
        Ok(Panel {
            badge: Some(badge),
            footer: Some(format!(
                "{} this month",
                human_bytes(f64_of(&status["MonthSizeMB"]) * 1024.0 * 1024.0)
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
                self.rpc("pausedownload", json!([])).await?;
                Ok("queue paused".into())
            }
            "resume_all" => {
                self.rpc("resumedownload", json!([])).await?;
                Ok("queue resumed".into())
            }
            "group_pause" | "group_resume" => {
                let cmd = if action_id == "group_pause" {
                    "GroupPause"
                } else {
                    "GroupResume"
                };
                let id: i64 = row_key.parse().unwrap_or(0);
                self.rpc("editqueue", json!([cmd, "", [id]])).await?;
                Ok(if action_id == "group_pause" {
                    "download paused".into()
                } else {
                    "download resumed".into()
                })
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}
