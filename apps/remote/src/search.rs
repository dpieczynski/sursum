// SPDX-License-Identifier: GPL-3.0-or-later
//! Song search: the query goes to the server (`GET /api/songs`), which does all matching.

use sursum_protocol::SongSummary;

use crate::messages::Msg;

/// Where a search stands. "Nothing found" is only said once the server answered; a search
/// still on its way or one that never reached the server reads differently.
#[derive(Debug, Clone, Default)]
pub enum Search {
    /// Nothing typed.
    #[default]
    Idle,
    /// The first answer for the typed text is on its way.
    Waiting,
    Found(Vec<SongSummary>),
    /// The server could not be asked (no connection).
    Failed,
}

impl Search {
    /// What to say instead of a list, if anything (an empty `Found` is the caller's own case).
    pub fn notice(&self) -> Option<Msg> {
        match self {
            Search::Waiting => Some(Msg::Searching),
            Search::Failed => Some(Msg::SearchFailed),
            Search::Idle | Search::Found(_) => None,
        }
    }
}

/// Asks the server for songs matching `query`; `None` if it could not be asked.
pub async fn fetch_songs(query: &str) -> Option<Vec<SongSummary>> {
    let url = format!("/api/songs?q={}", js_sys::encode_uri_component(query));
    crate::api::get_json(&url).await
}
