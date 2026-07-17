// The *arr family source: sonarr/radarr/lidarr queues and prowlarr indexers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! The *arr family: sonarr, radarr, lidarr (download queues) and prowlarr
//! (indexer inventory). One source, four hats.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};

use crate::config::SourceCfg;

use super::util::{f64_of, human_bytes, pct_bar, str_of, trunc};
use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct Arr {
    kind: String,
    base: String,
    key: String,
    client: reqwest::Client,
}

impl Arr {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            kind: cfg.kind.clone(),
            base: cfg.url()?,
            key: cfg.api_key()?,
            client: super::util::client(),
        })
    }

    fn api(&self) -> &'static str {
        match self.kind.as_str() {
            "sonarr" | "radarr" => "v3",
            _ => "v1", // lidarr, prowlarr
        }
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{}/api/{}/{path}", self.base, self.api());
        let resp = self
            .client
            .get(&url)
            .header("X-Api-Key", &self.key)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn post_command(&self, name: &str) -> Result<()> {
        let url = format!("{}/api/{}/command", self.base, self.api());
        self.client
            .post(&url)
            .header("X-Api-Key", &self.key)
            .json(&json!({ "name": name }))
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }

    async fn health_count(&self) -> usize {
        match self.get("health").await {
            Ok(Value::Array(a)) => a.len(),
            _ => 0,
        }
    }

    /// Best display title for a queue record, per kind.
    fn record_title(&self, r: &Value) -> String {
        match self.kind.as_str() {
            "sonarr" => {
                let series = str_of(&r["series"]["title"]);
                if series.is_empty() {
                    str_of(&r["title"])
                } else {
                    format!(
                        "{series} S{:02}E{:02}",
                        f64_of(&r["seasonNumber"]) as u32,
                        f64_of(&r["episode"]["episodeNumber"]) as u32,
                    )
                }
            }
            "radarr" => {
                let movie = str_of(&r["movie"]["title"]);
                if movie.is_empty() {
                    str_of(&r["title"])
                } else {
                    format!("{movie} ({})", f64_of(&r["movie"]["year"]) as u32)
                }
            }
            "lidarr" => {
                let artist = str_of(&r["artist"]["artistName"]);
                let album = str_of(&r["album"]["title"]);
                if artist.is_empty() {
                    str_of(&r["title"])
                } else {
                    format!("{artist} — {album}")
                }
            }
            _ => str_of(&r["title"]),
        }
    }

    async fn poll_queue(&self) -> Result<Panel> {
        let include = match self.kind.as_str() {
            "sonarr" => "&includeSeries=true&includeEpisode=true",
            "radarr" => "&includeMovie=true",
            "lidarr" => "&includeArtist=true&includeAlbum=true",
            _ => "",
        };
        let queue_path = format!("queue?pageSize=12{include}");
        let (q, health) = tokio::join!(self.get(&queue_path), self.health_count());
        let q = q?;
        let total = f64_of(&q["totalRecords"]) as usize;

        let rows = q["records"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .map(|r| {
                let size = f64_of(&r["size"]);
                let left = f64_of(&r["sizeleft"]);
                let prog = if size > 0.0 { 1.0 - left / size } else { 0.0 };
                let status = str_of(&r["status"]);
                let tracked = str_of(&r["trackedDownloadState"]);
                let (stxt, stone) = if tracked == "importPending" || tracked == "importBlocked" {
                    ("import".to_string(), Tone::Warn)
                } else {
                    let t = match status.as_str() {
                        "downloading" => Tone::Good,
                        "paused" => Tone::Warn,
                        "warning" | "failed" => Tone::Bad,
                        _ => Tone::Muted,
                    };
                    (status.clone(), t)
                };
                let timeleft = str_of(&r["timeleft"]);
                RowItem {
                    key: f64_of(&r["id"]).to_string(),
                    cells: vec![
                        cell(trunc(&self.record_title(r), 38), Tone::Default),
                        cell(pct_bar(prog, 8), Tone::Info),
                        cell(stxt, stone),
                        cell(
                            if timeleft.is_empty() {
                                human_bytes(left)
                            } else {
                                timeleft
                            },
                            Tone::Muted,
                        ),
                    ],
                    actions: vec![action("remove", "remove from queue (and client)", true)],
                }
            })
            .collect::<Vec<_>>();

        let mut badge = format!("{total} queued");
        if health > 0 {
            badge.push_str(&format!(" · ⚠ {health}"));
        }
        Ok(Panel {
            badge: Some(badge),
            footer: if rows.is_empty() {
                Some(match self.kind.as_str() {
                    "sonarr" => "no episodes in flight".into(),
                    "radarr" => "no movies in flight".into(),
                    _ => "queue empty".into(),
                })
            } else if total > rows.len() {
                Some(format!("showing {} of {total}", rows.len()))
            } else {
                None
            },
            rows,
            panel_actions: vec![action("rss", "trigger RSS sync", false)],
            ..Default::default()
        })
    }

    async fn poll_prowlarr(&self) -> Result<Panel> {
        let (idx, health) = tokio::join!(self.get("indexer"), self.health_count());
        let idx = idx?;
        let list = idx.as_array().cloned().unwrap_or_default();
        let enabled = list.iter().filter(|i| i["enable"] == true).count();
        let rows = list
            .iter()
            .map(|i| {
                let on = i["enable"] == true;
                RowItem {
                    key: f64_of(&i["id"]).to_string(),
                    cells: vec![
                        cell(
                            if on { "●" } else { "○" },
                            if on { Tone::Good } else { Tone::Muted },
                        ),
                        cell(trunc(&str_of(&i["name"]), 30), Tone::Default),
                        cell(str_of(&i["protocol"]), Tone::Muted),
                    ],
                    actions: vec![],
                }
            })
            .collect::<Vec<_>>();
        let mut badge = format!("{enabled}/{} indexers", list.len());
        if health > 0 {
            badge.push_str(&format!(" · ⚠ {health}"));
        }
        Ok(Panel {
            badge: Some(badge),
            rows,
            panel_actions: vec![action("testall", "test all indexers", false)],
            ..Default::default()
        })
    }
}

#[async_trait]
impl Source for Arr {
    async fn poll(&mut self) -> Result<Panel> {
        if self.kind == "prowlarr" {
            self.poll_prowlarr().await
        } else {
            self.poll_queue().await
        }
    }

    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String> {
        match action_id {
            "remove" => {
                let url = format!(
                    "{}/api/{}/queue/{row_key}?removeFromClient=true&blocklist=false",
                    self.base,
                    self.api()
                );
                self.client
                    .delete(&url)
                    .header("X-Api-Key", &self.key)
                    .send()
                    .await?
                    .error_for_status()?;
                Ok("removed from queue".into())
            }
            "rss" => {
                self.post_command("RssSync").await?;
                Ok("RSS sync started".into())
            }
            "testall" => {
                let url = format!("{}/api/v1/indexer/testall", self.base);
                self.client
                    .post(&url)
                    .header("X-Api-Key", &self.key)
                    .send()
                    .await?
                    .error_for_status()?;
                Ok("testing all indexers".into())
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}
