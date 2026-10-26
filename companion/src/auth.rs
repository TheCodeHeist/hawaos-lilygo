//! Encryption for network traffic with the device (DESIGN.md Section 11.6):
//! AES-256-GCM, one key per direction, both derived from the token handed to
//! the device at setup.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::aead::{Aead, Generate, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};

/// Last `n` this process sent; keeps `n` strictly increasing even when two
/// messages go out in the same millisecond.
static LAST_N: AtomicU64 = AtomicU64::new(0);
/// Newest (echo, counter) accepted from the device.
static LAST_IN: Mutex<(u64, u64)> = Mutex::new((0, 0));
/// A device message must echo one of our `n` from at most this long ago.
/// Covers a large firmware download, during which the device can't read.
const FRESH_MS: u64 = 120_000;

fn token_bytes(token_hex: &str) -> Option<Vec<u8>> {
    (0..token_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(token_hex.get(i..i + 2)?, 16).ok())
        .collect()
}

fn cipher(token_hex: &str, label: &[u8]) -> Option<Aes256Gcm> {
    let mut h = Sha256::new();
    h.update(token_bytes(token_hex)?);
    h.update(label);
    Aes256Gcm::new_from_slice(&h.finalize()).ok()
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// HMAC-SHA256(token, msg) in hex, or None if the token isn't valid hex.
pub fn mac(token_hex: &str, msg: &[u8]) -> Option<String> {
    let mut m = Hmac::<Sha256>::new_from_slice(&token_bytes(token_hex)?).ok()?;
    m.update(msg);
    Some(hex(&m.finalize().into_bytes()))
}

/// Encrypts `body` (a JSON message) as `{"type":"sealed","n","iv","ct"}`.
/// `n` is Unix milliseconds, authenticated as associated data; the device
/// refuses any `n` it has already seen.
pub fn seal(token_hex: &str, body: &str) -> Option<String> {
    let now = now_ms();
    let n = LAST_N
        .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |last| Some(now.max(last + 1)))
        .map(|last| now.max(last + 1))
        .ok()?;
    seal_with(token_hex, body, n)
}

fn seal_with(token_hex: &str, body: &str, n: u64) -> Option<String> {
    let c = cipher(token_hex, b"hawaos-link-c2d")?;
    let iv = Nonce::generate();
    let aad = n.to_string();
    let ct = c.encrypt(&iv, Payload { msg: body.as_bytes(), aad: aad.as_bytes() }).ok()?;
    Some(
        serde_json::json!({"type": "sealed", "n": n, "iv": B64.encode(iv.as_slice()), "ct": B64.encode(ct)})
            .to_string(),
    )
}

/// Encrypts a binary blob for the device as `iv || ciphertext || tag`, bound
/// to `aad` (album art uses `"art:<hash>"`, so an image can't be swapped onto
/// another track).
pub fn seal_blob(token_hex: &str, aad: &str, data: &[u8]) -> Option<Vec<u8>> {
    let c = cipher(token_hex, b"hawaos-link-c2d")?;
    let iv = Nonce::generate();
    let ct = c.encrypt(&iv, Payload { msg: data, aad: aad.as_bytes() }).ok()?;
    let mut out = Vec::with_capacity(12 + ct.len());
    out.extend_from_slice(iv.as_slice());
    out.extend_from_slice(&ct);
    Some(out)
}

/// Decrypts a device envelope `{"type":"sealed","e","d","iv","ct"}`. `e`
/// echoes our newest `n` the device accepted and `d` counts its messages;
/// the pair must be recent and newer than any seen, so replays fail.
pub fn open_from_device(token_hex: &str, env: &serde_json::Value) -> Option<String> {
    let e = env["e"].as_u64()?;
    let d = env["d"].as_u64()?;
    let iv: [u8; 12] = B64.decode(env["iv"].as_str()?).ok()?.try_into().ok()?;
    let ct = B64.decode(env["ct"].as_str()?).ok()?;
    let c = cipher(token_hex, b"hawaos-link-d2c")?;
    let aad = format!("{e}:{d}");
    let pt = c.decrypt(&Nonce::from(iv), Payload { msg: &ct, aad: aad.as_bytes() }).ok()?;
    if now_ms().saturating_sub(e) > FRESH_MS {
        return None;
    }
    let mut last = LAST_IN.lock().ok()?;
    if (e, d) <= *last {
        return None;
    }
    *last = (e, d);
    String::from_utf8(pt).ok()
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOKEN: &str = "000102030405060708090a0b0c0d0e0f";

    fn open_c2d(env: &serde_json::Value) -> Option<String> {
        let iv: [u8; 12] = B64.decode(env["iv"].as_str()?).ok()?.try_into().ok()?;
        let aad = env["n"].as_u64()?.to_string();
        let pt = cipher(TOKEN, b"hawaos-link-c2d")?
            .decrypt(&Nonce::from(iv), Payload { msg: &B64.decode(env["ct"].as_str()?).ok()?, aad: aad.as_bytes() })
            .ok()?;
        String::from_utf8(pt).ok()
    }

    fn device_env(e: u64, d: u64, body: &str) -> serde_json::Value {
        let iv = Nonce::generate();
        let aad = format!("{e}:{d}");
        let ct = cipher(TOKEN, b"hawaos-link-d2c")
            .unwrap()
            .encrypt(&iv, Payload { msg: body.as_bytes(), aad: aad.as_bytes() })
            .unwrap();
        serde_json::json!({"type": "sealed", "e": e, "d": d, "iv": B64.encode(iv.as_slice()), "ct": B64.encode(ct)})
    }

    #[test]
    fn seal_round_trips_and_hides_body() {
        let s = seal(TOKEN, r#"{"type":"notify","body":"secret"}"#).unwrap();
        assert!(!s.contains("secret"));
        let env: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(open_c2d(&env).unwrap(), r#"{"type":"notify","body":"secret"}"#);
    }

    #[test]
    fn n_is_authenticated() {
        let mut env: serde_json::Value = serde_json::from_str(&seal_with(TOKEN, "{}", 5).unwrap()).unwrap();
        env["n"] = 6.into();
        assert!(open_c2d(&env).is_none());
    }

    #[test]
    fn n_strictly_increases() {
        let a: serde_json::Value = serde_json::from_str(&seal(TOKEN, "{}").unwrap()).unwrap();
        let b: serde_json::Value = serde_json::from_str(&seal(TOKEN, "{}").unwrap()).unwrap();
        assert!(b["n"].as_u64() > a["n"].as_u64());
    }

    #[test]
    fn art_blob_is_bound_to_its_hash() {
        let art = vec![7u8; 32768];
        let blob = seal_blob(TOKEN, "art:abc", &art).unwrap();
        assert_eq!(blob.len(), 12 + 32768 + 16);
        let c = cipher(TOKEN, b"hawaos-link-c2d").unwrap();
        let iv: [u8; 12] = blob[..12].try_into().unwrap();
        let open = |aad: &str| c.decrypt(&Nonce::from(iv), Payload { msg: &blob[12..], aad: aad.as_bytes() }).ok();
        assert_eq!(open("art:abc").unwrap(), art);
        assert!(open("art:abd").is_none());
    }

    #[test]
    fn device_messages_must_be_fresh_and_new() {
        let e = now_ms();
        let first = device_env(e, 7, r#"{"type":"cmd"}"#);
        assert_eq!(open_from_device(TOKEN, &first).unwrap(), r#"{"type":"cmd"}"#);
        assert!(open_from_device(TOKEN, &first).is_none(), "replay");
        assert!(open_from_device(TOKEN, &device_env(e, 8, "{}")).is_some());
        assert!(open_from_device(TOKEN, &device_env(e - FRESH_MS - 1, 99, "{}")).is_none(), "stale echo");
        let mut tampered = device_env(e + 1, 1, "{}");
        tampered["d"] = 2.into();
        assert!(open_from_device(TOKEN, &tampered).is_none(), "aad");
    }
}
