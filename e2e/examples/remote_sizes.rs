// SPDX-License-Identifier: GPL-3.0-or-later
//! Screenshots of the remote on a phone, an upright tablet and a landscape tablet, as an operator
//! and in admin mode, while a song is live: `just remote-sizes [dir]` (needs `just browsers`).
//! Used to compare layouts before and after a change.

use std::path::PathBuf;
use std::time::Duration;

use sursum_e2e::{Browser, Server, wait_until};
use sursum_protocol::Command;
use thirtyfour::prelude::*;

const PIN: &str = "246813";
/// The connection indicator in the old and in the new remote.
const CONNECTED: &str = ".status-dot.connected, [data-testid=status-connected]";
const SIZES: [(&str, u32, u32); 3] = [("phone", 360, 800), ("tablet", 800, 1280), ("landscape", 1280, 800)];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let output = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| "remote-sizes".to_owned()));
    std::fs::create_dir_all(&output)?;
    let projector = sursum_e2e::fake_projector().await?;
    let config =
        format!("transition_ms = 0\n\n[display_power]\nbackend = \"pjlink\"\nhost = \"{projector}\"\n");
    let server = Server::start_configured_with_pin(PIN, &config).await?;
    let mut remote = server.paired_remote("sizes", PIN).await?;
    remote.command(Command::GoToItemNumber { number: 1 }).await?;
    remote.command(Command::Next).await?;
    let browser = Browser::from_env();
    for (name, width, height) in SIZES {
        let page = browser.open(width, height).await?;
        page.goto(server.url("/remote/")).await?;
        pair(&page).await?;
        tokio::time::sleep(Duration::from_millis(800)).await;
        page.screenshot(&output.join(format!("{name}-operator.png"))).await?;
        unlock_admin(&page).await?;
        tokio::time::sleep(Duration::from_millis(800)).await;
        page.screenshot(&output.join(format!("{name}-admin.png"))).await?;
        page.quit().await?;
    }
    println!("saved to {}", output.display());
    Ok(())
}

/// Pairs the page with the PIN; the page reloads into the remote.
async fn pair(page: &WebDriver) -> anyhow::Result<()> {
    let pin = page.query(By::Css(".pairing .pin")).first().await?;
    pin.wait_until().displayed().await?;
    pin.send_keys(PIN).await?;
    page.find(By::Css(".pairing button[type=submit]")).await?.click().await?;
    wait_until(Duration::from_secs(10), || async { page.find(By::Css(CONNECTED)).await.is_ok() }).await
}

/// Unlocks admin mode through the API from the page itself (the UI path differs between layouts).
async fn unlock_admin(page: &WebDriver) -> anyhow::Result<()> {
    let script = format!(
        "return fetch('/api/admin', {{method: 'POST', headers: {{'Content-Type': 'application/json'}}, \
         body: JSON.stringify({{pin: '{PIN}'}})}}).then(r => r.status);"
    );
    page.execute(&script, Vec::new()).await?;
    page.refresh().await?;
    wait_until(Duration::from_secs(10), || async { page.find(By::Css(CONNECTED)).await.is_ok() }).await
}
