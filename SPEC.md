# Flicker — the space between frames

**Status: v0.1, built and running against a real homelab.** A Liminal HQ project.

## 1. The name, and why it belongs here

Cinema is a trick of the between: twenty-four still frames a second, and the darkness between them that your eye never sees. The *flicker* is that between-space — the thing that makes still pictures move. A homelab is the same trick. The household sees "the movie" (Plex plays, requests appear, downloads finish); underneath is a strip of machinery advancing frame by frame — queues, daemons, disks, containers — that nobody is supposed to notice.

Flicker is the console for the person who threads the projector. It is the view *between* the frames: what's playing, what's arriving, what's straining, on every machine at once — in one terminal, with some soul.

It was born as the terminal sibling of a [gethomepage](https://gethomepage.dev) dashboard called **ScottFlix+ Mission Control**, but it is not a port of it. A browser dashboard is a poster in the lobby; Flicker is the booth.

## 2. Liminal contract

Flicker keeps the house rules that Liminal HQ projects share:

- **Pull, not push.** Flicker never runs in the background, never notifies, never watches you. It is a room you *go to*. When you close it, it is gone.
- **Bounded.** Each panel shows a capped, curated slice (the top of the queue, the active streams, the hottest disks) — never an unbounded scroll of everything.
- **Glance first, detail on demand.** Badges and gauges up front; rows underneath; actions only behind an explicit keystroke plus a confirm for anything destructive.
- **Explorable with zero setup.** `flicker --demo` runs a full fake homelab so anyone can walk the rooms before wiring their own.

## 3. Architecture: a projector that takes any reel

The core knows nothing about Plex or Sonarr. It knows about **sources** — plugins that each poll something and hand back a `Panel` (badge, gauges, rows, sparkline, footer, actions). The UI renders panels; the config decides which sources exist and which **screen** each one sits on. Other people's homelabs are different films; the projector doesn't care.

```
config.toml ──> registry::build(kind, cfg) ──> Box<dyn Source>
                                                    │ (tokio task each)
                    SourceCmd::{Refresh, Execute} ──┤
                                                    ▼
                              AppEvent::Panel { id, Result<Panel> }
                                                    │
                        main loop ── draws screens ─┘
```

### The `Source` trait (the whole plugin API)

```rust
#[async_trait]
pub trait Source: Send {
    async fn poll(&mut self) -> Result<Panel>;
    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String>;
}
```

Everything is async (tokio + reqwest); each source instance is owned by one tokio task. The app sends it `Refresh` or `Execute`; it sends back panels. Sources never touch ratatui — a `Panel` is plain data with semantic `Tone`s (`Good`, `Warn`, `Bad`, `Accent`, …) that the theme maps to colour. Adding a plugin is: one file in `src/plugin/`, one arm in `registry::build`, one demo panel in `demo.rs`.

### First reel of plugins (v0.1)

| kind | polls | actions |
|---|---|---|
| `tautulli` | active Plex streams, per-stream progress/quality, total bandwidth + sparkline | terminate stream ⚠ |
| `plex` | sessions straight from Plex itself (no Tautulli required) | terminate session ⚠ |
| `sonarr` / `radarr` / `lidarr` | download queue with progress + health warnings | RSS sync, remove queue item ⚠ |
| `prowlarr` | indexer inventory + health | test all indexers |
| `qbittorrent` | transfer rates, torrents with state/progress/ETA | pause/resume, toggle alt speed, delete torrent ⚠ |
| `nzbget` | rate, remaining, queue groups | pause/resume queue |
| `sabnzbd` | rate, remaining, queue slots, download-disk space | pause/resume, delete slot ⚠ |
| `overseerr` | pending requests with requester + resolved titles | approve, decline ⚠ |
| `glances` | CPU/mem/swap gauges + filesystems from Glances v4 API | — |
| `ssh` | load, memory, disks, `docker ps` over plain ssh (BatchMode) | restart/stop container ⚠ |
| `prometheus` | scrape-target health (`up`), plus custom instant queries from config | — |
| `uptime-kuma` | monitor states parsed from Kuma's Prometheus `/metrics` text | — |
| `speedtest` | latest + average results from speedtest-tracker | run a test now |
| `jax` | Jax 2.0's shift log (the mascot is a plugin too — the UI animates his panel; the source ships only data) | pet Jax, toss him a snack |

⚠ = destructive: always behind the confirm modal (curtain-red border; only an explicit `y` fires — Enter and Esc take the default No). The policy line: pause/resume is reversible and never confirms; anything that deletes, removes, terminates, stops, or restarts always does.

Deliberately absent: Grafana iframes (a terminal is not an iframe; the numbers come from Prometheus, the horse's mouth) and Ombi (its API key on the reference lab is dead and Overseerr already covers the job — easy to add later if anyone still runs it).

## 4. Screens are rooms

The config groups sources into named screens; keys `1–9` jump between them, `[`/`]` walk them. The default reel for a media-server homelab:

1. **NOW SHOWING** — who is watching what, right now (tautulli)
2. **COMING SOON** — requests awaiting a decision, and the *arr queues fetching the future (overseerr, sonarr, radarr, lidarr, prowlarr)
3. **FREIGHT** — the trucks: qbittorrent, nzbget
4. **BACK LOT** — the machines themselves: glances + ssh per host

Within a screen, panels lay out in two balanced columns. `Tab` moves panel focus, `j/k` move row selection, `Enter` opens the action menu for the selection, `:` opens the command palette, `r`/`R` refresh one/all.

## 5. Look: projector-booth palette

Terminal-native dark, warmed by the lamp:

- **Marquee amber** `#FFB454` — focus, title, accents
- **Curtain crimson** `#E25D75` — danger, confirms, Jax's box
- **Projector cyan** `#6ECDDC` — informational values
- **Screen-glow green** `#98D279` — healthy / downloading / up
- **House-lights gold** `#F0C864` — warnings, paused states
- **Dust** `#787682` — chrome, muted text

Header is a marquee: film-sprocket strip (`▪ ▪ ▪`) over the title and screen tabs. Footer is the hint bar plus a status lamp that flashes action results.

## 6. Jax 2.0

Jax (the ambient mascot from [jira-tui](https://github.com/smorrisods/jira-tui)) moved into the booth and got the 2.0 upgrade: **moods**. He watches the same panels you do:

- someone's streaming → 🎬 he runs the projector (the reel spins, the beam breathes)
- queues/torrents moving → 📦 he hauls reels
- a source is erroring → 😰 he's at the splice bench, sweating
- an action just succeeded → 🎉 party
- all quiet → the classic hobbies: waving, napping, fishing, otter breaks, reading SPEC.md

`J` toggles the ambient companion (bottom-left, out of modals' way). He is *also a plugin*: `kind = "jax"` gives him his own panel with an animated scene and a rolling shift log ("rewound reel 3 by hand, character building"). Pet him. Toss him a snack. He's keeping count.

## 7. Config

`$XDG_CONFIG_HOME/flicker/config.toml` (`~/.config` fallback; or `--config <path>`; `--init` writes a commented example; `--check` polls every configured source once and prints a booth-inspection report — the fastest way to debug a new setup):

```toml
refresh_secs = 15        # default poll cadence; per-source override available

[[screens]]
name = "NOW SHOWING"

  [[screens.sources]]
  kind = "tautulli"
  url = "http://192.168.1.221:8181"
  api_key = "…"

[[screens]]
name = "BACK LOT"

  [[screens.sources]]
  kind = "ssh"
  host = "192.168.1.68"
  name = "filey"
```

Secrets live only in that file, mode 0600, never in the repo. `--demo` ignores config entirely.

## 8. Non-goals (v0.1)

- No daemon, no history database, no alerting — Grafana/Prometheus already do that (and do it on the BACK LOT hosts this very console watches).
- No mouse (yet) — jira-tui's OSC-52/mouse layer is a known quantity to lift later.
- No dynamic loading of plugins (dylib/WASM). "Pluggable" here means the trait boundary is clean and adding a source is a 100-line file — recompiling is fine for v0.1. Revisit if anyone who isn't Scott actually wants one.

## 9. Later reels

- Mouse + OSC-52 clipboard from jira-tui
- Per-panel fullscreen zoom (`z`)
- Action audit log (a booth notebook: what was restarted, when)
