// SPDX-License-Identifier: GPL-3.0-or-later
//! The window on Linux: GTK 4 with WebKitGTK 6.0.

use anyhow::Context as _;
use gtk4::prelude::*;
use gtk4::{gio, glib};
use webkit6::prelude::*;

/// Lets the desktop find the program's icon and keeps one copy running (D-Bus).
const APP_ID: &str = "io.github.dpieczynski.Sursum";

/// Starts the server with `start` (which returns the remote's URL) unless a copy of the program
/// is already running, in which case that copy's window comes to the front instead; then shows
/// the window until it is closed.
pub fn run(title: &str, start: impl FnOnce() -> anyhow::Result<String>) -> anyhow::Result<()> {
    let app = gtk4::Application::builder().application_id(APP_ID).build();
    app.register(gio::Cancellable::NONE).context("registering with the desktop session")?;
    if app.is_remote() {
        app.activate();
        return Ok(());
    }
    let url = start()?;
    let title = title.to_owned();
    app.connect_activate(move |app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }
        let view = webkit6::WebView::new();
        ask_where_to_save_downloads(&view);
        view.load_uri(&url);
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

/// Downloads (the library export) open a "save as" dialog, like in a browser.
fn ask_where_to_save_downloads(view: &webkit6::WebView) {
    let Some(session) = view.network_session() else { return };
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
}
