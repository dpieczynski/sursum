// SPDX-License-Identifier: GPL-3.0-or-later
//! Screen tests (run on Chromium and, per ADR 0015, on WPE WebKit). They use classic WebDriver
//! only. Ignored by default because they need a WebDriver server: run them with `just e2e`.

use std::time::Duration;

use sursum_e2e::{
    SCREEN_ROOT, Server, attribute, difference, is_black, screenshot, wait_for_slide, wait_until,
    with_browser,
};
use sursum_protocol::{Command, Overlay};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn first_frame_is_black_before_the_app_loads() -> anyhow::Result<()> {
    // The server delays the WebAssembly, so only the HTML (with its inline black style) is there.
    let server = Server::start_with(&["--wasm-delay-ms", "4000"], None).await?;
    with_browser(WIDTH, HEIGHT, async |driver| {
        driver.goto(server.url("/screen/")).await?;
        let background =
            driver.execute("return getComputedStyle(document.body).backgroundColor;", Vec::new()).await?;
        anyhow::ensure!(
            background.json().as_str() == Some("rgb(0, 0, 0)"),
            "body background is {:?}",
            background.json()
        );
        let image = screenshot(driver).await?;
        anyhow::ensure!(
            is_black(&image, 0, 0, image.width(), image.height()),
            "the first frame must be fully black"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn screen_follows_remote_commands() -> anyhow::Result<()> {
    let server = Server::start().await?;
    with_browser(WIDTH, HEIGHT, async |driver| {
        driver.goto(server.url("/screen/")).await?;
        let mut remote = server.remote("organist").await?;
        remote.command(Command::Next).await?;
        wait_for_slide(driver, "v1#").await?;
        remote.command(Command::Next).await?;
        wait_for_slide(driver, "v2#").await?;
        let image = screenshot(driver).await?;
        anyhow::ensure!(!is_black(&image, 0, 0, image.width(), image.height()), "lyrics must be visible");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn last_slide_stays_while_the_server_is_down_and_reconnect_has_no_transition() -> anyhow::Result<()> {
    let mut server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_browser(WIDTH, HEIGHT, async |driver| {
        driver.goto(server.url("/screen/")).await?;
        server.remote("organist").await?.command(Command::Next).await?;
        wait_for_slide(driver, "v1#").await?;
        let transitions = attribute(driver, SCREEN_ROOT, "data-transitions").await;
        let before = screenshot(driver).await?;

        server.kill();
        tokio::time::sleep(Duration::from_secs(2)).await;
        let during = screenshot(driver).await?;
        anyhow::ensure!(difference(&before, &during) < 0.001, "the screen changed while the server was down");

        server.restart().await?;
        // Give the client time to reconnect (backoff) and receive the snapshot.
        tokio::time::sleep(Duration::from_secs(6)).await;
        let after = screenshot(driver).await?;
        anyhow::ensure!(difference(&before, &after) < 0.001, "the screen changed after reconnecting");
        anyhow::ensure!(
            attribute(driver, SCREEN_ROOT, "data-transitions").await == transitions,
            "reconnect must not crossfade"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn one_font_size_for_all_slides_of_a_song() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_browser(WIDTH, HEIGHT, async |driver| {
        driver.goto(server.url("/screen/")).await?;
        let mut remote = server.remote("organist").await?;
        // The second demo song has verses of different length, a refrain and a bridge.
        remote.command(Command::GoToItemNumber { number: 2 }).await?;
        wait_for_slide(driver, "v1#").await?;
        let mut sizes = Vec::new();
        for _ in 0..7 {
            let key = attribute(driver, SCREEN_ROOT, "data-slide-key").await;
            sizes.push(attribute(driver, SCREEN_ROOT, "data-font-px").await);
            remote.command(Command::Next).await?;
            wait_until(Duration::from_secs(5), || async {
                attribute(driver, SCREEN_ROOT, "data-slide-key").await != key
            })
            .await?;
        }
        anyhow::ensure!(
            sizes.iter().all(|size| !size.is_empty() && size == &sizes[0]),
            "font sizes differ: {sizes:?}"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn black_overlay_hides_content_and_navigation_under_it_stays_hidden() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_browser(WIDTH, HEIGHT, async |driver| {
        driver.goto(server.url("/screen/")).await?;
        let mut remote = server.remote("organist").await?;
        remote.command(Command::Next).await?;
        wait_for_slide(driver, "v1#").await?;
        remote.command(Command::SetOverlay { overlay: Overlay::Black }).await?;
        remote.command(Command::Next).await?;
        wait_for_slide(driver, "v2#").await?;
        tokio::time::sleep(Duration::from_millis(400)).await;
        let image = screenshot(driver).await?;
        anyhow::ensure!(
            is_black(&image, 0, 0, image.width(), image.height()),
            "black overlay must hide everything"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn text_stays_out_of_exclusion_zones_and_zones_are_black() -> anyhow::Result<()> {
    let config = "transition_ms = 0\n\n[screen_profile]\nsafe_area = { top = 40, right = 40, bottom = 40, left = 40 }\n\n\
                  [[screen_profile.exclusions]]\nname = \"pillar\"\nrect = { x = 0, y = 0, width = 600, height = 1080 }\n";
    let server = Server::start_with(&[], Some(config)).await?;
    with_browser(WIDTH, HEIGHT, async |driver| {
        driver.goto(server.url("/screen/")).await?;
        server.remote("organist").await?.command(Command::Next).await?;
        wait_for_slide(driver, "v1#").await?;
        // Convert client coordinates to scene coordinates through the stage's real scale: the
        // browser window may be smaller than 1920×1080 (e.g. headless WPE).
        let measured = driver.execute(MEASURE_LINES, Vec::new()).await?;
        let measured = measured.json();
        let lefts: Vec<f64> = serde_json::from_value(measured["lefts"].clone())?;
        anyhow::ensure!(!lefts.is_empty(), "no visible lines found");
        anyhow::ensure!(lefts.iter().all(|left| *left >= 600.0), "text entered the pillar zone: {lefts:?}");
        let scale = measured["scale"].as_f64().unwrap_or(1.0);
        let left = measured["stageLeft"].as_f64().unwrap_or(0.0) as u32;
        let top = measured["stageTop"].as_f64().unwrap_or(0.0) as u32;
        let to_pixels = |scene: f64| (scene * scale).floor() as u32;
        let image = screenshot(driver).await?;
        // Sample slightly inside the zone to stay clear of anti-aliased edges.
        let zone_is_black = is_black(
            &image,
            left + 2,
            top + 2,
            to_pixels(600.0).saturating_sub(4),
            to_pixels(1080.0).saturating_sub(4),
        );
        anyhow::ensure!(zone_is_black, "the pillar zone must be black");
        Ok(())
    })
    .await
}

/// Left edges of visible text lines in scene coordinates, plus the stage's position and scale.
const MEASURE_LINES: &str = "
    const stage = document.querySelector('.sursum-screen .sursum-stage').getBoundingClientRect();
    const scale = stage.width / 1920;
    const lefts = [...document.querySelectorAll('.sursum-line')]
        .filter(line => line.closest('.sursum-measure') === null
            && getComputedStyle(line.closest('.sursum-layer')).opacity === '1')
        .map(line => {
            const range = document.createRange();
            range.selectNodeContents(line);
            return (range.getBoundingClientRect().left - stage.left) / scale;
        });
    return { lefts, stageLeft: stage.left, stageTop: stage.top, scale };
";
