// SPDX-License-Identifier: GPL-3.0-or-later
//! A set changed in two places (ADR 0025): every device shows the version with more changes, and
//! a line above the items offers the comparison. Each version lists its items; next to an item
//! another version lacks, "Dodaj" marks it to be taken along, and "Zostaw tę" keeps one version
//! with the marked items (the server puts them in place and ends the review everywhere).

use std::collections::BTreeSet;

use leptos::prelude::*;
use sursum_protocol::{Command, ItemId, RemoteState, SetId, SetVersionItem, SetVersionView, SetVersions};

use crate::Commands;
use crate::api::get_json;
use crate::messages::{Msg, pl, set_date_label, song_number};
use crate::ui::Sheet;

/// "Zestaw zmieniony w dwóch miejscach", shown only while the active set waits for review.
#[component]
pub fn SetReviewLine(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let in_review = move || {
        state.with(|state| {
            state
                .as_ref()
                .and_then(|state| state.active_set.as_ref())
                .filter(|set| set.review)
                .map(|set| set.set_id.clone())
        })
    };
    let comparing = RwSignal::new(None::<SetId>);
    let open = move |_| comparing.set(in_review());
    let sheet = move || comparing.get().map(|set_id| view! { <Comparison set_id open=comparing commands /> });
    view! {
        <Show when=move || in_review().is_some()>
            <button class="review-line" data-testid="set-review-line" on:click=open>
                {pl(Msg::SetReviewLine)}
            </button>
        </Show>
        {sheet}
    }
}

/// The versions of the set side by side (one below the other on a phone).
#[component]
fn Comparison(set_id: SetId, open: RwSignal<Option<SetId>>, commands: Commands) -> impl IntoView {
    let versions = RwSignal::new(None::<SetVersions>);
    let added = RwSignal::new(BTreeSet::<ItemId>::new());
    let url = format!("/api/sets/{}/versions", js_sys::encode_uri_component(set_id.as_str()));
    leptos::task::spawn_local(async move {
        versions.set(get_json::<SetVersions>(&url).await);
    });
    let columns = move || {
        versions.get().map(|loaded| {
            let set_id = loaded.set_id.clone();
            loaded
                .versions
                .into_iter()
                .map(|version| version_column(set_id.clone(), version, added, open, commands))
                .collect_view()
        })
    };
    view! {
        <Sheet title=pl(Msg::CompareTitle).to_owned() on_close=move || open.set(None) testid="set-compare">
            <p class="hint">{pl(Msg::SetReviewHint)}</p>
            <div class="review-columns">{columns}</div>
        </Sheet>
    }
}

fn version_column(
    set_id: SetId,
    version: SetVersionView,
    added: RwSignal<BTreeSet<ItemId>>,
    open: RwSignal<Option<SetId>>,
    commands: Commands,
) -> impl IntoView {
    let label = if version.shown { pl(Msg::VersionShown) } else { pl(Msg::VersionOther) };
    let date = version.date.as_deref().map(set_date_label);
    let keep_version = version.version.clone();
    let keep = move |_| {
        let add = added.get_untracked().into_iter().collect();
        let command = Command::KeepSetVersion { set_id: set_id.clone(), version: keep_version.clone(), add };
        if commands.try_send(command) {
            open.set(None);
        }
    };
    let items = version.items.into_iter().map(|item| item_row(item, added)).collect_view();
    view! {
        <section class="review-version" class:shown=version.shown data-testid="set-version">
            <h3 class="review-label">{label}</h3>
            <p class="card-title">{version.name} " " <span class="sets-date">{date}</span></p>
            <ol class="review-items">{items}</ol>
            <button class="btn btn--primary" data-testid="keep-set-version" on:click=keep>
                {pl(Msg::KeepThis)}
            </button>
        </section>
    }
}

/// One item; one that another version lacks is marked and can be taken along ("Dodaj").
fn item_row(item: SetVersionItem, added: RwSignal<BTreeSet<ItemId>>) -> impl IntoView {
    let item_id = item.item_id.clone();
    let chosen = {
        let item_id = item_id.clone();
        move || added.with(|added| added.contains(&item_id))
    };
    let toggle = move |_| {
        added.update(|added| {
            if !added.remove(&item_id) {
                added.insert(item_id.clone());
            }
        });
    };
    let chosen_label = chosen.clone();
    let button = item.missing_elsewhere.then(|| {
        view! {
            <button
                class="btn btn--sm"
                class:btn--primary=chosen
                data-testid="add-from-version"
                on:click=toggle
            >
                {move || if chosen_label() { pl(Msg::Added) } else { pl(Msg::Add) }}
            </button>
        }
    });
    let number = song_number(item.song_number.as_deref());
    view! {
        <li class="review-item" class:differs=item.missing_elsewhere>
            <span class="review-item-title">{item.title}</span>
            <span class="review-item-number">{number}</span>
            {button}
        </li>
    }
}
