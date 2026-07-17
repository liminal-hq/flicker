// Prometheus source: scrape-target health plus any instant queries you fancy
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! Default view: `up` — how many scrape targets are alive, and which are not.
//! Config may add custom instant queries as `queries = ["label|expr", …]`;
//! each becomes a row showing the first sample's value.

use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;

use crate::config::SourceCfg;

use super::util::{f64_of, str_of, trunc};
use super::{cell, Panel, RowItem, Source, Tone};

pub struct Prometheus {
    base: String,
    queries: Vec<(String, String)>,
    client: reqwest::Client,
}

impl Prometheus {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        let queries = cfg
            .queries
            .clone()
            .unwrap_or_default()
            .iter()
            .map(|q| match q.split_once('|') {
                Some((label, expr)) => (label.trim().to_string(), expr.trim().to_string()),
                None => (q.clone(), q.clone()),
            })
            .collect();
        Ok(Self {
            base: cfg.url()?,
            queries,
            client: super::util::client(),
        })
    }

    async fn query(&self, expr: &str) -> Result<Vec<Value>> {
        let v: Value = self
            .client
            .get(format!("{}/api/v1/query", self.base))
            .query(&[("query", expr)])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        anyhow::ensure!(
            v["status"] == "success",
            "prometheus: {}",
            str_of(&v["error"])
        );
        Ok(v["data"]["result"].as_array().cloned().unwrap_or_default())
    }
}

/// A vector sample's value lives at `value[1]` as a string.
fn sample_value(s: &Value) -> f64 {
    f64_of(&s["value"][1])
}

#[async_trait]
impl Source for Prometheus {
    async fn poll(&mut self) -> Result<Panel> {
        let up = self.query("up").await?;
        let total = up.len();
        let alive = up.iter().filter(|s| sample_value(s) > 0.0).count();

        let mut rows: Vec<RowItem> = up
            .iter()
            .filter(|s| sample_value(s) < 1.0)
            .map(|s| RowItem {
                key: String::new(),
                cells: vec![
                    cell("○", Tone::Bad),
                    cell(str_of(&s["metric"]["job"]), Tone::Default),
                    cell(trunc(&str_of(&s["metric"]["instance"]), 30), Tone::Muted),
                    cell("down", Tone::Bad),
                ],
                actions: vec![],
            })
            .collect();

        let results =
            futures::future::join_all(self.queries.iter().map(|(_, expr)| self.query(expr))).await;
        for ((label, _), result) in self.queries.iter().zip(results) {
            let cells = match result {
                Ok(samples) => match samples.first() {
                    Some(s) => vec![
                        cell("·", Tone::Info),
                        cell(label.clone(), Tone::Default),
                        cell(format!("{:.4}", sample_value(s)), Tone::Info),
                    ],
                    None => vec![
                        cell("·", Tone::Muted),
                        cell(label.clone(), Tone::Default),
                        cell("no data", Tone::Muted),
                    ],
                },
                Err(e) => vec![
                    cell("✗", Tone::Bad),
                    cell(label.clone(), Tone::Default),
                    cell(trunc(&e.to_string(), 40), Tone::Bad),
                ],
            };
            rows.push(RowItem {
                key: String::new(),
                cells,
                actions: vec![],
            });
        }

        Ok(Panel {
            badge: Some(format!("{alive}/{total} targets up")),
            footer: (alive == total && self.queries.is_empty())
                .then(|| "every scrape target answering the roll call".into()),
            rows,
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, _row_key: &str) -> Result<String> {
        anyhow::bail!("prometheus has no actions (tried {action_id})")
    }
}
