// SPDX-License-Identifier: GPL-3.0-or-later
//! Remote tests: operating the screen from the remote page, and the preview matching the screen.
//! The remote targets Blink browsers (Android Chrome, desktop Chromium/Edge), so these run on
//! Chromium. Ignored by default: run them with `just e2e`.

use std::time::Duration;

use sursum_e2e::{Browser, SCREEN_ROOT, Server, attribute, wait_for_slide, wait_until, with_engine};
use thirtyfour::prelude::*;

const PREVIEW_LIVE: &str = "[data-testid=preview-live] [data-font-px]";

/// The remote is always Chromium; the screen uses the engine under test.
fn screen_engine() -> Browser {
    Browser::from_env()
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn buttons_on_the_remote_drive_the_screen() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_engine(screen_engine(), 1920, 1080, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::Chromium, 1280, 800, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            let next = remote.query(By::Css("[data-testid=next]")).first().await?;
            next.wait_until().clickable().await?;
            // Wait for the connection before clicking.
            wait_until(Duration::from_secs(10), || async {
                remote.find(By::Css(".status-dot.connected")).await.is_ok()
            })
            .await?;
            next.click().await?;
            wait_for_slide(screen, "v1#").await?;
            remote.find(By::Css("[data-testid=black]")).await?.click().await?;
            wait_until(Duration::from_secs(5), || async {
                attribute(screen, SCREEN_ROOT, "data-overlay").await == "black"
            })
            .await?;
            // Black is a toggle: one more tap shows the slide again.
            remote.find(By::Css("[data-testid=black]")).await?.click().await?;
            wait_until(Duration::from_secs(5), || async {
                attribute(screen, SCREEN_ROOT, "data-overlay").await == "none"
            })
            .await?;
            Ok(())
        })
        .await
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn preview_on_the_remote_matches_the_screen() -> anyhow::Result<()> {
    let config = "transition_ms = 0\n\n[screen_profile]\nsafe_area = { top = 48, right = 48, bottom = 48, left = 48 }\n\n\
                  [[screen_profile.exclusions]]\nname = \"pillar\"\nrect = { x = 1500, y = 0, width = 420, height = 1080 }\n";
    let server = Server::start_with(&[], Some(config)).await?;
    with_engine(screen_engine(), 1920, 1080, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::Chromium, 1280, 800, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            let mut control = server.remote("organist").await?;
            // Walk through the second song (verses of different lengths) and compare each slide.
            control.command(sursum_protocol::Command::GoToItemNumber { number: 2 }).await?;
            wait_for_slide(screen, "v1#").await?;
            for _ in 0..4 {
                let key = attribute(screen, SCREEN_ROOT, "data-slide-key").await;
                let screen_size = attribute(screen, SCREEN_ROOT, "data-font-px").await;
                let screen_lines = visible_line_count(screen).await?;
                wait_until(Duration::from_secs(5), || async {
                    attribute(remote, PREVIEW_LIVE, "data-font-px").await == screen_size
                        && attribute(remote, PREVIEW_LIVE, "data-line-count").await
                            == screen_lines.to_string()
                })
                .await
                .map_err(|_| anyhow::anyhow!("preview differs from the screen on slide {key}"))?;
                control.command(sursum_protocol::Command::Next).await?;
                wait_until(Duration::from_secs(5), || async {
                    attribute(screen, SCREEN_ROOT, "data-slide-key").await != key
                })
                .await?;
            }
            Ok(())
        })
        .await
    })
    .await
}

/// Number of text lines on the slide the screen shows (the opaque layer).
async fn visible_line_count(driver: &WebDriver) -> anyhow::Result<u64> {
    let count = driver
        .execute(
            "return [...document.querySelectorAll('.sursum-line')]
                 .filter(line => line.closest('.sursum-measure') === null
                     && getComputedStyle(line.closest('.sursum-layer')).opacity === '1').length;",
            Vec::new(),
        )
        .await?;
    Ok(count.json().as_u64().unwrap_or(0))
}
