// Config model: screens of sources, XDG paths, example generation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

//! Config: screens of sources. Secrets live here and only here.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    /// Default poll cadence in seconds; sources may override.
    pub refresh_secs: Option<u64>,
    pub screens: Vec<ScreenCfg>,
}

#[derive(Debug, Deserialize)]
pub struct ScreenCfg {
    pub name: String,
    #[serde(default)]
    pub sources: Vec<SourceCfg>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SourceCfg {
    pub kind: String,
    /// Display name; defaults to the kind.
    pub name: Option<String>,
    pub url: Option<String>,
    pub api_key: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    /// For `ssh` sources: the host to reach (name or IP, ~/.ssh/config applies).
    pub host: Option<String>,
    pub interval_secs: Option<u64>,
}

impl SourceCfg {
    pub fn url(&self) -> Result<String> {
        let u = self
            .url
            .clone()
            .context(format!("source '{}' needs url", self.kind))?;
        Ok(u.trim_end_matches('/').to_string())
    }
    pub fn api_key(&self) -> Result<String> {
        self.api_key
            .clone()
            .context(format!("source '{}' needs api_key", self.kind))
    }
    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| self.kind.clone())
    }
}

pub fn default_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("flicker")
        .join("config.toml")
}

pub fn load(path: &Path) -> Result<Config> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("reading config {}", path.display()))?;
    let cfg: Config = toml::from_str(&raw).context("parsing config")?;
    anyhow::ensure!(!cfg.screens.is_empty(), "config has no [[screens]]");
    Ok(cfg)
}

pub const EXAMPLE: &str = r#"# flicker — the space between frames
# Screens are rooms; each holds one or more sources (panels).
# Available kinds: tautulli, sonarr, radarr, lidarr, prowlarr,
#                  qbittorrent, nzbget, overseerr, glances, ssh

refresh_secs = 15

[[screens]]
name = "NOW SHOWING"

  [[screens.sources]]
  kind = "tautulli"
  url = "http://192.168.1.10:8181"
  api_key = "changeme"

[[screens]]
name = "COMING SOON"

  [[screens.sources]]
  kind = "overseerr"
  url = "http://192.168.1.10:5055"
  api_key = "changeme"

  [[screens.sources]]
  kind = "sonarr"
  url = "http://192.168.1.10:8989"
  api_key = "changeme"

  [[screens.sources]]
  kind = "radarr"
  url = "http://192.168.1.10:7878"
  api_key = "changeme"

[[screens]]
name = "FREIGHT"

  [[screens.sources]]
  kind = "qbittorrent"
  url = "http://192.168.1.10:8090"
  username = "admin"
  password = "changeme"

  [[screens.sources]]
  kind = "nzbget"
  url = "http://192.168.1.10:6789"
  username = "nzbget"
  password = "changeme"

[[screens]]
name = "BACK LOT"

  [[screens.sources]]
  kind = "glances"
  name = "media-box"
  url = "http://192.168.1.10:61208"

  [[screens.sources]]
  kind = "ssh"
  name = "nas"
  host = "192.168.1.20"
"#;

pub fn write_example(path: &Path) -> Result<()> {
    anyhow::ensure!(
        !path.exists(),
        "{} already exists — not overwriting",
        path.display()
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, EXAMPLE)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_parses() {
        let cfg: Config = toml::from_str(EXAMPLE).expect("EXAMPLE must stay valid");
        assert_eq!(cfg.refresh_secs, Some(15));
        assert_eq!(cfg.screens.len(), 4);
        assert_eq!(cfg.screens[0].name, "NOW SHOWING");
        assert_eq!(cfg.screens[0].sources[0].kind, "tautulli");
    }

    #[test]
    fn every_example_kind_is_registered() {
        let cfg: Config = toml::from_str(EXAMPLE).unwrap();
        for screen in &cfg.screens {
            for s in &screen.sources {
                assert!(
                    crate::plugin::registry::KINDS.contains(&s.kind.as_str()),
                    "example uses unregistered kind {}",
                    s.kind
                );
            }
        }
    }

    #[test]
    fn url_trims_trailing_slash() {
        let s = SourceCfg {
            kind: "tautulli".into(),
            url: Some("http://x:8181/".into()),
            ..Default::default()
        };
        assert_eq!(s.url().unwrap(), "http://x:8181");
    }

    #[test]
    fn unknown_kind_is_a_helpful_error() {
        let s = SourceCfg {
            kind: "betamax".into(),
            ..Default::default()
        };
        let err = match crate::plugin::registry::build(&s) {
            Err(e) => e.to_string(),
            Ok(_) => panic!("betamax should not build"),
        };
        assert!(err.contains("betamax"));
        assert!(
            err.contains("tautulli"),
            "error should list available kinds"
        );
    }
}
