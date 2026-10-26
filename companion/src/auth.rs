//! Signing for messages that change the device over the network (DESIGN.md
//! Section 11.6). Keyed by the token handed to the device at setup.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Last `n` this process sent; keeps `n` strictly increasing even when two
/// messages go out in the same millisecond.
static LAST_N: AtomicU64 = AtomicU64::new(0);

/// HMAC-SHA256(token, msg) in hex, or None if the token isn't valid hex.
pub fn mac(token_hex: &str, msg: &[u8]) -> Option<String> {
    let key: Vec<u8> = (0..token_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(token_hex.get(i..i + 2)?, 16).ok())
        .collect::<Option<_>>()?;
    let mut m = Hmac::<Sha256>::new_from_slice(&key).ok()?;
    m.update(msg);
    Some(hex(&m.finalize().into_bytes()))
}

/// Wraps `body` (a JSON message) as `{"type":"signed","n","body","mac"}`.
/// `n` is Unix milliseconds, so it keeps increasing across restarts; the
/// device refuses any `n` it has already seen.
pub fn seal(token_hex: &str, body: &str) -> Option<String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_millis() as u64;
    let n = LAST_N
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |last| Some(now.max(last + 1)))
        .map(|last| now.max(last + 1))
        .ok()?;
    seal_with(token_hex, body, n)
}

fn seal_with(token_hex: &str, body: &str, n: u64) -> Option<String> {
    let mac = mac(token_hex, format!("{n}:{body}").as_bytes())?;
    Some(serde_json::json!({"type": "signed", "n": n, "body": body, "mac": mac}).to_string())
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_matches_firmware_recipe() {
        // Python: hmac.new(bytes.fromhex("00"*16), b'5:{"type":"config"}', sha256).hexdigest()
        let env: serde_json::Value =
            serde_json::from_str(&seal_with(&"00".repeat(16), r#"{"type":"config"}"#, 5).unwrap()).unwrap();
        assert_eq!(env["type"], "signed");
        assert_eq!(env["n"], 5);
        assert_eq!(env["body"], r#"{"type":"config"}"#);
        assert_eq!(env["mac"], "1cf3989ceefd292d5eb8f1ccaac2dbb039c459c8cd3dcda00567c0daf288b934");
    }

    #[test]
    fn n_strictly_increases() {
        let a: serde_json::Value = serde_json::from_str(&seal(&"00".repeat(16), "{}").unwrap()).unwrap();
        let b: serde_json::Value = serde_json::from_str(&seal(&"00".repeat(16), "{}").unwrap()).unwrap();
        assert!(b["n"].as_u64() > a["n"].as_u64());
    }
}
