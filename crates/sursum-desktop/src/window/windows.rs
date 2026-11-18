// SPDX-License-Identifier: GPL-3.0-or-later
//! The window on Windows: WebView2 (the Edge engine that ships with Windows) through wry.

use anyhow::Context as _;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoop};
use tao::platform::run_return::EventLoopExtRunReturn as _;
use tao::window::WindowBuilder;
use wry::WebViewBuilder;

/// Starts the server with `start` (which returns the remote's URL), then shows the window until
/// it is closed. Downloads (the library export) use WebView2's own download prompt.
pub fn run(title: &str, start: impl FnOnce() -> anyhow::Result<String>) -> anyhow::Result<()> {
    let url = start()?;
    let mut event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title(title)
        .with_inner_size(LogicalSize::new(1280.0, 800.0))
        .build(&event_loop)
        .context("opening the window")?;
    let _view = WebViewBuilder::new().with_url(&url).build(&window).context("starting WebView2")?;
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
