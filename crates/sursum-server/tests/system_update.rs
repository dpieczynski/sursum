// SPDX-License-Identifier: GPL-3.0-or-later
//! System updates over HTTP (ADR 0019): on a device running the sursum image, a signed
//! `.sursum-system` uploaded from the remote is verified and handed to the root unit; anything
//! else changes nothing. Writing the slots runs as root and is covered by the unit tests.

#![cfg(unix)]

use std::path::Path;

use sursum_protocol::{InstallOutcome, InstallRefusal, UpdateStatus};
use sursum_server::{Config, Platform, RunningServer};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

struct Supervised;

impl Platform for Supervised {
    fn boot_id(&self) -> Option<String> {
        Some("boot-1".into())
    }

    fn supervised(&self) -> bool {
        true
    }
}

fn key() -> ed25519_dalek::SigningKey {
    sursum_server::bundle::signing_key(&sursum_server::bundle::new_key().unwrap().0).unwrap()
}

/// A server on an "imaged device": `system-release` holds `v1`.
async fn start(dir: &Path, key: &ed25519_dalek::SigningKey) -> RunningServer {
    std::fs::write(dir.join("system-release"), "v1\n").unwrap();
    let mut config = Config::with_data_dir(dir.join("data"));
    config.listen = "127.0.0.1:0".parse().unwrap();
    config.access = sursum_server::auth::AccessMode::Local;
    config.bundle_key = Some(key.verifying_key());
    config.system_release = dir.join("system-release");
    config.system_request = dir.join("request");
    sursum_server::start(config, &Supervised).await.unwrap()
}

fn system_update(dir: &Path, key: &ed25519_dalek::SigningKey, version: &str) -> Vec<u8> {
    let boot = dir.join("boot.vfat");
    let system = dir.join("system.erofs");
    std::fs::write(&boot, vec![7; 4096]).unwrap();
    std::fs::write(&system, b"a system").unwrap();
    let path = dir.join(format!("{version}.sursum-system"));
    sursum_server::system_update::create(&boot, &system, version, key, std::fs::File::create(&path).unwrap())
        .unwrap();
    std::fs::read(path).unwrap()
}

async fn request(server: &RunningServer, head: &str, body: &[u8]) -> String {
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or_default()
}

async fn upload(server: &RunningServer, file: &[u8]) -> InstallOutcome {
    let head = format!(
        "POST /api/system/update HTTP/1.1\r\nHost: localhost\r\nX-Sursum-Upload: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        file.len()
    );
    serde_json::from_str(&request(server, &head, file).await).unwrap()
}

async fn status(server: &RunningServer) -> UpdateStatus {
    let head = "GET /api/update/status HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
    serde_json::from_str(&request(server, head, b"").await).unwrap()
}

#[tokio::test]
async fn a_signed_system_update_is_handed_to_the_root_unit_once() {
    let dir = tempfile::tempdir().unwrap();
    let key = key();
    let server = start(dir.path(), &key).await;
    let system = status(&server).await.system.expect("an imaged device reports its system");
    assert_eq!((system.version.as_str(), system.can_install), ("v1", true));

    let update = system_update(dir.path(), &key, "v2");
    assert_eq!(upload(&server, &update).await, InstallOutcome::Installed { version: "v2".into() });
    assert!(dir.path().join("request").exists(), "the root unit is asked to install it");
    assert!(dir.path().join("data/system-update/ready.sursum-system").exists());
    assert!(!status(&server).await.system.unwrap().can_install);
    assert_eq!(upload(&server, &update).await, InstallOutcome::Refused { reason: InstallRefusal::Busy });
    server.stop().await.unwrap();
}

#[tokio::test]
async fn foreign_or_current_system_updates_change_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let key = key();
    let server = start(dir.path(), &key).await;
    let foreign = system_update(dir.path(), &self::key(), "v2");
    assert_eq!(
        upload(&server, &foreign).await,
        InstallOutcome::Refused { reason: InstallRefusal::BadBundle }
    );
    assert_eq!(upload(&server, b"junk").await, InstallOutcome::Refused { reason: InstallRefusal::BadBundle });
    let current = system_update(dir.path(), &key, "v1");
    assert_eq!(
        upload(&server, &current).await,
        InstallOutcome::Refused { reason: InstallRefusal::AlreadyCurrent }
    );
    assert!(!dir.path().join("request").exists());
    assert!(status(&server).await.system.unwrap().can_install);
    server.stop().await.unwrap();
}

#[tokio::test]
async fn off_the_image_there_is_no_system_to_update() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::with_data_dir(dir.path());
    config.listen = "127.0.0.1:0".parse().unwrap();
    config.access = sursum_server::auth::AccessMode::Local;
    config.system_release = dir.path().join("no-such-file");
    config.system_request = dir.path().join("request");
    let server = sursum_server::start(config, &Supervised).await.unwrap();
    assert_eq!(status(&server).await.system, None);
    let refused = upload(&server, b"anything").await;
    assert_eq!(refused, InstallOutcome::Refused { reason: InstallRefusal::NotSupervised });
    server.stop().await.unwrap();
}
