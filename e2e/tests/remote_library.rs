// SPDX-License-Identifier: GPL-3.0-or-later
//! The remote in the desktop program's library (ADR 0020): no screen, no service, no PIN; songs,
//! the editor, sets for any church of the parish (ADR 0025) and the library file. The server runs as `sursum` runs it (`--library`).
//! Ignored by default: run them with `just e2e`.

use std::time::Duration;

use anyhow::Context as _;
use sursum_e2e::{Browser, Server, open_tab, shown_text, typed, wait_until, with_engine};
use thirtyfour::prelude::*;

async fn shown(driver: &WebDriver, selector: &str) -> bool {
    match driver.find(By::Css(selector)).await {
        Ok(element) => element.is_displayed().await.unwrap_or(false),
        Err(_) => false,
    }
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn the_library_offers_songs_sets_and_settings_only() -> anyhow::Result<()> {
    let server = Server::start_library().await?;
    with_engine(Browser::for_remote(), 1280, 800, async |remote| {
        remote.goto(server.url("/remote/")).await?;
        remote.query(By::Css("[data-testid=songs-view]")).first().await.context("no Pieśni view")?;
        anyhow::ensure!(shown(remote, "[data-testid=tab-piesni]").await, "no Pieśni tab");
        anyhow::ensure!(shown(remote, "[data-testid=tab-zestaw]").await, "no Zestaw tab");
        for absent in ["[data-testid=tab-msza]", "[data-testid=screen-status]"] {
            anyhow::ensure!(!shown(remote, absent).await, "{absent} is shown in the library");
        }
        // Nothing about a screen, even though no screen is connected.
        anyhow::ensure!(!shown(remote, "[data-testid=screen-missing]").await, "screen banner shown");

        open_tab(remote, "ustawienia").await?;
        remote.query(By::Css("a[href='/api/library/export']")).first().await.context("no export")?;
        // Restoring a backup needs a service manager, which the desktop program does not have.
        for absent in
            ["[data-testid=calibrate]", "[data-testid=settings-power]", "input[accept='.sursum-backup']"]
        {
            anyhow::ensure!(!shown(remote, absent).await, "{absent} is shown in the library");
        }
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn a_song_written_in_the_library_can_be_found_and_edited() -> anyhow::Result<()> {
    let server = Server::start_library().await?;
    with_engine(Browser::for_remote(), 1280, 800, async |remote| {
        remote.goto(server.url("/remote/")).await?;
        let new_song = remote.query(By::Css("[data-testid=new-song]")).first().await?;
        new_song.wait_until().clickable().await?;
        new_song.click().await?;
        remote.find(By::Css("[data-testid=editor-title]")).await?.send_keys("Pieśń napisana w domu").await?;
        let lyrics = remote.find(By::Css("[data-testid=editor-lyrics]")).await?;
        lyrics.clear().await?;
        lyrics.send_keys(typed("1. Pierwsza zwrotka z domu\n\nRef. Refren z domu")).await?;
        anyhow::ensure!(!shown(remote, "[data-testid=save-and-show]").await, "nothing to show it on");
        remote.find(By::Css("[data-testid=save]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            let status = remote.find(By::Css("[data-testid=editor-status]")).await;
            match status {
                Ok(status) => !shown_text(&status).await.unwrap_or_default().is_empty(),
                Err(_) => false,
            }
        })
        .await
        .context("no save status")?;
        remote.find(By::Css("[data-testid=editor-close]")).await?.click().await?;

        let search = remote.query(By::Css("[data-testid=library-search]")).first().await?;
        search.send_keys("napisana w domu").await?;
        let song = remote.query(By::Css("[data-testid=library-song]")).first().await.context("not found")?;
        song.click().await?;
        let title = remote.query(By::Css("[data-testid=editor-title]")).first().await?;
        wait_until(Duration::from_secs(5), || async {
            title.prop("value").await.ok().flatten().as_deref() == Some("Pieśń napisana w domu")
        })
        .await
        .context("the editor did not open the saved song")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn a_set_prepared_at_home_for_a_church_appears_there_after_loading_the_file() -> anyhow::Result<()> {
    use sursum_protocol::Command;
    let church = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    let mut organist = church.remote("organist").await?;
    organist.command(Command::NameChurch { name: "Tulce".into() }).await?;
    let home = Server::start_library().await?;
    church.send_library_to(&home).await?;

    with_engine(Browser::for_remote(), 1280, 800, async |remote| {
        remote.goto(home.url("/remote/")).await?;
        remote.query(By::Css("[data-testid=tab-zestaw]")).first().await?.wait_until().clickable().await?;
        open_tab(remote, "zestaw").await?;
        let choice = remote.query(By::Css("[data-testid=church-choice]")).first().await?;
        thirtyfour::components::SelectElement::new(&choice).await?.select_by_visible_text("Tulce").await?;
        // The Zestaw view shows nothing that would go on a screen.
        anyhow::ensure!(!shown(remote, "[data-testid=card-show-now]").await, "Pokaż teraz in the library");
        remote.find(By::Css("[data-testid=change-set]")).await?.click().await?;
        remote.find(By::Css("[data-testid=new-set]")).await?.click().await?;
        let input = remote.query(By::Css("[data-testid=set-name-input]")).first().await?;
        input.send_keys(typed("Adwent")).await?;
        input.send_keys(Key::Enter).await?;
        wait_until(Duration::from_secs(5), || async {
            match remote.find(By::Css("[data-testid=set-name]")).await {
                Ok(name) => shown_text(&name).await.unwrap_or_default().starts_with("Adwent"),
                Err(_) => false,
            }
        })
        .await
        .context("the new set is not active")?;
        Ok(())
    })
    .await?;

    home.send_library_to(&church).await?;
    let state = organist.state().await?;
    anyhow::ensure!(
        state.sets.iter().any(|set| set.name == "Adwent"),
        "the set did not arrive: {:?}",
        state.sets
    );
    Ok(())
}
