//! Synced lyrics for the playing song (DESIGN.md 9.6), from LRCLIB
//! (lrclib.net: free, no account). The song's title, artist, album and length
//! are the only things sent, and only while the Lyrics setting is on.
//!
//! The worker asks for the line at the song's position several times a
//! second and sends the device a `lyric` message when it changes.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const API: &str = "https://lrclib.net/api";

/// Timed lines, in order.
#[derive(Debug, PartialEq)]
pub struct Lyrics {
    lines: Vec<(u32, String)>,
}

impl Lyrics {
    /// Parses LRC: `[mm:ss.xx] text` lines, a line may carry several times;
    /// tags like `[ar:...]` are skipped. None without any timed line.
    pub fn parse_lrc(text: &str) -> Option<Self> {
        let mut lines = Vec::new();
        for raw in text.lines() {
            let mut rest = raw.trim();
            let mut times = Vec::new();
            while let Some(after) = rest.strip_prefix('[') {
                let Some(end) = after.find(']') else { break };
                match parse_time(&after[..end]) {
                    Some(ms) => times.push(ms),
                    None if times.is_empty() => break, // a tag line
                    None => {}
                }
                rest = &after[end + 1..];
            }
            for ms in times {
                lines.push((ms, rest.trim().to_string()));
            }
        }
        lines.sort_by_key(|(ms, _)| *ms);
        (!lines.is_empty()).then_some(Self { lines })
    }

    /// The line sung at `pos_ms` ("" before the first and in gaps).
    pub fn line_at(&self, pos_ms: u32) -> &str {
        match self.lines.partition_point(|(ms, _)| *ms <= pos_ms) {
            0 => "",
            i => &self.lines[i - 1].1,
        }
    }
}

/// `mm:ss`, `mm:ss.xx` or `mm:ss.xxx`.
fn parse_time(t: &str) -> Option<u32> {
    let (min, sec) = t.split_once(':')?;
    let min: u32 = min.trim().parse().ok()?;
    let (s, frac) = sec.split_once('.').unwrap_or((sec, "0"));
    let s: u32 = s.trim().parse().ok()?;
    if s >= 60 || frac.is_empty() || !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let frac_ms = match frac.len() {
        1 => frac.parse::<u32>().ok()? * 100,
        2 => frac.parse::<u32>().ok()? * 10,
        _ => frac[..3].parse::<u32>().ok()?,
    };
    Some(min * 60_000 + s * 1000 + frac_ms)
}

/// Identifies a song for the cache.
fn song_key(title: &str, artist: &str, dur_ms: u32) -> String {
    format!("{}\u{1f}{}\u{1f}{}", title.to_lowercase(), artist.to_lowercase(), dur_ms / 1000)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(10))).build().into()
}

fn get_json(req: ureq::RequestBuilder<ureq::typestate::WithoutBody>) -> Option<serde_json::Value> {
    let body = req
        .header("User-Agent", concat!("hawaos-companion/", env!("CARGO_PKG_VERSION"), " (https://github.com/TheCodeHeist/hawaos-lilygo)"))
        .call()
        .ok()?
        .body_mut()
        .read_to_string()
        .ok()?;
    serde_json::from_str(&body).ok()
}

fn synced(v: &serde_json::Value) -> Option<Lyrics> {
    Lyrics::parse_lrc(v.get("syncedLyrics")?.as_str()?)
}

/// Asks LRCLIB: the exact song first (title, artist, album, length within
/// 2 s), then a search, taking a result whose length is close.
fn fetch(title: &str, artist: &str, album: &str, dur_ms: u32) -> Option<Lyrics> {
    let agent = agent();
    let secs = (dur_ms / 1000).to_string();
    let mut exact = agent
        .get(format!("{API}/get"))
        .query("track_name", title)
        .query("artist_name", artist);
    if !album.is_empty() {
        exact = exact.query("album_name", album);
    }
    if dur_ms > 0 {
        exact = exact.query("duration", &secs);
    }
    if let Some(found) = get_json(exact).as_ref().and_then(synced) {
        return Some(found);
    }
    let results = get_json(agent.get(format!("{API}/search")).query("track_name", title).query("artist_name", artist))?;
    results.as_array()?.iter().find_map(|r| {
        let len = r.get("duration").and_then(|d| d.as_f64()).unwrap_or(0.0);
        let close = dur_ms == 0 || (len * 1000.0 - dur_ms as f64).abs() <= 3000.0;
        if close { synced(r) } else { None }
    })
}

/// What's known about one song: still asking, or the answer (None: LRCLIB
/// has no synced lyrics for it).
#[derive(Clone)]
enum Entry {
    Pending,
    Done(Option<Arc<Lyrics>>),
}

/// Lyrics of songs played this session, fetched in the background.
#[derive(Clone, Default)]
pub struct Library {
    songs: Arc<Mutex<HashMap<String, Entry>>>,
}

impl Library {
    /// The song's lyrics if they're here; otherwise starts looking them up
    /// and returns None for now.
    pub fn get(&self, title: &str, artist: &str, album: &str, dur_ms: u32) -> Option<Arc<Lyrics>> {
        if title.is_empty() {
            return None;
        }
        let key = song_key(title, artist, dur_ms);
        let mut songs = self.songs.lock().ok()?;
        match songs.get(&key) {
            Some(Entry::Done(found)) => return found.clone(),
            Some(Entry::Pending) => return None,
            None => {}
        }
        // A long session shouldn't grow this without end.
        if songs.len() > 200 {
            songs.clear();
        }
        songs.insert(key.clone(), Entry::Pending);
        drop(songs);

        let songs = Arc::clone(&self.songs);
        let (title, artist, album) = (title.to_string(), artist.to_string(), album.to_string());
        thread::spawn(move || {
            let found = fetch(&title, &artist, &album, dur_ms).map(Arc::new);
            eprintln!(
                "[Lyrics] {} for '{}' by '{}'",
                if found.is_some() { "Synced lyrics" } else { "No synced lyrics" },
                title,
                artist
            );
            if let Ok(mut songs) = songs.lock() {
                songs.insert(key, Entry::Done(found));
            }
        });
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc() {
        let l = Lyrics::parse_lrc(
            "[ar:Someone]\n[00:12.50] First line\n[00:15.00][01:02.3] Chorus\n[00:20.123]\nnot a line\n",
        )
        .unwrap();
        assert_eq!(l.line_at(0), "");
        assert_eq!(l.line_at(12_499), "");
        assert_eq!(l.line_at(12_500), "First line");
        assert_eq!(l.line_at(16_000), "Chorus");
        assert_eq!(l.line_at(20_200), "", "an empty timed line is a gap");
        assert_eq!(l.line_at(62_300), "Chorus", "a line can come back");
        assert!(Lyrics::parse_lrc("[ti:Only tags]\nplain text").is_none());
    }

    #[test]
    fn parses_times() {
        assert_eq!(parse_time("01:02.34"), Some(62_340));
        assert_eq!(parse_time("01:02.345"), Some(62_345));
        assert_eq!(parse_time("01:02"), Some(62_000));
        assert_eq!(parse_time("ar"), None);
        assert_eq!(parse_time("00:61.00"), None);
    }
}
