// SPDX-License-Identifier: GPL-3.0-or-later
//! The parish logo row in Ekran: the logo as uploaded, a button to upload another one, and one to
//! remove it (hold, then undo), so a wrong or test logo does not stay until the next upload.
//! The server remembers the removed logo and owns the undo (`UndoRemoveLogo`), so undo never
//! replaces a logo someone set meanwhile.

use leptos::prelude::*;
use sursum_protocol::{AssetId, Command, RemoteState};

use crate::Commands;
use crate::messages::{Msg, pl};
use crate::picture::LogoButton;
use crate::ui::{HoldButton, Toasts};

#[component]
pub fn LogoSetting(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    // Memoized: snapshots arrive on every change, the thumbnail only changes with the logo.
    let logo =
        Memo::new(move |_| state.with(|state| state.as_ref().and_then(|state| state.program.logo.clone())));
    let toasts = expect_context::<Toasts>();
    let remove = move || {
        if logo.get_untracked().is_some() && commands.try_send(Command::SetLogo { asset_id: None }) {
            toasts.show_with_undo(pl(Msg::LogoRemoved), Command::UndoRemoveLogo);
        }
    };
    let logo_view = move || match logo.get() {
        Some(asset_id) => thumbnail(&asset_id).into_any(),
        None => view! { <span class="setting-value">{pl(Msg::NoLogoYet)}</span> }.into_any(),
    };
    view! {
        <div class="setting-row" data-testid="settings-logo">
            <span class="setting-label">{pl(Msg::ParishLogo)}</span>
            {logo_view}
            <LogoButton commands />
            <Show when=move || logo.with(Option::is_some)>
                <HoldButton
                    label=pl(Msg::RemoveLogo)
                    on_hold=remove
                    testid="remove-logo"
                    class="btn--sm btn--quiet-danger"
                />
            </Show>
        </div>
    }
}

fn thumbnail(asset_id: &AssetId) -> impl IntoView {
    view! {
        <img
            class="logo-thumb"
            data-testid="logo-thumb"
            src=sursum_slide::slide_view::asset_url(asset_id)
            alt=pl(Msg::ParishLogo)
        />
    }
}
