// App state: screens, slots, focus, overlays, and the key-event state machine
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tokio::sync::mpsc::UnboundedSender;

use crate::plugin::{ActionSpec, AppEvent, Panel, SourceCmd};

/// One configured source instance on one screen.
pub struct Slot {
    pub name: String,
    pub kind: String,
    pub screen: usize,
    pub panel: Option<Panel>,
    pub error: Option<String>,
    pub updated: Option<Instant>,
    pub selected: usize,
    /// None in demo mode — actions are pantomimed.
    pub cmd: Option<UnboundedSender<SourceCmd>>,
}

pub struct Status {
    pub msg: String,
    pub ok: bool,
    pub at: Instant,
}

pub enum Overlay {
    None,
    Help,
    Menu {
        slot: usize,
        items: Vec<(ActionSpec, String)>,
        sel: usize,
    },
    Confirm {
        slot: usize,
        action: ActionSpec,
        row_key: String,
        context: String,
    },
    Palette {
        input: String,
        sel: usize,
    },
}

pub enum PalCmd {
    Screen(usize),
    RefreshAll,
    ToggleJax,
    Help,
    Quit,
    Slot {
        slot: usize,
        action: ActionSpec,
        row_key: String,
    },
}

pub struct PaletteItem {
    pub label: String,
    pub cmd: PalCmd,
}

pub struct App {
    pub screens: Vec<String>,
    pub slots: Vec<Slot>,
    pub screen_idx: usize,
    /// Focused slot index per screen (index into `slots_in`'s vec).
    pub focus: Vec<usize>,
    pub tick: u64,
    pub jax: bool,
    pub overlay: Overlay,
    pub status: Option<Status>,
    pub last_action_ok: Option<Instant>,
    pub demo: bool,
    pub quit: bool,
}

impl App {
    pub fn new(screens: Vec<String>, slots: Vec<Slot>, demo: bool) -> Self {
        let n = screens.len();
        Self {
            screens,
            slots,
            screen_idx: 0,
            focus: vec![0; n],
            tick: 0,
            jax: true,
            overlay: Overlay::None,
            status: None,
            last_action_ok: None,
            demo,
            quit: false,
        }
    }

    pub fn slots_in(&self, screen: usize) -> Vec<usize> {
        (0..self.slots.len())
            .filter(|&i| self.slots[i].screen == screen)
            .collect()
    }

    pub fn focused_slot(&self) -> Option<usize> {
        let ids = self.slots_in(self.screen_idx);
        ids.get(self.focus[self.screen_idx].min(ids.len().saturating_sub(1)))
            .copied()
    }

    pub fn set_status(&mut self, msg: impl Into<String>, ok: bool) {
        self.status = Some(Status {
            msg: msg.into(),
            ok,
            at: Instant::now(),
        });
        if ok {
            self.last_action_ok = Some(Instant::now());
        }
    }

    pub fn status_line(&self) -> Option<(&str, bool)> {
        self.status
            .as_ref()
            .filter(|s| s.at.elapsed() < Duration::from_secs(6))
            .map(|s| (s.msg.as_str(), s.ok))
    }

    /// True while any slot has never delivered a panel (spinner state).
    pub fn any_loading(&self) -> bool {
        self.slots
            .iter()
            .any(|s| s.panel.is_none() && s.error.is_none())
    }

    pub fn on_app_event(&mut self, ev: AppEvent) {
        match ev {
            AppEvent::Panel { id, result } => {
                if let Some(slot) = self.slots.get_mut(id) {
                    match result {
                        Ok(p) => {
                            slot.selected = slot.selected.min(p.rows.len().saturating_sub(1));
                            slot.panel = Some(p);
                            slot.error = None;
                        }
                        Err(e) => slot.error = Some(e),
                    }
                    slot.updated = Some(Instant::now());
                }
            }
            AppEvent::ActionDone { id, result } => {
                let name = self
                    .slots
                    .get(id)
                    .map(|s| s.name.clone())
                    .unwrap_or_default();
                match result {
                    Ok(msg) => self.set_status(format!("{name}: {msg}"), true),
                    Err(e) => self.set_status(format!("{name}: {e}"), false),
                }
            }
        }
    }

    fn refresh(&mut self, slot: usize) {
        if let Some(tx) = &self.slots[slot].cmd {
            let _ = tx.send(SourceCmd::Refresh);
        }
    }

    fn dispatch(&mut self, slot: usize, action: ActionSpec, row_key: String) {
        let name = self.slots[slot].name.clone();
        match &self.slots[slot].cmd {
            Some(tx) => {
                let _ = tx.send(SourceCmd::Execute {
                    action_id: action.id,
                    row_key,
                });
                self.set_status(format!("{name}: {}…", action.label), true);
            }
            None => self.set_status(format!("demo: pretended to {} ✓", action.label), true),
        }
    }

    /// All actions available on a slot right now: selected row's, then panel's.
    fn slot_actions(&self, slot: usize) -> Vec<(ActionSpec, String)> {
        let mut items = Vec::new();
        if let Some(p) = &self.slots[slot].panel {
            if let Some(row) = p.rows.get(self.slots[slot].selected) {
                for a in &row.actions {
                    items.push((a.clone(), row.key.clone()));
                }
            }
            for a in &p.panel_actions {
                items.push((a.clone(), String::new()));
            }
        }
        items
    }

    /// Human context for a confirm: the selected row's first meaty cell.
    fn row_context(&self, slot: usize) -> String {
        self.slots[slot]
            .panel
            .as_ref()
            .and_then(|p| p.rows.get(self.slots[slot].selected))
            .map(|r| {
                r.cells
                    .iter()
                    .map(|c| c.text.as_str())
                    .filter(|t| t.chars().count() > 2)
                    .take(2)
                    .collect::<Vec<_>>()
                    .join(" · ")
            })
            .unwrap_or_default()
    }

    pub fn palette_items(&self, filter: &str) -> Vec<PaletteItem> {
        let mut all = Vec::new();
        for (i, name) in self.screens.iter().enumerate() {
            all.push(PaletteItem {
                label: format!("go to {name}"),
                cmd: PalCmd::Screen(i),
            });
        }
        all.push(PaletteItem {
            label: "refresh everything".into(),
            cmd: PalCmd::RefreshAll,
        });
        all.push(PaletteItem {
            label: "toggle Jax".into(),
            cmd: PalCmd::ToggleJax,
        });
        all.push(PaletteItem {
            label: "help".into(),
            cmd: PalCmd::Help,
        });
        all.push(PaletteItem {
            label: "quit flicker".into(),
            cmd: PalCmd::Quit,
        });
        if let Some(slot) = self.focused_slot() {
            let name = self.slots[slot].name.clone();
            for (a, key) in self.slot_actions(slot) {
                all.push(PaletteItem {
                    label: format!("{name}: {}{}", a.label, if a.danger { " ⚠" } else { "" }),
                    cmd: PalCmd::Slot {
                        slot,
                        action: a,
                        row_key: key,
                    },
                });
            }
        }
        let f = filter.to_lowercase();
        all.retain(|i| f.is_empty() || i.label.to_lowercase().contains(&f));
        all
    }

    fn run_pal_cmd(&mut self, cmd: PalCmd) {
        match cmd {
            PalCmd::Screen(i) => self.screen_idx = i,
            PalCmd::RefreshAll => {
                for i in 0..self.slots.len() {
                    self.refresh(i);
                }
                self.set_status("refreshing everything", true);
            }
            PalCmd::ToggleJax => self.jax = !self.jax,
            PalCmd::Help => self.overlay = Overlay::Help,
            PalCmd::Quit => self.quit = true,
            PalCmd::Slot {
                slot,
                action,
                row_key,
            } => {
                if action.danger {
                    let context = self.row_context(slot);
                    self.overlay = Overlay::Confirm {
                        slot,
                        action,
                        row_key,
                        context,
                    };
                } else {
                    self.dispatch(slot, action, row_key);
                }
            }
        }
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.quit = true;
            return;
        }
        match std::mem::replace(&mut self.overlay, Overlay::None) {
            Overlay::None => self.on_key_normal(key),
            Overlay::Help => match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') | KeyCode::Enter => {}
                _ => self.overlay = Overlay::Help,
            },
            Overlay::Menu {
                slot,
                items,
                mut sel,
            } => match key.code {
                KeyCode::Esc | KeyCode::Char('q') => {}
                KeyCode::Char('j') | KeyCode::Down => {
                    sel = (sel + 1) % items.len().max(1);
                    self.overlay = Overlay::Menu { slot, items, sel };
                }
                KeyCode::Char('k') | KeyCode::Up => {
                    sel = sel.checked_sub(1).unwrap_or(items.len().saturating_sub(1));
                    self.overlay = Overlay::Menu { slot, items, sel };
                }
                KeyCode::Enter => {
                    if let Some((action, row_key)) = items.into_iter().nth(sel) {
                        self.run_pal_cmd(PalCmd::Slot {
                            slot,
                            action,
                            row_key,
                        });
                    }
                }
                _ => self.overlay = Overlay::Menu { slot, items, sel },
            },
            Overlay::Confirm {
                slot,
                action,
                row_key,
                context,
            } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => self.dispatch(slot, action, row_key),
                KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('q') => {
                    self.set_status("cancelled — nothing touched", true);
                }
                _ => {
                    self.overlay = Overlay::Confirm {
                        slot,
                        action,
                        row_key,
                        context,
                    }
                }
            },
            Overlay::Palette { mut input, mut sel } => match key.code {
                KeyCode::Esc => {}
                KeyCode::Enter => {
                    let items = self.palette_items(&input);
                    if let Some(item) = items.into_iter().nth(sel) {
                        self.run_pal_cmd(item.cmd);
                    }
                }
                KeyCode::Backspace => {
                    input.pop();
                    self.overlay = Overlay::Palette { input, sel: 0 };
                }
                KeyCode::Down => {
                    sel = (sel + 1) % self.palette_items(&input).len().max(1);
                    self.overlay = Overlay::Palette { input, sel };
                }
                KeyCode::Up => {
                    let n = self.palette_items(&input).len().max(1);
                    sel = sel.checked_sub(1).unwrap_or(n - 1);
                    self.overlay = Overlay::Palette { input, sel };
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    self.overlay = Overlay::Palette { input, sel: 0 };
                }
                _ => self.overlay = Overlay::Palette { input, sel },
            },
        }
    }

    fn on_key_normal(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('?') => self.overlay = Overlay::Help,
            KeyCode::Char(':') | KeyCode::Char('p') => {
                self.overlay = Overlay::Palette {
                    input: String::new(),
                    sel: 0,
                }
            }
            KeyCode::Char('J') => self.jax = !self.jax,
            KeyCode::Char(c @ '1'..='9') => {
                let i = c as usize - '1' as usize;
                if i < self.screens.len() {
                    self.screen_idx = i;
                }
            }
            KeyCode::Char('[') | KeyCode::Char('h') | KeyCode::Left => {
                self.screen_idx = self
                    .screen_idx
                    .checked_sub(1)
                    .unwrap_or(self.screens.len() - 1);
            }
            KeyCode::Char(']') | KeyCode::Char('l') | KeyCode::Right => {
                self.screen_idx = (self.screen_idx + 1) % self.screens.len();
            }
            KeyCode::Tab => {
                let n = self.slots_in(self.screen_idx).len().max(1);
                self.focus[self.screen_idx] = (self.focus[self.screen_idx] + 1) % n;
            }
            KeyCode::BackTab => {
                let n = self.slots_in(self.screen_idx).len().max(1);
                let f = &mut self.focus[self.screen_idx];
                *f = f.checked_sub(1).unwrap_or(n - 1);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(i) = self.focused_slot() {
                    let n = self.slots[i].panel.as_ref().map_or(0, |p| p.rows.len());
                    if n > 0 {
                        self.slots[i].selected = (self.slots[i].selected + 1) % n;
                    }
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(i) = self.focused_slot() {
                    let n = self.slots[i].panel.as_ref().map_or(0, |p| p.rows.len());
                    if n > 0 {
                        let s = &mut self.slots[i].selected;
                        *s = s.checked_sub(1).unwrap_or(n - 1);
                    }
                }
            }
            KeyCode::Char('r') => {
                if let Some(i) = self.focused_slot() {
                    self.refresh(i);
                    self.set_status(format!("refreshing {}", self.slots[i].name), true);
                }
            }
            KeyCode::Char('R') => self.run_pal_cmd(PalCmd::RefreshAll),
            KeyCode::Enter | KeyCode::Char('a') => {
                if let Some(slot) = self.focused_slot() {
                    let items = self.slot_actions(slot);
                    if items.is_empty() {
                        self.set_status("no actions here", false);
                    } else {
                        self.overlay = Overlay::Menu {
                            slot,
                            items,
                            sel: 0,
                        };
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{action, cell, Panel, RowItem, Tone};
    use crossterm::event::{KeyCode, KeyEvent};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn demo_app() -> App {
        let panel = Panel {
            rows: vec![
                RowItem {
                    key: "r1".into(),
                    cells: vec![cell("first row", Tone::Default)],
                    actions: vec![action("zap", "zap it", true)],
                },
                RowItem {
                    key: "r2".into(),
                    cells: vec![cell("second row", Tone::Default)],
                    actions: vec![action("boop", "boop it", false)],
                },
            ],
            panel_actions: vec![action("sweep", "sweep the booth", false)],
            ..Default::default()
        };
        let slots = vec![
            Slot {
                name: "one".into(),
                kind: "tautulli".into(),
                screen: 0,
                panel: Some(panel),
                error: None,
                updated: None,
                selected: 0,
                cmd: None,
            },
            Slot {
                name: "two".into(),
                kind: "ssh".into(),
                screen: 1,
                panel: Some(Panel::default()),
                error: None,
                updated: None,
                selected: 0,
                cmd: None,
            },
        ];
        App::new(vec!["A".into(), "B".into()], slots, true)
    }

    #[test]
    fn screen_navigation_wraps() {
        let mut app = demo_app();
        assert_eq!(app.screen_idx, 0);
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(app.screen_idx, 1);
        app.on_key(key(KeyCode::Char(']')));
        assert_eq!(app.screen_idx, 0);
        app.on_key(key(KeyCode::Char('[')));
        assert_eq!(app.screen_idx, 1);
        app.on_key(key(KeyCode::Char('1')));
        assert_eq!(app.screen_idx, 0);
        // out-of-range screen number is ignored
        app.on_key(key(KeyCode::Char('9')));
        assert_eq!(app.screen_idx, 0);
    }

    #[test]
    fn row_selection_wraps_within_focused_slot() {
        let mut app = demo_app();
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.slots[0].selected, 1);
        app.on_key(key(KeyCode::Char('j')));
        assert_eq!(app.slots[0].selected, 0);
        app.on_key(key(KeyCode::Char('k')));
        assert_eq!(app.slots[0].selected, 1);
    }

    #[test]
    fn danger_action_demands_confirmation() {
        let mut app = demo_app();
        // open menu: selected row r1 has danger action "zap"
        app.on_key(key(KeyCode::Enter));
        assert!(matches!(app.overlay, Overlay::Menu { .. }));
        app.on_key(key(KeyCode::Enter)); // choose "zap it"
        match &app.overlay {
            Overlay::Confirm {
                action, row_key, ..
            } => {
                assert_eq!(action.id, "zap");
                assert_eq!(row_key, "r1");
            }
            _ => panic!("danger action must route through Confirm"),
        }
        // 'n' backs out without dispatching
        app.on_key(key(KeyCode::Char('n')));
        assert!(matches!(app.overlay, Overlay::None));
        let (msg, ok) = app.status_line().expect("cancel sets a status");
        assert!(ok);
        assert!(msg.contains("cancelled"));
    }

    #[test]
    fn safe_action_dispatches_directly() {
        let mut app = demo_app();
        app.on_key(key(KeyCode::Char('j'))); // row r2: "boop" is safe
        app.on_key(key(KeyCode::Enter));
        app.on_key(key(KeyCode::Enter));
        assert!(matches!(app.overlay, Overlay::None));
        let (msg, ok) = app.status_line().expect("demo dispatch sets a status");
        assert!(ok);
        assert!(msg.contains("boop"));
    }

    #[test]
    fn palette_filters_by_substring() {
        let app = demo_app();
        let all = app.palette_items("");
        assert!(all.len() >= 5);
        let filtered = app.palette_items("jax");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].label, "toggle Jax");
        // slot actions appear with the slot name
        let zap = app.palette_items("zap");
        assert_eq!(zap.len(), 1);
        assert!(zap[0].label.contains('⚠'));
    }

    #[test]
    fn panel_update_clamps_selection() {
        let mut app = demo_app();
        app.slots[0].selected = 1;
        app.on_app_event(AppEvent::Panel {
            id: 0,
            result: Ok(Panel {
                rows: vec![RowItem {
                    key: "only".into(),
                    cells: vec![cell("lonely row", Tone::Default)],
                    actions: vec![],
                }],
                ..Default::default()
            }),
        });
        assert_eq!(app.slots[0].selected, 0);
        assert!(app.slots[0].error.is_none());
    }

    #[test]
    fn poll_error_keeps_last_panel() {
        let mut app = demo_app();
        app.on_app_event(AppEvent::Panel {
            id: 0,
            result: Err("lamp burnt out".into()),
        });
        assert!(
            app.slots[0].panel.is_some(),
            "stale panel survives an error"
        );
        assert_eq!(app.slots[0].error.as_deref(), Some("lamp burnt out"));
    }

    #[test]
    fn quit_keys_quit() {
        let mut app = demo_app();
        app.on_key(key(KeyCode::Char('q')));
        assert!(app.quit);
        let mut app = demo_app();
        app.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.quit);
    }
}
