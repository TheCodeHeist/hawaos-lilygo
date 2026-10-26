//! BLE pairing session, mirroring `SetupService` on the device: X25519 key
//! exchange, key = SHA-256(shared || label || 6-digit code), AES-256-GCM.
//! Only someone who can read the code off the device's screen derives the
//! same key, so WiFi credentials never cross the air in the clear.

use aes_gcm::aead::{Aead, Generate, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use sha2::{Digest, Sha256};
use x25519_dalek::{EphemeralSecret, PublicKey};

const KDF_LABEL: &[u8] = b"hawaos-setup-v1";
/// Plaintext the device expects in `setup_verify`.
pub const VERIFY_TEXT: &str = "hawaos-setup-v1";

/// Our half of the exchange, waiting for the device's public key.
pub struct KeyExchange {
    secret: EphemeralSecret,
    public_b64: String,
}

impl KeyExchange {
    pub fn new() -> Self {
        let secret = EphemeralSecret::random();
        let public_b64 = B64.encode(PublicKey::from(&secret).as_bytes());
        Self { secret, public_b64 }
    }

    pub fn public_b64(&self) -> &str {
        &self.public_b64
    }

    pub fn finish(self, device_pub_b64: &str, code: &str) -> Option<Session> {
        let bytes: [u8; 32] = B64.decode(device_pub_b64).ok()?.try_into().ok()?;
        let shared = self.secret.diffie_hellman(&PublicKey::from(bytes));
        Some(Session::from_shared(shared.as_bytes(), code))
    }
}

pub struct Session {
    cipher: Aes256Gcm,
}

impl Session {
    fn from_shared(shared: &[u8; 32], code: &str) -> Self {
        let key = derive_key(shared, code);
        let cipher = Aes256Gcm::new_from_slice(&key).expect("32-byte key");
        Self { cipher }
    }

    /// `{"nonce": b64, "ct": b64(ciphertext || tag)}` fields for a message.
    pub fn seal(&self, plaintext: &str) -> (String, String) {
        let nonce = Nonce::generate();
        let ct = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .expect("AES-GCM encryption cannot fail for small inputs");
        (B64.encode(nonce.as_slice()), B64.encode(ct))
    }

    #[cfg(test)]
    fn open(&self, nonce_b64: &str, ct_b64: &str) -> Option<String> {
        let nonce: [u8; 12] = B64.decode(nonce_b64).ok()?.try_into().ok()?;
        let pt = self
            .cipher
            .decrypt(&Nonce::from(nonce), B64.decode(ct_b64).ok()?.as_slice())
            .ok()?;
        String::from_utf8(pt).ok()
    }
}

fn derive_key(shared: &[u8; 32], code: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(shared);
    h.update(KDF_LABEL);
    h.update(code.as_bytes());
    h.finalize().into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_derivation_matches_firmware_recipe() {
        // SHA-256(0x00*32 || "hawaos-setup-v1" || "123456"), computed
        // independently with Python's hashlib.
        let key = derive_key(&[0u8; 32], "123456");
        let hex: String = key.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, "1842c24c759dcd641e9321849f3be4e7d624bf2fd38846b547f78b60ed676530");
    }

    #[test]
    fn both_sides_agree_only_with_the_same_code() {
        let a = KeyExchange::new();
        let b = KeyExchange::new();
        let (a_pub, b_pub) = (a.public_b64().to_string(), b.public_b64().to_string());
        let sa = a.finish(&b_pub, "482913").unwrap();
        let sb = b.finish(&a_pub, "482913").unwrap();
        let (n, ct) = sa.seal(VERIFY_TEXT);
        assert_eq!(sb.open(&n, &ct).as_deref(), Some(VERIFY_TEXT));

        let c = KeyExchange::new();
        let d = KeyExchange::new();
        let (c_pub, d_pub) = (c.public_b64().to_string(), d.public_b64().to_string());
        let sc = c.finish(&d_pub, "482913").unwrap();
        let sd = d.finish(&c_pub, "000000").unwrap();
        let (n, ct) = sc.seal(VERIFY_TEXT);
        assert_eq!(sd.open(&n, &ct), None);
    }
}
