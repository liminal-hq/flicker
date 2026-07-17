// Modals: action menu, danger confirm, command palette, help
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::plugin::ActionSpec;
use crate::theme::{ACCENT, ACCENT2, BAD, GOOD, INFO, MUTED, SELECT_BG};

use super::StyleExt;

fn centred(width: u16, height: u16, r: Rect) -> Rect {
    let w = width.min(r.width.saturating_sub(2));
    let h = height.min(r.height.saturating_sub(2));
    Rect::new(r.x + (r.width - w) / 2, r.y + (r.height - h) / 2, w, h)
}

fn frame_block(title: &str, colour: Color) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(colour))
        .title(Span::styled(
            format!("  {title}  "),
            Style::default().fg(colour).bold(),
        ))
}

pub fn draw_menu(
    f: &mut Frame,
    app: &App,
    slot: usize,
    items: &[(ActionSpec, String)],
    sel: usize,
) {
    let name = &app.slots[slot].name;
    let h = (items.len() as u16 + 2).min(14);
    let area = centred(46, h, f.area());
    f.render_widget(Clear, area);
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(a, _)| {
            let style = if a.danger {
                Style::default().fg(BAD)
            } else {
                Style::default().fg(Color::Reset)
            };
            ListItem::new(Line::from(vec![
                Span::styled(if a.danger { "⚠ " } else { "· " }, style),
                Span::styled(a.label.clone(), style),
            ]))
        })
        .collect();
    let mut state = ListState::default();
    state.select(Some(sel));
    let title = format!("{name} — actions");
    let list = List::new(list_items)
        .block(frame_block(&title, ACCENT))
        .highlight_style(Style::default().bg(SELECT_BG).bold())
        .highlight_symbol("▸ ");
    f.render_stateful_widget(list, area, &mut state);
}

pub fn draw_confirm(f: &mut Frame, _app: &App, _slot: usize, action: &ActionSpec, context: &str) {
    let area = centred(52, 7, f.area());
    f.render_widget(Clear, area);
    let block = frame_block("hold it — you sure?", ACCENT2);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let mut lines = vec![Line::from(Span::styled(
        action.label.clone(),
        Style::default().fg(BAD).bold(),
    ))];
    if !context.is_empty() {
        lines.push(Line::from(Span::styled(
            crate::plugin::util::trunc(context, 46),
            Style::default().fg(MUTED),
        )));
    }
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("[y]", Style::default().fg(BAD).bold()),
        Span::styled(" do it   ", Style::default().fg(MUTED)),
        Span::styled("[n]", Style::default().fg(GOOD).bold()),
        Span::styled(" leave it alone", Style::default().fg(MUTED)),
    ]));
    f.render_widget(Paragraph::new(lines).alignment(Alignment::Center), inner);
}

pub fn draw_palette(f: &mut Frame, app: &App, input: &str, sel: usize) {
    let items = app.palette_items(input);
    let h = (items.len() as u16 + 4).min(16);
    let area = centred(56, h, f.area());
    f.render_widget(Clear, area);
    let block = frame_block("command palette", INFO);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let zones = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(inner);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("› ", Style::default().fg(ACCENT)),
            Span::raw(input.to_string()),
            Span::styled("▌", Style::default().fg(ACCENT)),
        ])),
        zones[0],
    );
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|i| {
            let danger = i.label.contains('⚠');
            ListItem::new(Span::styled(
                i.label.clone(),
                Style::default().fg(if danger { BAD } else { Color::Reset }),
            ))
        })
        .collect();
    let mut state = ListState::default();
    if !items.is_empty() {
        state.select(Some(sel.min(items.len() - 1)));
    }
    let list = List::new(list_items)
        .highlight_style(Style::default().bg(SELECT_BG).bold())
        .highlight_symbol("▸ ");
    f.render_stateful_widget(list, zones[1], &mut state);
}

pub fn draw_help(f: &mut Frame) {
    let area = centred(58, 18, f.area());
    f.render_widget(Clear, area);
    let block = frame_block("flicker — projectionist's manual", ACCENT);
    let inner = block.inner(area);
    f.render_widget(block, area);
    let key = |k: &str, what: &str| {
        Line::from(vec![
            Span::styled(format!("  {k:<12}"), Style::default().fg(ACCENT)),
            Span::styled(what.to_string(), Style::default().fg(Color::Reset)),
        ])
    };
    let lines = vec![
        key("1-9", "jump to screen"),
        key("[ ] / h l", "previous / next screen"),
        key("tab", "cycle panel focus"),
        key("j k / ↑ ↓", "move row selection"),
        key("enter / a", "actions for the selection"),
        key(": / p", "command palette"),
        key("r / R", "refresh focused panel / everything"),
        key("J", "toggle Jax (he understands)"),
        key("q", "leave the booth"),
        Line::default(),
        Line::from(Span::styled(
            "  destructive actions always ask first.",
            Style::default().fg(MUTED).italic(),
        )),
        Line::from(Span::styled(
            "  the reels keep turning either way.",
            Style::default().fg(MUTED).italic(),
        )),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}
