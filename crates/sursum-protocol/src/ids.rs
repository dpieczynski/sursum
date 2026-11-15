// SPDX-License-Identifier: GPL-3.0-or-later
//! Identifier newtypes. They are opaque strings on the wire: the server decides the format
//! (UUIDs, content hashes), clients only compare and send them back.

use serde::{Deserialize, Serialize};

macro_rules! string_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Borrows the identifier as a string slice.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self(value.to_owned())
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

string_id!(
    /// Stable identity of a song across edits and devices.
    SongId
);
string_id!(
    /// Stable identity of one entry in a set (the same song may appear twice).
    ItemId
);
string_id!(
    /// Identity of a set (a service's running order).
    SetId
);
string_id!(
    /// Identity of an uploaded asset (image, logo).
    AssetId
);
string_id!(
    /// Content hash of a compiled deck; equal ids mean identical slides.
    DeckId
);
string_id!(
    /// Position-independent name of a slide inside a deck, e.g. `c#0@1`, so the same slide can be
    /// found again after the song was edited.
    SlideKey
);
string_id!(
    /// Random identity a client picks for itself (used for the `Stale` rule and presence).
    ClientId
);

/// Monotonic revision of the server state; increases on every client-visible change.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Rev(pub u32);

impl Rev {
    /// The revision that follows this one.
    pub fn next(self) -> Self {
        Rev(self.0.wrapping_add(1))
    }
}
