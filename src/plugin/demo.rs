// Demo reel: a full, plausible fake homelab so --demo needs no network at all
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use super::util::bar;
use super::{action, cell, GaugeItem, Panel, RowItem, Tone};

pub struct DemoSlot {
    pub kind: &'static str,
    pub name: &'static str,
    pub panel: Panel,
}

/// The screens `--demo` boots with: same rooms, imaginary film.
pub fn screens() -> Vec<(String, Vec<DemoSlot>)> {
    vec![
        (
            "NOW SHOWING".into(),
            vec![DemoSlot {
                kind: "tautulli",
                name: "plex",
                panel: tautulli(),
            }],
        ),
        (
            "COMING SOON".into(),
            vec![
                DemoSlot {
                    kind: "overseerr",
                    name: "overseerr",
                    panel: overseerr(),
                },
                DemoSlot {
                    kind: "sonarr",
                    name: "sonarr",
                    panel: sonarr(),
                },
                DemoSlot {
                    kind: "radarr",
                    name: "radarr",
                    panel: radarr(),
                },
                DemoSlot {
                    kind: "prowlarr",
                    name: "prowlarr",
                    panel: prowlarr(),
                },
            ],
        ),
        (
            "FREIGHT".into(),
            vec![
                DemoSlot {
                    kind: "qbittorrent",
                    name: "qbittorrent",
                    panel: qbit(),
                },
                DemoSlot {
                    kind: "nzbget",
                    name: "nzbget",
                    panel: nzbget(),
                },
            ],
        ),
        (
            "BACK LOT".into(),
            vec![
                DemoSlot {
                    kind: "glances",
                    name: "media-box",
                    panel: glances(),
                },
                DemoSlot {
                    kind: "ssh",
                    name: "nas",
                    panel: ssh(),
                },
                DemoSlot {
                    kind: "jax",
                    name: "the booth",
                    panel: jax(),
                },
            ],
        ),
    ]
}

fn stream(user: &str, title: &str, pct: f64, decision: &str, mbps: f64, player: &str) -> RowItem {
    let dtone = if decision == "transcode" {
        Tone::Warn
    } else {
        Tone::Good
    };
    RowItem {
        key: user.into(),
        cells: vec![
            cell("▶", Tone::Good),
            cell(user, Tone::Accent2),
            cell(title, Tone::Default),
            cell(format!("{} {:.0}%", bar(pct, 10), pct * 100.0), Tone::Info),
            cell(decision, dtone),
            cell(format!("{mbps:.1} Mbps"), Tone::Muted),
            cell(player, Tone::Muted),
        ],
        actions: vec![action("terminate", "terminate this stream", true)],
    }
}

fn tautulli() -> Panel {
    Panel {
        badge: Some("3 on air · 31.2 Mbps".into()),
        spark: Some((
            "bandwidth".into(),
            vec![
                4, 6, 9, 14, 12, 18, 22, 25, 24, 28, 30, 31, 29, 31, 32, 31, 30, 31,
            ],
        )),
        rows: vec![
            stream(
                "margot",
                "The Space Between Frames (2026)",
                0.42,
                "direct play",
                18.4,
                "LG OLED",
            ),
            stream(
                "dee",
                "Changeover · S02E07 Nine Frames Early",
                0.87,
                "transcode",
                8.6,
                "iPhone",
            ),
            stream(
                "harold",
                "Threading the Projector (1978)",
                0.13,
                "direct play",
                4.2,
                "Shield",
            ),
        ],
        ..Default::default()
    }
}

fn queue_row(title: &str, pct: f64, status: &str, tone: Tone, left: &str) -> RowItem {
    RowItem {
        key: title.into(),
        cells: vec![
            cell(title, Tone::Default),
            cell(format!("{} {:.0}%", bar(pct, 8), pct * 100.0), Tone::Info),
            cell(status, tone),
            cell(left, Tone::Muted),
        ],
        actions: vec![action("remove", "remove from queue (and client)", true)],
    }
}

fn sonarr() -> Panel {
    Panel {
        badge: Some("4 queued".into()),
        rows: vec![
            queue_row("Changeover S02E08", 0.72, "downloading", Tone::Good, "12m"),
            queue_row("The Booth S01E01", 0.31, "downloading", Tone::Good, "48m"),
            queue_row("Sprocket Holes S05E03", 1.0, "import", Tone::Warn, "0 B"),
            queue_row("Intermission S03E11", 0.0, "queued", Tone::Muted, "2.1 GB"),
        ],
        panel_actions: vec![action("rss", "trigger RSS sync", false)],
        ..Default::default()
    }
}

fn radarr() -> Panel {
    Panel {
        badge: Some("2 queued · ⚠ 1".into()),
        rows: vec![
            queue_row(
                "Persistence of Vision (2025)",
                0.55,
                "downloading",
                Tone::Good,
                "1h 02m",
            ),
            queue_row("The Last Reel (1962)", 0.98, "import", Tone::Warn, "80 MB"),
        ],
        panel_actions: vec![action("rss", "trigger RSS sync", false)],
        ..Default::default()
    }
}

fn prowlarr() -> Panel {
    let idx = |on: bool, name: &str, proto: &str| RowItem {
        key: name.into(),
        cells: vec![
            cell(
                if on { "●" } else { "○" },
                if on { Tone::Good } else { Tone::Muted },
            ),
            cell(name, Tone::Default),
            cell(proto, Tone::Muted),
        ],
        actions: vec![],
    };
    Panel {
        badge: Some("3/4 indexers".into()),
        rows: vec![
            idx(true, "CinephileTracker", "torrent"),
            idx(true, "UsenetExpress", "usenet"),
            idx(true, "The Vault", "torrent"),
            idx(false, "DustyArchive", "usenet"),
        ],
        panel_actions: vec![action("testall", "test all indexers", false)],
        ..Default::default()
    }
}

fn overseerr() -> Panel {
    let req = |icon: &str, title: &str, who: &str, date: &str| RowItem {
        key: title.into(),
        cells: vec![
            cell(icon, Tone::Default),
            cell(title, Tone::Default),
            cell(format!("by {who}"), Tone::Accent2),
            cell(date, Tone::Muted),
        ],
        actions: vec![
            action("approve", "approve request", false),
            action("decline", "decline request", true),
        ],
    };
    Panel {
        badge: Some("2 pending".into()),
        rows: vec![
            req("🎬", "Flicker Fusion (2024)", "margot", "2026-07-15"),
            req("📺", "The Projectionist's Daughter", "harold", "2026-07-16"),
        ],
        ..Default::default()
    }
}

fn qbit() -> Panel {
    let t = |icon: &str, tone: Tone, name: &str, pct: f64, extra: Vec<(String, Tone)>| {
        let mut cells = vec![
            cell(icon, tone),
            cell(name, Tone::Default),
            cell(format!("{} {:.0}%", bar(pct, 8), pct * 100.0), Tone::Info),
        ];
        for (t2, tn) in extra {
            cells.push(cell(t2, tn));
        }
        RowItem {
            key: name.into(),
            cells,
            actions: vec![
                action("pause", "pause torrent", false),
                action("resume", "resume torrent", false),
                action("delete", "delete torrent (keep files)", true),
            ],
        }
    };
    Panel {
        badge: Some("▼ 8.4 MB/s ▲ 420 KB/s · 42 torrents".into()),
        rows: vec![
            t(
                "▼",
                Tone::Good,
                "the.space.between.frames.2026.2160p",
                0.64,
                vec![
                    ("6.1 MB/s".into(), Tone::Good),
                    ("14m 20s".into(), Tone::Muted),
                ],
            ),
            t(
                "▼",
                Tone::Good,
                "changeover.s02.complete.1080p",
                0.22,
                vec![
                    ("2.3 MB/s".into(), Tone::Good),
                    ("1h 08m".into(), Tone::Muted),
                ],
            ),
            t(
                "▲",
                Tone::Info,
                "threading.the.projector.1978.remaster",
                1.0,
                vec![("uploading".into(), Tone::Muted)],
            ),
            t(
                "⏸",
                Tone::Warn,
                "intermission.s03.720p",
                0.91,
                vec![("pausedDL".into(), Tone::Muted)],
            ),
        ],
        panel_actions: vec![
            action("alt", "toggle alternative speed limits", false),
            action("pause_all", "pause ALL torrents", true),
            action("resume_all", "resume all torrents", false),
        ],
        ..Default::default()
    }
}

fn nzbget() -> Panel {
    Panel {
        badge: Some("12.1 MB/s · 9.8 GB left".into()),
        rows: vec![
            queue_row(
                "PersistenceOfVision.2025.2160p.HDR",
                0.44,
                "downloading",
                Tone::Good,
                "5.2 GB",
            ),
            queue_row(
                "TheLastReel.1962.Criterion",
                0.0,
                "queued",
                Tone::Muted,
                "4.6 GB",
            ),
        ],
        footer: Some("341 GB this month".into()),
        panel_actions: vec![
            action("pause_all", "pause the whole queue", true),
            action("resume_all", "resume the queue", false),
        ],
        ..Default::default()
    }
}

fn glances() -> Panel {
    Panel {
        badge: Some("load 34%".into()),
        gauges: vec![
            GaugeItem {
                label: "cpu".into(),
                ratio: 0.27,
                note: "27%".into(),
            },
            GaugeItem {
                label: "mem".into(),
                ratio: 0.61,
                note: "61%".into(),
            },
            GaugeItem {
                label: "/tank".into(),
                ratio: 0.83,
                note: "9.1 TB / 11 TB".into(),
            },
            GaugeItem {
                label: "/".into(),
                ratio: 0.44,
                note: "210 GB / 480 GB".into(),
            },
        ],
        footer: Some("up 117 days, 2:30:11".into()),
        ..Default::default()
    }
}

fn ssh() -> Panel {
    let c = |on: bool, name: &str, status: &str| RowItem {
        key: name.into(),
        cells: vec![
            cell(
                if on { "●" } else { "○" },
                if on { Tone::Good } else { Tone::Bad },
            ),
            cell(name, Tone::Default),
            cell(status, if on { Tone::Good } else { Tone::Bad }),
        ],
        actions: vec![
            action("restart", "restart container", true),
            action("stop", "stop container", true),
        ],
    };
    Panel {
        badge: Some("load 0.42 0.51 0.48".into()),
        gauges: vec![
            GaugeItem {
                label: "mem".into(),
                ratio: 0.37,
                note: "12 GB / 32 GB".into(),
            },
            GaugeItem {
                label: "/pool".into(),
                ratio: 0.71,
                note: "31 TB / 44 TB".into(),
            },
        ],
        rows: vec![
            c(true, "plex", "Up 9 days"),
            c(true, "tautulli", "Up 9 days"),
            c(false, "speedtest", "Exited (1) 3 hours ago"),
        ],
        footer: Some("up 1 day, 19 hours, 57 minutes".into()),
        ..Default::default()
    }
}

fn jax() -> Panel {
    Panel {
        badge: Some("on shift · 3 snacks".into()),
        rows: vec![
            RowItem {
                key: String::new(),
                cells: vec![
                    cell("·", Tone::Accent2),
                    cell("practised the changeover cue — 9 frames early", Tone::Muted),
                ],
                actions: vec![],
            },
            RowItem {
                key: String::new(),
                cells: vec![
                    cell("·", Tone::Accent2),
                    cell("swept the booth, found a 2019 popcorn kernel", Tone::Muted),
                ],
                actions: vec![],
            },
        ],
        footer: Some("Jax 2.0 — now with object permanence".into()),
        panel_actions: vec![
            action("pet", "pet Jax", false),
            action("snack", "toss Jax a snack", false),
        ],
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_reel_covers_the_rooms() {
        let screens = screens();
        assert_eq!(screens.len(), 4);
        for (name, slots) in &screens {
            assert!(!name.is_empty());
            assert!(!slots.is_empty(), "screen {name} is an empty theatre");
        }
    }

    #[test]
    fn demo_kinds_are_all_registered() {
        for (_, slots) in screens() {
            for s in slots {
                assert!(
                    crate::plugin::registry::KINDS.contains(&s.kind),
                    "demo kind {} not in registry",
                    s.kind
                );
            }
        }
    }

    #[test]
    fn danger_actions_are_marked() {
        // every "terminate"/"delete"/"remove"/"stop"/"restart" in the demo reel
        // must carry danger=true — the confirm modal contract.
        for (_, slots) in screens() {
            for s in slots {
                for row in &s.panel.rows {
                    for a in &row.actions {
                        if ["terminate", "delete", "remove", "stop", "restart"]
                            .contains(&a.id.as_str())
                        {
                            assert!(a.danger, "{} should be danger", a.id);
                        }
                    }
                }
            }
        }
    }
}
