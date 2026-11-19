// SPDX-License-Identifier: GPL-3.0-or-later
//! Harness for the end-to-end tests: starts the real server binary, talks to it like a remote
//! over WebSocket, and opens browser sessions through a WebDriver server.
//!
//! Environment:
//! - `SURSUM_SERVER_BIN`: server binary (default: `target/debug/sursum-server`; build it with
//!   `--features e2e-hooks` for the first-frame test);
//! - `SURSUM_E2E_BROWSER`: `chromium` (default) or `wpe`, the engine under test (the screen's);
//! - `SURSUM_E2E_REMOTE_BROWSER`: `chromium` (default) or `webkitgtk`, the remotes' engine (the
//!   remote on WebKit, ADR 0021: the desktop program's Linux window is WebKitGTK; headless WPE
//!   cannot stand in, its view is fixed at 800×600);
//! - `SURSUM_CHROMIUM_BINARY`: Chromium or Chrome (default `/usr/bin/chromium`, as in the
//!   browsers image);
//! - `SURSUM_WEBDRIVER_URL`: WebDriver server (default `http://127.0.0.1:9515` for Chromium,
//!   `http://127.0.0.1:9516` for WPE, `http://127.0.0.1:9517` for WebKitGTK).
//!
//! The browsers run in the `sursum-browsers` container with host networking (`just browsers`).

use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as Process, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context as _, bail};
use futures_util::{SinkExt, StreamExt};
use sursum_protocol::{ClientId, ClientMsg, CmdOutcome, Command, PROTOCOL_VERSION, Rev, Role, ServerMsg};
use thirtyfour::prelude::*;
use tokio_tungstenite::tungstenite::Message;

/// A running server process with its own data directory.
pub struct Server {
    pub addr: SocketAddr,
    data_dir: tempfile::TempDir,
    extra_args: Vec<String>,
    child: Option<Child>,
}

/// How remotes get in on a test server.
enum Access<'a> {
    /// No PIN, development mode: every remote gets in (most tests).
    Open,
    /// Remotes must pair with this PIN.
    Pin(&'a str),
    /// No PIN yet: the first remote must set one.
    NewDevice,
    /// The desktop program's library: no screen, no PIN (ADR 0020).
    Library,
}

impl Server {
    /// Starts a server with an empty data directory.
    pub async fn start() -> anyhow::Result<Server> {
        Self::start_with(&[], None).await
    }

    /// Starts a server with extra arguments and an optional `config.toml`.
    pub async fn start_with(extra_args: &[&str], config_toml: Option<&str>) -> anyhow::Result<Server> {
        Self::start_prepared(extra_args, config_toml, Access::Open).await
    }

    /// Starts a server whose remotes must pair with `pin`.
    pub async fn start_with_pin(pin: &str) -> anyhow::Result<Server> {
        Self::start_prepared(&[], Some("transition_ms = 0\n"), Access::Pin(pin)).await
    }

    /// Starts a server with a `config.toml` whose remotes must pair with `pin`.
    pub async fn start_configured_with_pin(pin: &str, config_toml: &str) -> anyhow::Result<Server> {
        Self::start_prepared(&[], Some(config_toml), Access::Pin(pin)).await
    }

    /// Starts a server as the desktop program runs it: a library without a screen or a PIN.
    pub async fn start_library() -> anyhow::Result<Server> {
        Self::start_prepared(&[], None, Access::Library).await
    }

    /// Starts a server as freshly installed: no PIN, so the first remote must set one.
    pub async fn start_new_device() -> anyhow::Result<Server> {
        Self::start_prepared(&[], Some("transition_ms = 0\n"), Access::NewDevice).await
    }

    async fn start_prepared(
        extra_args: &[&str],
        config_toml: Option<&str>,
        access: Access<'_>,
    ) -> anyhow::Result<Server> {
        let data_dir = tempfile::tempdir()?;
        if let Some(config) = config_toml {
            std::fs::write(data_dir.path().join("config.toml"), config)?;
        }
        let mut extra_args: Vec<String> = extra_args.iter().map(|arg| (*arg).to_owned()).collect();
        match access {
            Access::Open => extra_args.push("--open-without-pin".to_owned()),
            Access::Library => extra_args.push("--library".to_owned()),
            Access::Pin(_) | Access::NewDevice => {}
        }
        if let Access::Pin(pin) = access {
            let status = Process::new(server_binary())
                .arg("--data-dir")
                .arg(data_dir.path())
                .args(["set-pin", pin])
                .stdout(Stdio::null())
                .status()?;
            anyhow::ensure!(status.success(), "set-pin failed");
        }
        let port = TcpListener::bind("127.0.0.1:0")?.local_addr()?.port();
        let mut server =
            Server { addr: SocketAddr::from(([127, 0, 0, 1], port)), data_dir, extra_args, child: None };
        server.spawn().await?;
        Ok(server)
    }

    /// The server's data directory (to plant files a real device would have).
    pub fn data_dir(&self) -> &std::path::Path {
        self.data_dir.path()
    }

    /// The URL of a page on this server, e.g. `url("/screen/")`.
    pub fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.addr)
    }

    /// Kills the process (like a crash), keeping the data directory.
    pub fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    /// Starts the process again on the same port and data directory.
    pub async fn restart(&mut self) -> anyhow::Result<()> {
        self.kill();
        self.spawn().await
    }

    async fn spawn(&mut self) -> anyhow::Result<()> {
        let child = Process::new(server_binary())
            .arg("--data-dir")
            .arg(self.data_dir.path())
            .arg("--listen")
            .arg(self.addr.to_string())
            .args(&self.extra_args)
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(self.log_path())?)
            .spawn()
            .with_context(|| format!("starting {}", server_binary().display()))?;
        self.child = Some(child);
        let listening = wait_until(Duration::from_secs(10), || async {
            tokio::net::TcpStream::connect(self.addr).await.is_ok()
        })
        .await;
        listening.with_context(|| {
            let log = std::fs::read_to_string(self.log_path()).unwrap_or_default();
            format!("the server did not start; its log:\n{log}")
        })
    }

    /// Server output (stderr), kept in the data directory for diagnosing failures.
    fn log_path(&self) -> PathBuf {
        self.data_dir.path().join("server.log")
    }

    /// Connects a scripted remote.
    pub async fn remote(&self, name: &str) -> anyhow::Result<Remote> {
        Remote::connect(self.addr, name, None).await
    }

    /// Pairs a scripted remote with `pin` (`POST /api/pair`) and connects it with the cookie.
    pub async fn paired_remote(&self, name: &str, pin: &str) -> anyhow::Result<Remote> {
        let cookie = self.pair(name, pin).await?;
        Remote::connect(self.addr, name, Some(&cookie)).await
    }

    /// Pairs a device with `pin` (`POST /api/pair`) and returns its cookie (`name=value`).
    pub async fn pair(&self, name: &str, pin: &str) -> anyhow::Result<String> {
        let body =
            serde_json::to_string(&sursum_protocol::PairRequest { pin: pin.into(), name: name.into() })?;
        let response = self.http("POST", "/api/pair", None, &body).await?;
        let cookie = response
            .lines()
            .find_map(|line| line.strip_prefix("set-cookie: "))
            .and_then(|value| value.split(';').next())
            .with_context(|| format!("pairing refused: {response}"))?;
        Ok(cookie.to_owned())
    }

    /// Revokes every paired device except the one holding `cookie`, through the device list
    /// (`GET /api/devices`, `POST /api/devices/{id}/revoke`) like an operator would.
    pub async fn revoke_others(&self, cookie: &str) -> anyhow::Result<()> {
        let response = self.http("GET", "/api/devices", Some(cookie), "").await?;
        let body = response.split_once("\r\n\r\n").map(|(_, body)| body).unwrap_or_default();
        let devices: Vec<sursum_protocol::DeviceView> =
            serde_json::from_str(body).with_context(|| format!("no device list: {response}"))?;
        for device in devices.iter().filter(|device| !device.this_device) {
            let path = format!("/api/devices/{}/revoke", device.device_id);
            let response = self.http("POST", &path, Some(cookie), "").await?;
            anyhow::ensure!(response.starts_with("HTTP/1.1 2"), "revocation refused: {response}");
        }
        Ok(())
    }

    /// One HTTP/1.1 request with a JSON body; returns the whole response (head and body).
    async fn http(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        body: &str,
    ) -> anyhow::Result<String> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(self.addr).await?;
        let cookie = cookie.map(|cookie| format!("Cookie: {cookie}\r\n")).unwrap_or_default();
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\n{cookie}Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).await?;
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        Ok(response)
    }

    /// Uploads a picture like the remote does (`POST /api/assets`) and returns its id.
    pub async fn upload_asset(&self, bytes: &[u8]) -> anyhow::Result<sursum_protocol::AssetId> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = tokio::net::TcpStream::connect(self.addr).await?;
        let head = format!(
            "POST /api/assets HTTP/1.1\r\nHost: localhost\r\nX-Sursum-Upload: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            bytes.len()
        );
        stream.write_all(head.as_bytes()).await?;
        stream.write_all(bytes).await?;
        let mut response = String::new();
        stream.read_to_string(&mut response).await?;
        let body = response.split_once("\r\n\r\n").map(|(_, body)| body).unwrap_or_default();
        serde_json::from_str(body).with_context(|| format!("upload refused: {response}"))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.kill();
    }
}

fn server_binary() -> PathBuf {
    std::env::var_os("SURSUM_SERVER_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/debug/sursum-server"))
}

/// A remote driven from the test over WebSocket (faster and more precise than clicking).
pub struct Remote {
    socket: tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    seq: u32,
}

impl Remote {
    async fn connect(addr: SocketAddr, name: &str, cookie: Option<&str>) -> anyhow::Result<Remote> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest as _;
        let mut request = format!("ws://{addr}/ws/remote").into_client_request()?;
        if let Some(cookie) = cookie {
            request.headers_mut().insert("cookie", cookie.parse()?);
        }
        let (socket, _) = tokio_tungstenite::connect_async(request).await?;
        let mut remote = Remote { socket, seq: 0 };
        let hello = ClientMsg::Hello {
            protocol: PROTOCOL_VERSION,
            role: Role::Remote,
            client_id: ClientId::from(name),
            build_id: "e2e".into(),
        };
        remote.send(&hello).await?;
        Ok(remote)
    }

    async fn send(&mut self, message: &ClientMsg) -> anyhow::Result<()> {
        self.socket.send(Message::Text(serde_json::to_string(message)?.into())).await?;
        Ok(())
    }

    /// Sends a command and waits for its outcome. Uses the highest base revision, so the
    /// `Stale` rule never interferes with scripted steps.
    pub async fn command(&mut self, cmd: Command) -> anyhow::Result<CmdOutcome> {
        self.seq += 1;
        let seq = self.seq;
        self.send(&ClientMsg::Cmd { seq, base_rev: Rev(u32::MAX), cmd }).await?;
        loop {
            let frame = tokio::time::timeout(Duration::from_secs(5), self.socket.next())
                .await?
                .context("socket closed")??;
            if let Message::Text(text) = frame
                && let ServerMsg::CmdResult { seq: got, outcome } = serde_json::from_str(text.as_str())?
                && got == seq
            {
                return Ok(outcome);
            }
        }
    }
}

/// A screen profile with `margin` on every side and one exclusion zone, set the way the
/// calibration panel sets it (`Command::SetScreenProfile`).
pub fn profile_with_zone(margin: u32, zone: sursum_protocol::Rect) -> Command {
    use sursum_protocol::{ExclusionZone, Insets, MaskMode, ScreenProfile};
    let zone = ExclusionZone { name: "Strefa 1".into(), rect: zone, mask: MaskMode::Black };
    Command::SetScreenProfile {
        profile: ScreenProfile { safe_area: Insets::uniform(margin), exclusions: vec![zone] },
    }
}

/// Which browser engine the tests drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Browser {
    Chromium,
    /// WPE WebKit through cog (ADR 0015).
    Wpe,
    /// WebKitGTK's MiniBrowser: WebKit in a window whose size can be set, for the remote
    /// (ADR 0021).
    WebKitGtk,
}

impl Browser {
    /// From `SURSUM_E2E_BROWSER`.
    pub fn from_env() -> Browser {
        Browser::named_in("SURSUM_E2E_BROWSER")
    }

    /// The remotes' engine, from `SURSUM_E2E_REMOTE_BROWSER`.
    pub fn for_remote() -> Browser {
        Browser::named_in("SURSUM_E2E_REMOTE_BROWSER")
    }

    fn named_in(variable: &str) -> Browser {
        match std::env::var(variable).as_deref() {
            Ok("wpe") => Browser::Wpe,
            Ok("webkitgtk") => Browser::WebKitGtk,
            _ => Browser::Chromium,
        }
    }

    fn webdriver_url(self) -> String {
        std::env::var("SURSUM_WEBDRIVER_URL").unwrap_or_else(|_| match self {
            Browser::Chromium => "http://127.0.0.1:9515".to_owned(),
            Browser::Wpe => "http://127.0.0.1:9516".to_owned(),
            Browser::WebKitGtk => "http://127.0.0.1:9517".to_owned(),
        })
    }

    fn capabilities(self, width: u32, height: u32) -> Capabilities {
        let value = match self {
            Browser::Chromium => serde_json::json!({
                "browserName": "chrome",
                "goog:chromeOptions": {
                    "binary": std::env::var("SURSUM_CHROMIUM_BINARY")
                        .unwrap_or_else(|_| "/usr/bin/chromium".to_owned()),
                    "args": [
                        "--headless=new", "--no-sandbox", "--hide-scrollbars", "--force-device-scale-factor=1",
                        format!("--window-size={width},{height}"),
                    ],
                },
            }),
            Browser::Wpe => serde_json::json!({
                "wpe:browserOptions": {
                    "binary": "cog",
                    "args": ["--automation", "--platform=headless", "--bg-color=black"],
                },
            }),
            Browser::WebKitGtk => serde_json::json!({
                "webkitgtk:browserOptions": {
                    "binary": "/usr/lib/x86_64-linux-gnu/webkitgtk-6.0/MiniBrowser",
                    "args": ["--automation"],
                },
            }),
        };
        let serde_json::Value::Object(map) = value else { unreachable!("literal object") };
        Capabilities::from(map)
    }

    /// Opens a browser window of the given size.
    pub async fn open(self, width: u32, height: u32) -> anyhow::Result<WebDriver> {
        let driver = WebDriver::new(self.webdriver_url(), self.capabilities(width, height)).await?;
        // WPE headless ignores the window-size flag; ask through WebDriver as well.
        let _ = driver.set_window_rect(0, 0, width, height).await;
        if self == Browser::WebKitGtk {
            // The size WebDriver sets includes the window's frame: grow it by the difference.
            let inner: (u32, u32) =
                driver.execute("return [innerWidth, innerHeight]", Vec::new()).await?.convert()?;
            let (extra_width, extra_height) = (width.saturating_sub(inner.0), height.saturating_sub(inner.1));
            if extra_width > 0 || extra_height > 0 {
                driver.set_window_rect(0, 0, width + extra_width, height + extra_height).await?;
            }
        }
        Ok(driver)
    }
}

/// Opens a browser window, runs `test` with it and always closes the session afterwards (WPE
/// WebDriver allows only one session at a time, so a leaked session would break later tests).
pub async fn with_browser(
    width: u32,
    height: u32,
    test: impl AsyncFnOnce(&WebDriver) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    with_engine(Browser::from_env(), width, height, test).await
}

/// Like [`with_browser`], but with an explicit engine (for cross-engine tests).
pub async fn with_engine(
    browser: Browser,
    width: u32,
    height: u32,
    test: impl AsyncFnOnce(&WebDriver) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let driver = browser.open(width, height).await?;
    let result = test(&driver).await;
    let _ = driver.quit().await;
    result
}

/// Polls `condition` until it holds or `timeout` passes.
pub async fn wait_until<F, Fut>(timeout: Duration, mut condition: F) -> anyhow::Result<()>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let start = Instant::now();
    while start.elapsed() < timeout {
        if condition().await {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    bail!("condition not met within {timeout:?}")
}

/// Keys that type `text`, line breaks as the Enter key: chromedriver turns "\n" into Enter, but
/// the WebDriver standard (and WebKit's driver) types nothing for it.
pub fn typed(text: &str) -> String {
    text.replace('\n', "\u{E007}")
}

/// The text an element shows (`innerText`). WebDriver's own "element text" first asks whether the
/// element is displayed, and WebKit's driver answers no for anything with `overflow: hidden`
/// (every text cut off with an ellipsis), returning "".
pub async fn shown_text(element: &WebElement) -> WebDriverResult<String> {
    Ok(element.prop("innerText").await?.unwrap_or_default())
}

/// Clicks `button` until `shown` appears, up to three times, and returns what appeared. Right
/// after a view opens, WebKit's driver can lose a click on a button that is still being laid
/// out (seen in CI on WebKitGTK); the remote itself is fine, a person taps again anyway.
pub async fn click_until_shown(driver: &WebDriver, button: By, shown: By) -> anyhow::Result<WebElement> {
    for _ in 0..3 {
        driver.find(button.clone()).await?.click().await?;
        let found =
            wait_until(Duration::from_secs(3), || async { driver.find(shown.clone()).await.is_ok() }).await;
        if found.is_ok() {
            return Ok(driver.find(shown).await?);
        }
    }
    bail!("{shown:?} did not appear after three clicks on {button:?}")
}

/// Opens a tab of the remote: "msza", "zestaw" or "ustawienia".
pub async fn open_tab(driver: &WebDriver, tab: &str) -> anyhow::Result<()> {
    driver.find(By::Css(format!("[data-testid=tab-{tab}]"))).await?.click().await?;
    Ok(())
}

/// Reads an attribute of the first element matching a CSS selector ("" if missing).
pub async fn attribute(driver: &WebDriver, selector: &str, name: &str) -> String {
    let script = format!(
        "const el = document.querySelector({selector:?}); return el ? (el.getAttribute({name:?}) || '') : '';"
    );
    match driver.execute(&script, Vec::new()).await {
        Ok(result) => result.json().as_str().unwrap_or_default().to_owned(),
        Err(_) => String::new(),
    }
}

/// Selector of the screen's root layer, which carries the test attributes.
pub const SCREEN_ROOT: &str = ".sursum-screen [data-transitions]";

/// Waits until the screen shows a slide with a given key prefix (e.g. `v1#`).
pub async fn wait_for_slide(driver: &WebDriver, key_prefix: &str) -> anyhow::Result<()> {
    wait_until(Duration::from_secs(10), || async {
        attribute(driver, SCREEN_ROOT, "data-slide-key").await.starts_with(key_prefix)
    })
    .await
    .with_context(|| format!("screen never showed a slide starting with {key_prefix:?}"))
}

/// A screenshot decoded to RGBA pixels.
pub async fn screenshot(driver: &WebDriver) -> anyhow::Result<image::RgbaImage> {
    let png = driver.screenshot_as_png().await?;
    Ok(image::load_from_memory(&png)?.to_rgba8())
}

/// Fraction of pixels that differ noticeably between two images of the same size.
pub fn difference(a: &image::RgbaImage, b: &image::RgbaImage) -> f64 {
    if a.dimensions() != b.dimensions() {
        return 1.0;
    }
    let differing = a
        .pixels()
        .zip(b.pixels())
        .filter(|(p, q)| p.0.iter().zip(q.0.iter()).any(|(x, y)| x.abs_diff(*y) > 16))
        .count();
    differing as f64 / f64::from(a.width() * a.height())
}

/// True if every pixel inside the rectangle is (nearly) black.
pub fn is_black(image: &image::RgbaImage, x: u32, y: u32, width: u32, height: u32) -> bool {
    (y..(y + height).min(image.height()))
        .flat_map(|row| (x..(x + width).min(image.width())).map(move |column| (column, row)))
        .all(|(column, row)| image.get_pixel(column, row).0[..3].iter().all(|channel| *channel < 12))
}

/// A fake PJLink projector without a password (`PJLINK 0`) that remembers its power state, for
/// testing the remote's display power buttons. Returns its address for `config.toml`.
pub async fn fake_projector() -> anyhow::Result<SocketAddr> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    let power = std::sync::Arc::new(std::sync::Mutex::new('0'));
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let power = power.clone();
            tokio::spawn(async move {
                let (reader, mut writer) = stream.into_split();
                let mut reader = BufReader::new(reader);
                if writer.write_all(b"PJLINK 0\r").await.is_err() {
                    return;
                }
                let mut line = Vec::new();
                if reader.read_until(b'\r', &mut line).await.unwrap_or(0) == 0 {
                    return;
                }
                let reply = match String::from_utf8_lossy(&line).trim_end_matches('\r') {
                    "%1POWR 1" => {
                        *power.lock().expect("not poisoned") = '1';
                        "%1POWR=OK".to_owned()
                    }
                    "%1POWR 0" => {
                        *power.lock().expect("not poisoned") = '0';
                        "%1POWR=OK".to_owned()
                    }
                    "%1POWR ?" => format!("%1POWR={}", power.lock().expect("not poisoned")),
                    _ => "%1POWR=ERR1".to_owned(),
                };
                let _ = writer.write_all(format!("{reply}\r").as_bytes()).await;
            });
        }
    });
    Ok(address)
}
