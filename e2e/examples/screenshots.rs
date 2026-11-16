// SPDX-License-Identifier: GPL-3.0-or-later
//! Saves screenshots of the screen and the remote for a quick visual check:
//! `cargo run -p sursum-e2e --example screenshots -- <output-dir>` (needs a WebDriver server).

use std::path::PathBuf;
use std::time::Duration;

use sursum_e2e::{Browser, Server, wait_for_slide};
use sursum_protocol::Command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let output = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "screenshots".to_owned()));
    std::fs::create_dir_all(&output)?;
    let config = "transition_ms = 0\n\n[screen_profile]\nsafe_area = { top = 48, right = 48, bottom = 48, left = 48 }\n\n\
                  [[screen_profile.exclusions]]\nname = \"Strefa 1\"\nrect = { x = 1500, y = 0, width = 420, height = 1080 }\n";
    // A fake PJLink projector, so the display power controls show too.
    let projector = sursum_e2e::fake_projector().await?;
    let config = format!("{config}\n[display_power]\nbackend = \"pjlink\"\nhost = \"{projector}\"\n");
    let server = Server::start_with(&[], Some(&config)).await?;
    let browser = Browser::from_env();
    let screen = browser.open(1920, 1080).await?;
    screen.goto(server.url("/screen/")).await?;
    let remote_page = browser.open(1280, 900).await?;
    remote_page.goto(server.url("/remote/")).await?;

    let mut remote = server.remote("screenshots").await?;
    remote.command(Command::GoToItemNumber { number: 2 }).await?;
    remote.command(Command::Next).await?;
    wait_for_slide(&screen, "c#").await?;
    tokio::time::sleep(Duration::from_millis(800)).await;
    screen.screenshot(&output.join("screen.png")).await?;
    remote_page.screenshot(&output.join("remote.png")).await?;
    // The search sheet.
    remote_page.find(thirtyfour::By::Css("[data-testid=search]")).await?.click().await?;
    let search = remote_page.find(thirtyfour::By::Css(".search-input")).await?;
    search.send_keys("zorze").await?;
    tokio::time::sleep(Duration::from_millis(600)).await;
    remote_page.screenshot(&output.join("remote-search.png")).await?;
    remote_page
        .find(thirtyfour::By::Css("[data-testid=search-sheet] .sheet-header button"))
        .await?
        .click()
        .await?;
    // The Zestaw view and the editor with a song from the library.
    sursum_e2e::open_tab(&remote_page, "zestaw").await?;
    tokio::time::sleep(Duration::from_millis(400)).await;
    remote_page.screenshot(&output.join("set.png")).await?;
    let search = remote_page.find(thirtyfour::By::Css(".search-input")).await?;
    search.send_keys("zorze").await?;
    tokio::time::sleep(Duration::from_millis(600)).await;
    remote_page.find(thirtyfour::By::Css(".result-action")).await?.click().await?;
    tokio::time::sleep(Duration::from_millis(1200)).await;
    remote_page.screenshot(&output.join("editor.png")).await?;
    remote_page.find(thirtyfour::By::XPath("//button[text()='Zamknij']")).await?.click().await?;
    // The Ustawienia view.
    sursum_e2e::open_tab(&remote_page, "ustawienia").await?;
    tokio::time::sleep(Duration::from_millis(400)).await;
    remote_page.screenshot(&output.join("settings.png")).await?;
    // Calibration: the test card on the screen and the panel on the remote.
    remote.command(Command::Calibrate { on: true }).await?;
    let open = remote_page.find(thirtyfour::By::XPath("//button[contains(text(), 'Kalibracja')]")).await?;
    remote_page.action_chain().click_and_hold_element(&open).perform().await?;
    tokio::time::sleep(Duration::from_millis(1000)).await;
    remote_page.action_chain().release().perform().await?;
    tokio::time::sleep(Duration::from_millis(800)).await;
    screen.screenshot(&output.join("screen-calibration.png")).await?;
    remote_page.screenshot(&output.join("calibration.png")).await?;
    remote.command(Command::Calibrate { on: false }).await?;
    // A tablet held upright.
    let portrait = browser.open(800, 1280).await?;
    portrait.goto(server.url("/remote/")).await?;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    portrait.screenshot(&output.join("remote-portrait.png")).await?;
    portrait.quit().await?;
    screen.quit().await?;
    remote_page.quit().await?;
    println!("saved to {}", output.display());
    Ok(())
}
