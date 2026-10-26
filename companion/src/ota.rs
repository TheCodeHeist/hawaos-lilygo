//! Firmware updates (DESIGN.md Section 11.5). The image is staged in memory
//! and served by the art HTTP server at `/fw/<id>.bin`; the device downloads
//! it, checks the SHA-256 and switches app slots.

use std::sync::{Arc, Mutex};

use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

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
    let key: Vec<u8> = (0..token_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(token_hex.get(i..i + 2)?, 16).ok())
        .collect::<Option<_>>()?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).ok()?;
    mac.update(format!("{sha256}:{size}").as_bytes());
    Some(hex(&mac.finalize().into_bytes()))
}

/// Fresh 128-bit token for a newly set-up device.
pub fn new_token() -> String {
    let mut b = [0u8; 16];
    getrandom::fill(&mut b).expect("getrandom");
    hex(&b)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
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
}
