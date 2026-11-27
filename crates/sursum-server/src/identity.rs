// SPDX-License-Identifier: GPL-3.0-or-later
//! Who this server is, so a paired tablet keeps its pairing when the device gets a new address
//! (ADR 0028).
//!
//! The pairing cookie belongs to the host the remote was loaded from. Where a tablet cannot
//! resolve `sursum.local`, that host is an IP address, and DHCP may hand the device another one.
//! The Android launcher then finds the device again by DNS-SD and moves its cookie to the new
//! address, but only after two steps:
//!
//! 1. [`ServerId`]: a random identity created at the first start (`GET /api/identity`). It is
//!    public, a cheap filter that says "this is probably the device you were paired with".
//! 2. A proof (`POST /api/identity/proof`): the server shows that it holds the tablet's token
//!    hash by answering an HMAC over the launcher's nonce and the address the request reached.
//!    A host that only copied the identity cannot answer, and one that relays the question to
//!    the real device gets an answer bound to the real device's address, which the launcher
//!    refuses for any other address.
//!
//! The identity is treated like the PIN: it stays with the device. A device backup does not
//! carry it and restoring one keeps this device's identity, just as it keeps the PIN and the
//! paired remotes ([`crate::db::Database::adopt_credentials`]).

use std::net::SocketAddr;

use sha2::{Digest, Sha256};
use sursum_protocol::ServerId;

use crate::db::Database;

/// Settings key of the server's identity.
pub const SERVER_ID_SETTING: &str = "server_id";
/// Prefix of the message a pairing key is hashed from; names the purpose, so the key can never
/// equal another hash of the token hash.
const KEY_PREFIX: &str = "sursum-pairing-key\n";
/// Prefix of the message a proof signs.
const PROOF_PREFIX: &str = "sursum-pairing-proof\n";
/// SHA-256's block size, for HMAC.
const BLOCK: usize = 64;

/// The stored identity, or a new one stored now (the first start of this build on a device).
pub fn load_or_create(database: &Database) -> anyhow::Result<ServerId> {
    if let Some(id) = database.setting::<ServerId>(SERVER_ID_SETTING)? {
        return Ok(id);
    }
    let id = ServerId(format!("server-{}", uuid::Uuid::new_v4()));
    database.set_setting(SERVER_ID_SETTING, &id)?;
    Ok(id)
}

/// The public name of a pairing: hex SHA-256 of [`KEY_PREFIX`] and the stored token hash. The
/// launcher computes the same from its cookie, so it can say which pairing it means without
/// sending anything that lets a listener prove it.
pub fn pairing_key(token_hash: &str) -> String {
    hex(&Sha256::digest(format!("{KEY_PREFIX}{token_hash}").as_bytes()))
}

/// A pairing key as the launcher sends it: 64 lowercase hex characters.
pub fn valid_key(key: &str) -> bool {
    key.len() == 64 && is_lower_hex(key)
}

/// A nonce as the launcher sends it: 32 to 128 lowercase hex characters (at least 128 bits).
pub fn valid_nonce(nonce: &str) -> bool {
    (32..=128).contains(&nonce.len()) && is_lower_hex(nonce)
}

/// The address a request reached, written the same way every time: an IPv4 client of a
/// dual-stack socket shows as `192.168.1.20:80`, not `[::ffff:192.168.1.20]:80`.
pub fn canonical_address(local: SocketAddr) -> String {
    SocketAddr::new(local.ip().to_canonical(), local.port()).to_string()
}

/// The proof that this server holds `token_hash`: hex HMAC-SHA256 keyed with the token hash
/// over the server's identity, the address the request reached and the launcher's nonce.
pub fn proof(token_hash: &str, server: &ServerId, address: &str, nonce: &str) -> String {
    let message = format!("{PROOF_PREFIX}{server}\n{address}\n{nonce}");
    hex(&hmac_sha256(token_hash.as_bytes(), message.as_bytes()))
}

/// HMAC-SHA256 (RFC 2104), written out because it is ten lines over the SHA-256 already used
/// here; checked against the RFC 4231 test vectors below.
fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    let mut block = [0_u8; BLOCK];
    if key.len() > BLOCK {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(block.map(|byte| byte ^ 0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(block.map(|byte| byte ^ 0x5c));
    outer.update(inner.finalize());
    let mut mac = [0_u8; 32];
    mac.copy_from_slice(&outer.finalize());
    mac
}

fn is_lower_hex(text: &str) -> bool {
    text.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc_4231() {
        let cases: [(&[u8], &[u8], &str); 3] = [
            (&[0x0b; 20], b"Hi There", "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"),
            (
                b"Jefe",
                b"what do ya want for nothing?",
                "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
            ),
            (
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First",
                "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54",
            ),
        ];
        for (key, message, expected) in cases {
            assert_eq!(hex(&hmac_sha256(key, message)), expected);
        }
    }

    #[test]
    fn the_identity_is_created_once_and_kept() {
        let database = Database::in_memory().unwrap();
        let first = load_or_create(&database).unwrap();
        assert!(first.as_str().starts_with("server-"));
        assert_eq!(load_or_create(&database).unwrap(), first);
        let other = load_or_create(&Database::in_memory().unwrap()).unwrap();
        assert_ne!(other, first, "every device gets its own");
    }

    /// The launcher computes the same values in Java (`android/…/PairingMove.java`); this
    /// example, also in `docs/protocol.md`, was computed independently (Python's `hmac`).
    #[test]
    fn keys_and_proofs_are_pinned() {
        let token_hash = crate::auth::token_hash("tablet-token");
        assert_eq!(token_hash, "6fe728b3f33f84b6bb0f253a8967fdd1253b1d330f954a6a7672f3ec8fa9cc9e");
        let key = pairing_key(&token_hash);
        assert!(valid_key(&key));
        assert_eq!(key, "7c7f22942f78fe11c0f517024c9ee6a573bc5d0f32a06b4c5d5eb4aa74275918");
        let server = ServerId::from("server-1");
        let nonce = "00112233445566778899aabbccddeeff";
        let proof = proof(&token_hash, &server, "192.168.1.20:80", nonce);
        assert_eq!(proof, "41b3ff232498bf28c6322eb82cbe888c1e35be462fc404e866c1b18817aeeec8");
        assert_ne!(
            proof,
            super::proof(&token_hash, &server, "192.168.1.21:80", nonce),
            "bound to the address"
        );
        assert_ne!(
            proof,
            super::proof(&token_hash, &server, "192.168.1.20:80", "ffeeddccbbaa99887766554433221100"),
            "bound to the nonce"
        );
        assert_ne!(proof, super::proof(&crate::auth::token_hash("other"), &server, "192.168.1.20:80", nonce));
    }

    #[test]
    fn addresses_are_written_one_way() {
        assert_eq!(canonical_address("192.168.1.20:80".parse().unwrap()), "192.168.1.20:80");
        assert_eq!(canonical_address("[::ffff:192.168.1.20]:18080".parse().unwrap()), "192.168.1.20:18080");
        assert_eq!(canonical_address("[fd00::5]:80".parse().unwrap()), "[fd00::5]:80");
    }

    #[test]
    fn keys_and_nonces_must_be_lowercase_hex_of_the_right_length() {
        assert!(valid_nonce(&"a".repeat(32)) && valid_nonce(&"0".repeat(128)));
        assert!(!valid_nonce(&"a".repeat(31)) && !valid_nonce(&"a".repeat(129)));
        assert!(!valid_nonce(&"A".repeat(32)) && !valid_nonce(&"g".repeat(32)));
        assert!(valid_key(&"f".repeat(64)) && !valid_key(&"f".repeat(63)) && !valid_key(&"F".repeat(64)));
    }
}
