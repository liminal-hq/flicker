// Glances source: CPU/memory/swap gauges and filesystems from the Glances v4 REST API
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::config::SourceCfg;

use super::util::{f64_of, human_bytes, str_of};
use super::{GaugeItem, Panel, Source};

pub struct Glances {
    base: String,
    client: reqwest::Client,
}

impl Glances {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            client: super::util::client(),
        })
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let resp = self
            .client
            .get(format!("{}/api/4/{path}", self.base))
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }
}

const SKIP_MOUNTS: &[&str] = &["/boot", "/snap", "/run", "/dev", "/var/lib/docker"];

#[async_trait]
impl Source for Glances {
    async fn poll(&mut self) -> Result<Panel> {
        let quick = self.get("quicklook").await?;
        let fs = self.get("fs").await.unwrap_or(Value::Array(vec![]));
        let uptime = self
            .get("uptime")
            .await
            .map(|v| str_of(&v))
            .unwrap_or_default();

        let mut gauges = vec![
            GaugeItem {
                label: "cpu".into(),
                ratio: f64_of(&quick["cpu"]) / 100.0,
                note: format!("{:.0}%", f64_of(&quick["cpu"])),
            },
            GaugeItem {
                label: "mem".into(),
                ratio: f64_of(&quick["mem"]) / 100.0,
                note: format!("{:.0}%", f64_of(&quick["mem"])),
            },
        ];
        let swap = f64_of(&quick["swap"]);
        if swap > 0.5 {
            gauges.push(GaugeItem {
                label: "swap".into(),
                ratio: swap / 100.0,
                note: format!("{swap:.0}%"),
            });
        }
        let mut disks: Vec<&Value> = fs
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|d| {
                        let m = str_of(&d["mnt_point"]);
                        !SKIP_MOUNTS.iter().any(|s| m.starts_with(s))
                    })
                    .collect()
            })
            .unwrap_or_default();
        disks.sort_by(|a, b| f64_of(&b["size"]).total_cmp(&f64_of(&a["size"])));
        disks.dedup_by_key(|d| str_of(&d["mnt_point"]));
        for d in disks.into_iter().take(5) {
            let pct = f64_of(&d["percent"]);
            gauges.push(GaugeItem {
                label: str_of(&d["mnt_point"]),
                ratio: pct / 100.0,
                note: format!(
                    "{} / {}",
                    human_bytes(f64_of(&d["used"])),
                    human_bytes(f64_of(&d["size"]))
                ),
            });
        }

        let load = f64_of(&quick["load"]);
        Ok(Panel {
            badge: (load > 0.0).then(|| format!("load {load:.0}%")),
            gauges,
            footer: (!uptime.is_empty()).then(|| format!("up {uptime}")),
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, _row_key: &str) -> Result<String> {
        anyhow::bail!("glances has no actions (tried {action_id})")
    }
}
