//! What differs between the desktops the companion runs on (DESIGN.md
//! Section 16): media players, notifications, autostart, the Wi-Fi scan for
//! setup, finding the board's serial port, and where settings live. Each
//! backend provides the same functions; the rest of the companion only calls
//! these.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

/// A media player as the desktop reports it.
#[derive(Clone, Debug, Default)]
pub struct Player {
    /// Identifies the player for media_command().
    pub name: String,
    /// "playing", "paused" or "stopped".
    pub status: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub pos_us: u64,
    pub dur_us: u64,
    pub art: Art,
}

/// Where a player's cover comes from.
#[derive(Clone, Debug, Default)]
pub enum Art {
    #[default]
    None,
    /// `file://` or `http(s)://` (MPRIS).
    #[cfg_attr(windows, allow(dead_code))]
    Url(String),
    /// The image itself (Windows hands over a stream); `key` names this
    /// cover so it's decoded once.
    #[cfg_attr(not(windows), allow(dead_code))]
    Image { key: String, bytes: std::sync::Arc<Vec<u8>> },
}

impl Art {
    /// Names the cover for the art cache: the same cover, the same key.
    pub fn key(&self) -> &str {
        match self {
            Art::None => "",
            Art::Url(u) => u,
            Art::Image { key, .. } => key,
        }
    }
}

/// Media keys from the device or the tray.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaCommand {
    PlayPause,
    Next,
    Previous,
}

impl MediaCommand {
    /// From the device's `cmd` field.
    pub fn from_device(cmd: &str) -> Option<Self> {
        match cmd {
            "play_pause" => Some(Self::PlayPause),
            "next" => Some(Self::Next),
            "prev" => Some(Self::Previous),
            _ => None,
        }
    }
}

/// The player to show: the first one playing, else the first paused one
/// with a title, else the first.
pub fn active_player(players: Vec<Player>) -> Option<Player> {
    let pick = players
        .iter()
        .position(|p| p.status == "playing")
        .or_else(|| players.iter().position(|p| p.status == "paused" && !p.title.is_empty()))
        .or(if players.is_empty() { None } else { Some(0) })?;
    players.into_iter().nth(pick)
}

/// The settings file, `config.json`, in the platform's per-user config folder.
pub fn config_file_path() -> std::path::PathBuf {
    config_dir().join("config.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str, status: &str, title: &str) -> Player {
        Player { name: name.into(), status: status.into(), title: title.into(), ..Default::default() }
    }

    #[test]
    fn picks_the_player_to_show() {
        let chosen = |v: Vec<Player>| active_player(v).map(|p| p.name);
        assert_eq!(chosen(vec![p("a", "paused", "x"), p("b", "playing", "y")]).as_deref(), Some("b"));
        assert_eq!(chosen(vec![p("a", "paused", ""), p("b", "paused", "y")]).as_deref(), Some("b"));
        assert_eq!(chosen(vec![p("a", "stopped", "")]).as_deref(), Some("a"));
        assert_eq!(chosen(vec![]), None);
    }
}
