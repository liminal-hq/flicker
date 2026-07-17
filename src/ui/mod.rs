// UI root: marquee header, screen tabs, panel grid, footer, overlays, Jax
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app::{App, Overlay};
use crate::theme::{ACCENT, GOOD, MUTED, WARN};

pub mod jax;
pub mod modal;
pub mod panels;

const SPINNER: [char; 6] = ['⠋', '⠙', '⠸', '⠴', '⠦', '⠇'];

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    draw_header(f, app, chunks[0]);
    draw_screen(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);

    match &app.overlay {
        Overlay::None => {
            if app.jax {
                jax::draw_companion(f, app, chunks[1]);
            }
        }
        Overlay::Help => modal::draw_help(f),
        Overlay::Menu { slot, items, sel } => modal::draw_menu(f, app, *slot, items, *sel),
        Overlay::Confirm {
            slot,
            action,
            context,
            ..
        } => modal::draw_confirm(f, app, *slot, action, context),
        Overlay::Palette { input, sel } => modal::draw_palette(f, app, input, *sel),
    }
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    let mode = if app.demo {
        Span::styled(
            " DEMO REEL ",
            Style::default().fg(Color::Black).bg(WARN).bold(),
        )
    } else {
        Span::styled(" LIVE ", Style::default().fg(Color::Black).bg(GOOD).bold())
    };
    let title = Line::from(vec![
        Span::styled(" 🎬 FLICKER ", Style::default().fg(ACCENT).bold()),
        Span::styled(
            "· the space between frames  ",
            Style::default().fg(MUTED).italic(),
        ),
        mode,
    ]);
    f.render_widget(Paragraph::new(title), rows[0]);

    let mut spans = vec![Span::raw(" ")];
    for (i, name) in app.screens.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled(" ▪ ", Style::default().fg(MUTED)));
        }
        let label = format!(" {} {} ", i + 1, name);
        if i == app.screen_idx {
            spans.push(Span::styled(
                label,
                Style::default().fg(Color::Black).bg(ACCENT).bold(),
            ));
        } else {
            spans.push(Span::styled(label, Style::default().fg(MUTED)));
        }
    }
    f.render_widget(Paragraph::new(Line::from(spans)), rows[1]);
}

fn draw_screen(f: &mut Frame, app: &App, area: Rect) {
    let ids = app.slots_in(app.screen_idx);
    if ids.is_empty() {
        f.render_widget(
            Paragraph::new("this screen is an empty theatre")
                .style(Style::default().fg(MUTED))
                .centered(),
            area,
        );
        return;
    }
    let focus = app.focus[app.screen_idx].min(ids.len() - 1);

    let areas: Vec<Rect> = if ids.len() == 1 {
        vec![area]
    } else {
        let left_n = ids.len().div_ceil(2);
        let right_n = ids.len() - left_n;
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);
        let lefts = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Ratio(1, left_n as u32); left_n])
            .split(cols[0]);
        let rights = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Ratio(1, right_n.max(1) as u32);
                right_n.max(1)
            ])
            .split(cols[1]);
        lefts
            .iter()
            .chain(rights.iter().take(right_n))
            .copied()
            .collect()
    };

    for (pos, (&slot, rect)) in ids.iter().zip(areas.iter()).enumerate() {
        panels::draw_slot(f, app, slot, *rect, pos == focus);
    }
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let spin = if app.any_loading() {
        format!(" {} ", SPINNER[(app.tick / 2) as usize % SPINNER.len()])
    } else {
        "   ".into()
    };
    let line = if let Some((msg, ok)) = app.status_line() {
        Line::from(vec![
            Span::styled(spin, Style::default().fg(ACCENT)),
            Span::styled(
                msg.to_string(),
                Style::default().fg(if ok { GOOD } else { crate::theme::BAD }),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(spin, Style::default().fg(ACCENT)),
            Span::styled(
                "1-9 screens · tab focus · j/k rows · enter actions · : palette · r/R refresh · J jax · ? help · q quit",
                Style::default().fg(MUTED),
            ),
        ])
    };
    f.render_widget(Paragraph::new(line), area);
}

/// Style shorthand used across the UI modules.
pub trait StyleExt {
    fn bold(self) -> Self;
    fn italic(self) -> Self;
}
impl StyleExt for Style {
    fn bold(self) -> Self {
        self.add_modifier(Modifier::BOLD)
    }
    fn italic(self) -> Self {
        self.add_modifier(Modifier::ITALIC)
    }
}
