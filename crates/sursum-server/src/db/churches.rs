// SPDX-License-Identifier: GPL-3.0-or-later
//! Churches as versioned documents (ADR 0025): a church can be renamed and the name travels in
//! library files like a song's text. Which church this device serves is a local setting.

use anyhow::Context as _;
use sursum_core::churches::church_id;
use sursum_core::versions::{Heads, Version};
use sursum_protocol::ChurchId;

use super::Database;
use super::versions::{Kind, canonical_json};

/// Settings key of the church this device serves (never in a library file).
const THIS_CHURCH: &str = "church";

/// A church as the remote shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Church {
    pub id: ChurchId,
    pub name: String,
}

/// The stored form of a church: `{"name": …}`.
fn church_body(name: &str) -> anyhow::Result<Vec<u8>> {
    canonical_json(&serde_json::json!({ "name": name }))
}

/// A church's name from its stored form; refuses anything without a name.
pub fn church_name(body: &[u8]) -> anyhow::Result<String> {
    let value: serde_json::Value = serde_json::from_slice(body).context("a church body is not JSON")?;
    let name = value.get("name").and_then(serde_json::Value::as_str).context("a church has no name")?;
    anyhow::ensure!(!name.trim().is_empty(), "a church has an empty name");
    Ok(name.to_owned())
}

impl Database {
    /// Every live church, by name.
    pub fn churches(&self) -> anyhow::Result<Vec<Church>> {
        let mut churches = Vec::new();
        for (id, heads) in self.all_heads(Kind::Church)? {
            if let Some(body) = heads.current().and_then(Version::live_body) {
                churches.push(Church { id: ChurchId(id), name: church_name(body)? });
            }
        }
        churches.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        Ok(churches)
    }

    /// Every church with its heads, for library files.
    pub fn church_documents(&self) -> anyhow::Result<Vec<(ChurchId, Heads)>> {
        Ok(self.all_heads(Kind::Church)?.into_iter().map(|(id, heads)| (ChurchId(id), heads)).collect())
    }

    /// The heads of one church (empty if unknown).
    pub fn church_heads(&self, id: &ChurchId) -> anyhow::Result<Heads> {
        self.heads(Kind::Church, id.as_str())
    }

    /// Records a church's heads after a merge.
    pub fn store_church_heads(&self, id: &ChurchId, before: &Heads, after: &Heads) -> anyhow::Result<()> {
        self.store_heads(Kind::Church, id.as_str(), before, after)
    }

    /// The church this device serves, if it was named.
    pub fn this_church(&self) -> anyhow::Result<Option<ChurchId>> {
        self.setting(THIS_CHURCH)
    }

    /// Makes this device serve a church this device knows; false if it does not know it.
    pub fn use_church(&self, id: &ChurchId) -> anyhow::Result<bool> {
        if !self.church_heads(id)?.is_live() {
            return Ok(false);
        }
        self.set_setting(THIS_CHURCH, id)?;
        Ok(true)
    }

    /// Names this device's church: renames it if the device serves one, otherwise creates (or,
    /// if a church of that name is already known, joins) the church whose id comes from the
    /// name. Returns the church; `None` for an empty name.
    pub fn name_church(&self, name: &str) -> anyhow::Result<Option<ChurchId>> {
        let name = name.trim();
        let id = match self.this_church()? {
            Some(id) => id,
            None => match church_id(name) {
                Some(id) => ChurchId(id),
                None => return Ok(None),
            },
        };
        if name.is_empty() {
            return Ok(None);
        }
        let body = church_body(name)?;
        let heads = self.church_heads(&id)?;
        let version = match heads.current() {
            None => Some(Version::first(body)),
            Some(current) if current.live_body() == Some(body.as_slice()) && heads.versions().len() == 1 => {
                None
            }
            Some(current) => heads.save(&current.id().clone(), body),
        };
        if let Some(version) = version {
            self.store_church_heads(&id, &heads, &heads.with(version))?;
        }
        self.set_setting(THIS_CHURCH, &id)?;
        Ok(Some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_church_is_named_renamed_and_found_again_by_its_first_name() {
        let database = Database::in_memory().unwrap();
        assert_eq!(database.this_church().unwrap(), None);
        assert_eq!(database.name_church("  ").unwrap(), None);
        let tulce = database.name_church("Tulce").unwrap().unwrap();
        assert_eq!(database.name_church("Parafia w Tulcach").unwrap(), Some(tulce.clone()), "a rename");
        assert_eq!(database.churches().unwrap()[0].name, "Parafia w Tulcach");
        assert_eq!(database.this_church().unwrap(), Some(tulce.clone()));

        // A device set up again under the first name is the same church.
        let again = Database::in_memory().unwrap();
        assert_eq!(again.name_church("tulce").unwrap(), Some(tulce));
        assert!(!again.use_church(&ChurchId::from("church-unknown")).unwrap());
    }
}
