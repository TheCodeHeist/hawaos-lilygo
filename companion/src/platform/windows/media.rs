//! Media sessions from the System Media Transport Controls: what Windows
//! shows in its own media flyout (Spotify, browsers, the Media Player app).

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;
use windows::core::HSTRING;

use super::winrt_init;
use crate::platform::{Art, MediaCommand, Player};

/// The last cover read, by track: thumbnails are streams, read once.
static COVER: Mutex<Option<(String, Arc<Vec<u8>>)>> = Mutex::new(None);

fn manager() -> Option<Manager> {
    winrt_init();
    Manager::RequestAsync().ok()?.join().ok()
}

fn sessions() -> Vec<Session> {
    manager()
        .and_then(|m| m.GetSessions().ok())
        .map(|list| list.into_iter().collect())
        .unwrap_or_default()
}

/// 100 ns ticks since 1601 (Windows' DateTime), now.
fn now_ticks() -> i64 {
    const UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
    let since_unix = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    UNIX_EPOCH_TICKS + (since_unix.as_nanos() / 100) as i64
}

fn player(session: &Session) -> Option<Player> {
    let name = session.SourceAppUserModelId().ok()?.to_string();
    let status = match session.GetPlaybackInfo().ok()?.PlaybackStatus().ok()? {
        Status::Playing => "playing",
        Status::Paused => "paused",
        _ => "stopped",
    };
    let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
    let title = props.Title().map(|s| s.to_string()).unwrap_or_default();
    let artist = props.Artist().map(|s| s.to_string()).unwrap_or_default();
    let album = props.AlbumTitle().map(|s| s.to_string()).unwrap_or_default();

    // Players report their position now and then; while playing, add the
    // time since that report.
    let (mut pos, dur) = match session.GetTimelineProperties() {
        Ok(t) => {
            let start = t.StartTime().map(|d| d.Duration).unwrap_or(0);
            let pos = t.Position().map(|d| d.Duration).unwrap_or(0) - start;
            let end = t.EndTime().map(|d| d.Duration).unwrap_or(0) - start;
            if status == "playing" {
                let updated = t.LastUpdatedTime().map(|d| d.UniversalTime).unwrap_or(0);
                if updated > 0 {
                    let pos = pos + (now_ticks() - updated).max(0);
                    (pos, end)
                } else {
                    (pos, end)
                }
            } else {
                (pos, end)
            }
        }
        Err(_) => (0, 0),
    };
    if dur > 0 {
        pos = pos.min(dur);
    }

    let key = format!("{name}\u{1f}{title}\u{1f}{artist}\u{1f}{album}");
    let art = cover(&key, || {
        let stream = props.Thumbnail().ok()?.OpenReadAsync().ok()?.join().ok()?;
        let size = u32::try_from(stream.Size().ok()?).ok()?;
        if size == 0 {
            return None;
        }
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).ok()?).ok()?;
        reader.LoadAsync(size).ok()?.join().ok()?;
        let mut bytes = vec![0u8; size as usize];
        reader.ReadBytes(&mut bytes).ok()?;
        Some(bytes)
    });

    Some(Player {
        name,
        status: status.to_string(),
        title,
        artist,
        album,
        pos_us: (pos.max(0) / 10) as u64,
        dur_us: (dur.max(0) / 10) as u64,
        art,
    })
}

/// The track's cover, read only when the track changes.
fn cover(key: &str, read: impl FnOnce() -> Option<Vec<u8>>) -> Art {
    let mut last = COVER.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((k, bytes)) = last.as_ref() {
        if k == key {
            return Art::Image { key: key.to_string(), bytes: Arc::clone(bytes) };
        }
    }
    match read() {
        Some(bytes) => {
            let bytes = Arc::new(bytes);
            *last = Some((key.to_string(), Arc::clone(&bytes)));
            Art::Image { key: key.to_string(), bytes }
        }
        None => Art::None,
    }
}

/// Every media session Windows knows about.
pub fn players() -> Vec<Player> {
    sessions().iter().filter_map(player).collect()
}

pub fn media_command(cmd: MediaCommand, player: Option<&str>) {
    let all = sessions();
    let target = player
        .and_then(|name| {
            let name = HSTRING::from(name);
            all.iter().find(|s| s.SourceAppUserModelId().is_ok_and(|id| id == name)).cloned()
        })
        .or_else(|| manager().and_then(|m| m.GetCurrentSession().ok()));
    let Some(session) = target else { return };
    let _ = match cmd {
        MediaCommand::PlayPause => session.TryTogglePlayPauseAsync(),
        MediaCommand::Next => session.TrySkipNextAsync(),
        MediaCommand::Previous => session.TrySkipPreviousAsync(),
    };
}
