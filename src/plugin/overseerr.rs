// Overseerr source: pending media requests awaiting a thumbs up or down
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::config::SourceCfg;

use super::util::{f64_of, str_of, trunc};
use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct Overseerr {
    base: String,
    key: String,
    client: reqwest::Client,
    /// tmdbId → title cache so we don't re-look-up every poll.
    titles: HashMap<i64, String>,
}

impl Overseerr {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            base: cfg.url()?,
            key: cfg.api_key()?,
            client: super::util::client(),
            titles: HashMap::new(),
        })
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let resp = self
            .client
            .get(format!("{}/api/v1/{path}", self.base))
            .header("X-Api-Key", &self.key)
            .send()
            .await?
            .error_for_status()?;
        Ok(resp.json().await?)
    }

    async fn title_for(&mut self, media_type: &str, tmdb: i64) -> String {
        if let Some(t) = self.titles.get(&tmdb) {
            return t.clone();
        }
        let path = if media_type == "movie" {
            format!("movie/{tmdb}")
        } else {
            format!("tv/{tmdb}")
        };
        let title = match self.get(&path).await {
            Ok(v) => {
                let t = str_of(&v["title"]);
                if t.is_empty() {
                    str_of(&v["name"])
                } else {
                    t
                }
            }
            Err(_) => format!("tmdb:{tmdb}"),
        };
        self.titles.insert(tmdb, title.clone());
        title
    }
}

#[async_trait]
impl Source for Overseerr {
    async fn poll(&mut self) -> Result<Panel> {
        let v = self.get("request?take=8&filter=pending&sort=added").await?;
        let pending = f64_of(&v["pageInfo"]["results"]) as usize;
        let results = v["results"].as_array().cloned().unwrap_or_default();

        let mut rows = Vec::new();
        for r in &results {
            let media_type = str_of(&r["media"]["mediaType"]);
            let tmdb = f64_of(&r["media"]["tmdbId"]) as i64;
            let title = self.title_for(&media_type, tmdb).await;
            let icon = if media_type == "movie" {
                "🎬"
            } else {
                "📺"
            };
            let who = str_of(&r["requestedBy"]["displayName"]);
            let date = str_of(&r["createdAt"]).chars().take(10).collect::<String>();
            rows.push(RowItem {
                key: f64_of(&r["id"]).to_string(),
                cells: vec![
                    cell(icon, Tone::Default),
                    cell(trunc(&title, 36), Tone::Default),
                    cell(format!("by {who}"), Tone::Accent2),
                    cell(date, Tone::Muted),
                ],
                actions: vec![
                    action("approve", "approve request", false),
                    action("decline", "decline request", true),
                ],
            });
        }

        Ok(Panel {
            badge: Some(format!("{pending} pending")),
            footer: rows
                .is_empty()
                .then(|| "no pending requests — the lobby is quiet".into()),
            rows,
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String> {
        match action_id {
            "approve" | "decline" => {
                self.client
                    .post(format!(
                        "{}/api/v1/request/{row_key}/{action_id}",
                        self.base
                    ))
                    .header("X-Api-Key", &self.key)
                    .send()
                    .await?
                    .error_for_status()?;
                Ok(format!("request {row_key} {action_id}d"))
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}
