// Plugin boundary: Panel data model, Source trait, worker tasks, registry
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! The plugin boundary. Sources produce plain-data `Panel`s with semantic
//! `Tone`s; they never touch ratatui. The UI decides what a tone looks like.
//!
//! Everything is async: each source instance is owned by one tokio task that
//! polls on an interval and services `SourceCmd`s.

use std::time::Duration;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::mpsc::{self, UnboundedSender};

use crate::config::SourceCfg;

pub mod arr;
pub mod demo;
pub mod glances;
pub mod jax;
pub mod nzbget;
pub mod overseerr;
pub mod qbittorrent;
pub mod ssh_host;
pub mod tautulli;
pub mod util;

/// Semantic colour intent. The theme maps these to actual colours.
/// `Accent` is part of the palette contract even while no built-in source
/// uses it — third-party plugins may.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Tone {
    Default,
    Accent,
    Accent2,
    Info,
    Good,
    Warn,
    Bad,
    Muted,
}

#[derive(Clone, Debug)]
pub struct Cell {
    pub text: String,
    pub tone: Tone,
}

pub fn cell(text: impl Into<String>, tone: Tone) -> Cell {
    Cell {
        text: text.into(),
        tone,
    }
}

#[derive(Clone, Debug)]
pub struct ActionSpec {
    pub id: String,
    pub label: String,
    pub danger: bool,
}

pub fn action(id: &str, label: impl Into<String>, danger: bool) -> ActionSpec {
    ActionSpec {
        id: id.into(),
        label: label.into(),
        danger,
    }
}

#[derive(Clone, Debug)]
pub struct RowItem {
    /// Opaque key handed back to `Source::execute` (session id, hash, name…).
    pub key: String,
    pub cells: Vec<Cell>,
    pub actions: Vec<ActionSpec>,
}

#[derive(Clone, Debug)]
pub struct GaugeItem {
    pub label: String,
    /// 0.0..=1.0
    pub ratio: f64,
    pub note: String,
}

/// Everything a source knows, as plain data.
#[derive(Clone, Debug, Default)]
pub struct Panel {
    /// Short status shown in the panel's top-right corner ("2 on air · 12 Mbps").
    pub badge: Option<String>,
    pub gauges: Vec<GaugeItem>,
    /// Optional sparkline: (label, series).
    pub spark: Option<(String, Vec<u64>)>,
    pub rows: Vec<RowItem>,
    /// One quiet line at the bottom ("the house is dark — nobody is watching").
    pub footer: Option<String>,
    /// Actions that apply to the whole panel, not one row.
    pub panel_actions: Vec<ActionSpec>,
}

/// The whole plugin API. One instance per configured source, owned by its task.
#[async_trait]
pub trait Source: Send {
    async fn poll(&mut self) -> Result<Panel>;
    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String>;
}

#[derive(Debug)]
pub enum SourceCmd {
    Refresh,
    Execute { action_id: String, row_key: String },
}

#[derive(Debug)]
pub enum AppEvent {
    Panel {
        id: usize,
        result: Result<Panel, String>,
    },
    ActionDone {
        id: usize,
        result: Result<String, String>,
    },
}

/// Spawn the worker task for one source. It polls immediately, then again
/// every `interval` or whenever asked; `Execute` re-polls right after so the
/// screen reflects the action without waiting a full cycle.
pub fn spawn_worker(
    id: usize,
    mut src: Box<dyn Source>,
    interval: Duration,
    tx: UnboundedSender<AppEvent>,
) -> UnboundedSender<SourceCmd> {
    let (ctx, mut crx) = mpsc::unbounded_channel::<SourceCmd>();
    tokio::spawn(async move {
        loop {
            let result = src.poll().await.map_err(|e| e.to_string());
            if tx.send(AppEvent::Panel { id, result }).is_err() {
                return;
            }
            tokio::select! {
                cmd = crx.recv() => match cmd {
                    Some(SourceCmd::Refresh) => {}
                    Some(SourceCmd::Execute { action_id, row_key }) => {
                        let result = src
                            .execute(&action_id, &row_key)
                            .await
                            .map_err(|e| e.to_string());
                        let _ = tx.send(AppEvent::ActionDone { id, result });
                    }
                    None => return,
                },
                _ = tokio::time::sleep(interval) => {}
            }
        }
    });
    ctx
}

pub mod registry {
    use super::*;

    pub const KINDS: &[&str] = &[
        "tautulli",
        "sonarr",
        "radarr",
        "lidarr",
        "prowlarr",
        "qbittorrent",
        "nzbget",
        "overseerr",
        "glances",
        "ssh",
        "jax",
    ];

    /// The one place a new plugin gets wired in.
    pub fn build(cfg: &SourceCfg) -> Result<Box<dyn Source>> {
        Ok(match cfg.kind.as_str() {
            "tautulli" => Box::new(tautulli::Tautulli::new(cfg)?),
            "sonarr" | "radarr" | "lidarr" | "prowlarr" => Box::new(arr::Arr::new(cfg)?),
            "qbittorrent" => Box::new(qbittorrent::Qbit::new(cfg)?),
            "nzbget" => Box::new(nzbget::Nzbget::new(cfg)?),
            "overseerr" => Box::new(overseerr::Overseerr::new(cfg)?),
            "glances" => Box::new(glances::Glances::new(cfg)?),
            "ssh" => Box::new(ssh_host::SshHost::new(cfg)?),
            "jax" => Box::new(jax::JaxSource::new(cfg)),
            other => anyhow::bail!(
                "unknown source kind '{other}' (available: {})",
                KINDS.join(", ")
            ),
        })
    }
}
