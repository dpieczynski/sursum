// SPDX-License-Identifier: GPL-3.0-or-later
//! A problem banner under the top bar: no connection, no screen, display asleep. Laid over the
//! page so it never moves the controls.

use leptos::prelude::*;

/// How serious the problem is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BannerKind {
    /// Nothing the operator does reaches the screen.
    Danger,
    /// Worth knowing, but the remote still works.
    Warn,
}

/// One banner.
#[component]
pub fn Banner(kind: BannerKind, testid: &'static str, children: Children) -> impl IntoView {
    let class = match kind {
        BannerKind::Danger => "banner banner--danger",
        BannerKind::Warn => "banner banner--warn",
    };
    view! {
        <div class=class role="alert" data-testid=testid>
            {children()}
        </div>
    }
}
