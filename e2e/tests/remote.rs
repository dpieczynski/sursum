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

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn search_without_diacritics_and_show_now() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_engine(screen_engine(), 1920, 1080, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::Chromium, 1280, 800, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            let input = remote.query(By::Css(".search-input")).first().await?;
            input.wait_until().displayed().await?;
            // "przykladowa" without the Polish ł finds "Pieśń przykładowa z refrenem".
            input.send_keys("przykladowa").await?;
            let show = remote.query(By::Css("[data-testid=show-now]")).first().await?;
            show.wait_until().clickable().await?;
            show.click().await?;
            wait_for_slide(screen, "v1#").await?;
            let deck = attribute(screen, SCREEN_ROOT, "data-deck-id").await;
            anyhow::ensure!(!deck.is_empty(), "the screen shows no deck");
            // The search field is cleared and the set list is back, with the new item.
            wait_until(Duration::from_secs(5), || async {
                remote.find_all(By::Css(".set-item")).await.map(|items| items.len()).unwrap_or(0) == 4
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
async fn new_song_typed_in_the_editor_goes_on_screen() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_engine(screen_engine(), 1920, 1080, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::Chromium, 1280, 900, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            let new_song = remote.query(By::Css(".new-song")).first().await?;
            new_song.wait_until().clickable().await?;
            new_song.click().await?;
            let inputs = remote.find_all(By::Css(".editor-form input")).await?;
            inputs[0].send_keys("Pieśń wpisana z klawiatury").await?;
            let lyrics = remote.find(By::Css(".editor-form textarea")).await?;
            lyrics.clear().await?;
            lyrics.send_keys("[V1]\n Pierwsza linia nowej pieśni\n[C]\n Refren nowej pieśni").await?;
            // The server-side preview shows two slides.
            wait_until(Duration::from_secs(5), || async {
                remote.find_all(By::Css(".editor-slide")).await.map(|slides| slides.len()).unwrap_or(0) == 2
            })
            .await?;
            remote.find(By::Css(".editor-buttons .primary")).await?.click().await?;
            wait_for_slide(screen, "v1#").await?;
            wait_until(Duration::from_secs(5), || async { remote.find(By::Css(".editor")).await.is_err() })
                .await?;
            Ok(())
        })
        .await
    })
    .await?;
    let found = reqwest_like_get(&server.url("/api/songs?q=wpisana%20z%20klawiatury")).await?;
    anyhow::ensure!(found.contains("Pieśń wpisana z klawiatury"), "the new song is not searchable: {found}");
    Ok(())
}

/// A tiny HTTP GET for checks outside the browser.
async fn reqwest_like_get(url: &str) -> anyhow::Result<String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let rest = url.strip_prefix("http://").unwrap_or(url);
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let mut stream = tokio::net::TcpStream::connect(host).await?;
    stream
        .write_all(format!("GET /{path} HTTP/1.1\r\nHost: {host}\r\nConnection: close\r\n\r\n").as_bytes())
        .await?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await?;
    Ok(response)
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn editing_the_set_on_the_remote_never_changes_the_screen() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_engine(screen_engine(), 1920, 1080, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        server
            .remote("organist")
            .await?
            .command(sursum_protocol::Command::GoToItemNumber { number: 2 })
            .await?;
        wait_for_slide(screen, "v1#").await?;
        let before = (
            attribute(screen, SCREEN_ROOT, "data-deck-id").await,
            attribute(screen, SCREEN_ROOT, "data-slide-key").await,
            attribute(screen, SCREEN_ROOT, "data-transitions").await,
        );
        with_engine(Browser::Chromium, 1280, 900, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            let rows =
                || async { remote.find_all(By::Css(".set-item")).await.map(|rows| rows.len()).unwrap_or(0) };
            wait_until(Duration::from_secs(10), || async { rows().await == 3 }).await?;
            remote.find(By::XPath("//button[text()='Edytuj zestaw']")).await?.click().await?;
            // Move the last item up.
            let up_buttons = remote.find_all(By::XPath("//button[text()='↑']")).await?;
            up_buttons[2].click().await?;
            // Wait for the new snapshot to re-render the list before touching it again.
            tokio::time::sleep(Duration::from_millis(500)).await;
            // A quick tap on "remove" does nothing…
            let remove = remote.find_all(By::Css(".row-button.hold")).await?;
            remove[1].click().await?;
            tokio::time::sleep(Duration::from_millis(500)).await;
            anyhow::ensure!(rows().await == 3, "a tap must not remove an item");
            // …holding it removes the item (here: the live one).
            let live_row = remote.find(By::Css(".set-item.live .row-button.hold")).await?;
            remote.action_chain().click_and_hold_element(&live_row).perform().await?;
            tokio::time::sleep(Duration::from_millis(1000)).await;
            remote.action_chain().release().perform().await?;
            wait_until(Duration::from_secs(5), || async { rows().await == 2 }).await?;
            // Undo brings it back.
            remote.find(By::Css(".set-tool.undo")).await?.click().await?;
            wait_until(Duration::from_secs(5), || async { rows().await == 3 }).await?;
            Ok(())
        })
        .await?;
        let after = (
            attribute(screen, SCREEN_ROOT, "data-deck-id").await,
            attribute(screen, SCREEN_ROOT, "data-slide-key").await,
            attribute(screen, SCREEN_ROOT, "data-transitions").await,
        );
        anyhow::ensure!(before == after, "set edits changed the screen: {before:?} -> {after:?}");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn remote_pairs_with_the_pin_and_unlocks_admin_mode_with_it_again() -> anyhow::Result<()> {
    let server = Server::start_with_pin("2468").await?;
    with_engine(Browser::Chromium, 1280, 800, async |remote| {
        remote.goto(server.url("/remote/")).await?;
        let pin = remote.query(By::Css(".pairing .pin")).first().await?;
        pin.wait_until().displayed().await?;
        pin.send_keys("1111").await?;
        remote.find(By::Css(".pairing button")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            remote.source().await.map(|html| html.contains("Błędny PIN")).unwrap_or(false)
        })
        .await?;
        let pin = remote.find(By::Css(".pairing .pin")).await?;
        pin.send_keys("2468").await?;
        remote.find(By::Css(".pairing button")).await?.click().await?;
        // After pairing the page reloads into the remote, which connects.
        wait_until(Duration::from_secs(10), || async {
            remote.find(By::Css(".status-dot.connected")).await.is_ok()
        })
        .await?;
        // An operator sees no setup tools until admin mode is unlocked with the PIN again.
        let import = By::XPath("//label[contains(text(), 'Import z OpenSong')]");
        anyhow::ensure!(remote.find(import.clone()).await.is_err(), "import must need admin mode");
        remote.find(By::XPath("//button[text()='Tryb administratora']")).await?.click().await?;
        let pin = remote.query(By::Css(".dialog .pin")).first().await?;
        pin.wait_until().displayed().await?;
        pin.send_keys("2468").await?;
        remote.find(By::XPath("//button[text()='Odblokuj']")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async { remote.find(import.clone()).await.is_ok() }).await?;
        // The device list shows this tablet (which cannot revoke itself).
        remote.find(By::XPath("//button[text()='Urządzenia']")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            remote.find(By::Css("[data-testid=devices] .device-self")).await.is_ok()
        })
        .await?;
        remote.find(By::XPath("//button[text()='Zakończ tryb administratora']")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async { remote.find(import.clone()).await.is_err() }).await?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn pillar_zone_is_drawn_on_the_remote_and_masked_on_the_screen() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_engine(screen_engine(), 1920, 1080, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::Chromium, 1280, 900, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            wait_until(Duration::from_secs(10), || async {
                remote.find(By::Css(".status-dot.connected")).await.is_ok()
            })
            .await?;
            // Calibration needs a hold, so a stray tap during Mass cannot start it.
            let open = remote.find(By::XPath("//button[contains(text(), 'Kalibracja')]")).await?;
            remote.action_chain().click_and_hold_element(&open).perform().await?;
            tokio::time::sleep(Duration::from_millis(1000)).await;
            remote.action_chain().release().perform().await?;
            wait_until(Duration::from_secs(5), || async {
                screen.find(By::Css("[data-calibration]")).await.is_ok()
            })
            .await
            .map_err(|error| error.context("the screen never showed the test card"))?;

            // Drag a strip over the left fifth of the frame on the tablet.
            // The frame has the on-screen size; the stage inside is scaled with a transform.
            let surface = remote.find(By::Css(".calibration-frame")).await?;
            let bounds = surface.rect().await?;
            let (width, height) = (bounds.width, bounds.height);
            remote
                .action_chain()
                .move_to_element_with_offset(&surface, (-width * 0.4) as i64, (-height * 0.45) as i64)
                .click_and_hold()
                .move_by_offset((width * 0.1) as i64, (height * 0.45) as i64)
                .move_by_offset((width * 0.05) as i64, (height * 0.45) as i64)
                .release()
                .perform()
                .await?;
            wait_until(Duration::from_secs(5), || async {
                screen.find(By::Css(".sursum-calibration-zone")).await.is_ok()
            })
            .await
            .map_err(|error| error.context("the drawn zone never reached the screen"))?;

            remote.find(By::XPath("//button[text()='Zamknij']")).await?.click().await?;
            wait_until(Duration::from_secs(5), || async {
                screen.find(By::Css("[data-calibration]")).await.is_err()
            })
            .await?;
            // The zone stays as a black mask: x ≈ 0.1 × 1920 = 192 px, width ≈ 0.15 × 1920 = 288 px.
            let style = screen.find(By::Css(".sursum-mask")).await?.attr("style").await?.unwrap_or_default();
            let pixels = |name: &str| -> Option<f64> {
                let start = style.find(&format!("{name}:"))? + name.len() + 1;
                style[start..].split("px").next()?.trim().parse().ok()
            };
            let (left, mask_width) = (pixels("left").unwrap_or(-1.0), pixels("width").unwrap_or(-1.0));
            anyhow::ensure!((left - 192.0).abs() < 40.0, "mask starts at {left} px ({style})");
            anyhow::ensure!((mask_width - 288.0).abs() < 40.0, "mask is {mask_width} px wide ({style})");
            Ok(())
        })
        .await
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn display_goes_on_with_a_tap_and_to_standby_only_with_a_long_hold() -> anyhow::Result<()> {
    let projector = sursum_e2e::fake_projector().await?;
    let config = format!("[display_power]\nbackend = \"pjlink\"\nhost = \"{projector}\"\n");
    let server = Server::start_with(&[], Some(&config)).await?;
    with_engine(Browser::Chromium, 1280, 900, async |remote| {
        remote.goto(server.url("/remote/")).await?;
        let wait_for = async |text: &'static str| {
            wait_until(Duration::from_secs(10), || async { power_state(remote).await == text })
                .await
                .map_err(|error| error.context(format!("never showed {text:?}")))
        };
        wait_for("Ekran: uśpiony").await?;
        remote.find(By::Css(".power-on")).await?.click().await?;
        wait_for("Ekran: włączony").await?;
        // A tap on "standby" does nothing…
        let standby = remote.find(By::XPath("//button[contains(text(), 'Uśpij ekran')]")).await?;
        standby.click().await?;
        tokio::time::sleep(Duration::from_millis(800)).await;
        wait_for("Ekran: włączony").await?;
        // …only a long hold does.
        remote.action_chain().click_and_hold_element(&standby).perform().await?;
        tokio::time::sleep(Duration::from_millis(1800)).await;
        remote.action_chain().release().perform().await?;
        wait_for("Ekran: uśpiony").await?;
        Ok(())
    })
    .await
}

/// The display power state as the remote shows it (empty if not shown).
async fn power_state(remote: &WebDriver) -> String {
    match remote.find(By::Css("[data-testid=display-power] .power-state")).await {
        Ok(element) => element.text().await.unwrap_or_default(),
        Err(_) => String::new(),
    }
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn a_pedal_key_can_be_learned_and_page_keys_work_even_while_typing() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_engine(screen_engine(), 1920, 1080, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::Chromium, 1280, 900, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            wait_until(Duration::from_secs(10), || async {
                remote.find(By::Css(".status-dot.connected")).await.is_ok()
            })
            .await?;
            // The default page-turner key moves on.
            remote.action_chain().send_keys(Key::PageDown).perform().await?;
            wait_for_slide(screen, "v1#").await?;

            // Learn mode: "Naucz" next to "Dalej", then press the pedal (here: F8).
            remote.find(By::XPath("//button[text()='Pedał i klawisze']")).await?.click().await?;
            remote.find(By::Css("[data-action=next] button")).await?.click().await?;
            remote.action_chain().send_keys(Key::F8).perform().await?;
            wait_until(Duration::from_secs(5), || async {
                match remote.find(By::Css("[data-action=next] .key-names")).await {
                    Ok(names) => names.text().await.unwrap_or_default().contains("F8"),
                    Err(_) => false,
                }
            })
            .await?;
            remote.action_chain().send_keys(Key::F8).perform().await?;
            wait_for_slide(screen, "v2#").await?;

            // Typing a "b" in the search field does not black the screen, but PageDown still turns.
            let search = remote.find(By::Css(".search-input")).await?;
            search.send_keys("b").await?;
            tokio::time::sleep(Duration::from_millis(300)).await;
            anyhow::ensure!(
                attribute(screen, SCREEN_ROOT, "data-overlay").await == "none",
                "typing blacked out"
            );
            search.send_keys(Key::PageDown).await?;
            wait_until(Duration::from_secs(5), || async {
                !attribute(screen, SCREEN_ROOT, "data-slide-key").await.starts_with("v2#")
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
async fn the_keypad_jumps_to_an_item_number() -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    with_engine(screen_engine(), 1920, 1080, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::Chromium, 1280, 900, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            wait_until(Duration::from_secs(10), || async {
                remote.find(By::Css(".status-dot.connected")).await.is_ok()
            })
            .await?;
            remote.find(By::Css("[data-testid=jump]")).await?.click().await?;
            // "0" first is ignored; "3" then "Pokaż" shows the third item (the announcements).
            remote.find(By::XPath("//div[@data-testid='keypad']//button[text()='0']")).await?.click().await?;
            remote.find(By::XPath("//div[@data-testid='keypad']//button[text()='3']")).await?.click().await?;
            remote.find(By::Css("[data-testid=keypad-go]")).await?.click().await?;
            wait_for_slide(screen, "t#").await?;
            anyhow::ensure!(remote.find(By::Css("[data-testid=keypad]")).await.is_err(), "keypad stays open");
            Ok(())
        })
        .await
    })
    .await
}
