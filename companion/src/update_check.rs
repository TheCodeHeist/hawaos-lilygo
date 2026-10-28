//! Tells the user when a newer HawaOS release is out (DESIGN.md Section 16).
//! Shortly after start and then twice a day, unless turned off, it asks
//! GitHub for the latest release; nothing about the user or the device is
//! sent. The window and the tray show what was found.

use std::thread;
use std::time::Duration;

use crate::models::SharedState;

/// The release page the notice links to.
pub const RELEASES_URL: &str = "https://github.com/TheCodeHeist/hawaos-lilygo/releases/latest";
const LATEST_API: &str = "https://api.github.com/repos/TheCodeHeist/hawaos-lilygo/releases/latest";

pub fn spawn(state: SharedState) {
    thread::spawn(move || {
        // Not in the way of startup, the device link and the first frames.
        let delay = if std::env::var_os("HAWAOS_DEV_LATEST").is_some() { 2 } else { 30 };
        thread::sleep(Duration::from_secs(delay));
        loop {
            let enabled = state.lock().is_ok_and(|g| g.config.check_updates);
            if enabled {
                if let Some(latest) = latest_version() {
                    if let Ok(mut g) = state.lock() {
                        g.update_available = newer(&latest, env!("CARGO_PKG_VERSION")).then_some(latest);
                    }
                }
            }
            thread::sleep(Duration::from_secs(12 * 60 * 60));
        }
    });
}

/// The latest published release's version (drafts and pre-releases aren't
/// "latest" on GitHub).
fn latest_version() -> Option<String> {
    // Dev aid for screenshots: HAWAOS_DEV_LATEST=9.9.9 pretends it's out.
    if let Ok(v) = std::env::var("HAWAOS_DEV_LATEST") {
        return Some(v);
    }
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().into();
    let body = agent
        .get(LATEST_API)
        // GitHub's API wants a User-Agent.
        .header("User-Agent", concat!("hawaos-companion/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/vnd.github+json")
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;
    let json: serde_json::Value = serde_json::from_str(&body).ok()?;
    let tag = json.get("tag_name")?.as_str()?;
    Some(tag.trim_start_matches('v').to_string())
}

fn parse(v: &str) -> Option<(u32, u32, u32)> {
    let mut parts = v.trim().trim_start_matches('v').splitn(3, '.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    // "1.2.3-beta" counts as 1.2.3.
    let patch = parts.next()?.split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()?;
    Some((major, minor, patch))
}

/// `candidate` is a later version than `current`.
pub fn newer(candidate: &str, current: &str) -> bool {
    matches!((parse(candidate), parse(current)), (Some(a), Some(b)) if a > b)
}

/// How to update this kind of install, for the notice.
pub fn how_to_update() -> &'static str {
    if cfg!(windows) {
        "Download and run the new installer from the release page."
    } else if std::env::var_os("FLATPAK_ID").is_some() {
        "Run the install command again, or install the new .flatpak from the release page."
    } else if std::env::current_exe().is_ok_and(|p| p.starts_with("/usr/")) {
        // The .deb or .rpm.
        "Run the install command again, or install the new package from the release page."
    } else {
        "See the release page for what's new."
    }
}

#[cfg(test)]
mod tests {
    use super::newer;

    #[test]
    fn compares_versions() {
        assert!(newer("0.9.0", "0.8.0"));
        assert!(newer("v1.0.0", "0.9.3"));
        assert!(newer("0.10.0", "0.9.0"), "numeric, not text");
        assert!(!newer("0.8.0", "0.8.0"));
        assert!(!newer("0.7.1", "0.8.0"));
        assert!(!newer("garbage", "0.8.0"));
    }
}
