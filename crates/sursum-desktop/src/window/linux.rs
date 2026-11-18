// SPDX-License-Identifier: GPL-3.0-or-later
//! The window on Linux: GTK 4 with WebKitGTK 6.0.

use std::path::Path;

use anyhow::Context as _;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use webkit6::prelude::*;

use super::{Content, is_web_link, stays_inside};

/// Lets the desktop find the program's icon and keeps one copy running (D-Bus).
const APP_ID: &str = "io.github.dpieczynski.Sursum";

/// Shows what `start` returns, unless a copy of the program is already running, in which case
/// that copy's window comes to the front instead; returns when the window is closed. The web
/// view keeps its own data in `web_data`.
pub fn run(title: &str, web_data: Option<&Path>, start: impl FnOnce() -> Content) -> anyhow::Result<()> {
    let app = gtk4::Application::builder().application_id(APP_ID).build();
    app.register(gio::Cancellable::NONE).context("registering with the desktop session")?;
    if app.is_remote() {
        // Running the remote instance asks the first copy to activate (raise its window).
        app.run_with_args::<&str>(&[]);
        return Ok(());
    }
    let content = std::rc::Rc::new(start());
    let session = network_session(web_data);
    let title = title.to_owned();
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let view = webkit6::WebView::builder().network_session(&session).build();
        match content.as_ref() {
            Content::Remote { origin } => {
                keep_the_remote_inside(&view, origin.clone());
                view.load_uri(&Content::remote_url(origin));
            }
            Content::Message(html) => view.load_html(html, None),
        }
        let window = gtk4::ApplicationWindow::builder()
            .application(app)
            .title(title.as_str())
            .default_width(1280)
            .default_height(800)
            .child(&view)
            .build();
        window.present();
    });
    let code = app.run_with_args::<&str>(&[]);
    anyhow::ensure!(code == glib::ExitCode::SUCCESS, "the window ended with {code:?}");
    Ok(())
}

/// The web view's own storage and cache, apart from the database; downloads (the library
/// export) ask where to save, like in a browser.
fn network_session(web_data: Option<&Path>) -> webkit6::NetworkSession {
    let session = match web_data {
        Some(dir) => {
            let data = dir.to_string_lossy();
            let cache = dir.join("cache");
            webkit6::NetworkSession::new(Some(&data), Some(&cache.to_string_lossy()))
        }
        None => webkit6::NetworkSession::new_ephemeral(),
    };
    session.connect_download_started(|_, download| {
        download.connect_decide_destination(|download, suggested_name| {
            let window = download.web_view().and_then(|view| view.root()).and_downcast::<gtk4::Window>();
            let dialog = gtk4::FileDialog::builder().initial_name(suggested_name).modal(true).build();
            let download = download.clone();
            dialog.save(window.as_ref(), gio::Cancellable::NONE, move |chosen| {
                match chosen.ok().and_then(|file| file.path()) {
                    Some(path) => {
                        download.set_allow_overwrite(true);
                        download.set_destination(&path.to_string_lossy());
                    }
                    None => download.cancel(),
                }
            });
            true
        });
    });
    session
}

/// Pages of the server load in the window; other links, and anything asking for a new window,
/// open in the system's browser.
fn keep_the_remote_inside(view: &webkit6::WebView, origin: String) {
    view.connect_decide_policy(move |view, decision, kind| {
        let leaving = match kind {
            webkit6::PolicyDecisionType::NavigationAction => true,
            webkit6::PolicyDecisionType::NewWindowAction => false,
            _ => return false,
        };
        let Some(uri) = decision
            .downcast_ref::<webkit6::NavigationPolicyDecision>()
            .and_then(|navigation| navigation.navigation_action())
            .and_then(|action| action.request())
            .and_then(|request| request.uri())
        else {
            return false;
        };
        if leaving && stays_inside(&origin, &uri) {
            return false;
        }
        decision.ignore();
        if is_web_link(&uri) {
            let window = view.root().and_downcast::<gtk4::Window>();
            gtk4::UriLauncher::new(&uri).launch(window.as_ref(), gio::Cancellable::NONE, |_| {});
        }
        true
    });
}
