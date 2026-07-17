// Jax 2.0: the booth mascot — mood engine, animated scenes, companion + panel
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! Jax came over from jira-tui and got the 2.0 upgrade: he now *reacts to the
//! homelab*. Streams playing? He runs the projector. Downloads hauling? He
//! shifts crates. Something erroring? He's at the splice bench, sweating.
//! An action just succeeded? Party. Otherwise: the classic hobbies.

use std::time::Duration;

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::plugin::Panel;
use crate::theme::{ACCENT, ACCENT2, BAD, INFO, MUTED};

use super::StyleExt;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mood {
    Party,
    Alarm,
    Showtime,
    Hauling,
    Chill,
}

/// What is the lab doing? Jax wants to know.
pub fn mood(app: &App) -> Mood {
    if app
        .last_action_ok
        .is_some_and(|t| t.elapsed() < Duration::from_secs(6))
    {
        return Mood::Party;
    }
    if app.slots.iter().any(|s| s.error.is_some()) {
        return Mood::Alarm;
    }
    let busy = |kinds: &[&str]| {
        app.slots.iter().any(|s| {
            kinds.contains(&s.kind.as_str()) && s.panel.as_ref().is_some_and(|p| !p.rows.is_empty())
        })
    };
    if busy(&["tautulli"]) {
        return Mood::Showtime;
    }
    if busy(&["qbittorrent", "nzbget", "sonarr", "radarr", "lidarr"]) {
        return Mood::Hauling;
    }
    Mood::Chill
}

/// The ambient companion, bottom-left, exactly where he lived in jira-tui —
/// only now he's watching the same panels you are.
pub fn draw_companion(f: &mut Frame, app: &App, area: Rect) {
    let w = 30u16.min(area.width.saturating_sub(2));
    let h = 8u16.min(area.height.saturating_sub(1));
    if w < 16 || h < 6 {
        return;
    }
    let rect = Rect::new(area.x + 2, area.y + area.height.saturating_sub(h + 1), w, h);
    f.render_widget(Clear, rect);

    let (caption, body) = scene(app.tick, mood(app));
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT2))
        .title(Span::styled(
            format!("  jax 2.0 · {caption}  "),
            Style::default().fg(ACCENT2).bold(),
        ));
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    f.render_widget(
        Paragraph::new(Text::from(body)).alignment(Alignment::Center),
        inner,
    );
}

/// Jax's own panel (`kind = "jax"`): big scene up top, shift log below.
pub fn draw_booth_panel(f: &mut Frame, app: &App, panel: &Panel, area: Rect) {
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    let (caption, mut body) = scene(app.tick, mood(app));
    body.insert(
        0,
        Line::from(Span::styled(
            format!("· {caption} ·"),
            Style::default().fg(ACCENT2).italic(),
        )),
    );
    f.render_widget(
        Paragraph::new(Text::from(body)).alignment(Alignment::Center),
        zones[0],
    );

    let log: Vec<Line> = panel
        .rows
        .iter()
        .map(|r| {
            let mut spans = vec![Span::styled("  · ", Style::default().fg(ACCENT2))];
            if let Some(c) = r.cells.last() {
                spans.push(Span::styled(c.text.clone(), Style::default().fg(MUTED)));
            }
            Line::from(spans)
        })
        .collect();
    f.render_widget(Paragraph::new(Text::from(log)), zones[1]);

    if let Some(foot) = &panel.footer {
        f.render_widget(
            Paragraph::new(Span::styled(
                foot.clone(),
                Style::default().fg(MUTED).italic(),
            ))
            .alignment(Alignment::Center),
            zones[2],
        );
    }
}

/// The rotating cast of scenes. Mood picks the show; tick runs the frames.
fn scene(tick: u64, mood: Mood) -> (&'static str, Vec<Line<'static>>) {
    let frame = (tick / 3) % 4;
    let blink = (tick / 6).is_multiple_of(9);
    let eyes = if blink { "- ‿ -" } else { "●‿●" };
    let a = Style::default().fg(ACCENT);
    let w = Style::default().fg(Color::White);
    let m = Style::default().fg(MUTED);
    let ln = |s: String, st: Style| Line::from(Span::styled(s, st));

    match mood {
        Mood::Showtime => {
            // Running the projector: the reel spins, the beam breathes.
            let reel = ['◐', '◓', '◑', '◒'][frame as usize];
            let beam = ["░▒▓▓▒░", "▒▓▓▒░░", "▓▓▒░░▒", "▓▒░░▒▓"][frame as usize];
            (
                "🎬 now showing",
                vec![
                    ln(format!(" .---.  ▄{reel}▄"), a),
                    ln(format!(" |{eyes}|══╣ ║"), w),
                    ln("  '--'  ▀▀▀".into(), a),
                    ln(format!("   {beam}→"), Style::default().fg(INFO)),
                ],
            )
        }
        Mood::Alarm => {
            // At the splice bench. It's fine. It's FINE.
            let snip = if frame.is_multiple_of(2) {
                "✂"
            } else {
                "✄"
            };
            let sweat = ["°", "!", "°", "!!"][frame as usize];
            (
                "😰 splicing!",
                vec![
                    ln(format!(" .---. {sweat}"), a),
                    ln(" |°□°|".into(), w),
                    ln(format!("  '--' {snip}╌╌╌"), a),
                    ln(" ~film~film~".into(), Style::default().fg(BAD)),
                ],
            )
        }
        Mood::Hauling => {
            // Freight shift: crates go that way.
            let pad = " ".repeat((frame as usize * 2).min(8));
            (
                "📦 hauling reels",
                vec![
                    ln(" .---.".into(), a),
                    ln(format!(" |{eyes}|→ {pad}▣▣"), w),
                    ln("  '--'".into(), a),
                    ln("▔▔▔▔▔▔▔▔▔▔▔▔".into(), m),
                ],
            )
        }
        Mood::Party => {
            let confetti = ["✦ ˚ ✧", "˚ ✧ ✦", "✧ ✦ ˚", "✦ ✧ ˚"][frame as usize];
            (
                "🎉 it worked!",
                vec![
                    ln(confetti.into(), Style::default().fg(ACCENT2)),
                    ln(" .---.".into(), a),
                    ln(format!(" |{eyes}| 🪅"), w),
                    ln(" \\'--'/".into(), a),
                ],
            )
        }
        Mood::Chill => chill_scene(tick, frame, eyes, a, w, m),
    }
}

/// Quiet lab: the classic jira-tui hobbies, plus reading this repo's spec.
fn chill_scene(
    tick: u64,
    frame: u64,
    eyes: &str,
    a: Style,
    w: Style,
    m: Style,
) -> (&'static str, Vec<Line<'static>>) {
    let ln = |s: String, st: Style| Line::from(Span::styled(s, st));
    match (tick / 45) % 5 {
        0 => {
            let arm = if frame.is_multiple_of(2) {
                "  o/"
            } else {
                "  \\o"
            };
            (
                "👋 hi!",
                vec![
                    ln(" .---.".into(), a),
                    ln(format!(" |{eyes}|"), w),
                    ln(format!("{arm}|  |"), a),
                    ln("  '--'".into(), a),
                ],
            )
        }
        1 => {
            let z = ["z  ", " Z ", "  z", " Z "][frame as usize];
            (
                "😴 zzz…",
                vec![
                    ln(format!("      {z}"), m),
                    ln(" .---.".into(), a),
                    ln(" |-‿-|".into(), w),
                    ln("  '--'".into(), a),
                ],
            )
        }
        2 => {
            let cur = if frame.is_multiple_of(2) { "▌" } else { " " };
            (
                "🤓 reading SPEC.md",
                vec![
                    ln(" .---.  __".into(), a),
                    ln(format!(" |◕‿◕| |{cur}|"), w),
                    ln("  '--'  ‾‾".into(), a),
                    ln(" // the space between".into(), m),
                ],
            )
        }
        3 => {
            let bob = ["°", ".", "°", "o"][frame as usize];
            let fish = if frame == 3 { "><>" } else { "   " };
            (
                "🎣 gone fishin'",
                vec![
                    ln(" .---. /".into(), a),
                    ln(format!(" |{eyes}|/"), w),
                    ln("  '--' ".into(), a),
                    ln(
                        format!("~~~~{bob}~{fish}~~"),
                        Style::default().fg(Color::Blue),
                    ),
                ],
            )
        }
        _ => {
            let pos = (frame * 3) as usize;
            let pad = " ".repeat(pos.min(10));
            (
                "🦦 otter break",
                vec![
                    ln(" .---.".into(), a),
                    ln(format!(" |{eyes}|"), w),
                    ln("  '--'".into(), a),
                    ln(format!("{pad}🦦~~"), Style::default().fg(Color::Blue)),
                ],
            )
        }
    }
}
