// SPDX-License-Identifier: GPL-3.0-or-later
//! Sets as versioned documents (ADR 0025): every change the organist makes saves a version, so
//! "more changes" counts their actions, and a set travels between devices like a song. A set's
//! body is the set without its id, as JSON with sorted keys.

use anyhow::Context as _;
use sursum_core::model::ServiceSet;
use sursum_core::versions::{Heads, Version};
use sursum_protocol::SetId;

use super::Database;
use super::versions::{Kind, canonical_json};

/// The stored form of a set: without its id, keys sorted.
pub fn set_body(set: &ServiceSet) -> anyhow::Result<Vec<u8>> {
    let mut value = serde_json::to_value(set)?;
    if let serde_json::Value::Object(map) = &mut value {
        map.remove("id");
    }
    canonical_json(&value)
}

/// A set from its stored form.
pub fn set_from_body(id: &SetId, body: &[u8]) -> anyhow::Result<ServiceSet> {
    let mut value: serde_json::Value = serde_json::from_slice(body).context("a set body is not JSON")?;
    let map = value.as_object_mut().context("a set body is not a JSON object")?;
    map.insert("id".to_owned(), serde_json::Value::String(id.as_str().to_owned()));
    serde_json::from_value(value).context("decoding a set")
}

impl Database {
    /// Every live set (of every church) in its current version, in the order this device came to
    /// know them (oldest first; the list puts sets without a date in reverse of this order).
    pub fn load_sets(&self) -> anyhow::Result<Vec<ServiceSet>> {
        let mut known: std::collections::HashMap<String, i64> = std::collections::HashMap::new();
        let mut statement = self.connection.prepare_cached(
            "SELECT document_id, min(rowid) FROM versions WHERE kind = 'set' GROUP BY document_id",
        )?;
        for row in statement.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))? {
            let (id, first) = row?;
            known.insert(id, first);
        }
        let mut sets = Vec::new();
        for (id, heads) in self.all_heads(Kind::Set)? {
            if let Some(body) = heads.current().and_then(Version::live_body) {
                sets.push(set_from_body(&SetId(id), body)?);
            }
        }
        sets.sort_by_key(|set| known.get(set.id.as_str()).copied().unwrap_or(i64::MAX));
        Ok(sets)
    }

    /// Saves a set as a new version on top of the version shown now (a set the device has never
    /// had becomes its first version). Nothing is written when the content did not change.
    pub fn save_set(&mut self, set: &ServiceSet) -> anyhow::Result<()> {
        let body = set_body(set)?;
        let heads = self.heads(Kind::Set, set.id.as_str())?;
        let version = match heads.current() {
            None => Version::first(body),
            Some(current) if current.live_body() == Some(body.as_slice()) => return Ok(()),
            Some(current) => {
                heads.save(&current.id().clone(), body).context("the current version is a head")?
            }
        };
        self.store_heads(Kind::Set, set.id.as_str(), &heads, &heads.with(version))
    }

    /// Deletes a set (a version replacing all of its heads); undoing saves it again.
    pub fn delete_set(&self, id: &str) -> anyhow::Result<()> {
        let heads = self.heads(Kind::Set, id)?;
        if let Some(deletion) = heads.delete() {
            self.store_heads(Kind::Set, id, &heads, &heads.with(deletion))?;
        }
        Ok(())
    }

    /// Every set with its heads, deleted ones included, for library files.
    pub fn set_documents(&self) -> anyhow::Result<Vec<(SetId, Heads)>> {
        Ok(self.all_heads(Kind::Set)?.into_iter().map(|(id, heads)| (SetId(id), heads)).collect())
    }

    /// Every picture a live version of any set uses: of every church (this device keeps and
    /// passes on other churches' sets) and of every head, also one waiting for review.
    pub fn pictures_in_sets(&self) -> anyhow::Result<std::collections::HashSet<sursum_protocol::AssetId>> {
        let mut pictures = std::collections::HashSet::new();
        for (id, heads) in self.set_documents()? {
            for body in heads.versions().iter().filter_map(Version::live_body) {
                let set = set_from_body(&id, body)?;
                pictures.extend(set.items.into_iter().filter_map(|item| match item.kind {
                    sursum_core::model::ItemKind::Image { asset_id, .. } => Some(asset_id),
                    _ => None,
                }));
            }
        }
        Ok(pictures)
    }

    /// The heads of one set (empty if unknown).
    pub fn set_heads(&self, id: &SetId) -> anyhow::Result<Heads> {
        self.heads(Kind::Set, id.as_str())
    }

    /// Sets changed in two places that nobody has reviewed yet (two live heads, different bodies).
    pub fn sets_in_review(&self) -> anyhow::Result<Vec<SetId>> {
        let documents = self.all_heads(Kind::Set)?;
        Ok(documents.into_iter().filter(|(_, heads)| heads.needs_review()).map(|(id, _)| SetId(id)).collect())
    }

    /// Ends a review: `set` becomes a version replacing every head (deleted ones included).
    pub fn resolve_set(&self, set: &ServiceSet) -> anyhow::Result<()> {
        let heads = self.heads(Kind::Set, set.id.as_str())?;
        let resolved = heads.resolve(set_body(set)?);
        self.store_heads(Kind::Set, set.id.as_str(), &heads, &heads.with(resolved))
    }

    /// Records a set's heads after a merge.
    pub fn store_set_heads(&self, id: &SetId, before: &Heads, after: &Heads) -> anyhow::Result<()> {
        self.store_heads(Kind::Set, id.as_str(), before, after)
    }
}
