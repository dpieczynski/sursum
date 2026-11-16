// SPDX-License-Identifier: GPL-3.0-or-later
//! Size limits for uploaded archives. A ZIP entry of a few kilobytes can unpack to gigabytes (a
//! "ZIP bomb"); on a Raspberry Pi that ends in the out-of-memory killer in the middle of a Mass.
//! Every entry is therefore read through a hard cap, and so is the archive as a whole.

use std::io::Read;

use anyhow::ensure;

/// Largest uncompressed size of one archive entry we read (a song file or `songs.jsonl`).
pub const ENTRY_BYTES: u64 = 64 * 1024 * 1024;
/// Largest total uncompressed size of all entries of one upload.
pub const ARCHIVE_BYTES: u64 = 256 * 1024 * 1024;
/// Most entries one upload may have (a parish library has a few thousand songs).
pub const ARCHIVE_ENTRIES: usize = 50_000;

/// Reads at most `limit` bytes; more than that is an error instead of an allocation.
pub fn read_capped(reader: impl Read, limit: u64, what: &str) -> anyhow::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "{what} is larger than {} MB unpacked", limit / 1024 / 1024);
    Ok(bytes)
}

/// Counts what an upload has unpacked so far, against [`ARCHIVE_BYTES`].
#[derive(Debug, Default)]
pub struct Budget {
    used: u64,
}

impl Budget {
    /// Reads one entry within both the per-entry limit and what is left of the total.
    pub fn read(&mut self, reader: impl Read, what: &str) -> anyhow::Result<Vec<u8>> {
        let left = ARCHIVE_BYTES.saturating_sub(self.used);
        let bytes = read_capped(reader, ENTRY_BYTES.min(left), what)?;
        self.used += bytes.len() as u64;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reading_stops_at_the_limit() {
        assert_eq!(read_capped(&b"12345"[..], 5, "x").unwrap(), b"12345");
        assert!(read_capped(&b"123456"[..], 5, "x").is_err());
        let endless = std::io::repeat(0);
        assert!(read_capped(endless, 1024, "zeros").is_err(), "an endless stream is cut off");
    }
}
