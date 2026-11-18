// SPDX-License-Identifier: GPL-3.0-or-later
//! The window on Windows: WebView2 (the Edge engine that ships with Windows) through wry.

use std::path::Path;

use anyhow::Context as _;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::platform::run_return::EventLoopExtRunReturn as _;
use tao::window::{Icon, WindowBuilder};
use wry::{NewWindowResponse, WebContext, WebViewBuilder};

use super::{Content, is_web_link, stays_inside};

/// Shows what `start` returns until the window is closed. The web view keeps its own data in
/// `web_data` (by default WebView2 would put it next to the program, which may be read-only).
/// Downloads (the library export) use WebView2's own download prompt.
pub fn run(title: &str, web_data: Option<&Path>, start: impl FnOnce() -> Content) -> anyhow::Result<()> {
    let content = start();
    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .with_window_icon(window_icon())
        .build(&event_loop)
        .context("opening the window")?;
    let mut context = WebContext::new(web_data.map(Path::to_path_buf));
    let builder = WebViewBuilder::new_with_web_context(&mut context).with_new_window_req_handler(|url, _| {
        open_in_browser(&url);
        NewWindowResponse::Deny
    });
    let builder = match &content {
        Content::Remote { origin } => {
            let origin = origin.clone();
            builder.with_url(Content::remote_url(&origin)).with_navigation_handler(move |url| {
                let inside = stays_inside(&origin, &url);
                if !inside {
                    open_in_browser(&url);
                }
                inside
            })
        }
        Content::Message(html) => builder.with_html(html.as_str()),
    };
    let _view = builder.build(&window).context("starting WebView2")?;
    // `run_return`, unlike `run`, comes back when the window closes, so the server can stop
    // cleanly (and finish writing the database) before the program exits.
    event_loop.run_return(|event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = event {
            *control_flow = ControlFlow::Exit;
        }
    });
    Ok(())
}

/// The program's icon for the window's title bar and the taskbar (the `.exe` carries its own, see
/// `build.rs`).
fn window_icon() -> Option<Icon> {
    const SIZE: u32 = 64;
    let rgba = include_bytes!("../../assets/sursum-64.rgba").to_vec();
    Icon::from_rgba(rgba, SIZE, SIZE).ok()
}

/// Hands a web link to the default browser (Explorer opens URLs with it).
fn open_in_browser(url: &str) {
    if !is_web_link(url) {
        return;
    }
    if let Err(error) = std::process::Command::new("explorer").arg(url).spawn() {
        tracing::warn!(?error, url, "could not open the browser");
    }
}
