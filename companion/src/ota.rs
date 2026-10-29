//! Firmware updates (DESIGN.md Section 11.5). The image is staged in memory
//! and served by the art HTTP server at `/fw/<id>.bin`; the device downloads
//! it, checks the SHA-256 and switches app slots.
//!
//! The image is a file the user picks, or the firmware from this companion's
//! own GitHub release, downloaded and checked against the release's
//! SHA256SUMS: companion and firmware share version numbers.

use std::io::Read;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::auth::hex;
use crate::models::{OtaProgress, OutboundAction, SharedState};

const RELEASE_DOWNLOADS: &str = "https://github.com/TheCodeHeist/hawaos-lilygo/releases/download";

/// ESP32 app images start with this magic byte.
const ESP_IMAGE_MAGIC: u8 = 0xE9;
/// Larger than any app slot in `default_16MB.csv` (6.25 MiB).
const MAX_IMAGE_BYTES: usize = 0x640000;

pub struct Staged {
    pub id: String,
    pub bytes: Arc<Vec<u8>>,
}

static STAGED: Mutex<Option<Staged>> = Mutex::new(None);

/// Checks the file looks like HawaOS firmware and stages it for download.
pub fn stage(bytes: Vec<u8>) -> Result<(String, String, usize), String> {
    if bytes.first() != Some(&ESP_IMAGE_MAGIC) {
        return Err("That file isn't an ESP32 firmware image (firmware.bin).".into());
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("That image is too large for the device.".into());
    }
    let sha256 = hex(&Sha256::digest(&bytes));
    let id = sha256[..16].to_string();
    let size = bytes.len();
    *STAGED.lock().unwrap() = Some(Staged { id: id.clone(), bytes: Arc::new(bytes) });
    Ok((id, sha256, size))
}

/// Bytes for `GET /fw/<id>.bin`, if that image is staged.
pub fn served(path: &str) -> Option<Arc<Vec<u8>>> {
    let id = path.strip_prefix("/fw/")?.strip_suffix(".bin")?;
    let g = STAGED.lock().ok()?;
    g.as_ref().filter(|s| s.id == id).map(|s| Arc::clone(&s.bytes))
}

/// HMAC-SHA256(token, "<sha256>:<size>") in hex: the device accepts network
/// updates only with this, keyed by the token from setup.
pub fn sign(token_hex: &str, sha256: &str, size: usize) -> Option<String> {
    crate::auth::mac(token_hex, format!("{sha256}:{size}").as_bytes())
}

/// Fresh 128-bit token for a newly set-up device.
pub fn new_token() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("getrandom");
    hex(&b)
}

/// This companion's version, when the device runs older firmware: the update
/// the Firmware card offers.
pub fn release_for(device_fw: Option<&str>) -> Option<&'static str> {
    let ours = env!("CARGO_PKG_VERSION");
    device_fw.filter(|fw| crate::update_check::newer(ours, fw)).map(|_| ours)
}

/// Downloads the firmware of release `version` in the background, then
/// installs it like a picked file. The card shows "fetching" meanwhile.
pub fn install_release(state: SharedState, version: &'static str) {
    if let Ok(mut g) = state.lock() {
        g.ota = Some(OtaProgress::new("fetching", 0, None));
    }
    thread::spawn(move || {
        let fetching = |g: &crate::models::SharedDaemonState| g.ota.as_ref().is_some_and(|o| o.state == "fetching");
        let res = download_release(version, |pct| {
            if let Ok(mut g) = state.lock() {
                if fetching(&g) {
                    g.ota = Some(OtaProgress::new("fetching", pct, None));
                }
            }
        });
        let Ok(mut g) = state.lock() else { return };
        // The card gave up on a stalled download; don't start one now.
        if !fetching(&g) {
            return;
        }
        match res {
            Ok(path) => {
                g.push_log(format!("Downloaded firmware {version} from GitHub"));
                g.ota = Some(OtaProgress::new("starting", 0, None));
                g.outbound_queue.push_back(OutboundAction::StartOta { path });
            }
            Err(e) => g.ota = Some(OtaProgress::new("failed", 0, Some(e))),
        }
    });
}

/// `hawaos-<version>-update.bin` from that release, checked against its
/// SHA256SUMS and saved to a temporary file.
fn download_release(version: &str, progress: impl Fn(u8)) -> Result<PathBuf, String> {
    let agent: ureq::Agent =
        ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(120))).build().into();
    let ua = concat!("hawaos-companion/", env!("CARGO_PKG_VERSION"));
    let base = format!("{RELEASE_DOWNLOADS}/v{version}");
    let name = format!("hawaos-{version}-update.bin");
    let offline = |e: ureq::Error| match e {
        ureq::Error::StatusCode(404) => format!("Release {version} has no firmware update to download."),
        e => format!("Couldn't download the update from GitHub: {e}"),
    };

    let sums = agent
        .get(format!("{base}/SHA256SUMS"))
        .header("User-Agent", ua)
        .call()
        .map_err(offline)?
        .body_mut()
        .read_to_string()
        .map_err(offline)?;
    let want = expected_sha256(&sums, &name)
        .ok_or_else(|| format!("Release {version} has no firmware update to download."))?;

    let mut resp = agent.get(format!("{base}/{name}")).header("User-Agent", ua).call().map_err(offline)?;
    let total = resp.body().content_length().unwrap_or(0) as usize;
    let mut reader = resp.body_mut().with_config().limit(MAX_IMAGE_BYTES as u64 + 1).reader();
    let mut bytes = Vec::with_capacity(total);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf).map_err(|e| format!("Couldn't download the update from GitHub: {e}"))?;
        if n == 0 {
            break;
        }
        bytes.extend_from_slice(&buf[..n]);
        if total > 0 {
            progress((bytes.len() * 100 / total).min(100) as u8);
        }
    }
    if hex(&Sha256::digest(&bytes)) != want {
        return Err("The download didn't match the release's checksum. Try again.".into());
    }
    let path = std::env::temp_dir().join(&name);
    std::fs::write(&path, &bytes).map_err(|e| format!("Couldn't save the update: {e}"))?;
    Ok(path)
}

/// The checksum `sha256sum` wrote for `file` (`<hex>  <name>` lines).
fn expected_sha256(sums: &str, file: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (sha, name) = line.trim().split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file && sha.len() == 64).then(|| sha.to_ascii_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_matches_firmware_recipe() {
        // Python: hmac.new(bytes.fromhex("00"*16), b"ab:3", sha256).hexdigest()
        assert_eq!(
            sign(&"00".repeat(16), "ab", 3).unwrap(),
            "b91e19ff32484abdcb3e224b1413ad060d89e089dffd75bc839f1bd16fe6a67f"
        );
    }

    #[test]
    fn rejects_non_firmware() {
        assert!(stage(b"hello".to_vec()).is_err());
        let (id, sha, size) = stage(vec![0xE9, 1, 2, 3]).unwrap();
        assert_eq!(size, 4);
        assert!(served(&format!("/fw/{id}.bin")).is_some());
        assert!(served("/fw/nope.bin").is_none());
        assert_eq!(sha.len(), 64);
    }

    #[test]
    fn reads_release_checksums() {
        let a = "a".repeat(64);
        let sums = format!("{a}  hawaos-1.0.0-full.bin\n{}  hawaos-1.0.0-update.bin\n", "B".repeat(64));
        assert_eq!(expected_sha256(&sums, "hawaos-1.0.0-update.bin"), Some("b".repeat(64)));
        assert_eq!(expected_sha256(&sums, "hawaos-1.0.0-full.bin"), Some(a));
        assert_eq!(expected_sha256(&sums, "other.bin"), None);
    }

    #[test]
    fn offers_this_release_to_older_devices() {
        let ours = env!("CARGO_PKG_VERSION");
        assert_eq!(release_for(Some("0.0.1")), Some(ours));
        assert_eq!(release_for(Some(ours)), None);
        assert_eq!(release_for(Some("999.0.0")), None);
        assert_eq!(release_for(None), None);
    }
}
