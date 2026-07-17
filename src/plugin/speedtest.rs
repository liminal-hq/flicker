// Speedtest-tracker source: how fast is the pipe, and can we kick a new run
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::config::SourceCfg;

use super::util::{f64_of, str_of, trunc};
use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct Speedtest {
    base: String,
    client: reqwest::Client,
}

impl Speedtest {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            client: super::util::client(),
        })
    }
}

fn result_row(label: &str, v: &Value, tone: Tone) -> RowItem {
    RowItem {
        key: String::new(),
        cells: vec![
            cell(format!("{label:<7}"), Tone::Muted),
            cell(format!("▼ {:.1} Mbps", f64_of(&v["download"])), tone),
            cell(format!("▲ {:.1} Mbps", f64_of(&v["upload"])), Tone::Info),
            cell(format!("{:.0} ms", f64_of(&v["ping"])), Tone::Muted),
        ],
        actions: vec![],
    }
}

#[async_trait]
impl Source for Speedtest {
    async fn poll(&mut self) -> Result<Panel> {
        let v: Value = self
            .client
            .get(format!("{}/api/speedtest/latest", self.base))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let data = &v["data"];
        let failed = f64_of(&data["failed"]) > 0.0;

        let mut rows = vec![result_row(
            "latest",
            data,
            if failed { Tone::Bad } else { Tone::Good },
        )];
        if v["average"].is_object() {
            rows.push(result_row("average", &v["average"], Tone::Default));
        }
        let when = str_of(&data["created_at"])
            .chars()
            .take(10)
            .collect::<String>();
        let server = trunc(&str_of(&data["server_name"]), 30);

        Ok(Panel {
            badge: Some(format!(
                "▼ {:.0} ▲ {:.0} Mbps",
                f64_of(&data["download"]),
                f64_of(&data["upload"])
            )),
            rows,
            footer: Some(format!("last run {when} · {server}")),
            panel_actions: vec![action("run", "run a speedtest now", false)],
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, _row_key: &str) -> Result<String> {
        match action_id {
            "run" => {
                self.client
                    .get(format!("{}/api/speedtest/run", self.base))
                    .send()
                    .await?
                    .error_for_status()?;
                Ok("speedtest started — results in a minute or two".into())
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}
