// SPDX-License-Identifier: GPL-3.0-or-later
//! Uploaded pictures (announcement posters), stored by content hash in `<data>/assets/`.
//!
//! The id is `<sha256>.<ext>`: the same picture uploaded twice is stored once, an id can never
//! point at different bytes, and the screen may cache it forever. Only PNG, JPEG and WebP are
//! accepted, recognised by their first bytes (not by the file name), so the screen never gets a
//! file it cannot draw.

use std::io::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use sursum_protocol::AssetId;

/// Largest accepted picture. Posters are photos or exported slides; this is plenty for 1080p.
pub const MAX_ASSET_BYTES: usize = 25 * 1024 * 1024;
/// Most space all pictures together may take, so uploads can never fill the data partition (a
/// full disk would stop the database from saving the program position).
pub const MAX_TOTAL_BYTES: u64 = 2 * 1024 * 1024 * 1024;

const FOLDER: &str = "assets";
/// An upload's temporary file older than this was left behind by a crash.
const STALE_PART: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Why an upload was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AssetError {
    #[error("not a PNG, JPEG or WebP picture")]
    UnsupportedType,
    #[error("the picture is larger than {} MB", MAX_ASSET_BYTES / 1024 / 1024)]
    TooLarge,
    #[error("no room for more pictures ({} GB in use)", MAX_TOTAL_BYTES / 1024 / 1024 / 1024)]
    Full,
    #[error("could not store the picture: {0}")]
    Storage(String),
}

/// A picture type the screen can draw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Png,
    Jpeg,
    Webp,
}

impl Kind {
    fn sniff(bytes: &[u8]) -> Option<Kind> {
        if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Kind::Png)
        } else if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
            Some(Kind::Jpeg)
        } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            Some(Kind::Webp)
        } else {
            None
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Kind::Png => "png",
            Kind::Jpeg => "jpg",
            Kind::Webp => "webp",
        }
    }

    fn from_extension(extension: &str) -> Option<Kind> {
        match extension {
            "png" => Some(Kind::Png),
            "jpg" => Some(Kind::Jpeg),
            "webp" => Some(Kind::Webp),
            _ => None,
        }
    }

    /// The `Content-Type` to serve.
    fn mime(self) -> &'static str {
        match self {
            Kind::Png => "image/png",
            Kind::Jpeg => "image/jpeg",
            Kind::Webp => "image/webp",
        }
    }
}

/// Stores a picture and returns its id. It is written to a temporary file of its own (two
/// uploads of the same picture at once must not truncate each other's file), synced, renamed, and
/// the folder is synced, so a power cut never leaves half a file under a valid id.
pub fn store(data_dir: &Path, bytes: &[u8]) -> Result<AssetId, AssetError> {
    store_within(data_dir, bytes, MAX_TOTAL_BYTES)
}

fn store_within(data_dir: &Path, bytes: &[u8], quota: u64) -> Result<AssetId, AssetError> {
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(AssetError::TooLarge);
    }
    let kind = Kind::sniff(bytes).ok_or(AssetError::UnsupportedType)?;
    let hash: String = Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect();
    let id = AssetId(format!("{hash}.{}", kind.extension()));
    let folder = data_dir.join(FOLDER);
    let path = folder.join(id.as_str());
    if path.exists() {
        return Ok(id);
    }
    let storage = |error: std::io::Error| AssetError::Storage(error.to_string());
    std::fs::create_dir_all(&folder).map_err(storage)?;
    if used_bytes(&folder).map_err(storage)? + bytes.len() as u64 > quota {
        return Err(AssetError::Full);
    }
    let temporary = folder.join(format!(".{}.{}.part", id.as_str(), uuid::Uuid::new_v4()));
    let written = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, &path)?;
        std::fs::File::open(&folder)?.sync_all()
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written.map_err(storage)?;
    Ok(id)
}

/// Bytes taken by stored pictures. Temporary files of uploads interrupted by a crash or a
/// power cut (older than [`STALE_PART`]) are removed on the way, so they never eat the quota.
fn used_bytes(folder: &Path) -> std::io::Result<u64> {
    let mut total = 0;
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        let stale = entry.file_name().to_string_lossy().ends_with(".part")
            && metadata
                .modified()
                .ok()
                .and_then(|time| time.elapsed().ok())
                .is_some_and(|age| age > STALE_PART);
        if stale {
            let _ = std::fs::remove_file(entry.path());
        } else {
            total += metadata.len();
        }
    }
    Ok(total)
}

/// The file and content type of a stored picture; `None` for ids that are not well-formed (so a
/// request can never escape the folder) or not stored.
pub fn locate(data_dir: &Path, id: &str) -> Option<(PathBuf, &'static str)> {
    let (hash, extension) = id.split_once('.')?;
    let well_formed = hash.len() == 64 && hash.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
    let kind = Kind::from_extension(extension).filter(|_| well_formed)?;
    let path = data_dir.join(FOLDER).join(id);
    path.is_file().then_some((path, kind.mime()))
}

/// Every stored picture as (id, file), sorted by id; unfinished uploads are left out.
pub fn list(data_dir: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    let entries = match std::fs::read_dir(data_dir.join(FOLDER)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut pictures = Vec::new();
    for entry in entries {
        let name = entry?.file_name().to_string_lossy().into_owned();
        if let Some((path, _)) = locate(data_dir, &name) {
            pictures.push((name, path));
        }
    }
    pictures.sort();
    Ok(pictures)
}

/// True if a picture with this id is stored.
pub fn exists(data_dir: &Path, id: &AssetId) -> bool {
    locate(data_dir, id.as_str()).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot really a picture";

    #[test]
    fn pictures_are_stored_once_by_content_and_found_again() {
        let dir = tempfile::tempdir().unwrap();
        let id = store(dir.path(), PNG).unwrap();
        assert!(id.as_str().ends_with(".png") && id.as_str().len() == 64 + 4);
        assert_eq!(store(dir.path(), PNG).unwrap(), id, "same bytes, same id");
        let (path, mime) = locate(dir.path(), id.as_str()).unwrap();
        assert_eq!((std::fs::read(path).unwrap().as_slice(), mime), (PNG, "image/png"));
        assert!(exists(dir.path(), &id));
    }

    #[test]
    fn only_pictures_are_accepted_and_ids_cannot_escape() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(store(dir.path(), b"<svg/>"), Err(AssetError::UnsupportedType));
        assert_eq!(store(dir.path(), &vec![0xff; MAX_ASSET_BYTES + 1]), Err(AssetError::TooLarge));
        assert!(locate(dir.path(), "../sursum.db").is_none());
        assert!(locate(dir.path(), &format!("{}.png", "a".repeat(64))).is_none(), "not stored");
        assert!(locate(dir.path(), &format!("{}.exe", "a".repeat(64))).is_none());
        assert_eq!(store_within(dir.path(), b"\x89PNG\r\n\x1a\nanother", 10), Err(AssetError::Full));
        let webp = b"RIFF\x10\x00\x00\x00WEBPVP8 ";
        assert!(store(dir.path(), webp).unwrap().as_str().ends_with(".webp"));
        assert!(store(dir.path(), &[0xff, 0xd8, 0xff, 0xe0]).unwrap().as_str().ends_with(".jpg"));
    }
}
