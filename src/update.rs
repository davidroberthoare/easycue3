//! Notify-only "check for updates": compares the latest GitHub release tag
//! against the running build's version. Never downloads or replaces the
//! binary — on finding a newer release it just surfaces a link to the
//! GitHub Releases page for the user to act on manually.

use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const REPO_API_URL: &str =
    "https://api.github.com/repos/davidroberthoare/easycue3/releases/latest";

#[derive(Debug, Clone)]
pub struct UpdateInfo {
    pub latest_version: String,
    pub html_url: String,
}

#[derive(Debug, Clone, Default)]
pub enum UpdateCheckState {
    #[default]
    Unknown,
    Checking,
    UpToDate,
    UpdateAvailable(UpdateInfo),
    Failed,
}

#[derive(serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
}

/// Parses a GitHub release tag like "v0.7.0" into (major, minor, patch).
fn parse_version(tag: &str) -> Option<(u64, u64, u64)> {
    let s = tag.strip_prefix('v').unwrap_or(tag);
    let mut parts = s.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    Some((major, minor, patch))
}

fn fetch_latest_release() -> anyhow::Result<GithubRelease> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .build()
        .into();

    let body = agent
        .get(REPO_API_URL)
        .header("User-Agent", concat!("easycue3/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()?
        .into_body()
        .read_to_string()?;

    Ok(serde_json::from_str(&body)?)
}

/// Spawns a background thread that checks once for a newer release and
/// reports the outcome back over the returned channel. Shared by both the
/// manual "Check for Updates" menu action and the throttled startup check.
/// Never blocks the calling (UI) thread; wakes `ctx` so an idle app still
/// notices the result land.
pub fn spawn_check(ctx: egui::Context) -> Receiver<UpdateCheckState> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let state = match fetch_latest_release() {
            Ok(release) => match parse_version(&release.tag_name) {
                Some(latest) => {
                    let current = parse_version(concat!("v", env!("CARGO_PKG_VERSION")))
                        .expect("CARGO_PKG_VERSION must be well-formed major.minor.patch");
                    if latest > current {
                        UpdateCheckState::UpdateAvailable(UpdateInfo {
                            latest_version: format!("{}.{}.{}", latest.0, latest.1, latest.2),
                            html_url: release.html_url,
                        })
                    } else {
                        UpdateCheckState::UpToDate
                    }
                }
                None => {
                    log::warn!("[update] Could not parse release tag: {}", release.tag_name);
                    UpdateCheckState::Failed
                }
            },
            Err(e) => {
                log::warn!("[update] Update check failed (ignored): {}", e);
                UpdateCheckState::Failed
            }
        };
        let _ = tx.send(state);
        ctx.request_repaint();
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_well_formed_tags() {
        assert_eq!(parse_version("v0.6.0"), Some((0, 6, 0)));
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
    }

    #[test]
    fn rejects_malformed_tags() {
        assert_eq!(parse_version("not-a-version"), None);
        assert_eq!(parse_version("v1.2"), None);
        assert_eq!(parse_version(""), None);
    }

    #[test]
    fn newer_version_compares_correctly() {
        assert!(parse_version("v0.7.0") > parse_version("v0.6.0"));
        assert!(parse_version("v1.0.0") > parse_version("v0.99.99"));
        assert!(!(parse_version("v0.6.0") > parse_version("v0.6.0")));
    }
}
