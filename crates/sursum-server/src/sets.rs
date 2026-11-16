// SPDX-License-Identifier: GPL-3.0-or-later
//! All sets (running orders), which one is active, and the last removal for undo.
//!
//! Changing a set never changes the program (ADR 0005); the state actor tells the program about
//! removals so it can keep its ghost position.

use sursum_core::model::{ItemKind, ServiceSet, SetItem};
use sursum_protocol::{ItemId, SetId, SetSummary};

/// The last removed item, so one tap can put it back.
#[derive(Debug, Clone)]
struct Removed {
    set_id: SetId,
    index: usize,
    item: SetItem,
}

/// The sets kept by the server.
#[derive(Debug, Default)]
pub struct SetStore {
    sets: Vec<ServiceSet>,
    active: Option<SetId>,
    last_removed: Option<Removed>,
}

impl SetStore {
    /// Sets loaded from the database; `active` is used if it names an existing set, otherwise
    /// the newest set is active.
    pub fn new(sets: Vec<ServiceSet>, active: Option<SetId>) -> Self {
        let active = active
            .filter(|id| sets.iter().any(|set| &set.id == id))
            .or_else(|| sets.last().map(|set| set.id.clone()));
        SetStore { sets, active, last_removed: None }
    }

    /// The active set.
    pub fn active(&self) -> Option<&ServiceSet> {
        let id = self.active.as_ref()?;
        self.sets.iter().find(|set| &set.id == id)
    }

    fn active_mut(&mut self) -> Option<&mut ServiceSet> {
        let id = self.active.clone()?;
        self.sets.iter_mut().find(|set| set.id == id)
    }

    /// A set by id.
    pub fn get(&self, set_id: &SetId) -> Option<&ServiceSet> {
        self.sets.iter().find(|set| &set.id == set_id)
    }

    /// Id of the active set.
    pub fn active_id(&self) -> Option<&SetId> {
        self.active.as_ref()
    }

    /// The list for switching sets, newest first (the next service is usually the newest).
    pub fn summaries(&self) -> Vec<SetSummary> {
        self.sets
            .iter()
            .rev()
            .map(|set| SetSummary {
                set_id: set.id.clone(),
                name: set.name.clone(),
                items: u32::try_from(set.items.len()).unwrap_or(u32::MAX),
            })
            .collect()
    }

    /// Makes a set active; false if it does not exist.
    pub fn activate(&mut self, set_id: &SetId) -> bool {
        let exists = self.sets.iter().any(|set| &set.id == set_id);
        if exists {
            self.active = Some(set_id.clone());
        }
        exists
    }

    /// Renames a set; returns it, or `None` if it does not exist.
    pub fn rename(&mut self, set_id: &SetId, name: &str) -> Option<&ServiceSet> {
        let name = name.trim();
        if name.is_empty() {
            return None;
        }
        let set = self.sets.iter_mut().find(|set| &set.id == set_id)?;
        name.clone_into(&mut set.name);
        Some(set)
    }

    /// Deletes a set; false if it does not exist. If it was active, the newest remaining set
    /// becomes active.
    pub fn delete(&mut self, set_id: &SetId) -> bool {
        let Some(index) = self.sets.iter().position(|set| &set.id == set_id) else { return false };
        self.sets.remove(index);
        if self.last_removed.as_ref().is_some_and(|removed| &removed.set_id == set_id) {
            self.last_removed = None;
        }
        if self.active.as_ref() == Some(set_id) {
            self.active = self.sets.last().map(|set| set.id.clone());
        }
        true
    }

    /// Creates an empty set and makes it active.
    pub fn create(&mut self, id: SetId, name: &str) -> &ServiceSet {
        let name = if name.trim().is_empty() { "Zestaw" } else { name.trim() };
        self.sets.push(ServiceSet { id: id.clone(), name: name.to_owned(), items: Vec::new() });
        self.active = Some(id);
        self.sets.last().expect("just pushed")
    }

    /// Adds a set, or replaces the set with the same id (re-importing). The active set stays.
    pub fn upsert(&mut self, set: ServiceSet) {
        match self.sets.iter_mut().find(|existing| existing.id == set.id) {
            Some(existing) => *existing = set,
            None => self.sets.push(set),
        }
        if self.active.is_none() {
            self.active = self.sets.first().map(|set| set.id.clone());
        }
    }

    /// Inserts an item into the active set after `anchor` (or at the end).
    pub fn insert(
        &mut self,
        anchor: Option<&ItemId>,
        item_id: ItemId,
        kind: ItemKind,
    ) -> Option<&ServiceSet> {
        let set = self.active_mut()?;
        set.insert_after(anchor, SetItem { id: item_id, kind });
        Some(set)
    }

    /// Removes an item from the active set; returns its former index. It can be undone.
    pub fn remove(&mut self, item_id: &ItemId) -> Option<(usize, &ServiceSet)> {
        let set = self.active_mut()?;
        let (index, item) = set.remove(item_id)?;
        let set_id = set.id.clone();
        self.last_removed = Some(Removed { set_id, index, item });
        Some((index, self.active()?))
    }

    /// Moves an item of the active set.
    pub fn move_item(&mut self, item_id: &ItemId, to_index: usize) -> Option<&ServiceSet> {
        let set = self.active_mut()?;
        set.move_to(item_id, to_index).then_some(&*set)
    }

    /// Puts back the last removed item; returns the set it went back into.
    pub fn undo_remove(&mut self) -> Option<&ServiceSet> {
        let removed = self.last_removed.take()?;
        let set = self.sets.iter_mut().find(|set| set.id == removed.set_id)?;
        set.restore(removed.index, removed.item);
        Some(set)
    }

    /// True while the last removal can be undone.
    pub fn can_undo(&self) -> bool {
        self.last_removed.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(id: &str) -> (ItemId, ItemKind) {
        (ItemId::from(id), ItemKind::Text { title: id.into(), slides: vec![] })
    }

    fn ids(set: &ServiceSet) -> Vec<&str> {
        set.items.iter().map(|item| item.id.as_str()).collect()
    }

    #[test]
    fn newest_set_is_active_unless_one_is_named() {
        let set = |id: &str| ServiceSet { id: SetId::from(id), name: id.into(), items: vec![] };
        let store = SetStore::new(vec![set("a"), set("b")], None);
        assert_eq!(store.active_id().map(SetId::as_str), Some("b"));
        let store = SetStore::new(vec![set("a"), set("b")], Some(SetId::from("a")));
        assert_eq!(store.active_id().map(SetId::as_str), Some("a"));
        let store = SetStore::new(vec![set("a")], Some(SetId::from("gone")));
        assert_eq!(store.active_id().map(SetId::as_str), Some("a"));
    }

    #[test]
    fn remove_and_undo() {
        let mut store = SetStore::default();
        store.create(SetId::from("s"), " Niedziela ");
        for id in ["a", "b", "c"] {
            let (item_id, kind) = text(id);
            store.insert(None, item_id, kind);
        }
        let (index, set) = store.remove(&ItemId::from("b")).unwrap();
        assert_eq!((index, ids(set)), (1, vec!["a", "c"]));
        assert!(store.can_undo());
        assert_eq!(ids(store.undo_remove().unwrap()), ["a", "b", "c"]);
        assert!(!store.can_undo());
        assert_eq!(store.active().unwrap().name, "Niedziela");
    }

    #[test]
    fn sets_are_listed_newest_first_and_can_be_renamed_and_deleted() {
        let mut store = SetStore::default();
        store.create(SetId::from("a"), "Pierwszy");
        store.create(SetId::from("b"), "Drugi");
        let names: Vec<String> = store.summaries().into_iter().map(|set| set.name).collect();
        assert_eq!(names, ["Drugi", "Pierwszy"]);
        assert_eq!(
            store.rename(&SetId::from("a"), "  Niedziela ").map(|set| set.name.as_str()),
            Some("Niedziela")
        );
        assert!(store.rename(&SetId::from("a"), " ").is_none(), "a set needs a name");
        assert!(store.delete(&SetId::from("b")));
        assert_eq!(store.active_id(), Some(&SetId::from("a")), "the newest remaining set");
        assert!(!store.delete(&SetId::from("b")));
    }

    #[test]
    fn move_and_activate() {
        let mut store = SetStore::default();
        store.create(SetId::from("s1"), "");
        let (item_id, kind) = text("a");
        store.insert(None, item_id, kind);
        let (item_id, kind) = text("b");
        store.insert(None, item_id, kind);
        assert_eq!(ids(store.move_item(&ItemId::from("b"), 0).unwrap()), ["b", "a"]);
        store.create(SetId::from("s2"), "Drugi");
        assert_eq!(store.active().unwrap().items.len(), 0);
        assert!(store.activate(&SetId::from("s1")));
        assert!(!store.activate(&SetId::from("nope")));
        assert_eq!(store.summaries().len(), 2);
    }
}
