// SPDX-License-Identifier: GPL-3.0-or-later
//! The `versions` table (ADR 0025): every saved version of every document (songs, churches),
//! with its ancestors; `head` marks the versions nothing replaces. The kinds of documents keep
//! their own rules in their own modules; this one only stores and reads heads.

use anyhow::Context as _;
use rusqlite::params;
use sursum_core::versions::{Body, Heads, Version, VersionId};

use super::Database;

/// What kind of document a row of the `versions` table belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Song,
    Church,
}

impl Kind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Kind::Song => "song",
            Kind::Church => "church",
        }
    }
}

/// A body as stored: JSON with every object's keys in sorted order, so the same content gives
/// the same bytes (and the same version id) on every device and in every build.
pub(crate) fn canonical_json(value: &serde_json::Value) -> anyhow::Result<Vec<u8>> {
    Ok(serde_json::to_vec(&sorted(value.clone()))?)
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
pub(crate) struct StoredVersion {
    id: String,
    body: Option<Vec<u8>>,
    ancestors: String,
}

impl StoredVersion {
    /// The version, after checking that body and ancestors give the stored id.
    pub(crate) fn verified(self) -> anyhow::Result<Version> {
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

pub(crate) fn stored_version(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredVersion> {
    stored_version_at(row, 0)
}

fn stored_version_at(row: &rusqlite::Row<'_>, first: usize) -> rusqlite::Result<StoredVersion> {
    Ok(StoredVersion { id: row.get(first)?, body: row.get(first + 1)?, ancestors: row.get(first + 2)? })
}
