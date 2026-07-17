// SSH host source: load, memory, disks, and docker containers over plain ssh
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: MIT

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::process::Command;

use crate::config::SourceCfg;

use super::util::{human_bytes, trunc};
use super::{action, cell, GaugeItem, Panel, RowItem, Source, Tone};

pub struct SshHost {
    host: String,
}

const PROBE: &str = r#"
echo @@LOAD@@; cat /proc/loadavg
echo @@MEM@@; free -b | awk '/^Mem:/ {print $2, $3}'
echo @@DISK@@; df -B1 --output=target,size,used,pcent -x tmpfs -x devtmpfs -x overlay -x squashfs 2>/dev/null | tail -n +2
echo @@DOCKER@@; docker ps --format '{{.Names}}\t{{.Status}}' 2>/dev/null
echo @@UPTIME@@; uptime -p
"#;

impl SshHost {
    pub fn new(cfg: &SourceCfg) -> Result<Self> {
        Ok(Self {
            host: cfg.host.clone().context("ssh source needs host")?,
        })
    }

    async fn ssh(&self, script: &str) -> Result<String> {
        let out = Command::new("ssh")
            .args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=5",
                &self.host,
                script,
            ])
            .output()
            .await?;
        anyhow::ensure!(
            out.status.success(),
            "ssh {}: {}",
            self.host,
            String::from_utf8_lossy(&out.stderr).trim()
        );
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }
}

/// Collect the lines between `@@NAME@@` and the next marker.
fn section<'a>(raw: &'a str, name: &str) -> Vec<&'a str> {
    let marker = format!("@@{name}@@");
    let mut lines = Vec::new();
    let mut inside = false;
    for line in raw.lines() {
        if line.trim() == marker {
            inside = true;
            continue;
        }
        if inside && line.trim().starts_with("@@") {
            break;
        }
        if inside && !line.trim().is_empty() {
            lines.push(line);
        }
    }
    lines
}

const SKIP_MOUNTS: &[&str] = &["/boot", "/snap", "/run", "/dev", "/var/lib/docker", "/efi"];

#[async_trait]
impl Source for SshHost {
    async fn poll(&mut self) -> Result<Panel> {
        let raw = self.ssh(PROBE).await?;

        let load = section(&raw, "LOAD")
            .first()
            .map(|l| l.split_whitespace().take(3).collect::<Vec<_>>().join(" "))
            .unwrap_or_default();

        let mut gauges = Vec::new();
        if let Some(mem) = section(&raw, "MEM").first() {
            let parts: Vec<f64> = mem
                .split_whitespace()
                .filter_map(|p| p.parse().ok())
                .collect();
            if let [total, used] = parts[..] {
                gauges.push(GaugeItem {
                    label: "mem".into(),
                    ratio: used / total.max(1.0),
                    note: format!("{} / {}", human_bytes(used), human_bytes(total)),
                });
            }
        }
        let mut disks: Vec<(String, f64, f64, f64)> = section(&raw, "DISK")
            .iter()
            .filter_map(|l| {
                let p: Vec<&str> = l.split_whitespace().collect();
                if p.len() < 4 {
                    return None;
                }
                let mount = p[0].to_string();
                if SKIP_MOUNTS.iter().any(|s| mount.starts_with(s)) {
                    return None;
                }
                let size: f64 = p[1].parse().ok()?;
                let used: f64 = p[2].parse().ok()?;
                let pct: f64 = p[3].trim_end_matches('%').parse().ok()?;
                Some((mount, size, used, pct))
            })
            .collect();
        disks.sort_by(|a, b| b.1.total_cmp(&a.1));
        disks.dedup_by(|a, b| a.0 == b.0);
        for (mount, size, used, pct) in disks.into_iter().take(5) {
            gauges.push(GaugeItem {
                label: trunc(&mount, 14),
                ratio: pct / 100.0,
                note: format!("{} / {}", human_bytes(used), human_bytes(size)),
            });
        }

        let containers = section(&raw, "DOCKER");
        let rows = if containers.is_empty() {
            vec![RowItem {
                key: String::new(),
                cells: vec![cell("no containers (docker not present)", Tone::Muted)],
                actions: vec![],
            }]
        } else {
            containers
                .iter()
                .map(|l| {
                    let (name, status) = l.split_once('\t').unwrap_or((l, ""));
                    let tone = if status.starts_with("Up") {
                        Tone::Good
                    } else if status.contains("Restarting") || status.contains("Paused") {
                        Tone::Warn
                    } else {
                        Tone::Bad
                    };
                    RowItem {
                        key: name.to_string(),
                        cells: vec![
                            cell(
                                if status.starts_with("Up") {
                                    "●"
                                } else {
                                    "○"
                                },
                                tone,
                            ),
                            cell(trunc(name, 30), Tone::Default),
                            cell(status.to_string(), tone),
                        ],
                        actions: vec![
                            action("restart", "restart container", true),
                            action("stop", "stop container", true),
                        ],
                    }
                })
                .collect()
        };

        let uptime = section(&raw, "UPTIME")
            .first()
            .map(|s| s.to_string())
            .unwrap_or_default();
        Ok(Panel {
            badge: Some(format!("load {load}")),
            gauges,
            rows,
            footer: (!uptime.is_empty()).then_some(uptime),
            ..Default::default()
        })
    }

    async fn execute(&mut self, action_id: &str, row_key: &str) -> Result<String> {
        anyhow::ensure!(!row_key.is_empty(), "no container selected");
        // Container names come from `docker ps`; still, never let one become shell.
        anyhow::ensure!(
            row_key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)),
            "suspicious container name"
        );
        match action_id {
            "restart" | "stop" => {
                self.ssh(&format!("docker {action_id} {row_key}")).await?;
                Ok(format!("{action_id}ed {row_key} on {}", self.host))
            }
            other => anyhow::bail!("unknown action {other}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "@@LOAD@@\n0.42 0.51 0.48 1/234 5678\n@@MEM@@\n34359738368 12884901888\n@@DISK@@\n/ 500000000000 220000000000 44%\n/pool 44000000000000 31240000000000 71%\n@@DOCKER@@\nplex\tUp 9 days\nspeedtest\tExited (1) 3 hours ago\n@@UPTIME@@\nup 1 day, 19 hours\n";

    #[test]
    fn sections_split_on_markers() {
        assert_eq!(section(SAMPLE, "LOAD"), vec!["0.42 0.51 0.48 1/234 5678"]);
        assert_eq!(section(SAMPLE, "DOCKER").len(), 2);
        assert_eq!(section(SAMPLE, "UPTIME"), vec!["up 1 day, 19 hours"]);
        assert!(section(SAMPLE, "NOPE").is_empty());
    }

    #[test]
    fn container_names_are_vetted_before_shell() {
        // the same rule execute() enforces
        let ok = |s: &str| {
            s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        };
        assert!(ok("acquire-sonarr-1"));
        assert!(ok("gitea-db-1"));
        assert!(!ok("evil; rm -rf /"));
        assert!(!ok("$(boom)"));
    }
}
