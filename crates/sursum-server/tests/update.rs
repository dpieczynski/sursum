// SPDX-License-Identifier: GPL-3.0-or-later
//! Updates from the remote: a signed bundle uploaded over HTTP becomes the current release on
//! trial and the server asks to be restarted; anything else changes nothing.

#![cfg(unix)]

use std::time::Duration;

use sursum_protocol::{InstallOutcome, InstallRefusal, UpdateStatus};
use sursum_server::bundle;
use sursum_server::{Config, Platform, RunningServer};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

/// A platform whose service manager would restart the server.
struct Supervised(bool);

impl Platform for Supervised {
    fn boot_id(&self) -> Option<String> {
        Some("boot-1".into())
    }

    fn supervised(&self) -> bool {
        self.0
    }
}

async fn start(
    data_dir: &std::path::Path,
    key: Option<&ed25519_dalek::SigningKey>,
    supervised: bool,
) -> RunningServer {
    let mut config = Config::with_data_dir(data_dir);
    config.listen = "127.0.0.1:0".parse().unwrap();
    config.open_without_pin = true;
    config.bundle_key = key.map(ed25519_dalek::SigningKey::verifying_key);
    sursum_server::start(config, &Supervised(supervised)).await.unwrap()
}

async fn request(server: &RunningServer, head: &str, body: &[u8]) -> String {
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or_default()
}

async fn upload(server: &RunningServer, bundle: &[u8]) -> InstallOutcome {
    let head = format!(
        "POST /api/update HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/octet-stream\r\nX-Sursum-Upload: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bundle.len()
    );
    serde_json::from_str(&request(server, &head, bundle).await).unwrap()
}

async fn status(server: &RunningServer) -> UpdateStatus {
    let head = "GET /api/update/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    serde_json::from_str(&request(server, head, b"").await).unwrap()
}

fn key() -> ed25519_dalek::SigningKey {
    bundle::signing_key(&bundle::new_key().unwrap().0).unwrap()
}

#[tokio::test]
async fn a_signed_upload_becomes_the_release_on_trial_and_restarts_the_server() {
    let dir = tempfile::tempdir().unwrap();
    let key = key();
    let server = start(dir.path(), Some(&key), true).await;
    assert!(status(&server).await.can_install);

    let bundle = bundle::create(b"new server", "0.2.0", "b", &key).unwrap();
    assert_eq!(upload(&server, &bundle).await, InstallOutcome::Installed { version: "0.2.0".into() });
    tokio::time::timeout(Duration::from_secs(5), server.restart_requested())
        .await
        .expect("the server asks to be restarted into the update");
    let installed = std::fs::read(dir.path().join("current/sursum-server")).unwrap();
    assert_eq!(installed, b"new server");
    let status = status(&server).await;
    assert_eq!(status.release.as_deref(), Some("0.2.0"));
    assert!(status.on_trial);

    let again = upload(&server, &bundle).await;
    assert_eq!(again, InstallOutcome::Refused { reason: InstallRefusal::AlreadyCurrent });
    server.stop().await.unwrap();
}

#[tokio::test]
async fn foreign_or_unsupervised_uploads_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let key = key();
    let server = start(dir.path(), Some(&key), true).await;
    let foreign = bundle::create(b"x", "6.6.6", "b", &self::key()).unwrap();
    let refused = upload(&server, &foreign).await;
    assert_eq!(refused, InstallOutcome::Refused { reason: InstallRefusal::BadBundle });
    let junk = upload(&server, b"not a bundle").await;
    assert_eq!(junk, InstallOutcome::Refused { reason: InstallRefusal::BadBundle });
    assert!(!dir.path().join("current").exists());
    server.stop().await.unwrap();

    // Nobody would start the server again after it exits for the update.
    let server = start(dir.path(), Some(&key), false).await;
    assert!(!status(&server).await.can_install);
    let genuine = bundle::create(b"x", "0.2.0", "b", &key).unwrap();
    let refused = upload(&server, &genuine).await;
    assert_eq!(refused, InstallOutcome::Refused { reason: InstallRefusal::NotSupervised });
    server.stop().await.unwrap();

    // A development build has no key and takes no bundles.
    let server = start(dir.path(), None, true).await;
    assert_eq!(upload(&server, &genuine).await, InstallOutcome::Refused { reason: InstallRefusal::NoKey });
    assert!(!dir.path().join("current").exists());
    server.stop().await.unwrap();
}
