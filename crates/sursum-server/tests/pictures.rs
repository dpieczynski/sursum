// SPDX-License-Identifier: GPL-3.0-or-later
//! Removing unused pictures on request: pictures in any set (also another church's, which this
//! device keeps and passes on) and the logo stay, the rest goes.

use sursum_core::model::{ItemKind, ServiceSet, SetItem};
use sursum_protocol::{AssetId, ChurchId, ItemId, PictureCleanup, PictureCleanupRequest, SetId};
use sursum_server::db::Database;
use sursum_server::{Config, RunningServer};
use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
use tokio::net::TcpStream;

async fn clean_up(server: &RunningServer, dry_run: bool) -> PictureCleanup {
    let body = serde_json::to_string(&PictureCleanupRequest { dry_run }).unwrap();
    let head = format!(
        "POST /api/assets/cleanup HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    let (_, body) = response.split_once("\r\n\r\n").unwrap();
    serde_json::from_str(body).unwrap_or_else(|_| panic!("{response}"))
}

/// Stores a picture and makes it look uploaded an hour ago.
fn old_picture(data_dir: &std::path::Path, text: &str) -> AssetId {
    let bytes = [b"\x89PNG\r\n\x1a\n".as_slice(), text.as_bytes()].concat();
    let id = sursum_server::assets::store(data_dir, &bytes).unwrap();
    let (path, _) = sursum_server::assets::locate(data_dir, id.as_str()).unwrap();
    let an_hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
    std::fs::File::options().write(true).open(path).unwrap().set_modified(an_hour_ago).unwrap();
    id
}

#[tokio::test]
async fn pictures_in_a_set_and_the_logo_stay_and_the_rest_is_removed_on_request() {
    let dir = tempfile::tempdir().unwrap();
    let poster = old_picture(dir.path(), "poster");
    let logo = old_picture(dir.path(), "logo");
    let forgotten = old_picture(dir.path(), "forgotten");
    let elsewhere = old_picture(dir.path(), "another church's poster");
    let fresh = sursum_server::assets::store(dir.path(), b"\x89PNG\r\n\x1a\njust uploaded").unwrap();
    {
        let mut database = Database::open(dir.path()).unwrap();
        let item = SetItem {
            id: ItemId::from("i1"),
            kind: ItemKind::Image { title: "Ogłoszenia".into(), asset_id: poster.clone() },
        };
        database
            .save_set(&ServiceSet {
                id: SetId::from("s1"),
                name: "Niedziela".into(),
                items: vec![item],
                date: None,
                church: None,
            })
            .unwrap();
        // A set of another church: not shown here, but kept and passed on in library files.
        let item = SetItem {
            id: ItemId::from("i2"),
            kind: ItemKind::Image { title: "Plakat".into(), asset_id: elsewhere.clone() },
        };
        database
            .save_set(&ServiceSet {
                id: SetId::from("s2"),
                name: "Gdzie indziej".into(),
                items: vec![item],
                date: None,
                church: Some(ChurchId::from("inny-kosciol")),
            })
            .unwrap();
        database.set_setting("logo", &Some(logo.clone())).unwrap();
    }
    let mut config = Config::with_data_dir(dir.path());
    config.listen = "127.0.0.1:0".parse().unwrap();
    config.access = sursum_server::auth::AccessMode::Local;
    let server = sursum_server::start(config, &sursum_server::SystemPlatform).await.unwrap();

    let counted = clean_up(&server, true).await;
    assert_eq!(counted.pictures, 1);
    assert!(sursum_server::assets::exists(dir.path(), &forgotten), "counting removes nothing");
    assert_eq!(clean_up(&server, false).await, counted);
    assert!(!sursum_server::assets::exists(dir.path(), &forgotten));
    for kept in [&poster, &logo, &fresh, &elsewhere] {
        assert!(sursum_server::assets::exists(dir.path(), kept));
    }
    assert_eq!(clean_up(&server, true).await.pictures, 0);
    server.stop().await.unwrap();
}
