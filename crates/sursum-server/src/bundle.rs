// SPDX-License-Identifier: GPL-3.0-or-later
//! Signed application bundles (`.sursum-app`, ADR 0013 and its amendment): how a new sursum
//! version reaches a device that has a read-only system and no internet.
//!
//! A bundle is a ZIP with three entries:
//! - `manifest.json`: format, version, build id and the SHA-256 of the binary;
//! - `manifest.sig`: the Ed25519 signature of exactly those manifest bytes, in hex;
//! - `sursum-server`: the binary (web apps embedded).
//!
//! Only bundles signed with the key whose public half is compiled into the running binary are
//! installed. Installing writes `releases/<version>/sursum-server` in the data directory and
//! switches the `current` symlink atomically, remembering the old target as `previous`, so
//! [`rollback`] is one rename. Restarting the service and checking `/healthz` belong to the
//! device's systemd units (M4).

use std::io::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, ensure};
use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// Version of the bundle format.
const FORMAT: u32 = 1;
/// Largest binary accepted (the server with embedded apps is far smaller).
const MAX_BINARY_BYTES: u64 = 200 * 1024 * 1024;
const MAX_MANIFEST_BYTES: u64 = 64 * 1024;
const BINARY: &str = "sursum-server";

/// What a bundle says about itself (signed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    /// Release name, also the folder under `releases/`, e.g. `0.2.0` or `0.2.0+g1a2b3c`.
    pub version: String,
    pub build_id: String,
    pub binary_sha256: String,
}

/// A bundle whose signature and binary hash have been checked.
#[derive(Debug)]
pub struct Verified {
    pub manifest: Manifest,
    binary: Vec<u8>,
}

/// The public key bundles must be signed with, compiled in from `SURSUM_BUNDLE_KEY` (hex) at
/// build time. Builds without it (development) accept no bundles at all.
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

/// Builds a signed bundle around a server binary.
pub fn create(binary: &[u8], version: &str, build_id: &str, key: &SigningKey) -> anyhow::Result<Vec<u8>> {
    ensure!(valid_version(version), "invalid version {version:?} (letters, digits, '.', '+', '-')");
    let manifest = Manifest {
        format: FORMAT,
        version: version.to_owned(),
        build_id: build_id.to_owned(),
        binary_sha256: hex(&Sha256::digest(binary)),
    };
    let manifest = serde_json::to_vec_pretty(&manifest)?;
    let signature = key.sign(&manifest);
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut zip = zip::ZipWriter::new(&mut buffer);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("manifest.json", options)?;
    zip.write_all(&manifest)?;
    zip.start_file("manifest.sig", options)?;
    zip.write_all(hex(&signature.to_bytes()).as_bytes())?;
    zip.start_file(BINARY, options.unix_permissions(0o755))?;
    zip.write_all(binary)?;
    zip.finish()?;
    Ok(buffer.into_inner())
}

/// Checks the signature (with `trusted`) and the binary's hash; reads nothing unbounded.
pub fn verify(bytes: &[u8], trusted: &VerifyingKey) -> anyhow::Result<Verified> {
    use crate::limits::read_capped;
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).context("not a .sursum-app file")?;
    let manifest_bytes = read_capped(
        archive.by_name("manifest.json").context("no manifest")?,
        MAX_MANIFEST_BYTES,
        "manifest",
    )?;
    let signature_hex =
        read_capped(archive.by_name("manifest.sig").context("no signature")?, 1024, "signature")?;
    let signature = std::str::from_utf8(&signature_hex)
        .ok()
        .and_then(|text| from_hex::<64>(text.trim()))
        .context("malformed signature")?;
    trusted
        .verify_strict(&manifest_bytes, &Signature::from_bytes(&signature))
        .context("the bundle is not signed with this device's key")?;
    // Only now, with a genuine manifest, is anything else read.
    let manifest: Manifest = serde_json::from_slice(&manifest_bytes)?;
    ensure!(manifest.format == FORMAT, "unsupported bundle format {}", manifest.format);
    ensure!(valid_version(&manifest.version), "invalid version in the manifest");
    let binary = read_capped(archive.by_name(BINARY).context("no binary")?, MAX_BINARY_BYTES, "binary")?;
    ensure!(
        hex(&Sha256::digest(&binary)) == manifest.binary_sha256,
        "the binary does not match the manifest"
    );
    Ok(Verified { manifest, binary })
}

/// Writes the release and makes it `current`; returns the release folder.
#[cfg(unix)]
pub fn install(verified: &Verified, data_dir: &Path) -> anyhow::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;
    let release = data_dir.join("releases").join(&verified.manifest.version);
    std::fs::create_dir_all(&release)?;
    let target = release.join(BINARY);
    let temporary = release.join(format!(".{BINARY}.{}.part", uuid::Uuid::new_v4()));
    let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
    file.write_all(&verified.binary)?;
    file.set_permissions(std::fs::Permissions::from_mode(0o755))?;
    file.sync_all()?;
    std::fs::rename(&temporary, &target)?;
    std::fs::File::open(&release)?.sync_all()?;
    switch_current(data_dir, &PathBuf::from("releases").join(&verified.manifest.version))?;
    Ok(release)
}

/// Points `current` back to `previous` (and `previous` to what was current).
#[cfg(unix)]
pub fn rollback(data_dir: &Path) -> anyhow::Result<PathBuf> {
    let previous = std::fs::read_link(data_dir.join("previous")).context("there is no previous release")?;
    switch_current(data_dir, &previous)?;
    Ok(previous)
}

/// Replaces the `current` symlink atomically (a new link renamed over the old one) and keeps the
/// old target as `previous`. Links are relative, so the data directory can move.
#[cfg(unix)]
fn switch_current(data_dir: &Path, target: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::symlink;
    let current = data_dir.join("current");
    let old = std::fs::read_link(&current).ok();
    if old.as_deref() == Some(target) {
        return Ok(());
    }
    let replace = |name: &str, target: &Path| -> anyhow::Result<()> {
        let staged = data_dir.join(format!(".{name}.new"));
        let _ = std::fs::remove_file(&staged);
        symlink(target, &staged)?;
        std::fs::rename(&staged, data_dir.join(name))?;
        Ok(())
    };
    if let Some(old) = &old {
        replace("previous", old)?;
    }
    replace("current", target)?;
    std::fs::File::open(data_dir)?.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
pub fn install(_: &Verified, _: &Path) -> anyhow::Result<PathBuf> {
    anyhow::bail!("installing bundles is only supported on the Linux device")
}

#[cfg(not(unix))]
pub fn rollback(_: &Path) -> anyhow::Result<PathBuf> {
    anyhow::bail!("installing bundles is only supported on the Linux device")
}

/// Versions become folder names: no separators, no leading dot.
fn valid_version(version: &str) -> bool {
    (1..=64).contains(&version.len())
        && !version.starts_with('.')
        && version.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'+' | b'-'))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
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

    fn key() -> SigningKey {
        signing_key(&new_key().unwrap().0).unwrap()
    }

    #[test]
    fn a_signed_bundle_verifies_and_anything_else_does_not() {
        let key = key();
        let bundle = create(b"binary v2", "0.2.0", "abc", &key).unwrap();
        let verified = verify(&bundle, &key.verifying_key()).unwrap();
        assert_eq!(verified.manifest.version, "0.2.0");

        let stranger = self::key();
        let error = verify(&bundle, &stranger.verifying_key()).unwrap_err().to_string();
        assert!(error.contains("not signed"), "{error}");
        assert!(verify(b"not a zip", &key.verifying_key()).is_err());
        assert!(create(b"x", "../evil", "abc", &key).is_err());
    }

    #[test]
    fn a_swapped_binary_is_refused() {
        let key = key();
        let genuine = create(b"good binary", "0.2.0", "abc", &key).unwrap();
        // Take the genuine signed manifest, but put another binary next to it.
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&genuine)).unwrap();
        let read = |archive: &mut zip::ZipArchive<_>, name: &str| {
            crate::limits::read_capped(archive.by_name(name).unwrap(), 1 << 20, name).unwrap()
        };
        let (manifest, signature) = (read(&mut archive, "manifest.json"), read(&mut archive, "manifest.sig"));
        let mut buffer = std::io::Cursor::new(Vec::new());
        let mut zip = zip::ZipWriter::new(&mut buffer);
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in
            [("manifest.json", &manifest[..]), ("manifest.sig", &signature[..]), (BINARY, b"evil")]
        {
            zip.start_file(name, options).unwrap();
            zip.write_all(content).unwrap();
        }
        zip.finish().unwrap();
        let error = verify(&buffer.into_inner(), &key.verifying_key()).unwrap_err().to_string();
        assert!(error.contains("does not match"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn installing_switches_current_and_rollback_switches_back() {
        let dir = tempfile::tempdir().unwrap();
        let key = key();
        for (version, binary) in [("0.1.0", &b"one"[..]), ("0.2.0", &b"two"[..])] {
            let verified =
                verify(&create(binary, version, "b", &key).unwrap(), &key.verifying_key()).unwrap();
            install(&verified, dir.path()).unwrap();
        }
        let current = dir.path().join("current").join(BINARY);
        assert_eq!(std::fs::read(&current).unwrap(), b"two");
        assert_eq!(std::fs::read_link(dir.path().join("previous")).unwrap(), Path::new("releases/0.1.0"));
        rollback(dir.path()).unwrap();
        assert_eq!(std::fs::read(&current).unwrap(), b"one");
        assert_eq!(std::fs::read_link(dir.path().join("previous")).unwrap(), Path::new("releases/0.2.0"));
    }
}
