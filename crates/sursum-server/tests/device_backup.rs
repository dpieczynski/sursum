// SPDX-License-Identifier: GPL-3.0-or-later
//! Whole-device backups over HTTP (ADR 0018): a backup downloaded from one device, restored on
//! another, is staged, the server asks to be restarted, and the next start runs on its data.

#![cfg(unix)]

use std::time::Duration;

use sursum_protocol::{RestoreOutcome, RestoreRefusal};
use sursum_server::db::Database;
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

async fn start(data_dir: &std::path::Path, supervised: bool) -> RunningServer {
    let mut config = Config::with_data_dir(data_dir);
    config.listen = "127.0.0.1:0".parse().unwrap();
    config.open_without_pin = true;
    sursum_server::start(config, &Supervised(supervised)).await.unwrap()
}

/// Sends an HTTP/1.0 request (so a streamed answer comes without chunked encoding) and returns
/// the status line and the body.
async fn request(server: &RunningServer, head: &str, body: &[u8]) -> (String, Vec<u8>) {
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let split = response.windows(4).position(|window| window == b"\r\n\r\n").expect("a complete answer");
    let status = String::from_utf8_lossy(&response[..split]).lines().next().unwrap_or_default().to_owned();
    (status, response[split + 4..].to_vec())
}

async fn download(server: &RunningServer) -> Vec<u8> {
    let (status, body) = request(server, "GET /api/backup HTTP/1.0\r\nHost: localhost\r\n\r\n", b"").await;
    assert!(status.contains("200"), "{status}");
    body
}

async fn restore(server: &RunningServer, backup: &[u8]) -> RestoreOutcome {
    let head = format!(
        "POST /api/backup/restore HTTP/1.0\r\nHost: localhost\r\nContent-Type: application/octet-stream\r\nX-Sursum-Upload: 1\r\nContent-Length: {}\r\n\r\n",
        backup.len()
    );
    let (status, body) = request(server, &head, backup).await;
    serde_json::from_slice(&body).unwrap_or_else(|_| panic!("{status}: {}", String::from_utf8_lossy(&body)))
}

fn mark(data_dir: &std::path::Path, marker: &str) {
    Database::open(data_dir).unwrap().set_setting("marker", &marker).unwrap();
}

fn marker(data_dir: &std::path::Path) -> Option<String> {
    Database::open(data_dir).unwrap().setting("marker").unwrap()
}

#[tokio::test]
async fn a_backup_from_one_device_brings_another_to_the_same_state_after_a_restart() {
    let church = tempfile::tempdir().unwrap();
    mark(church.path(), "from the backup");
    let picture = sursum_server::assets::store(church.path(), b"\x89PNG\r\n\x1a\nposter").unwrap();
    let server = start(church.path(), true).await;
    let backup = download(&server).await;
    server.stop().await.unwrap();

    let spare = tempfile::tempdir().unwrap();
    mark(spare.path(), "replaced");
    let server = start(spare.path(), true).await;
    let RestoreOutcome::Restored { summary } = restore(&server, &backup).await else { panic!("refused") };
    assert_eq!(summary.pictures, 1);
    assert!(summary.songs > 0, "the demo songs travel along");
    tokio::time::timeout(Duration::from_secs(5), server.restart_requested())
        .await
        .expect("the server asks to be restarted into the backup");
    assert!(sursum_server::assets::exists(spare.path(), &picture));
    server.stop().await.unwrap();

    let server = start(spare.path(), true).await;
    server.stop().await.unwrap();
    assert_eq!(marker(spare.path()).as_deref(), Some("from the backup"));
}

#[tokio::test]
async fn junk_or_an_unsupervised_server_changes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    mark(dir.path(), "kept");
    let server = start(dir.path(), true).await;
    let backup = download(&server).await;
    let junk = restore(&server, b"not a backup").await;
    assert_eq!(junk, RestoreOutcome::Refused { reason: RestoreRefusal::NotABackup });
    server.stop().await.unwrap();

    // Nobody would start the server again to finish the restore.
    let server = start(dir.path(), false).await;
    let refused = restore(&server, &backup).await;
    assert_eq!(refused, RestoreOutcome::Refused { reason: RestoreRefusal::NotSupervised });
    server.stop().await.unwrap();
    assert!(!dir.path().join("restore").exists());
    assert_eq!(marker(dir.path()).as_deref(), Some("kept"));
}
