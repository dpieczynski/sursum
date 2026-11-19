// SPDX-License-Identifier: GPL-3.0-or-later
//! Churches of the parish (ADR 0025). Every device with a screen belongs to one and shows only
//! its sets; songs are shared by all.

use sha2::{Digest, Sha256};

use crate::search::normalize;

/// The id of a church named `name` when it is first created: derived from the name folded like
/// the search folds text (lower case, no Polish letters, single spaces), so a device set up
/// again where someone types "Tulce" (or " tulce ") is the same church as before. A later rename
/// changes the church's name, never its id. `None` for a name with nothing in it.
pub fn church_id(name: &str) -> Option<String> {
    let folded = normalize(name);
    if folded.is_empty() {
        return None;
    }
    let digest = Sha256::digest(folded.as_bytes());
    let hex: String = digest.iter().take(8).map(|byte| format!("{byte:02x}")).collect();
    Some(format!("church-{hex}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_name_typed_differently_is_the_same_church() {
        assert_eq!(church_id("Tulce"), church_id("  tulce "));
        assert_eq!(church_id("Gowarzewo"), church_id("GOWARZEWO"));
        assert_ne!(church_id("Tulce"), church_id("Gowarzewo"));
        assert_eq!(church_id("Łękno"), church_id("lekno"), "Polish letters fold");
        assert_eq!(church_id("  "), None);
        assert!(church_id("Tulce").unwrap().starts_with("church-"));
    }
}
