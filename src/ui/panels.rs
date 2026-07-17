// Panel renderer: one bordered box per source — badge, gauges, spark, rows, footer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, List, ListItem, ListState, Paragraph, Sparkline};
use ratatui::Frame;

use crate::app::App;
use crate::plugin::GaugeItem;
use crate::theme::{self, ACCENT, ACCENT2, BAD, GOOD, INFO, MUTED, SELECT_BG, WARN};

use super::StyleExt;

fn kind_colour(kind: &str) -> Color {
    match kind {
        "tautulli" | "plex" => ACCENT2,
        "sonarr" | "radarr" | "lidarr" | "prowlarr" => INFO,
        "qbittorrent" | "nzbget" | "sabnzbd" => GOOD,
        "prometheus" | "uptime-kuma" | "speedtest" => INFO,
        "overseerr" => WARN,
        "jax" => ACCENT2,
        _ => ACCENT,
    }
}

pub fn draw_slot(f: &mut Frame, app: &App, slot_id: usize, area: Rect, focused: bool) {
    let slot = &app.slots[slot_id];
    let border = if slot.error.is_some() {
        Style::default().fg(BAD)
    } else if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(MUTED)
    };

    let mut block = Block::bordered()
        .border_type(if focused {
            BorderType::Thick
        } else {
            BorderType::Rounded
        })
        .border_style(border)
        .title(Line::from(vec![
            Span::styled(" ▮ ", Style::default().fg(kind_colour(&slot.kind))),
            Span::styled(
                format!("{} ", slot.name),
                Style::default()
                    .fg(if focused { ACCENT } else { Color::Reset })
                    .bold(),
            ),
        ]));

    if let Some(p) = &slot.panel {
        if let Some(badge) = &p.badge {
            block = block.title(
                Line::from(Span::styled(
                    format!(" {badge} "),
                    Style::default().fg(INFO),
                ))
                .right_aligned(),
            );
        }
    }
    if let Some(err) = &slot.error {
        block = block.title_bottom(
            Line::from(Span::styled(
                format!(
                    " ✗ {} ",
                    crate::plugin::util::trunc(err, area.width.saturating_sub(6) as usize)
                ),
                Style::default().fg(BAD),
            ))
            .left_aligned(),
        );
    } else if let Some(t) = slot.updated {
        block = block.title_bottom(
            Line::from(Span::styled(
                format!(" {}s ", t.elapsed().as_secs()),
                Style::default().fg(MUTED),
            ))
            .right_aligned(),
        );
    }

    let inner = block.inner(area);
    f.render_widget(block, area);
    let Some(panel) = &slot.panel else {
        f.render_widget(
            Paragraph::new("threading the projector…")
                .style(Style::default().fg(MUTED).italic())
                .centered(),
            inner,
        );
        return;
    };

    if slot.kind == "jax" {
        super::jax::draw_booth_panel(f, app, panel, inner);
        return;
    }

    let mut constraints = Vec::new();
    let n_gauges = panel.gauges.len().min(inner.height as usize);
    if n_gauges > 0 {
        constraints.push(Constraint::Length(n_gauges as u16));
    }
    let has_spark = panel.spark.is_some() && inner.height > (n_gauges as u16 + 4);
    if has_spark {
        constraints.push(Constraint::Length(3));
    }
    constraints.push(Constraint::Min(0));
    let has_footer = panel.footer.is_some();
    if has_footer {
        constraints.push(Constraint::Length(1));
    }
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);
    let mut zi = 0;

    if n_gauges > 0 {
        draw_gauges(f, &panel.gauges[..n_gauges], zones[zi]);
        zi += 1;
    }
    if has_spark {
        if let Some((label, data)) = &panel.spark {
            let z = zones[zi];
            let split = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Length(2)])
                .split(z);
            f.render_widget(
                Paragraph::new(Span::styled(
                    format!("· {label}"),
                    Style::default().fg(MUTED),
                )),
                split[0],
            );
            f.render_widget(
                Sparkline::default()
                    .data(data)
                    .style(Style::default().fg(INFO)),
                split[1],
            );
        }
        zi += 1;
    }

    let rows_area = zones[zi];
    zi += 1;
    let items: Vec<ListItem> = panel
        .rows
        .iter()
        .map(|r| {
            let mut spans = Vec::new();
            for (i, c) in r.cells.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw("  "));
                }
                spans.push(Span::styled(
                    c.text.clone(),
                    Style::default().fg(theme::tone(c.tone)),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let mut state = ListState::default();
    if focused && !panel.rows.is_empty() {
        state.select(Some(slot.selected.min(panel.rows.len() - 1)));
    }
    let list = List::new(items)
        .highlight_style(Style::default().bg(SELECT_BG).bold())
        .highlight_symbol("▸ ");
    f.render_stateful_widget(list, rows_area, &mut state);

    if has_footer {
        if let Some(foot) = &panel.footer {
            f.render_widget(
                Paragraph::new(Span::styled(
                    foot.clone(),
                    Style::default().fg(MUTED).italic(),
                )),
                zones[zi],
            );
        }
    }
}

fn draw_gauges(f: &mut Frame, gauges: &[GaugeItem], area: Rect) {
    for (i, g) in gauges.iter().enumerate() {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        let row = Rect::new(area.x, y, area.width, 1);
        let label_w = 11usize;
        let note = &g.note;
        let bar_w = (area.width as usize)
            .saturating_sub(label_w + note.len() + 4)
            .clamp(6, 40);
        let filled = (g.ratio.clamp(0.0, 1.0) * bar_w as f64).round() as usize;
        let colour = if g.ratio < 0.7 {
            GOOD
        } else if g.ratio < 0.9 {
            WARN
        } else {
            BAD
        };
        let line = Line::from(vec![
            Span::styled(
                format!(
                    "{:<w$}",
                    crate::plugin::util::trunc(&g.label, label_w),
                    w = label_w
                ),
                Style::default().fg(MUTED),
            ),
            Span::styled("█".repeat(filled), Style::default().fg(colour)),
            Span::styled(
                "░".repeat(bar_w - filled),
                Style::default().fg(Color::Rgb(0x30, 0x2e, 0x38)),
            ),
            Span::styled(format!(" {note}"), Style::default().fg(MUTED)),
        ]);
        f.render_widget(Paragraph::new(line), row);
    }
}
