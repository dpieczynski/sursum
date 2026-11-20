// SPDX-License-Identifier: GPL-3.0-or-later
//! The update key (ADR 0019, ADR 0022): system updates are installed only if they are signed with
//! the Ed25519 key whose public half was compiled into the running server. The secret half lives
//! in the release environment's CI secret and on the maintainer's computer, never in the
//! repository. The names `SURSUM_BUNDLE_KEY` and `SURSUM_BUNDLE_SIGNING_KEY` come from the
//! application bundles this key first signed (removed with ADR 0022); the key itself stayed.

use anyhow::Context as _;
use ed25519_dalek::{SigningKey, VerifyingKey};

/// The public key updates must be signed with, compiled in from `SURSUM_BUNDLE_KEY` (hex) at
/// build time. Builds without it (development, the desktop program) accept no updates at all.
pub fn trusted_key() -> Option<VerifyingKey> {
    let hex = option_env!("SURSUM_BUNDLE_KEY")?;
    VerifyingKey::from_bytes(&from_hex::<32>(hex)?).ok()
}

/// A new signing key: the secret seed (hex, keep it outside the repository) and the public key
/// (hex, for `SURSUM_BUNDLE_KEY`).
pub fn new_key() -> anyhow::Result<(String, String)> {
    let mut seed = [0_u8; 32];
    getrandom::fill(&mut seed).map_err(|error| anyhow::anyhow!("no system randomness: {error}"))?;
    let key = SigningKey::from_bytes(&seed);
    Ok((hex(&seed), hex(&key.verifying_key().to_bytes())))
}

/// Reads a signing key written by [`new_key`].
pub fn signing_key(seed_hex: &str) -> anyhow::Result<SigningKey> {
    let seed = from_hex::<32>(seed_hex.trim()).context("the key file must hold 64 hex characters")?;
    Ok(SigningKey::from_bytes(&seed))
}

/// Versions name files and appear in the remote: no separators, no leading dot.
pub(crate) fn valid_version(version: &str) -> bool {
    (1..=64).contains(&version.len())
        && !version.starts_with('.')
        && version.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn from_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    if text.len() != N * 2 {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_key_reads_back_and_versions_cannot_name_other_folders() {
        let (secret, public) = new_key().unwrap();
        let key = signing_key(&format!("{secret}\n")).unwrap();
        assert_eq!(hex(&key.verifying_key().to_bytes()), public);
        assert!(signing_key("not hex").is_err());
        assert!(valid_version("v0.2.0-3-g1a2b3c4-dirty") && valid_version("0.2.0+local"));
        assert!(!valid_version("../evil") && !valid_version(".hidden") && !valid_version(""));
    }
}
