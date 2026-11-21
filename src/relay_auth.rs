//! Checks that a call comes from the relay: it carries an Ed25519 signature over the method, the address of this server, the time
//! and the SHA-256 of the body. Anyone else (another server included) cannot make one, and a call meant for another server, or an
//! old one, does not pass.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use ntex::web::HttpRequest;
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};

/// How far the time of the relay may be from ours (seconds).
pub const MAX_CLOCK_SKEW: u64 = 60;

pub const TIMESTAMP_HEADER: &str = "x-relay-timestamp";
pub const SIGNATURE_HEADER: &str = "x-relay-signature";

pub struct RelayVerifier {
    key: VerifyingKey,
    origin: String,
}

/// The bytes that are signed. `origin` is the address of the server the call is for, as the relay and the server both write it.
fn message(method: &str, origin: &str, timestamp: &str, body: &[u8]) -> Vec<u8> {
    format!(
        "sfs-relay v1\n{method}\n{origin}\n{timestamp}\n{}",
        hex::encode(Sha256::digest(body))
    )
    .into_bytes()
}

impl RelayVerifier {
    pub fn new(public_key: &str, origin: &str) -> Result<Self, String> {
        let bytes: [u8; 32] = URL_SAFE_NO_PAD
            .decode(public_key.trim())
            .ok()
            .and_then(|b| b.try_into().ok())
            .ok_or(
                "RELAY_PUBLIC_KEY is not a 32 byte key in base64url (make one with `sfs keygen`)",
            )?;
        let key = VerifyingKey::from_bytes(&bytes).map_err(|e| format!("RELAY_PUBLIC_KEY: {e}"))?;
        Ok(Self {
            key,
            origin: origin.to_owned(),
        })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    /// Is this call from the relay, meant for this server, and recent?
    pub fn verify(
        &self,
        method: &str,
        timestamp: &str,
        signature: &str,
        body: &[u8],
        now: u64,
    ) -> bool {
        let Ok(sent) = timestamp.parse::<u64>() else {
            return false;
        };
        if now.abs_diff(sent) > MAX_CLOCK_SKEW {
            return false;
        }
        let Some(signature) = URL_SAFE_NO_PAD
            .decode(signature)
            .ok()
            .and_then(|b| Signature::from_slice(&b).ok())
        else {
            return false;
        };
        self.key
            .verify(&message(method, &self.origin, timestamp, body), &signature)
            .is_ok()
    }

    pub fn verify_request(&self, req: &HttpRequest, body: &[u8]) -> bool {
        let header = |name: &str| req.headers().get(name).and_then(|v| v.to_str().ok());
        let (Some(timestamp), Some(signature)) =
            (header(TIMESTAMP_HEADER), header(SIGNATURE_HEADER))
        else {
            return false;
        };
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.verify(req.method().as_str(), timestamp, signature, body, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // made by the relay's own code path (Web Crypto, Ed25519) with the seed 0x07 x 32; see the relay's test
    const PUBLIC: &str = "6kpsY-KcUgq-9VB7Ey7F-ZVHdq6-vnuSQh7qaRRG0iw";
    const BODY: &[u8] = br#"{"origin":"https://b.example.com","files":[]}"#;
    const SIGNATURE: &str =
        "pkOO0fOehfDn1cbw9uVusxJiUXzuufYbyCC6GA4u9G5Yv2zFy0ntrfykkCDw5dUbp1xVj7Aopb1U0dAR2mwSAg";
    const NOW: u64 = 1_700_000_000;

    fn verifier(origin: &str) -> RelayVerifier {
        RelayVerifier::new(PUBLIC, origin).unwrap()
    }

    #[test]
    fn a_call_signed_by_the_relay_passes() {
        assert!(verifier("https://a.example.com").verify(
            "PUT",
            "1700000000",
            SIGNATURE,
            BODY,
            NOW
        ));
        assert!(verifier("https://a.example.com").verify(
            "PUT",
            "1700000000",
            SIGNATURE,
            BODY,
            NOW + MAX_CLOCK_SKEW
        ));
    }

    #[test]
    fn anything_else_does_not() {
        let v = verifier("https://a.example.com");
        assert!(
            !v.verify("DELETE", "1700000000", SIGNATURE, BODY, NOW),
            "other method"
        );
        assert!(
            !v.verify("PUT", "1700000000", SIGNATURE, b"{}", NOW),
            "other body"
        );
        assert!(
            !v.verify("PUT", "1700000001", SIGNATURE, BODY, NOW),
            "other time"
        );
        assert!(
            !v.verify(
                "PUT",
                "1700000000",
                SIGNATURE,
                BODY,
                NOW + MAX_CLOCK_SKEW + 1
            ),
            "too old"
        );
        assert!(
            !v.verify(
                "PUT",
                "1700000000",
                SIGNATURE,
                BODY,
                NOW - MAX_CLOCK_SKEW - 1
            ),
            "too new"
        );
        assert!(
            !verifier("https://c.example.com").verify("PUT", "1700000000", SIGNATURE, BODY, NOW),
            "meant for another server"
        );
        assert!(!v.verify("PUT", "soon", SIGNATURE, BODY, NOW));
        assert!(!v.verify("PUT", "1700000000", "", BODY, NOW));
        assert!(!v.verify("PUT", "1700000000", "AAAA", BODY, NOW));
    }

    #[test]
    fn a_bad_key_is_refused() {
        assert!(RelayVerifier::new("", "https://a.example.com").is_err());
        assert!(RelayVerifier::new("AAAA", "https://a.example.com").is_err());
        assert!(RelayVerifier::new("not base64!", "https://a.example.com").is_err());
    }
}
