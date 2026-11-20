// SPDX-License-Identifier: GPL-3.0-or-later
//! Reviewing a set changed in two places (ADR 0025, the sync plan's section 3.4). Every device
//! shows the version with more changes; the review shows all versions side by side, and the
//! organist keeps one, adding by hand ("Dodaj") items that only another version has. Sets are not
//! merged item by item automatically: one person manages a church's sets, so this is rare, and a
//! person sees better than a rule which of two additions belongs where.

use sursum_protocol::ItemId;

use crate::model::ServiceSet;

/// For each item of `version`, whether another of `versions` lacks it (by item id): those items
/// are what the versions disagree on, and the comparison offers "Dodaj" next to them.
pub fn items_missing_elsewhere(version: &ServiceSet, versions: &[ServiceSet]) -> Vec<bool> {
    version.items.iter().map(|item| versions.iter().any(|other| other.position(&item.id).is_none())).collect()
}

/// The version kept in a review, with the items chosen from the other versions added. Each added
/// item goes in after the item it follows in its own version (the nearest one the kept version
/// has), or at the end if none of them is there; items the kept version already has, or that no
/// version has, are skipped, so a repeated or stale choice changes nothing.
pub fn keep_with_items(kept: &ServiceSet, others: &[ServiceSet], add: &[ItemId]) -> ServiceSet {
    let mut result = kept.clone();
    for item_id in add {
        if result.position(item_id).is_some() {
            continue;
        }
        let Some((source, index)) =
            others.iter().find_map(|other| other.position(item_id).map(|index| (other, index)))
        else {
            continue;
        };
        let neighbour = source.items[..index].iter().rev().find(|item| result.position(&item.id).is_some());
        let at = neighbour.and_then(|item| result.position(&item.id)).map_or(result.items.len(), |at| at + 1);
        result.items.insert(at, source.items[index].clone());
    }
    result
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use sursum_protocol::SetId;

    use super::*;
    use crate::model::{ItemKind, SetItem};

    fn set(items: &[&str]) -> ServiceSet {
        ServiceSet {
            id: SetId::from("niedziela"),
            name: "Niedziela".into(),
            items: items
                .iter()
                .map(|id| SetItem { id: ItemId::from(*id), kind: ItemKind::Blank { title: (*id).into() } })
                .collect(),
            date: None,
            church: None,
        }
    }

    fn ids(set: &ServiceSet) -> Vec<&str> {
        set.items.iter().map(|item| item.id.as_str()).collect()
    }

    #[test]
    fn an_added_item_goes_after_the_item_it_follows_elsewhere() {
        let home = set(&["wejscie", "ofiarowanie", "komunia"]);
        let church = set(&["wejscie", "psalm", "komunia", "rozeslanie"]);
        let versions = [home.clone(), church.clone()];
        assert_eq!(items_missing_elsewhere(&church, &versions), [false, true, false, true]);
        assert_eq!(items_missing_elsewhere(&home, &versions), [false, true, false]);

        let add = [ItemId::from("psalm"), ItemId::from("rozeslanie")];
        let kept = keep_with_items(&home, std::slice::from_ref(&church), &add);
        assert_eq!(ids(&kept), ["wejscie", "psalm", "ofiarowanie", "komunia", "rozeslanie"]);
        assert_eq!(kept.name, home.name);
    }

    #[test]
    fn an_item_without_a_known_neighbour_goes_to_the_end_and_nothing_twice() {
        let kept = set(&["a", "b"]);
        let other = set(&["x", "a"]);
        let add = [ItemId::from("x"), ItemId::from("x"), ItemId::from("a"), ItemId::from("gone")];
        assert_eq!(ids(&keep_with_items(&kept, &[other], &add)), ["a", "b", "x"]);
    }

    proptest! {
        /// The kept version's items stay in their order, and every chosen item another version
        /// has is in the result exactly once.
        #[test]
        fn the_kept_order_stays_and_chosen_items_appear_once(
            kept in proptest::sample::subsequence(vec!["a", "b", "c", "d", "e", "f"], 0..=6),
            other in proptest::sample::subsequence(vec!["a", "b", "c", "d", "e", "f"], 0..=6),
            add in proptest::sample::subsequence(vec!["a", "b", "c", "d", "e", "f", "g"], 0..=7),
        ) {
            let (kept, other) = (set(&kept), set(&other));
            let add: Vec<ItemId> = add.into_iter().map(ItemId::from).collect();
            let result = keep_with_items(&kept, std::slice::from_ref(&other), &add);
            let kept_order: Vec<&str> =
                ids(&result).into_iter().filter(|id| kept.position(&ItemId::from(*id)).is_some()).collect();
            prop_assert_eq!(kept_order, ids(&kept));
            for id in &add {
                let wanted = kept.position(id).is_some() || other.position(id).is_some();
                let count = result.items.iter().filter(|item| &item.id == id).count();
                prop_assert_eq!(count, usize::from(wanted));
            }
        }
    }
}
