// SPDX-License-Identifier: GPL-3.0-or-later
//! Uploading pictures: a poster for the set, or the parish logo (admin).

use leptos::prelude::*;
use sursum_protocol::{AssetId, Command};
use wasm_bindgen::JsCast;

use crate::Commands;
use crate::api::post_file;
use crate::messages::{Msg, pl};

/// A file picker that uploads a PNG, JPEG or WebP picture, then calls `on_uploaded` with its id
/// and the file name without extension.
#[component]
fn UploadPicture(
    label: Msg,
    class: &'static str,
    on_uploaded: impl Fn(AssetId, String) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    let failed = RwSignal::new(false);
    let on_change = move |event: leptos::ev::Event| {
        let Some(input) =
            event.target().and_then(|target| target.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|files| files.get(0)) else { return };
        input.set_value("");
        let title = title_from_file_name(&file.name());
        leptos::task::spawn_local(async move {
            match post_file::<AssetId>("/api/assets", &file).await {
                Some(asset_id) => {
                    failed.set(false);
                    on_uploaded(asset_id, title);
                }
                None => failed.set(true),
            }
        });
    };
    view! {
        <label class=class>
            {pl(label)}
            <input type="file" accept="image/png,image/jpeg,image/webp" hidden=true on:change=on_change />
        </label>
        <Show when=move || failed.get()>
            <span class="status-text">{pl(Msg::PictureFailed)}</span>
        </Show>
    }
}

/// Appends an uploaded poster to the active set.
#[component]
pub fn AddPictureButton(commands: Commands) -> impl IntoView {
    let add = move |asset_id, title| commands.send(Command::AddImage { title, asset_id });
    view! { <UploadPicture label=Msg::Picture class="btn btn--sm" on_uploaded=add /> }
}

/// Sets the parish logo shown by the LOGO button (admin mode).
#[component]
pub fn LogoButton(commands: Commands) -> impl IntoView {
    let set = move |asset_id, _title| commands.send(Command::SetLogo { asset_id: Some(asset_id) });
    view! { <UploadPicture label=Msg::ParishLogo class="btn btn--sm" on_uploaded=set /> }
}

/// "plakat-adwent.jpg" → "plakat-adwent": the set list shows something recognisable.
fn title_from_file_name(name: &str) -> String {
    name.rsplit_once('.').map_or(name, |(stem, _)| stem).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_drop_the_extension() {
        assert_eq!(title_from_file_name("plakat-adwent.jpg"), "plakat-adwent");
        assert_eq!(title_from_file_name("bez rozszerzenia"), "bez rozszerzenia");
    }
}
