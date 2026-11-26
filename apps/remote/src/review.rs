// SPDX-License-Identifier: GPL-3.0-or-later
//! Songs changed in two places (ADR 0025): every device shows the version with more changes, and
//! the other one waits here. A line says how many there are; the list opens a comparison of the
//! versions, where "Zostaw tę" keeps one and replaces the rest. The server decides everything
//! (which songs, which version is shown); this only shows it and asks.

use leptos::prelude::*;
use sursum_protocol::{
    KeepVersion, RemoteState, SaveOutcome, SongId, SongSummary, SongVersionView, SongVersions,
};

use crate::api::{get_json, post_json};
use crate::messages::{Msg, pl, song_version_details, songs_to_review};
use crate::ui::{Sheet, Toasts};

/// "Pieśni do przejrzenia: 2", shown only while there are some; opens the list.
#[component]
pub fn ReviewLine(state: RwSignal<Option<RemoteState>>) -> impl IntoView {
    let count = move || state.with(|state| state.as_ref().map_or(0, |state| state.songs_to_review));
    let listing = RwSignal::new(false);
    let list = move || listing.get().then(|| view! { <ReviewList open=listing /> });
    view! {
        <Show when=move || { count() > 0 }>
            <button class="review-line" data-testid="review-line" on:click=move |_| listing.set(true)>
                {move || songs_to_review(count())}
            </button>
        </Show>
        {list}
    }
}

/// The songs waiting for review; tapping one opens the comparison.
#[component]
fn ReviewList(open: RwSignal<bool>) -> impl IntoView {
    let songs = RwSignal::new(None::<Vec<SongSummary>>);
    let reviewing = RwSignal::new(None::<SongId>);
    let load = move || {
        leptos::task::spawn_local(async move {
            songs.set(Some(get_json::<Vec<SongSummary>>("/api/songs/review").await.unwrap_or_default()));
        });
    };
    load();
    // Reload after a review ends, so the reviewed song leaves the list.
    Effect::new(move |previous: Option<bool>| {
        let closed = reviewing.with(Option::is_none);
        if closed && previous == Some(false) {
            load();
        }
        closed
    });
    let rows = move || {
        let songs = songs.get().unwrap_or_default();
        if songs.is_empty() {
            return view! { <p class="hint">{pl(Msg::NothingToReview)}</p> }.into_any();
        }
        let rows = songs.into_iter().map(|song| review_row(song, reviewing)).collect_view();
        view! { <ul class="search-results">{rows}</ul> }.into_any()
    };
    let comparison = move || reviewing.get().map(|song_id| view! { <Comparison song_id open=reviewing /> });
    view! {
        <Sheet title=pl(Msg::ReviewTitle).to_owned() on_close=move || open.set(false) testid="review-list">
            <p class="hint">{pl(Msg::ReviewHint)}</p>
            {rows}
        </Sheet>
        {comparison}
    }
}

fn review_row(song: SongSummary, reviewing: RwSignal<Option<SongId>>) -> impl IntoView {
    let song_id = song.song_id.clone();
    view! {
        <li class="search-result">
            <span class="row-number">{song.number.unwrap_or_default()}</span>
            <button
                class="result-text tappable"
                data-testid="review-song"
                on:click=move |_| reviewing.set(Some(song_id.clone()))
            >
                <span class="result-title">{song.title}</span>
                <span class="result-line">{song.first_line}</span>
            </button>
        </li>
    }
}

/// The versions of one song side by side (one below the other on a phone).
#[component]
fn Comparison(song_id: SongId, open: RwSignal<Option<SongId>>) -> impl IntoView {
    let versions = RwSignal::new(None::<SongVersions>);
    let url = format!("/api/songs/{}/versions", js_sys::encode_uri_component(song_id.as_str()));
    leptos::task::spawn_local(async move {
        versions.set(get_json::<SongVersions>(&url).await);
    });
    let columns = move || {
        versions.get().map(|loaded| {
            let song_id = loaded.song_id.clone();
            loaded
                .versions
                .into_iter()
                .map(|version| version_column(song_id.clone(), version, open))
                .collect_view()
        })
    };
    view! {
        <Sheet title=pl(Msg::CompareTitle).to_owned() on_close=move || open.set(None) testid="review-compare">
            <div class="review-columns">{columns}</div>
        </Sheet>
    }
}

fn version_column(
    song_id: SongId,
    version: SongVersionView,
    open: RwSignal<Option<SongId>>,
) -> impl IntoView {
    let toasts = expect_context::<Toasts>();
    let label = if version.shown { pl(Msg::VersionShown) } else { pl(Msg::VersionOther) };
    let details = song_version_details(version.number.as_deref(), &version.presentation);
    let request = KeepVersion { version: version.version };
    let keep = move |_| {
        let (song_id, request) = (song_id.clone(), request.clone());
        leptos::task::spawn_local(async move {
            let url = format!("/api/songs/{}/keep", js_sys::encode_uri_component(song_id.as_str()));
            match post_json::<_, SaveOutcome>(&url, &request).await {
                Some(SaveOutcome::Saved { .. }) => {
                    toasts.show(pl(Msg::VersionKept));
                    open.set(None);
                }
                Some(SaveOutcome::Conflict) => toasts.show(pl(Msg::Conflict)),
                _ => toasts.show(pl(Msg::NotSent)),
            }
        });
    };
    view! {
        <section class="review-version" class:shown=version.shown data-testid="review-version">
            <h3 class="review-label">{label}</h3>
            <p class="card-title">{version.title}</p>
            <p class="review-details" data-testid="version-details">
                {details}
            </p>
            <pre class="review-lyrics">{version.lyrics}</pre>
            <button class="btn btn--primary" data-testid="keep-version" on:click=keep>
                {pl(Msg::KeepThis)}
            </button>
        </section>
    }
}
