// SPDX-License-Identifier: GPL-3.0-or-later
//! Versions of a document (a song; later a set and a church) and how the versions two devices
//! know are merged (ADR 0025).
//!
//! A document keeps one id for good; every save adds a [`Version`] that names every version it
//! replaces (its ancestors). Merging is a union of versions minus everything another version
//! replaces, so it gives the same [`Heads`] whatever the order, the direction or the number of
//! times files are merged. Among the heads, [`Heads::current`] picks the one shown: a live one
//! before a deletion, then the one with more changes, then the greater id. No clock and no
//! device identity are involved: the church devices have neither a reliable clock nor a stable
//! identity (a restored backup is a second device with the same data).
//!
//! Bodies are opaque bytes here; the server decides what they hold (a song without its id).

use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};

/// A version's id: SHA-256 (lower-case hex) over its body and its sorted ancestors. The
/// document id is not part of it, so equal texts with equal histories have equal ids on every
/// device, and the same text in two songs is the same body.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionId(String);

impl VersionId {
    /// The id as stored and written into files.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// An id read from a file or the database: exactly 64 lower-case hex digits, else `None`
    /// (nothing from a file is trusted).
    pub fn parse(text: &str) -> Option<VersionId> {
        let valid = text.len() == 64 && text.bytes().all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        valid.then(|| VersionId(text.to_owned()))
    }
}

/// What a version holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Body {
    /// The document's content, as the bytes it was saved in (never re-encoded, so the version
    /// id stays valid when a later build of the program adds a field).
    Live(Vec<u8>),
    /// The document was deleted. It still has versions, so the deletion can travel to other
    /// devices and a stale file cannot bring the document back.
    Deleted,
}

/// One saved state of a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    id: VersionId,
    body: Body,
    ancestors: BTreeSet<VersionId>,
}

impl Version {
    /// A version with this body that replaces `ancestors` (every version it replaces, directly
    /// or not). The id is computed, never given.
    pub fn new(body: Body, ancestors: BTreeSet<VersionId>) -> Version {
        let mut hasher = Sha256::new();
        hasher.update(b"sursum-version-1\n");
        match &body {
            Body::Live(bytes) => {
                hasher.update(b"live ");
                hasher.update((bytes.len() as u64).to_be_bytes());
                hasher.update(bytes);
            }
            Body::Deleted => hasher.update(b"deleted"),
        }
        for ancestor in &ancestors {
            hasher.update(b"\n");
            hasher.update(ancestor.0.as_bytes());
        }
        let id = VersionId(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect());
        Version { id, body, ancestors }
    }

    /// The first version of a new document.
    pub fn first(body: Vec<u8>) -> Version {
        Version::new(Body::Live(body), BTreeSet::new())
    }

    /// This version's id.
    pub fn id(&self) -> &VersionId {
        &self.id
    }

    /// What it holds.
    pub fn body(&self) -> &Body {
        &self.body
    }

    /// The live content, `None` for a deletion.
    pub fn live_body(&self) -> Option<&[u8]> {
        match &self.body {
            Body::Live(bytes) => Some(bytes),
            Body::Deleted => None,
        }
    }

    /// Every version this one replaces.
    pub fn ancestors(&self) -> &BTreeSet<VersionId> {
        &self.ancestors
    }

    /// How many changes led to this version: the number of versions it replaces. "More changes
    /// win" (ADR 0025) compares this.
    pub fn changes(&self) -> usize {
        self.ancestors.len()
    }

    /// True unless this version is a deletion.
    pub fn is_live(&self) -> bool {
        matches!(self.body, Body::Live(_))
    }

    /// This version's ancestors plus itself: what a version made from it replaces.
    fn lineage(&self) -> impl Iterator<Item = &VersionId> {
        self.ancestors.iter().chain(std::iter::once(&self.id))
    }
}

/// The heads of one document: the versions no other known version replaces, sorted by id.
/// Usually one; more after the document was changed in two places.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Heads(Vec<Version>);

impl Heads {
    /// The heads among `versions`: duplicates and every version another one replaces are left
    /// out.
    pub fn of(versions: impl IntoIterator<Item = Version>) -> Heads {
        let unique: BTreeMap<VersionId, Version> =
            versions.into_iter().map(|version| (version.id.clone(), version)).collect();
        let replaced: BTreeSet<&VersionId> = unique.values().flat_map(|version| &version.ancestors).collect();
        let heads =
            unique.iter().filter(|(id, _)| !replaced.contains(id)).map(|(_, version)| version.clone());
        Heads(heads.collect())
    }

    /// The heads, sorted by id.
    pub fn versions(&self) -> &[Version] {
        &self.0
    }

    /// True if nothing is known of the document.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// A head by id.
    pub fn get(&self, id: &VersionId) -> Option<&Version> {
        self.0.iter().find(|version| &version.id == id)
    }

    /// What two devices know together. The same result in any order and however often.
    #[must_use]
    pub fn merge(&self, other: &Heads) -> Heads {
        Heads::of(self.0.iter().chain(&other.0).cloned())
    }

    /// These heads with one more version (a save, a deletion or a review).
    #[must_use]
    pub fn with(&self, version: Version) -> Heads {
        Heads::of(self.0.iter().cloned().chain(std::iter::once(version)))
    }

    /// The version shown: a live one before a deletion (a deletion never beats a change made
    /// elsewhere without knowing of it), then more changes, then the greater id. `None` only
    /// when nothing is known.
    pub fn current(&self) -> Option<&Version> {
        self.0.iter().max_by(|a, b| precedence(a, b))
    }

    /// The heads best first, by the rule of [`Heads::current`]: the review lists versions in
    /// this order, so the shown one comes first and the others follow in a fixed order on every
    /// device (not by id, which says nothing to a person).
    pub fn ranked(&self) -> Vec<&Version> {
        let mut versions: Vec<&Version> = self.0.iter().collect();
        versions.sort_by(|a, b| precedence(b, a));
        versions
    }

    /// True if the document is live (its current version is not a deletion).
    pub fn is_live(&self) -> bool {
        self.current().is_some_and(Version::is_live)
    }

    /// True if two live heads hold different content: someone should look at both ("do
    /// przejrzenia"). Until then every device shows the same [`Heads::current`].
    pub fn needs_review(&self) -> bool {
        let bodies: BTreeSet<&[u8]> = self.0.iter().filter_map(Version::live_body).collect();
        bodies.len() > 1
    }

    /// A save made from `base` (the version the user worked from): it replaces `base` and every
    /// deleted head, so an ordinary edit never drops another live version, and a deletion that
    /// already lost goes away. `None` if `base` is not a head any more (someone saved on top of
    /// it meanwhile) — the caller asks the user to reload.
    pub fn save(&self, base: &VersionId, body: Vec<u8>) -> Option<Version> {
        let base = self.get(base)?;
        let deleted = self.0.iter().filter(|head| !head.is_live());
        let ancestors = base.lineage().chain(deleted.flat_map(Version::lineage)).cloned().collect();
        Some(Version::new(Body::Live(body), ancestors))
    }

    /// A deletion of the whole document: it replaces every head (one deletes the song, not one
    /// of its texts). `None` if the document is not live.
    pub fn delete(&self) -> Option<Version> {
        self.is_live().then(|| Version::new(Body::Deleted, self.replaced_by_all()))
    }

    /// The outcome of a review: one version replacing every head.
    pub fn resolve(&self, body: Vec<u8>) -> Version {
        Version::new(Body::Live(body), self.replaced_by_all())
    }

    fn replaced_by_all(&self) -> BTreeSet<VersionId> {
        self.0.iter().flat_map(Version::lineage).cloned().collect()
    }
}

/// Which of two heads goes first: a live one before a deletion, then more changes, then the
/// greater id.
fn precedence(a: &Version, b: &Version) -> std::cmp::Ordering {
    (a.is_live(), a.changes(), &a.id).cmp(&(b.is_live(), b.changes(), &b.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(body: &str) -> Vec<u8> {
        body.as_bytes().to_vec()
    }

    fn current_text(heads: &Heads) -> Option<String> {
        heads.current().and_then(Version::live_body).map(|bytes| String::from_utf8(bytes.to_vec()).unwrap())
    }

    /// Saves `body` on top of the current version (what the editor does after loading it).
    fn edit(heads: &Heads, body: &str) -> Heads {
        let base = heads.current().unwrap().id().clone();
        heads.with(heads.save(&base, text(body)).unwrap())
    }

    #[test]
    fn ids_depend_on_body_and_history_but_not_on_where_they_were_made() {
        let first = Version::first(text("a"));
        assert_eq!(first.id(), Version::first(text("a")).id(), "same on every device");
        assert_ne!(first.id(), Version::first(text("b")).id());
        let deleted = Version::new(Body::Deleted, BTreeSet::new());
        assert_ne!(deleted.id(), Version::first(Vec::new()).id(), "a deletion is not an empty text");
        assert!(VersionId::parse(first.id().as_str()).is_some());
        assert!(VersionId::parse("abc").is_none() && VersionId::parse(&"G".repeat(64)).is_none());
    }

    #[test]
    fn a_text_typed_back_is_a_new_version_and_wins() {
        let x = Heads::of([Version::first(text("x"))]);
        let y = edit(&x, "y");
        let x_again = edit(&y, "x");
        assert_ne!(x_again.current().unwrap().id(), x.current().unwrap().id());
        // A device still on "y" gets "x" back: the newer decision.
        assert_eq!(current_text(&y.merge(&x_again)).as_deref(), Some("x"));
    }

    #[test]
    fn changed_in_two_places_more_changes_win_and_the_other_waits_for_review() {
        let common = Heads::of([Version::first(text("wspólna"))]);
        let home = edit(&edit(&common, "dom 1"), "dom 2");
        let church = edit(&common, "kościół");
        let merged = home.merge(&church);
        assert_eq!(merged.versions().len(), 2, "one song, two versions");
        assert_eq!(current_text(&merged).as_deref(), Some("dom 2"));
        assert!(merged.needs_review());
        assert_eq!(church.merge(&home), merged, "the same in both churches");
        // Reviewing replaces both; the result spreads like any correction.
        let reviewed = merged.with(merged.resolve(text("razem")));
        assert!(!reviewed.needs_review());
        assert_eq!(current_text(&church.merge(&reviewed)).as_deref(), Some("razem"));
    }

    #[test]
    fn the_review_lists_versions_best_first() {
        let common = Heads::of([Version::first(text("wspólna"))]);
        let three = edit(&edit(&edit(&common, "a1"), "a2"), "a3");
        let two = edit(&edit(&common, "b1"), "b2");
        let one = edit(&common, "c1");
        let merged = one.merge(&three).merge(&two);
        let bodies: Vec<&[u8]> = merged.ranked().into_iter().filter_map(Version::live_body).collect();
        assert_eq!(bodies, [b"a3".as_slice(), b"b2", b"c1"]);
        assert_eq!(merged.ranked().first().copied(), merged.current());
    }

    #[test]
    fn an_ordinary_edit_keeps_the_other_live_version_but_absorbs_a_lost_deletion() {
        let common = Heads::of([Version::first(text("a"))]);
        let merged = edit(&edit(&common, "b"), "c").merge(&edit(&common, "d"));
        let edited = edit(&merged, "e");
        assert!(edited.needs_review(), "the other text is still there to look at");

        let deleted = common.with(common.delete().unwrap());
        let kept = edit(&common, "poprawka").merge(&deleted);
        assert_eq!(current_text(&kept).as_deref(), Some("poprawka"), "a deletion never beats a change");
        assert_eq!(kept.versions().len(), 2);
        assert_eq!(edit(&kept, "dalej").versions().len(), 1, "the lost deletion goes away on the next save");
    }

    #[test]
    fn a_deletion_that_knows_the_change_wins_and_a_stale_file_changes_nothing() {
        let first = Heads::of([Version::first(text("a"))]);
        let edited = edit(&first, "b");
        let deleted = edited.with(edited.delete().unwrap());
        assert!(!deleted.is_live());
        assert!(deleted.delete().is_none(), "already deleted");
        assert_eq!(deleted.merge(&first), deleted, "an older file does not bring it back");
        assert_eq!(deleted.merge(&edited), deleted);
        let back = deleted.with(deleted.save(deleted.current().unwrap().id(), text("wraca")).unwrap());
        assert_eq!(current_text(&back).as_deref(), Some("wraca"));
    }

    #[test]
    fn saving_on_a_replaced_version_is_refused() {
        let first = Heads::of([Version::first(text("a"))]);
        let base = first.current().unwrap().id().clone();
        let edited = edit(&first, "b");
        assert!(edited.save(&base, text("c")).is_none());
    }

    /// One step of a random history: a device edits, deletes, reviews, or two devices merge.
    #[derive(Debug, Clone)]
    enum Step {
        Edit { device: usize, text: u8 },
        Delete { device: usize },
        Resolve { device: usize, text: u8 },
        Merge { from: usize, to: usize },
    }

    fn step() -> impl proptest::strategy::Strategy<Value = Step> {
        use proptest::prelude::*;
        prop_oneof![
            4 => (0..3usize, 0..4u8).prop_map(|(device, text)| Step::Edit { device, text }),
            1 => (0..3usize).prop_map(|device| Step::Delete { device }),
            1 => (0..3usize, 0..4u8).prop_map(|(device, text)| Step::Resolve { device, text }),
            4 => (0..3usize, 0..3usize).prop_map(|(from, to)| Step::Merge { from, to }),
        ]
    }

    /// Plays a history on three devices that all start with the same first version.
    fn play(steps: &[Step]) -> [Heads; 3] {
        let start = Heads::of([Version::first(text("start"))]);
        let mut devices = [start.clone(), start.clone(), start];
        for step in steps {
            match *step {
                Step::Edit { device, text: body } => {
                    let heads = &devices[device];
                    let base = heads.current().unwrap().id().clone();
                    let version = heads.save(&base, vec![body]).unwrap();
                    devices[device] = heads.with(version);
                }
                Step::Delete { device } => {
                    if let Some(version) = devices[device].delete() {
                        devices[device] = devices[device].with(version);
                    }
                }
                Step::Resolve { device, text: body } => {
                    let version = devices[device].resolve(vec![body]);
                    devices[device] = devices[device].with(version);
                }
                Step::Merge { from, to } => devices[to] = devices[to].merge(&devices[from]),
            }
        }
        devices
    }

    proptest::proptest! {
        #[test]
        fn merging_does_not_depend_on_order_grouping_or_repetition(
            steps in proptest::collection::vec(step(), 0..40)
        ) {
            let [a, b, c] = play(&steps);
            proptest::prop_assert_eq!(a.merge(&b), b.merge(&a));
            proptest::prop_assert_eq!(a.merge(&b).merge(&c), a.merge(&b.merge(&c)));
            proptest::prop_assert_eq!(a.merge(&b).merge(&b), a.merge(&b));
            proptest::prop_assert_eq!(a.merge(&a), a);
        }

        #[test]
        fn devices_that_exchanged_files_show_the_same_version(
            steps in proptest::collection::vec(step(), 0..40)
        ) {
            let [a, b, c] = play(&steps);
            // Files go round: a → b → c → a → b.
            let b = b.merge(&a);
            let c = c.merge(&b);
            let a = a.merge(&c);
            let b = b.merge(&a);
            proptest::prop_assert_eq!(&a, &b);
            proptest::prop_assert_eq!(&a, &c);
            proptest::prop_assert_eq!(a.current().map(Version::id), c.current().map(Version::id));
        }

        #[test]
        fn no_head_replaces_another_and_merging_never_loses_a_head(
            steps in proptest::collection::vec(step(), 0..40)
        ) {
            let [a, b, _] = play(&steps);
            let merged = a.merge(&b);
            for head in merged.versions() {
                for other in merged.versions() {
                    proptest::prop_assert!(!head.ancestors().contains(other.id()));
                }
            }
            // Every head of either side is a head of the merge or replaced by one of its heads.
            for head in a.versions().iter().chain(b.versions()) {
                let kept = merged.get(head.id()).is_some()
                    || merged.versions().iter().any(|version| version.ancestors().contains(head.id()));
                proptest::prop_assert!(kept);
            }
        }
    }
}
