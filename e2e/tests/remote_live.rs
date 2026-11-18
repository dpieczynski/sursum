// SPDX-License-Identifier: GPL-3.0-or-later
//! The Msza view of the remote: the layout on a phone, an upright and a landscape tablet, controls
//! that never move, what happens without a connection, black/logo, the gap between items, the
//! button side and the live strip. Ignored by default: run them with `just e2e`.

use std::time::Duration;

use anyhow::Context as _;
use sursum_e2e::{
    Browser, SCREEN_ROOT, Server, attribute, open_tab, wait_for_slide, wait_until, with_engine,
};
use sursum_protocol::{Command, EndOfItem, SongId};
use thirtyfour::prelude::*;

/// Window sizes and the least number of set rows that must be fully visible in the Msza view.
const SIZES: [(u32, u32, u64); 3] = [(360, 800, 3), (800, 1280, 7), (1280, 800, 9)];

/// A remote page, a screen page and a scripted remote on a server with a ten-item set.
async fn with_remote(
    width: u32,
    height: u32,
    test: impl AsyncFnOnce(&mut Server, &WebDriver, &WebDriver) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    let mut control = server.remote("organist").await?;
    for _ in 0..7 {
        control.command(Command::AddSong { song_id: SongId::from("demo-synthetic-refrain") }).await?;
    }
    control.command(Command::GoToItemNumber { number: 1 }).await?;
    drop(control);
    with_engine(Browser::Chromium, 640, 360, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::Chromium, width, height, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            wait_until(Duration::from_secs(10), || async {
                remote.find(By::Css("[data-testid=status-connected]")).await.is_ok()
                    && remote.find(By::Css("[data-testid=live-set] .set-row")).await.is_ok()
                    && remote.find(By::Css("[data-testid=screen-missing]")).await.is_err()
            })
            .await
            .context("the remote never connected (or never saw the screen)")?;
            test(&mut server, screen, remote).await
        })
        .await
    })
    .await
}

async fn number(driver: &WebDriver, script: &str) -> anyhow::Result<f64> {
    Ok(driver.execute(script, Vec::new()).await?.json().as_f64().unwrap_or(f64::NAN))
}

async fn truth(driver: &WebDriver, script: &str) -> anyhow::Result<bool> {
    Ok(driver.execute(script, Vec::new()).await?.json().as_bool().unwrap_or(false))
}

/// Left, top, width, height of the first element matching `selector`.
async fn rect(driver: &WebDriver, selector: &str) -> anyhow::Result<(i64, i64, i64, i64)> {
    let rect = driver.find(By::Css(selector)).await?.rect().await?;
    Ok((rect.x as i64, rect.y as i64, rect.width as i64, rect.height as i64))
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn the_msza_view_fits_phones_and_tablets() -> anyhow::Result<()> {
    for (width, height, rows) in SIZES {
        with_remote(width, height, async |_, _, remote| {
            let size = format!("{width}×{height}");
            let visible = number(
                remote,
                "const list = document.querySelector('[data-testid=live-set]').getBoundingClientRect();
                 return [...document.querySelectorAll('[data-testid=live-set] .set-row')].filter(row => {
                     const box = row.getBoundingClientRect();
                     return box.top >= list.top - 1 && box.bottom <= list.bottom + 1;
                 }).length;",
            )
            .await?;
            let debug = remote
                .execute(
                    "const q = s => { const r = document.querySelector(s)?.getBoundingClientRect(); return r ? [Math.round(r.top), Math.round(r.height)] : null; };
                     return JSON.stringify({inner: innerHeight, top: q('.area-top'), now: q('.area-now'), slides: q('.area-slides'),
                         content: q('.area-content'), list: q('[data-testid=live-set]'), pad: q('.area-pad'), row: q('.set-row')});",
                    Vec::new(),
                )
                .await?;
            anyhow::ensure!(
                visible >= rows as f64,
                "{size}: only {visible} set rows visible, want {rows}: {}",
                debug.json()
            );
            let fits = truth(
                remote,
                "return [...document.querySelectorAll('.area-pad button')]
                     .every(button => button.getBoundingClientRect().bottom <= innerHeight + 1)
                     && document.documentElement.scrollWidth <= innerWidth;",
            )
            .await?;
            anyhow::ensure!(fits, "{size}: the controls leave the window or the page scrolls sideways");
            let smallest = number(
                remote,
                "return Math.min(...[...document.querySelectorAll('.area-pad button, .set-row, .slide-tile')]
                     .map(element => element.getBoundingClientRect().height));",
            )
            .await?;
            anyhow::ensure!(smallest >= 55.5, "{size}: a control is only {smallest} px high");
            Ok(())
        })
        .await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn the_controls_never_move_and_stop_without_a_connection() -> anyhow::Result<()> {
    with_remote(360, 800, async |server, _, remote| {
        let before =
            (rect(remote, "[data-testid=next]").await?, rect(remote, ".area-pad [data-testid=black]").await?);
        let mut control = server.remote("organist").await?;
        control.command(Command::GoToItemNumber { number: 3 }).await?;
        control.command(Command::ToggleBlack).await?;
        drop(control);
        tokio::time::sleep(Duration::from_millis(500)).await;
        server.kill();
        wait_until(Duration::from_secs(10), || async {
            remote.find(By::Css("[data-testid=disconnected]")).await.is_ok()
        })
        .await
        .context("no banner without a connection")?;
        let after =
            (rect(remote, "[data-testid=next]").await?, rect(remote, ".area-pad [data-testid=black]").await?);
        anyhow::ensure!(before == after, "the controls moved: {before:?} -> {after:?}");
        let next = remote.find(By::Css("[data-testid=next]")).await?;
        anyhow::ensure!(!next.is_enabled().await?, "commands must not be possible without a connection");
        server.restart().await?;
        wait_until(Duration::from_secs(15), || async { next.is_enabled().await.unwrap_or(false) })
            .await
            .context("the controls never came back")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn black_turns_into_show_text_and_a_tapped_item_lifts_it() -> anyhow::Result<()> {
    with_remote(1280, 800, async |_, screen, remote| {
        let cover = remote.find(By::Css(".area-pad [data-testid=black]")).await?;
        cover.click().await?;
        wait_until(Duration::from_secs(5), || async {
            attribute(screen, SCREEN_ROOT, "data-overlay").await == "black"
        })
        .await?;
        // The remote gets its snapshot independently of the screen.
        wait_until(Duration::from_secs(5), || async {
            cover.text().await.is_ok_and(|text| text == "Pokaż tekst")
        })
        .await
        .context("the button must offer to show the text again")?;
        // Green means "the congregation sees this": not while black.
        anyhow::ensure!(
            remote.find(By::Css("[data-testid=screen-status] .status-dot.live")).await.is_err(),
            "the status dot stays green over black"
        );
        // "Dalej" moves on under black…
        let key = attribute(screen, SCREEN_ROOT, "data-slide-key").await;
        remote.find(By::Css("[data-testid=next]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            attribute(screen, SCREEN_ROOT, "data-slide-key").await != key
        })
        .await?;
        anyhow::ensure!(attribute(screen, SCREEN_ROOT, "data-overlay").await == "black", "next lifted black");
        // …but choosing an item shows it.
        remote.find(By::Css("[data-testid=live-set] .set-row[data-number='2']")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            attribute(screen, SCREEN_ROOT, "data-overlay").await == "none"
        })
        .await
        .context("tapping an item must lift black")?;
        wait_until(Duration::from_secs(5), || async {
            remote.find(By::Css("[data-testid=screen-status] .status-dot.live")).await.is_ok()
        })
        .await
        .context("the status dot must turn green when the text is back")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn next_shows_a_gap_before_the_next_song_and_says_so() -> anyhow::Result<()> {
    with_remote(1280, 800, async |server, screen, remote| {
        let caption = async || {
            remote
                .find(By::Css("[data-testid=next-caption]"))
                .await?
                .text()
                .await
                .map_err(anyhow::Error::from)
        };
        let next = remote.find(By::Css("[data-testid=next]")).await?;
        // One tap, then wait until the remote shows the new state (the caption always changes).
        let step = async || -> anyhow::Result<()> {
            let before = caption().await?;
            next.click().await?;
            wait_until(Duration::from_secs(5), || async { caption().await.is_ok_and(|now| now != before) })
                .await
                .context("the caption did not change after a tap")
        };
        // Walk to the last slide of the first song.
        while !caption().await?.contains("przerwa") {
            step().await?;
        }
        step().await?;
        wait_until(Duration::from_secs(5), || async {
            attribute(screen, SCREEN_ROOT, "data-slide-key").await.ends_with("~gap")
        })
        .await
        .context("no empty slide after the song")?;
        anyhow::ensure!(caption().await?.starts_with("→ 2."), "the caption must name the next song");
        next.click().await?;
        wait_for_slide(screen, "v1#").await?;

        // Turned off: straight to the next item.
        let mut control = server.remote("organist").await?;
        control.command(Command::SetEndOfItem { mode: EndOfItem::Continue }).await?;
        control.command(Command::GoToItemNumber { number: 3 }).await?;
        wait_for_slide(screen, "t#").await?;
        // Walk to the last slide of item 3, which now leads straight to item 4.
        for _ in 0..10 {
            if caption().await?.starts_with("→ 4.") {
                break;
            }
            step().await?;
        }
        next.click().await?;
        wait_for_slide(screen, "v1#").await.context("no gap expected when turned off")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn next_stops_at_the_end_of_the_set() -> anyhow::Result<()> {
    with_remote(1280, 800, async |server, screen, remote| {
        let mut control = server.remote("organist").await?;
        control.command(Command::GoToItemNumber { number: 10 }).await?;
        wait_for_slide(screen, "v1#").await?;
        let next = remote.find(By::Css("[data-testid=next]")).await?;
        wait_until(Duration::from_secs(10), || async {
            if next.is_enabled().await.unwrap_or(false) {
                let _ = next.click().await;
                tokio::time::sleep(Duration::from_millis(200)).await;
                false
            } else {
                true
            }
        })
        .await
        .context("next never stopped")?;
        let caption = remote.find(By::Css("[data-testid=next-caption]")).await?.text().await?;
        anyhow::ensure!(caption == "Koniec zestawu", "caption at the end: {caption:?}");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn the_controls_can_be_moved_to_the_left_and_stay_there() -> anyhow::Result<()> {
    with_remote(1280, 800, async |_, _, remote| {
        open_tab(remote, "ustawienia").await?;
        remote.find(By::Css("[data-testid=hand-left]")).await?.click().await?;
        open_tab(remote, "msza").await?;
        let left_of_list = async || -> anyhow::Result<bool> {
            let (next_x, ..) = rect(remote, "[data-testid=next]").await?;
            let (list_x, ..) = rect(remote, "[data-testid=live-set]").await?;
            Ok(next_x < list_x)
        };
        anyhow::ensure!(left_of_list().await?, "the controls did not move to the left");
        remote.refresh().await?;
        remote.query(By::Css("[data-testid=next]")).first().await?.wait_until().displayed().await?;
        anyhow::ensure!(left_of_list().await?, "the side was not remembered");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn black_is_one_tap_away_from_other_views_on_a_phone() -> anyhow::Result<()> {
    with_remote(360, 800, async |_, screen, remote| {
        open_tab(remote, "zestaw").await?;
        let strip = remote.find(By::Css("[data-testid=live-strip]")).await?;
        anyhow::ensure!(strip.is_displayed().await?, "no live strip outside the Msza view");
        strip.find(By::Css("[data-testid=black]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            attribute(screen, SCREEN_ROOT, "data-overlay").await == "black"
        })
        .await
        .context("the strip did not black the screen")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn search_adds_without_closing_and_shows_at_once() -> anyhow::Result<()> {
    with_remote(1280, 800, async |_, screen, remote| {
        remote.find(By::Css("[data-testid=search]")).await?.click().await?;
        let input = remote.find(By::Css("[data-testid=search-sheet] .search-input")).await?;
        input.send_keys("zorze").await?;
        let add = remote.query(By::Css("[data-testid=add-to-set]")).first().await?;
        add.wait_until().clickable().await?;
        add.click().await?;
        remote.query(By::Css("[data-testid=toast]")).first().await.context("no confirmation")?;
        anyhow::ensure!(
            remote.find(By::Css("[data-testid=search-sheet]")).await.is_ok(),
            "adding closed the search"
        );
        input.send_keys("zorze").await?;
        let show = remote.query(By::Css("[data-testid=show-now]")).first().await?;
        show.wait_until().clickable().await?;
        show.click().await?;
        wait_until(Duration::from_secs(5), || async {
            remote.find(By::Css("[data-testid=search-sheet]")).await.is_err()
        })
        .await
        .context("showing must close the search")?;
        wait_for_slide(screen, "v1#").await?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn the_gap_can_be_turned_off_in_the_settings() -> anyhow::Result<()> {
    with_remote(1280, 800, async |server, _, remote| {
        open_tab(remote, "ustawienia").await?;
        remote.find(By::Css("[data-testid=end-continue]")).await?.click().await?;
        open_tab(remote, "msza").await?;
        let mut control = server.remote("organist").await?;
        let last = remote.find_all(By::Css(".slide-tile")).await?.len() as u32 - 1;
        control.command(Command::GoToSlide { slide_index: last }).await?;
        wait_until(Duration::from_secs(5), || async {
            match remote.find(By::Css("[data-testid=next-caption]")).await {
                Ok(caption) => caption.text().await.unwrap_or_default().starts_with("→ 2."),
                Err(_) => false,
            }
        })
        .await
        .context("next should go straight to the next song")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn the_controls_work_in_browsers_that_cannot_vibrate() -> anyhow::Result<()> {
    with_remote(800, 1280, async |_, screen, remote| {
        // Safari and Firefox have no `navigator.vibrate`; calling it used to abort every press.
        let missing = remote
            .execute("delete Navigator.prototype.vibrate; return typeof navigator.vibrate;", Vec::new())
            .await?;
        anyhow::ensure!(missing.json().as_str() == Some("undefined"), "{:?}", missing.json());
        let key = attribute(screen, SCREEN_ROOT, "data-slide-key").await;
        remote.find(By::Css(".area-pad [data-testid=next]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            attribute(screen, SCREEN_ROOT, "data-slide-key").await != key
        })
        .await
        .context("next did nothing")?;
        remote.find(By::Css(".area-pad [data-testid=black]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            attribute(screen, SCREEN_ROOT, "data-overlay").await == "black"
        })
        .await
        .context("black did nothing")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn black_is_one_tap_away_while_a_sheet_is_open() -> anyhow::Result<()> {
    for (width, height, _) in SIZES {
        with_remote(width, height, async |_, screen, remote| {
            let size = format!("{width}×{height}");
            remote.find(By::Css("[data-testid=search]")).await?.click().await?;
            remote.find(By::Css("[data-testid=search-sheet]")).await?;
            // The sheet takes the typing at once (no tap into the field first).
            wait_until(Duration::from_secs(5), || async {
                truth(
                    remote,
                    "return document.activeElement?.matches('[data-testid=search-sheet] .search-input')",
                )
                .await
                .unwrap_or(false)
            })
            .await
            .with_context(|| format!("{size}: the search field did not get the cursor"))?;
            // WebDriver refuses to click an element that something else covers.
            remote
                .find(By::Css(".area-pad [data-testid=black]"))
                .await?
                .click()
                .await
                .with_context(|| format!("{size}: the sheet covers the black button"))?;
            wait_until(Duration::from_secs(5), || async {
                attribute(screen, SCREEN_ROOT, "data-overlay").await == "black"
            })
            .await
            .with_context(|| format!("{size}: black did not reach the screen"))?;
            anyhow::ensure!(
                remote.find(By::Css("[data-testid=search-sheet]")).await.is_ok(),
                "{size}: blacking out closed the search"
            );
            Ok(())
        })
        .await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn the_next_slide_preview_fits_when_the_tablet_turns_upright() -> anyhow::Result<()> {
    with_remote(1280, 800, async |_, _, remote| {
        let next = "[data-testid=preview-next]";
        anyhow::ensure!(
            !remote.find(By::Css(next)).await?.is_displayed().await?,
            "the next slide preview should be hidden on a landscape tablet"
        );
        remote.set_window_rect(0, 0, 800, 1280).await?;
        let sizes = async || {
            (
                attribute(remote, "[data-testid=preview-live] [data-font-px]", "data-font-px").await,
                attribute(remote, "[data-testid=preview-next] [data-font-px]", "data-font-px").await,
            )
        };
        wait_until(Duration::from_secs(5), || async {
            let (live, next) = sizes().await;
            !live.is_empty() && live == next
        })
        .await
        .context("the next slide keeps the size it got while hidden")?;
        Ok(())
    })
    .await
}
