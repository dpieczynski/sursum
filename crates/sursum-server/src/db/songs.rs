// SPDX-License-Identifier: GPL-3.0-or-later
//! Songs as versioned documents (ADR 0025). Every save adds a version to the `versions` table;
//! the heads decide what is shown ([`Heads::current`]), so a merge never makes a second song.
//!
//! A song version's body is the song without its id, as JSON with its keys sorted, so the same
//! text gives the same bytes (and the same version id) on every device.

use anyhow::Context as _;
use rusqlite::params;
use sursum_core::model::Song;
use sursum_core::versions::{Body, Heads, Version, VersionId};
use sursum_protocol::SongId;

use super::Database;

/// What kind of document a row of the `versions` table belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Song,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Song => "song",
        }
    }
}

/// Which version a save builds on.
#[derive(Debug, Clone, Copy)]
pub enum Base<'a> {
    /// The version shown now (imports, the demo songs).
    Current,
    /// The version the editor loaded; refused if it was replaced meanwhile.
    Loaded(&'a VersionId),
}

/// What saving a song did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SongSave {
    /// A new song.
    Created(VersionId),
    /// A new version of an existing song.
    Changed(VersionId),
    /// The text is the same as the version it was based on; nothing was written.
    Unchanged(VersionId),
    /// The loaded version was replaced meanwhile (or the song is gone); nothing was written.
    Outdated,
}

/// The stored form of a song: without its id, keys sorted.
pub fn song_body(song: &Song) -> anyhow::Result<Vec<u8>> {
    let mut value = serde_json::to_value(song)?;
    if let serde_json::Value::Object(map) = &mut value {
        map.remove("id");
    }
    Ok(serde_json::to_vec(&sorted(value))?)
}

/// The same JSON with every object's keys in sorted order. Done here rather than left to
/// `serde_json`, whose key order depends on a cargo feature (`preserve_order`) that another
/// crate in the build may switch on (the WebDriver tests' `thirtyfour` does): the server, the
/// desktop program and the tests must write the same bytes for the same song.
fn sorted(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            serde_json::Value::Object(entries.into_iter().map(|(key, value)| (key, sorted(value))).collect())
        }
        serde_json::Value::Array(items) => serde_json::Value::Array(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

/// A song from its stored form.
pub fn song_from_body(id: &SongId, body: &[u8]) -> anyhow::Result<Song> {
    let mut value: serde_json::Value = serde_json::from_slice(body).context("a song body is not JSON")?;
    let map = value.as_object_mut().context("a song body is not a JSON object")?;
    map.insert("id".to_owned(), serde_json::Value::String(id.as_str().to_owned()));
    serde_json::from_value(value).context("decoding a song")
}

impl Database {
    /// The heads of one document (empty if it is unknown).
    pub(crate) fn heads(&self, kind: Kind, id: &str) -> anyhow::Result<Heads> {
        let mut statement = self.connection.prepare_cached(
            "SELECT version_id, body, ancestors FROM versions WHERE kind = ?1 AND document_id = ?2 AND head = 1",
        )?;
        let rows = statement.query_map(params![kind.as_str(), id], stored_version)?;
        let versions = rows.map(|row| row?.verified()).collect::<anyhow::Result<Vec<_>>>()?;
        Ok(Heads::of(versions))
    }

    /// The heads of every document of a kind, deleted ones included, ordered by id.
    pub(crate) fn all_heads(&self, kind: Kind) -> anyhow::Result<Vec<(String, Heads)>> {
        let mut statement = self.connection.prepare_cached(
            "SELECT document_id, version_id, body, ancestors FROM versions
             WHERE kind = ?1 AND head = 1 ORDER BY document_id",
        )?;
        let rows = statement
            .query_map([kind.as_str()], |row| Ok((row.get::<_, String>(0)?, stored_version_at(row, 1)?)))?;
        let mut documents: Vec<(String, Vec<Version>)> = Vec::new();
        for row in rows {
            let (id, stored) = row?;
            let version = stored.verified()?;
            match documents.last_mut() {
                Some((last, versions)) if *last == id => versions.push(version),
                _ => documents.push((id, vec![version])),
            }
        }
        Ok(documents.into_iter().map(|(id, versions)| (id, Heads::of(versions))).collect())
    }

    /// Records that a document's heads went from `before` to `after`: new versions are added,
    /// replaced ones stay as history.
    pub(crate) fn store_heads(
        &self,
        kind: Kind,
        id: &str,
        before: &Heads,
        after: &Heads,
    ) -> anyhow::Result<()> {
        store_heads(&self.connection, kind, id, before, after)
    }

    /// Every live song in its current version, sorted by title.
    pub fn load_songs(&self) -> anyhow::Result<Vec<Song>> {
        let mut songs = Vec::new();
        for (id, heads) in self.all_heads(Kind::Song)? {
            if let Some(body) = heads.current().and_then(Version::live_body) {
                songs.push(song_from_body(&SongId(id), body)?);
            }
        }
        songs.sort_by(|a, b| a.title().cmp(b.title()));
        Ok(songs)
    }

    /// Every song with its heads, deleted ones included (a deletion travels to other devices).
    pub fn song_documents(&self) -> anyhow::Result<Vec<(SongId, Heads)>> {
        Ok(self.all_heads(Kind::Song)?.into_iter().map(|(id, heads)| (SongId(id), heads)).collect())
    }

    /// The heads of one song (empty if this device has never known it).
    pub fn song_heads(&self, id: &SongId) -> anyhow::Result<Heads> {
        self.heads(Kind::Song, id.as_str())
    }

    /// Records a song's heads after a merge (`before` as read by [`Database::song_heads`]).
    pub fn store_song_heads(&self, id: &SongId, before: &Heads, after: &Heads) -> anyhow::Result<()> {
        self.store_heads(Kind::Song, id.as_str(), before, after)
    }

    /// The version a song shows now; `None` if there is no such live song.
    pub fn song_version(&self, id: &str) -> anyhow::Result<Option<VersionId>> {
        let heads = self.heads(Kind::Song, id)?;
        Ok(heads.current().filter(|current| current.is_live()).map(|current| current.id().clone()))
    }

    /// Saves a song as a new version built on `base`.
    pub fn save_song(&mut self, song: &Song, base: Base<'_>) -> anyhow::Result<SongSave> {
        let body = song_body(song)?;
        let id = song.id.as_str();
        let heads = self.heads(Kind::Song, id)?;
        let base = match base {
            Base::Current => heads.current().map(|current| current.id().clone()),
            Base::Loaded(loaded) if heads.get(loaded).is_some() => Some(loaded.clone()),
            Base::Loaded(_) => return Ok(SongSave::Outdated),
        };
        let (version, created) = match base {
            None => (Version::first(body), true),
            Some(base) => {
                let unchanged = heads.get(&base).and_then(Version::live_body) == Some(body.as_slice())
                    && heads.versions().iter().all(Version::is_live);
                if unchanged {
                    return Ok(SongSave::Unchanged(base));
                }
                let Some(version) = heads.save(&base, body) else { return Ok(SongSave::Outdated) };
                (version, false)
            }
        };
        let saved = version.id().clone();
        store_heads(&self.connection, Kind::Song, id, &heads, &heads.with(version))?;
        Ok(if created { SongSave::Created(saved) } else { SongSave::Changed(saved) })
    }

    /// Deletes a song: a version that replaces all of its heads. It keeps its history and comes
    /// back if someone saves it again (or a newer version arrives in a file). False if there is
    /// no such live song.
    pub fn delete_song(&self, id: &str) -> anyhow::Result<bool> {
        let heads = self.heads(Kind::Song, id)?;
        let Some(deletion) = heads.delete() else { return Ok(false) };
        self.store_heads(Kind::Song, id, &heads, &heads.with(deletion))?;
        Ok(true)
    }
}

/// Writes a change of heads in one savepoint: all of it, or nothing. A savepoint (not a
/// transaction) so it also works inside the one transaction of a merge or an import.
fn store_heads(
    connection: &rusqlite::Connection,
    kind: Kind,
    id: &str,
    before: &Heads,
    after: &Heads,
) -> anyhow::Result<()> {
    connection.execute_batch("SAVEPOINT store_heads")?;
    let written = write_heads(connection, kind, id, before, after);
    if written.is_err() {
        let _ = connection.execute_batch("ROLLBACK TO store_heads");
    }
    connection.execute_batch("RELEASE store_heads")?;
    written
}

fn write_heads(
    connection: &rusqlite::Connection,
    kind: Kind,
    id: &str,
    before: &Heads,
    after: &Heads,
) -> anyhow::Result<()> {
    let mut insert = connection.prepare_cached(
        "INSERT INTO versions (kind, document_id, version_id, body, ancestors, head)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)
         ON CONFLICT (kind, document_id, version_id) DO UPDATE SET head = 1",
    )?;
    for version in after.versions() {
        let ancestors: Vec<&str> = version.ancestors().iter().map(VersionId::as_str).collect();
        insert.execute(params![
            kind.as_str(),
            id,
            version.id().as_str(),
            version.live_body(),
            ancestors.join(" "),
        ])?;
    }
    let mut replaced = connection.prepare_cached(
        "UPDATE versions SET head = 0 WHERE kind = ?1 AND document_id = ?2 AND version_id = ?3",
    )?;
    for version in before.versions().iter().filter(|version| after.get(version.id()).is_none()) {
        replaced.execute(params![kind.as_str(), id, version.id().as_str()])?;
    }
    Ok(())
}

/// A row of the `versions` table before its id is checked.
struct StoredVersion {
    id: String,
    body: Option<Vec<u8>>,
    ancestors: String,
}

impl StoredVersion {
    /// The version, after checking that body and ancestors give the stored id.
    fn verified(self) -> anyhow::Result<Version> {
        let ancestors = self
            .ancestors
            .split_ascii_whitespace()
            .map(|id| VersionId::parse(id).context("a stored ancestor id is malformed"))
            .collect::<anyhow::Result<_>>()?;
        let body = self.body.map_or(Body::Deleted, Body::Live);
        let version = Version::new(body, ancestors);
        anyhow::ensure!(version.id().as_str() == self.id, "a stored version does not match its id");
        Ok(version)
    }
}

fn stored_version(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredVersion> {
    stored_version_at(row, 0)
}

fn stored_version_at(row: &rusqlite::Row<'_>, first: usize) -> rusqlite::Result<StoredVersion> {
    Ok(StoredVersion { id: row.get(first)?, body: row.get(first + 1)?, ancestors: row.get(first + 2)? })
}
