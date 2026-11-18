// SPDX-License-Identifier: GPL-3.0-or-later
//! Ekran: the display's power (where configured), the parish logo and the screen calibration.

use leptos::prelude::*;
use sursum_protocol::{Command, DisplayPowerState, PowerAction, RemoteState};

use super::{AdminOnly, Section};
use crate::Commands;
use crate::calibration::Calibrating;
use crate::messages::{Msg, pl};
use crate::ui::{HoldButton, Sheet};

/// Standby needs a longer hold than other destructive actions.
const STANDBY_HOLD_MS: u32 = 1500;

#[component]
pub fn ScreenSection(state: RwSignal<Option<RemoteState>>, commands: Commands) -> impl IntoView {
    let power = move || {
        state.with(|state| state.as_ref().map(|state| state.display_power.clone()).unwrap_or_default())
    };
    let configured =
        move || !matches!(power(), DisplayPowerState::Unavailable { reason } if reason == "not_configured");
    let cooling = move || power() == DisplayPowerState::CoolingDown;
    let turn_on = move |_| commands.send(Command::DisplayPower { action: PowerAction::On });
    let standby = move || commands.send(Command::DisplayPower { action: PowerAction::Standby });
    let logo = move || state.with(|state| state.as_ref().and_then(|state| state.program.logo.clone()));
    let logo_view = move || match logo() {
        Some(asset_id) => view! {
            <img
                class="logo-thumb"
                src=sursum_slide::slide_view::asset_url(&asset_id)
                alt=pl(Msg::ParishLogo)
            />
        }
        .into_any(),
        None => view! { <span class="setting-value">{pl(Msg::NoLogoYet)}</span> }.into_any(),
    };
    let confirming = RwSignal::new(false);
    let calibrating = expect_context::<Calibrating>();
    let start = move |_| {
        confirming.set(false);
        calibrating.0.set(true);
    };
    let confirm = move || {
        confirming.get().then(|| {
            view! {
                <Sheet
                    title=pl(Msg::Calibrate).to_owned()
                    on_close=move || confirming.set(false)
                    testid="calibrate-confirm"
                >
                    <p>{pl(Msg::CalibrateHint)}</p>
                    <div class="sheet-actions">
                        <button class="btn btn--primary" data-testid="calibrate-start" on:click=start>
                            {pl(Msg::CalibrateStart)}
                        </button>
                    </div>
                </Sheet>
            }
        })
    };
    view! {
        <Section title=Msg::ScreenSheet>
            <Show when=configured>
                <div class="setting-row" data-testid="settings-power">
                    <span class="setting-label">{move || pl(crate::power::state_label(&power()))}</span>
                    <button class="btn btn--sm btn--live" on:click=turn_on prop:disabled=cooling>
                        {pl(Msg::DisplayOn)}
                    </button>
                    <HoldButton label=pl(Msg::DisplayStandby) on_hold=standby hold_ms=STANDBY_HOLD_MS />
                </div>
            </Show>
            <div class="setting-row">
                <span class="setting-label">{pl(Msg::ParishLogo)}</span>
                {logo_view}
                <AdminOnly label=Msg::UploadLogo>
                    <crate::picture::LogoButton commands />
                </AdminOnly>
            </div>
            <div class="setting-row">
                <AdminOnly label=Msg::Calibrate>
                    <button
                        class="btn btn--sm"
                        data-testid="calibrate"
                        on:click=move |_| confirming.set(true)
                    >
                        {pl(Msg::Calibrate)}
                    </button>
                </AdminOnly>
            </div>
        </Section>
        {confirm}
    }
}
