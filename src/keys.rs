//! The secrets: a token for each server, and the key pair of the relay.
//!
//! - A server has its own token (`TOKEN` in its `.env`). The relay does not keep the token, only its SHA-256, so the relay
//!   (its repository, its settings, its storage) cannot be used to act as a server.
//! - The relay signs what it sends to the servers with an Ed25519 key. A server knows only the public key (`RELAY_PUBLIC_KEY`),
//!   so a server cannot be used to act as the relay either.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};

/// 256 bits of randomness, as 43 characters.
pub fn new_token() -> String {
    URL_SAFE_NO_PAD.encode(random_32())
}

/// What the relay keeps of a token: the SHA-256 in lower case hex. (`printf %s "$TOKEN" | shasum -a 256` gives the same.)
pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// A new key pair of the relay: (private seed, public key), both as base64url.
pub fn new_relay_keys() -> (String, String) {
    let key = SigningKey::from_bytes(&random_32());
    (
        URL_SAFE_NO_PAD.encode(key.to_bytes()),
        URL_SAFE_NO_PAD.encode(key.verifying_key().to_bytes()),
    )
}

fn random_32() -> [u8; 32] {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("the system has no source of randomness");
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_long_and_different() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 43);
        assert_ne!(a, b);
    }

    #[test]
    fn the_hash_is_the_plain_sha256() {
        assert_eq!(
            token_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn relay_keys_are_a_pair() {
        let (private, public) = new_relay_keys();
        let seed: [u8; 32] = URL_SAFE_NO_PAD.decode(private).unwrap().try_into().unwrap();
        let expected = SigningKey::from_bytes(&seed).verifying_key().to_bytes();
        assert_eq!(URL_SAFE_NO_PAD.decode(public).unwrap(), expected);
    }
}
