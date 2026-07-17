// Jax 2.0 as a plugin: the booth mascot gets his own panel, shift log included
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! Jax's data side. The animation itself is drawn by the UI (the one permitted
//! special case at the plugin boundary — `Panel` stays plain data; the UI just
//! knows how to throw a party inside a panel whose kind is `jax`). This source
//! ships what data he has: his shift log, and responses to being bothered.

use std::collections::VecDeque;

use anyhow::Result;
use async_trait::async_trait;
use rand::seq::SliceRandom;

use crate::config::SourceCfg;

use super::{action, cell, Panel, RowItem, Source, Tone};

pub struct JaxSource {
    log: VecDeque<String>,
    shift_mins: u64,
    snacks: u32,
}

const LOG_LINES: &[&str] = &[
    "checked the projector gate — no dust",
    "rewound reel 3 by hand, character building",
    "spliced a frame back in, nobody noticed",
    "lamp running warm; within tolerance",
    "swept the booth, found a 2019 popcorn kernel",
    "waved at the folks in NOW SHOWING",
    "carbon arc? no, we're LED now. progress",
    "labelled the reels. re-labelled the reels",
    "practised the changeover cue — 9 frames early",
    "listened to the hum. the hum is good today",
    "tightened the take-up reel, felt professional",
    "watched the bandwidth sparkline like a fish finder",
    "oiled the sprockets (metaphorically)",
    "told the disks a bedtime story",
];

impl JaxSource {
    pub fn new(_cfg: &SourceCfg) -> Self {
        Self {
            log: VecDeque::new(),
            shift_mins: 0,
            snacks: 0,
        }
    }
}

#[async_trait]
impl Source for JaxSource {
    async fn poll(&mut self) -> Result<Panel> {
        self.shift_mins += 1;
        let line = LOG_LINES
            .choose(&mut rand::thread_rng())
            .copied()
            .unwrap_or("stared into the beam");
        // Avoid the same entry twice in a row; Jax has *some* variety.
        if self.log.back().map(|l| l.as_str()) != Some(line) {
            self.log.push_back(line.to_string());
        }
        while self.log.len() > 5 {
            self.log.pop_front();
        }

        let rows = self
            .log
            .iter()
            .rev()
            .map(|l| RowItem {
                key: String::new(),
                cells: vec![cell("·", Tone::Accent2), cell(l.clone(), Tone::Muted)],
                actions: vec![],
            })
            .collect();

        Ok(Panel {
            badge: Some(format!("on shift · {} snacks", self.snacks)),
            rows,
            footer: Some("Jax 2.0 — now with object permanence".into()),
            panel_actions: vec![
                action("pet", "pet Jax", false),
                action("snack", "toss Jax a snack", false),
            ],
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, _row_key: &str) -> Result<String> {
        match action_id {
            "pet" => {
                self.log.push_back("received pets. morale +100".into());
                Ok("Jax chirps happily 🦦".into())
            }
            "snack" => {
                self.snacks += 1;
                self.log
                    .push_back("caught a snack mid-air. still got it".into());
                Ok("Jax catches it without looking 🍿".into())
            }
            other => anyhow::bail!("Jax tilts his head at '{other}'"),
        }
    }
}
