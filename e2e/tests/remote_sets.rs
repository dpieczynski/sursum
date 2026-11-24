// SPDX-License-Identifier: GPL-3.0-or-later
//! The Zestaw view of the remote: adding songs with a search that stays open, the song card,
//! creating, renaming, switching and deleting sets. Nothing here may change the screen.
//! Ignored by default: run them with `just e2e`.

use std::time::Duration;

use anyhow::Context as _;
use sursum_e2e::{
    Browser, SCREEN_ROOT, Server, attribute, open_tab, shown_text, typed, wait_until, with_engine,
};
use thirtyfour::prelude::*;

/// The remote on the Zestaw tab, a screen showing the first item.
async fn with_sets(
    test: impl AsyncFnOnce(&Server, &WebDriver, &WebDriver) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let server = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    server.remote("organist").await?.command(sursum_protocol::Command::GoToItemNumber { number: 1 }).await?;
    with_engine(Browser::Chromium, 640, 360, async |screen| {
        screen.goto(server.url("/screen/")).await?;
        with_engine(Browser::for_remote(), 1280, 800, async |remote| {
            remote.goto(server.url("/remote/")).await?;
            remote.query(By::Css("[data-testid=tab-zestaw]")).first().await?.wait_until().clickable().await?;
            open_tab(remote, "zestaw").await?;
            remote
                .query(By::Css("[data-testid=set-items] .edit-row"))
                .first()
                .await
                .context("no set items")?;
            test(&server, screen, remote).await
        })
        .await
    })
    .await
}

async fn item_count(remote: &WebDriver) -> usize {
    remote.find_all(By::Css("[data-testid=set-items] .edit-row")).await.map(|rows| rows.len()).unwrap_or(0)
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn songs_are_added_from_a_search_that_stays_open() -> anyhow::Result<()> {
    with_sets(async |_, screen, remote| {
        let key = attribute(screen, SCREEN_ROOT, "data-slide-key").await;
        let search = remote.find(By::Css("[data-testid=sets-view] .search-input")).await?;
        search.send_keys("refren").await?;
        let add = remote.query(By::Css("[data-testid=add-song]")).first().await?;
        add.wait_until().clickable().await?;
        // Already in the demo set: marked.
        remote
            .query(By::Css("[data-testid=set-search-results] .row-badge"))
            .first()
            .await
            .context("no mark")?;
        add.click().await?;
        add.click().await?;
        remote.query(By::Css("[data-testid=toast]")).first().await.context("no confirmation")?;
        anyhow::ensure!(
            remote.find(By::Css("[data-testid=set-search-results]")).await.is_ok(),
            "search closed"
        );
        remote.find(By::Css("[data-testid=close-search]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async { item_count(remote).await == 5 })
            .await
            .context("not added twice")?;
        // A search without results offers a new song with that title.
        search.send_keys("zzzz brak").await?;
        let create = remote.query(By::Css("[data-testid=new-song-from-search]")).first().await?;
        create.wait_until().clickable().await?;
        create.click().await?;
        let title = remote.query(By::Css(".editor-form input")).first().await?;
        anyhow::ensure!(title.prop("value").await?.as_deref() == Some("zzzz brak"), "title not taken over");
        anyhow::ensure!(attribute(screen, SCREEN_ROOT, "data-slide-key").await == key, "the screen changed");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn a_hold_on_remove_survives_another_remote_changing_the_screen() -> anyhow::Result<()> {
    with_sets(async |server, _, remote| {
        let before = item_count(remote).await;
        let mut other = server.remote("second organist").await?;
        let removes = remote.find_all(By::Css("[data-testid=set-items] [data-testid=remove-item]")).await?;
        let last = removes.last().context("no remove button")?;
        remote.action_chain().click_and_hold_element(last).perform().await?;
        // Mid-hold, another remote changes what the screen shows: a new snapshot arrives.
        tokio::time::sleep(Duration::from_millis(250)).await;
        other
            .command(sursum_protocol::Command::SetOverlay { overlay: sursum_protocol::Overlay::Black })
            .await?;
        tokio::time::sleep(Duration::from_millis(750)).await;
        remote.action_chain().release().perform().await?;
        wait_until(Duration::from_secs(5), || async { item_count(remote).await == before - 1 })
            .await
            .context("the hold was cancelled by the snapshot")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn the_song_card_shows_the_words_and_adds_the_song() -> anyhow::Result<()> {
    with_sets(async |_, _, remote| {
        remote
            .find(By::Css("[data-testid=set-items] .edit-row[data-number='1'] .edit-title"))
            .await?
            .click()
            .await?;
        let card = remote.query(By::Css("[data-testid=song-card]")).first().await?;
        wait_until(Duration::from_secs(5), || async {
            card.find_all(By::Css(".card-slide")).await.map(|slides| slides.len()).unwrap_or(0) >= 2
        })
        .await
        .context("no slides on the card")?;
        card.find(By::Css("[data-testid=card-add]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async { item_count(remote).await == 4 }).await?;
        anyhow::ensure!(
            remote.find(By::Css("[data-testid=song-card]")).await.is_err(),
            "the card stays open"
        );
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn sets_are_created_renamed_switched_and_deleted() -> anyhow::Result<()> {
    with_sets(async |_, screen, remote| {
        let key = attribute(screen, SCREEN_ROOT, "data-slide-key").await;
        let name = async || {
            shown_text(&remote.find(By::Css("[data-testid=set-name]")).await?)
                .await
                .map_err(anyhow::Error::from)
        };
        remote.find(By::Css("[data-testid=change-set]")).await?.click().await?;
        remote.find(By::Css("[data-testid=new-set]")).await?.click().await?;
        let input = remote.query(By::Css("[data-testid=set-name-sheet] input")).first().await?;
        input.send_keys("Niedziela").await?;
        input.send_keys(Key::Enter).await?;
        wait_until(Duration::from_secs(5), || async { name().await.is_ok_and(|name| name == "Niedziela") })
            .await?;
        remote.find(By::Css("[data-testid=rename-set]")).await?.click().await?;
        let input = remote.query(By::Css("[data-testid=set-name-sheet] input")).first().await?;
        input.clear().await?;
        input.send_keys("Niedziela 10:00").await?;
        input.send_keys(Key::Enter).await?;
        wait_until(Duration::from_secs(5), || async {
            name().await.is_ok_and(|name| name == "Niedziela 10:00")
        })
        .await?;

        // Back to the demo set (newest first: it is second), then delete the new one.
        remote.find(By::Css("[data-testid=change-set]")).await?.click().await?;
        let picker = remote.query(By::Css("[data-testid=set-picker]")).first().await?;
        let rows = picker.find_all(By::Css(".picker-row")).await?;
        anyhow::ensure!(rows.len() == 2, "two sets expected");
        rows[1].find(By::Css("[data-testid=pick-set]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            name().await.is_ok_and(|name| name != "Niedziela 10:00")
        })
        .await?;
        remote.find(By::Css("[data-testid=change-set]")).await?.click().await?;
        let picker = remote.query(By::Css("[data-testid=set-picker]")).first().await?;
        let delete = picker.find_all(By::Css("[data-testid=delete-set]")).await?;
        remote.action_chain().click_and_hold_element(&delete[0]).perform().await?;
        tokio::time::sleep(Duration::from_millis(1000)).await;
        remote.action_chain().release().perform().await?;
        wait_until(Duration::from_secs(5), || async {
            picker.find_all(By::Css(".picker-row")).await.map(|rows| rows.len()).unwrap_or(0) == 1
        })
        .await
        .context("the set was not deleted")?;
        // The toast offers to bring it back.
        remote.find(By::Css("[data-testid=toast-undo]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            picker.find_all(By::Css(".picker-row")).await.map(|rows| rows.len()).unwrap_or(0) == 2
        })
        .await
        .context("undo did not bring the set back")?;
        anyhow::ensure!(attribute(screen, SCREEN_ROOT, "data-slide-key").await == key, "the screen changed");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn plain_text_becomes_slides_and_closing_asks_before_losing_it() -> anyhow::Result<()> {
    with_sets(async |_, _, remote| {
        remote.find(By::Css("[data-testid=new-song]")).await?.click().await?;
        let title = remote.query(By::Css("[data-testid=editor-title]")).first().await?;
        title.send_keys("Pieśń wklejona").await?;
        let lyrics = remote.find(By::Css("[data-testid=editor-lyrics]")).await?;
        lyrics
            .send_keys(typed(
                "1. Pierwsza zwrotka bez spacji\n\nRef. Refren wklejony\n\n2. Druga zwrotka\n\nRef.",
            ))
            .await?;
        wait_until(Duration::from_secs(5), || async {
            remote.find_all(By::Css(".editor-slide")).await.map(|slides| slides.len()).unwrap_or(0) == 4
        })
        .await
        .context("plain text did not become verse, refrain, verse, refrain")?;
        anyhow::ensure!(
            remote.find_all(By::Css(".editor-warnings li")).await?.is_empty(),
            "plain text must not produce warnings"
        );
        // Unsaved: closing asks first, and "back to editing" keeps the text.
        remote.find(By::Css("[data-testid=editor-close]")).await?.click().await?;
        let ask = remote.query(By::Css("[data-testid=discard]")).first().await.context("no question")?;
        ask.find(By::Css(".sheet-actions .btn:not(.btn--danger)")).await?.click().await?;
        anyhow::ensure!(
            lyrics.prop("value").await?.unwrap_or_default().contains("Refren wklejony"),
            "text lost"
        );
        // Saved: closing does not ask.
        remote.find(By::Css("[data-testid=save]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            match remote.find(By::Css("[data-testid=editor-status]")).await {
                Ok(status) => shown_text(&status).await.unwrap_or_default() == "Zapisano",
                Err(_) => false,
            }
        })
        .await?;
        remote.find(By::Css("[data-testid=editor-close]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async { remote.find(By::Css(".editor")).await.is_err() })
            .await?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn a_song_changed_in_two_places_is_reviewed_and_one_version_kept() -> anyhow::Result<()> {
    let tulce = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    let gowarzewo = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    let song = tulce.new_song("Pieśń do przejrzenia", "[V1]\n Wspólna").await?;
    tulce.send_library_to(&gowarzewo).await?;
    tulce.edit_song(&song, "[V1]\n Z Tulec").await?;
    tulce.edit_song(&song, "[V1]\n Z Tulec, druga poprawka").await?;
    gowarzewo.edit_song(&song, "[V1]\n Z Gowarzewa").await?;
    let report = gowarzewo.send_library_to(&tulce).await?;
    anyhow::ensure!(report.conflicts == ["Pieśń do przejrzenia"], "{report:?}");

    with_engine(Browser::for_remote(), 1280, 800, async |remote| {
        remote.goto(tulce.url("/remote/")).await?;
        remote.query(By::Css("[data-testid=tab-zestaw]")).first().await?.wait_until().clickable().await?;
        open_tab(remote, "zestaw").await?;
        let line = remote.query(By::Css("[data-testid=review-line]")).first().await?;
        anyhow::ensure!(shown_text(&line).await?.contains("1"), "one song to review");
        line.click().await?;
        remote.query(By::Css("[data-testid=review-song]")).first().await?.click().await?;
        let compare = remote.query(By::Css("[data-testid=review-compare]")).first().await?;
        wait_until(Duration::from_secs(5), || async {
            compare
                .find_all(By::Css("[data-testid=review-version]"))
                .await
                .is_ok_and(|found| found.len() == 2)
        })
        .await?;
        let versions = compare.find_all(By::Css("[data-testid=review-version]")).await?;
        anyhow::ensure!(shown_text(&versions[0]).await?.contains("druga poprawka"), "the shown one first");
        anyhow::ensure!(shown_text(&versions[1]).await?.contains("Z Gowarzewa"));
        versions[1].find(By::Css("[data-testid=keep-version]")).await?.click().await?;
        // The review ends: the line goes away (the remote's state says so).
        wait_until(Duration::from_secs(5), || async {
            remote.find(By::Css("[data-testid=review-line]")).await.is_err()
        })
        .await
        .context("the review line stayed")?;
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn a_set_changed_in_two_places_is_reviewed_with_an_item_added_from_the_other_version()
-> anyhow::Result<()> {
    use sursum_protocol::Command;
    let tulce = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    let gowarzewo = Server::start_with(&[], Some("transition_ms = 0\n")).await?;
    let mut organist = tulce.remote("organist").await?;
    organist.command(Command::NewSet { name: "Niedziela".into(), date: None }).await?;
    organist.command(Command::AddBlank { title: "Wejście".into() }).await?;
    let set_id = organist.state().await?.active_set.context("no active set")?.set_id;
    tulce.send_library_to(&gowarzewo).await?;
    let mut home = gowarzewo.remote("home").await?;
    home.command(Command::ActivateSet { set_id }).await?;
    home.command(Command::AddBlank { title: "Z domu".into() }).await?;
    organist.command(Command::AddBlank { title: "Z kościoła 1".into() }).await?;
    organist.command(Command::AddBlank { title: "Z kościoła 2".into() }).await?;
    let report = gowarzewo.send_library_to(&tulce).await?;
    anyhow::ensure!(report.sets_to_review == ["Niedziela"], "{report:?}");
    let state = organist.state().await?;
    anyhow::ensure!(state.active_set.is_some_and(|set| set.review), "the active set waits for review");

    with_engine(Browser::for_remote(), 1280, 800, async |remote| {
        remote.goto(tulce.url("/remote/")).await?;
        remote.query(By::Css("[data-testid=tab-zestaw]")).first().await?.wait_until().clickable().await?;
        open_tab(remote, "zestaw").await?;
        let line = remote.query(By::Css("[data-testid=set-review-line]")).first().await.context("line")?;
        line.click().await?;
        let compare = remote.query(By::Css("[data-testid=set-compare]")).first().await.context("sheet")?;
        wait_until(Duration::from_secs(5), || async {
            compare.find_all(By::Css("[data-testid=set-version]")).await.is_ok_and(|found| found.len() == 2)
        })
        .await?;
        let versions = compare.find_all(By::Css("[data-testid=set-version]")).await?;
        anyhow::ensure!(shown_text(&versions[0]).await?.contains("Z kościoła 2"), "the shown one first");
        versions[1].find(By::Css("[data-testid=add-from-version]")).await?.click().await?;
        versions[0].find(By::Css("[data-testid=keep-set-version]")).await?.click().await?;
        // The review ends with the item from home in place.
        wait_until(Duration::from_secs(5), || async {
            remote.find(By::Css("[data-testid=set-review-line]")).await.is_err()
        })
        .await
        .context("the review line stayed")?;
        let items = remote.find(By::Css("[data-testid=set-items]")).await?;
        let text = shown_text(&items).await?;
        let order: Vec<usize> = ["Wejście", "Z domu", "Z kościoła 1", "Z kościoła 2"]
            .iter()
            .map(|title| text.find(title).unwrap_or(usize::MAX))
            .collect();
        anyhow::ensure!(order.windows(2).all(|pair| pair[0] < pair[1]), "items out of order: {text}");
        Ok(())
    })
    .await
}

#[tokio::test]
#[ignore = "needs a WebDriver server (just e2e)"]
async fn a_set_is_copied_with_a_new_date_and_the_original_stays() -> anyhow::Result<()> {
    with_sets(async |_server, screen, remote| {
        let before_screen = attribute(screen, SCREEN_ROOT, "data-deck-id").await;
        let items = item_count(remote).await;
        remote.find(By::Css("[data-testid=copy-set]")).await?.click().await?;
        let sheet = remote.query(By::Css("[data-testid=set-name-sheet]")).first().await?;
        let name = sheet.find(By::Css("[data-testid=set-name-input]")).await?;
        name.clear().await?;
        name.send_keys("Następna niedziela").await?;
        // Date fields take typed dates in the browser's locale; set the value like a picker does.
        remote
            .execute(
                "const field = document.querySelector('[data-testid=set-date-input]'); \
                 field.value = '2026-11-29'; field.dispatchEvent(new Event('input', { bubbles: true }));",
                Vec::new(),
            )
            .await?;
        sheet.find(By::Css("button[type=submit]")).await?.click().await?;
        wait_until(Duration::from_secs(5), || async {
            match remote.find(By::Css("[data-testid=set-name]")).await {
                Ok(title) => shown_text(&title).await.is_ok_and(|text| text.contains("Następna niedziela")),
                Err(_) => false,
            }
        })
        .await
        .context("the copy did not become the active set")?;
        let date = shown_text(&remote.find(By::Css("[data-testid=set-date]")).await?).await?;
        anyhow::ensure!(date == "29.11.2026", "{date}");
        anyhow::ensure!(item_count(remote).await == items, "the copy has the same items");
        anyhow::ensure!(
            attribute(screen, SCREEN_ROOT, "data-deck-id").await == before_screen,
            "copying a set never changes the screen"
        );
        remote.find(By::Css("[data-testid=change-set]")).await?.click().await?;
        let rows = remote.query(By::Css("[data-testid=pick-set]")).all_from_selector_required().await?;
        anyhow::ensure!(rows.len() == 2, "the original stays");
        Ok(())
    })
    .await
}
