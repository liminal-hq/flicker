// Entrypoint: CLI args, terminal guard, and the async event loop
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

mod app;
mod config;
mod plugin;
mod theme;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{Event, EventStream, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::mpsc;

use app::{App, Slot};
use plugin::{registry, spawn_worker, AppEvent};

const HELP: &str = "\
flicker — the space between frames · a pluggable homelab ops console

usage: flicker [options]

options:
  --demo            run the offline demo reel (no config, no network)
  --config <path>   config file (default: $XDG_CONFIG_HOME/flicker/config.toml)
  --init            write a commented example config, then exit
  --check           poll every configured source once, report, exit
  -h, --help        this
  -V, --version     version
";

fn parse_args() -> Result<(bool, bool, bool, PathBuf)> {
    let mut demo = false;
    let mut init = false;
    let mut check = false;
    let mut path = config::default_path();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--demo" => demo = true,
            "--init" => init = true,
            "--check" => check = true,
            "--config" => {
                path = PathBuf::from(
                    args.next()
                        .ok_or_else(|| anyhow::anyhow!("--config needs a path"))?,
                )
            }
            "-h" | "--help" => {
                print!("{HELP}");
                std::process::exit(0);
            }
            "-V" | "--version" => {
                println!("flicker {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown flag {other} (try --help)"),
        }
    }
    Ok((demo, init, check, path))
}

/// `--check`: poll each configured source once, print a booth-inspection
/// report, exit non-zero if anything failed. No terminal takeover.
async fn check(cfg_path: &std::path::Path) -> Result<()> {
    let cfg = config::load(cfg_path)?;
    let mut failures = 0;
    println!("flicker --check · {}\n", cfg_path.display());
    for screen in &cfg.screens {
        println!("─ {} ─", screen.name);
        for scfg in &screen.sources {
            let name = scfg.display_name();
            match registry::build(scfg) {
                Ok(mut src) => match src.poll().await {
                    Ok(p) => {
                        let badge = p.badge.unwrap_or_else(|| format!("{} rows", p.rows.len()));
                        println!("  ✓ {name:<14} {badge}");
                    }
                    Err(e) => {
                        failures += 1;
                        println!("  ✗ {name:<14} {e:#}");
                    }
                },
                Err(e) => {
                    failures += 1;
                    println!("  ✗ {name:<14} {e:#}");
                }
            }
        }
    }
    println!();
    anyhow::ensure!(failures == 0, "{failures} source(s) failed");
    println!("all reels accounted for 🎬");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let (demo, init, do_check, cfg_path) = parse_args()?;
    if init {
        config::write_example(&cfg_path)?;
        println!(
            "wrote {} — fill in your urls and keys, then run flicker",
            cfg_path.display()
        );
        return Ok(());
    }
    if do_check {
        return check(&cfg_path).await;
    }

    let (tx, mut rx) = mpsc::unbounded_channel::<AppEvent>();
    let mut app = if demo {
        let mut screens = Vec::new();
        let mut slots = Vec::new();
        for (si, (name, demo_slots)) in plugin::demo::screens().into_iter().enumerate() {
            screens.push(name);
            for d in demo_slots {
                slots.push(Slot {
                    name: d.name.into(),
                    kind: d.kind.into(),
                    screen: si,
                    panel: Some(d.panel),
                    error: None,
                    updated: Some(Instant::now()),
                    selected: 0,
                    cmd: None,
                });
            }
        }
        App::new(screens, slots, true)
    } else {
        let cfg = config::load(&cfg_path).map_err(|e| {
            anyhow::anyhow!("{e}\n\nno config yet? try:  flicker --init   or   flicker --demo")
        })?;
        let default_interval = cfg.refresh_secs.unwrap_or(15).max(2);
        let mut screens = Vec::new();
        let mut slots = Vec::new();
        for (si, screen) in cfg.screens.iter().enumerate() {
            screens.push(screen.name.clone());
            for scfg in &screen.sources {
                let id = slots.len();
                // A bad source becomes an error panel, not a refusal to start —
                // the reels keep turning either way. Intervals floor at 2s so a
                // config typo can't busy-poll a service.
                let (cmd, error) = match registry::build(scfg) {
                    Ok(src) => {
                        let secs = scfg.interval_secs.unwrap_or(default_interval).max(2);
                        let interval = Duration::from_secs(secs);
                        (Some(spawn_worker(id, src, interval, tx.clone())), None)
                    }
                    Err(e) => (None, Some(e.to_string())),
                };
                slots.push(Slot {
                    name: scfg.display_name(),
                    kind: scfg.kind.clone(),
                    screen: si,
                    panel: None,
                    error,
                    updated: None,
                    selected: 0,
                    cmd,
                });
            }
        }
        App::new(screens, slots, false)
    };

    // Terminal guard: restore on unwind too, so a panic never eats the shell.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = crossterm::execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));
    enable_raw_mode()?;
    crossterm::execute!(io::stdout(), EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(io::stdout()))?;

    let result = run(&mut terminal, &mut app, &mut rx).await;

    disable_raw_mode()?;
    crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;
    result
}

async fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    rx: &mut mpsc::UnboundedReceiver<AppEvent>,
) -> Result<()> {
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    loop {
        terminal.draw(|f| ui::draw(f, app))?;
        tokio::select! {
            maybe = events.next() => {
                match maybe {
                    Some(Ok(Event::Key(key))) if key.kind != KeyEventKind::Release => {
                        app.on_key(key)
                    }
                    // Input stream ended (stdin EOF): leave rather than spin.
                    None => app.quit = true,
                    _ => {}
                }
            }
            Some(ev) = rx.recv() => {
                app.on_app_event(ev);
                while let Ok(ev) = rx.try_recv() {
                    app.on_app_event(ev);
                }
            }
            _ = ticker.tick() => app.tick += 1,
        }
        if app.quit {
            return Ok(());
        }
    }
}
