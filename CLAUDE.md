# CLAUDE.md

Read `AGENTS.md` for coding standards (Canadian spelling, conventional commits) and `SPEC.md` for the design.

Quick orientation:

- **Flicker** is a pluggable homelab ops TUI (ratatui + crossterm event-stream, fully async: tokio + reqwest, one task per source).
- Core loop: `src/app.rs`; plugin trait + registry: `src/plugin/mod.rs`; each source is one file in `src/plugin/`; all rendering in `src/ui/`.
- Sources run as tokio tasks and exchange `SourceCmd` / `AppEvent` over mpsc channels. They return `Panel` (plain data, semantic `Tone`s) — never ratatui types.
- `cargo run -- --demo` = full offline fake homelab; use it to eyeball any UI change. `cargo run -- --check` polls every configured source once and prints a report — use it to verify live integrations without taking over the terminal.
- The real config (`~/.config/flicker/config.toml`) contains live API keys for Scott's homelab — never commit it or paste its keys into the repo.
- Scott's lab: `192.168.1.113` (plex/scott-TestBox: homepage :3030, grafana :3010, prometheus :9090, glances :61208), `192.168.1.221` (bounty: the *arr stack behind a VPN container, qbittorrent :8090, nzbget :6789, tautulli :8181, overseerr :5055, glances :61208), `192.168.1.68` (filey: NAS, no docker). SSH works to all three.
