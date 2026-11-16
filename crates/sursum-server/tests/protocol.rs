// SPDX-License-Identifier: GPL-3.0-or-later
//! Protocol integration tests: a real server on a random port, real WebSocket clients.

use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use sursum_protocol::{
    ClientId, ClientMsg, CmdOutcome, Command, Overlay, PROTOCOL_VERSION, Rev, Role, ServerMsg,
};
use sursum_server::platform::FixedPlatform;
use sursum_server::{Config, RunningServer};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn start(data_dir: &std::path::Path, boot_id: &str) -> RunningServer {
    let mut config = Config::with_data_dir(data_dir);
    config.listen = "127.0.0.1:0".parse().unwrap();
    sursum_server::start(config, &FixedPlatform(Some(boot_id.to_owned()))).await.unwrap()
}

async fn send(socket: &mut Socket, message: &ClientMsg) {
    socket.send(Message::Text(serde_json::to_string(message).unwrap().into())).await.unwrap();
}

async fn receive(socket: &mut Socket) -> ServerMsg {
    loop {
        let frame = tokio::time::timeout(Duration::from_secs(5), socket.next())
            .await
            .expect("timed out")
            .unwrap()
            .unwrap();
        if let Message::Text(text) = frame {
            return serde_json::from_str(text.as_str()).unwrap();
        }
    }
}

/// Connects, says hello, and returns the socket after `Welcome`.
async fn connect(server: &RunningServer, role: Role, client: &str) -> Socket {
    let path = match role {
        Role::Screen => "screen",
        Role::Remote => "remote",
    };
    let (mut socket, _) =
        tokio_tungstenite::connect_async(format!("ws://{}/ws/{path}", server.addr)).await.unwrap();
    let hello = ClientMsg::Hello {
        protocol: PROTOCOL_VERSION,
        role,
        client_id: ClientId::from(client),
        build_id: "test".into(),
    };
    send(&mut socket, &hello).await;
    assert!(matches!(receive(&mut socket).await, ServerMsg::Welcome { protocol: PROTOCOL_VERSION, .. }));
    socket
}

async fn command(socket: &mut Socket, seq: u32, base_rev: Rev, cmd: Command) -> CmdOutcome {
    send(socket, &ClientMsg::Cmd { seq, base_rev, cmd }).await;
    loop {
        if let ServerMsg::CmdResult { seq: got, outcome } = receive(socket).await {
            assert_eq!(got, seq);
            return outcome;
        }
    }
}

async fn next_screen_state(socket: &mut Socket) -> sursum_protocol::ScreenState {
    loop {
        if let ServerMsg::Screen { state } = receive(socket).await {
            return state;
        }
    }
}

#[tokio::test]
async fn screen_receives_snapshot_and_follows_remote_commands() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    let initial = next_screen_state(&mut screen).await;
    assert!(initial.live.is_none());

    let mut remote = connect(&server, Role::Remote, "organist").await;
    let outcome = command(&mut remote, 1, initial.rev, Command::Next).await;
    assert!(matches!(outcome, CmdOutcome::Applied { .. }));

    let state = next_screen_state(&mut screen).await;
    let live = state.live.expect("a slide is live");
    assert_eq!(live.slide_index, 0);
    assert_eq!(live.deck.title, "Kiedy ranne wstają zorze");
    server.stop().await.unwrap();
}

#[tokio::test]
async fn screen_cannot_send_commands() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    let outcome = command(&mut screen, 7, Rev(0), Command::Next).await;
    assert!(matches!(outcome, CmdOutcome::Rejected { .. }));
    server.stop().await.unwrap();
}

#[tokio::test]
async fn simultaneous_next_from_two_remotes_advances_once() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut organist = connect(&server, Role::Remote, "organist").await;
    let mut sacristan = connect(&server, Role::Remote, "sacristan").await;
    let seen = Rev(1);
    assert!(matches!(command(&mut organist, 1, seen, Command::Next).await, CmdOutcome::Applied { .. }));
    // The sacristan pressed "next" based on the same revision: stale, nothing happens.
    assert!(matches!(command(&mut sacristan, 1, seen, Command::Next).await, CmdOutcome::Stale { .. }));
    server.stop().await.unwrap();
}

#[tokio::test]
async fn restart_in_the_same_boot_restores_everything() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    for seq in 0..2 {
        command(&mut remote, seq, Rev(u32::MAX), Command::Next).await;
    }
    command(&mut remote, 3, Rev(u32::MAX), Command::SetOverlay { overlay: Overlay::Black }).await;
    drop(remote);
    server.stop().await.unwrap();

    let server = start(dir.path(), "boot-1").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    let state = next_screen_state(&mut screen).await;
    assert_eq!(state.live.unwrap().slide_index, 1);
    assert_eq!(state.overlay, Overlay::Black);
    server.stop().await.unwrap();
}

#[tokio::test]
async fn reboot_restores_the_position_under_the_logo() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    command(&mut remote, 1, Rev(u32::MAX), Command::GoToItemNumber { number: 2 }).await;
    drop(remote);
    server.stop().await.unwrap();

    let server = start(dir.path(), "boot-2").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    let state = next_screen_state(&mut screen).await;
    assert_eq!(state.overlay, Overlay::Logo);
    assert_eq!(state.live.unwrap().deck.title, "Pieśń przykładowa z refrenem");
    server.stop().await.unwrap();
}

#[tokio::test]
async fn foreign_origin_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut request = tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(
        format!("ws://{}/ws/remote", server.addr),
    )
    .unwrap();
    request.headers_mut().insert("origin", "http://evil.example".parse().unwrap());
    assert!(tokio_tungstenite::connect_async(request).await.is_err());
    server.stop().await.unwrap();
}

#[tokio::test]
async fn healthz_answers() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    stream.write_all(b"GET /healthz HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.ends_with("ok"));
    server.stop().await.unwrap();
}

async fn http_get(server: &RunningServer, path: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or_default()
}

#[tokio::test]
async fn search_ignores_diacritics_and_ranks_numbers_first() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let results: Vec<sursum_protocol::SongSummary> =
        serde_json::from_str(&http_get(&server, "/api/songs?q=wstaja%20zorze").await).unwrap();
    assert_eq!(results[0].title, "Kiedy ranne wstają zorze");
    let by_number: Vec<sursum_protocol::SongSummary> =
        serde_json::from_str(&http_get(&server, "/api/songs?q=1").await).unwrap();
    assert_eq!(by_number[0].number.as_deref(), Some("1"));
    server.stop().await.unwrap();
}

async fn next_remote_state(socket: &mut Socket) -> sursum_protocol::RemoteState {
    loop {
        if let ServerMsg::Remote { state } = receive(socket).await {
            return state;
        }
    }
}

#[tokio::test]
async fn show_now_inserts_after_the_live_item_and_add_song_appends() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    let refrain = sursum_protocol::SongId::from("demo-synthetic-refrain");
    command(&mut remote, 1, Rev(u32::MAX), Command::GoToItemNumber { number: 1 }).await;
    let outcome = command(&mut remote, 2, Rev(0), Command::ShowNow { song_id: refrain.clone() }).await;
    assert!(matches!(outcome, CmdOutcome::Applied { .. }), "{outcome:?}");
    let outcome = command(&mut remote, 3, Rev(0), Command::AddSong { song_id: refrain }).await;
    assert!(matches!(outcome, CmdOutcome::Applied { .. }), "{outcome:?}");
    let missing = Command::ShowNow { song_id: sursum_protocol::SongId::from("no-such-song") };
    assert!(matches!(command(&mut remote, 4, Rev(0), missing).await, CmdOutcome::Rejected { .. }));

    let mut observer = connect(&server, Role::Remote, "observer").await;
    let state = next_remote_state(&mut observer).await;
    let items: Vec<&str> =
        state.active_set.as_ref().unwrap().items.iter().map(|item| item.item_id.as_str()).collect();
    assert_eq!(items.len(), 5);
    assert_eq!((items[0], items[2], items[3]), ("demo-item-1", "demo-item-2", "demo-item-3"));
    assert!(items[1].starts_with("item-") && items[4].starts_with("item-"));
    let live = state.program.live.unwrap();
    assert_eq!((live.deck.item_id.as_str(), live.slide_index), (items[1], 0));
    server.stop().await.unwrap();
}

async fn http_post(server: &RunningServer, path: &str, body: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or_default()
}

#[tokio::test]
async fn editor_creates_edits_detects_conflicts_and_validates() {
    use sursum_protocol::{DraftPreview, SaveOutcome, SongDraft};
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let draft = SongDraft {
        title: "Pieśń z edytora".into(),
        number: Some("777".into()),
        presentation: "V1 C".into(),
        lyrics: "[V1]\n Pierwsza zwrotka\n[C]\n Refren".into(),
        ..SongDraft::default()
    };
    let preview: DraftPreview = serde_json::from_str(
        &http_post(&server, "/api/songs/preview", &serde_json::to_string(&draft).unwrap()).await,
    )
    .unwrap();
    assert_eq!(preview.deck.slides.len(), 2);

    let saved: SaveOutcome = serde_json::from_str(
        &http_post(&server, "/api/songs/save", &serde_json::to_string(&draft).unwrap()).await,
    )
    .unwrap();
    let SaveOutcome::Saved { song_id, revision: 1 } = saved else { panic!("{saved:?}") };

    let found: Vec<sursum_protocol::SongSummary> =
        serde_json::from_str(&http_get(&server, "/api/songs?q=777").await).unwrap();
    assert_eq!(found[0].song_id, song_id);

    let mut edited: SongDraft =
        serde_json::from_str(&http_get(&server, &format!("/api/songs/{}/draft", song_id.as_str())).await)
            .unwrap();
    assert_eq!(edited.base_revision, Some(1));
    edited.lyrics.push_str("\n druga linia refrenu");
    let saved: SaveOutcome = serde_json::from_str(
        &http_post(&server, "/api/songs/save", &serde_json::to_string(&edited).unwrap()).await,
    )
    .unwrap();
    assert_eq!(saved, SaveOutcome::Saved { song_id: song_id.clone(), revision: 2 });

    // A second editor still holding revision 1 must not overwrite revision 2.
    let stale: SaveOutcome = serde_json::from_str(
        &http_post(&server, "/api/songs/save", &serde_json::to_string(&edited).unwrap()).await,
    )
    .unwrap();
    assert_eq!(stale, SaveOutcome::Conflict { current_revision: 2 });

    let empty = SongDraft { title: "Pusta".into(), ..SongDraft::default() };
    let invalid: SaveOutcome = serde_json::from_str(
        &http_post(&server, "/api/songs/save", &serde_json::to_string(&empty).unwrap()).await,
    )
    .unwrap();
    assert!(matches!(invalid, SaveOutcome::Invalid { .. }));
    server.stop().await.unwrap();
}

async fn http_post_bytes(server: &RunningServer, path: &str, body: &[u8]) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/zip\r\nX-Sursum-Upload: 1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes()).await.unwrap();
    stream.write_all(body).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or_default()
}

#[tokio::test]
async fn zip_upload_imports_songs_and_sets_into_the_running_library() {
    use std::io::Write as _;
    let fixtures = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/opensong/Songs");
    let mut buffer = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(&mut buffer);
    for name in ["Synthetic Basic", "Synthetic CP1250", "Synthetic Broken"] {
        writer.start_file(format!("Songs/{name}"), zip::write::SimpleFileOptions::default()).unwrap();
        writer.write_all(&std::fs::read(fixtures.join(name)).unwrap()).unwrap();
    }
    writer.start_file("Sets/Synthetic Sunday", zip::write::SimpleFileOptions::default()).unwrap();
    writer.write_all(&std::fs::read(fixtures.join("../Sets/Synthetic Sunday")).unwrap()).unwrap();
    writer.finish().unwrap();

    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    next_remote_state(&mut remote).await;
    let report: sursum_protocol::ImportReport = serde_json::from_str(
        &http_post_bytes(&server, "/api/import/opensong?songbook=Test", buffer.get_ref()).await,
    )
    .unwrap();
    assert_eq!((report.inserted, report.failed, report.sets), (2, 1, 1), "{report:?}");
    let state = next_remote_state(&mut remote).await;
    assert!(
        state.sets.iter().any(|set| set.name == "Syntetyczna niedziela" && set.items == 3),
        "{:?}",
        state.sets
    );
    let found: Vec<sursum_protocol::SongSummary> =
        serde_json::from_str(&http_get(&server, "/api/songs?q=windows%201250").await).unwrap();
    assert_eq!(found[0].title, "Syntetyczna pieśń w kodowaniu Windows-1250");
    assert!(dir.path().join("backups").read_dir().unwrap().count() >= 2, "startup and pre-import backups");
    server.stop().await.unwrap();
}

#[tokio::test]
async fn set_edits_persist_never_change_the_screen_and_removal_can_be_undone() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    next_screen_state(&mut screen).await;
    command(&mut remote, 1, Rev(u32::MAX), Command::GoToItemNumber { number: 2 }).await;
    let shown = next_screen_state(&mut screen).await.live.unwrap();

    // Remove the live item, move another, add an announcement: the screen does not change.
    let live_item = shown.deck.item_id.clone();
    for (seq, cmd) in [
        Command::RemoveItem { item_id: live_item.clone() },
        Command::MoveItem { item_id: sursum_protocol::ItemId::from("demo-item-3"), to_index: 0 },
        Command::AddText { title: "Ogłoszenia".into(), text: "Strona pierwsza\n\nStrona druga".into() },
    ]
    .into_iter()
    .enumerate()
    {
        assert!(matches!(
            command(&mut remote, seq as u32 + 2, Rev(0), cmd).await,
            CmdOutcome::Applied { .. }
        ));
        let state = next_screen_state(&mut screen).await;
        assert_eq!(state.live.as_ref().unwrap().deck, shown.deck, "a set edit changed the screen");
    }
    // Undo puts the removed item back.
    assert!(matches!(command(&mut remote, 9, Rev(0), Command::UndoRemove).await, CmdOutcome::Applied { .. }));
    drop(remote);
    server.stop().await.unwrap();

    // Everything survives a restart.
    let server = start(dir.path(), "boot-1").await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    let state = next_remote_state(&mut remote).await;
    let set = state.active_set.unwrap();
    let titles: Vec<&str> = set.items.iter().map(|item| item.title.as_str()).collect();
    // Undo restores the item at its former index (1).
    let expected = ["Ogłoszenia", "Pieśń przykładowa z refrenem", "Kiedy ranne wstają zorze", "Ogłoszenia"];
    assert_eq!(titles, expected);
    assert_eq!(state.program.live.unwrap().deck.item_id, live_item);

    // A new set becomes active; switching back works.
    command(&mut remote, 1, Rev(0), Command::NewSet { name: "Nowy".into() }).await;
    let state = next_remote_state(&mut remote).await;
    assert_eq!(state.sets.len(), 2);
    assert_eq!(state.active_set.as_ref().unwrap().name, "Nowy");
    let first = state.sets[0].set_id.clone();
    command(&mut remote, 2, Rev(0), Command::ActivateSet { set_id: first }).await;
    assert_eq!(next_remote_state(&mut remote).await.active_set.unwrap().items.len(), 4);
    server.stop().await.unwrap();
}

/// A raw HTTP request; returns the whole response (status line, headers, body).
async fn http_raw(server: &RunningServer, request: &str) -> String {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    response
}

async fn pair_with(server: &RunningServer, pin: &str) -> String {
    let body =
        serde_json::to_string(&sursum_protocol::PairRequest { pin: pin.into(), name: "Tablet".into() })
            .unwrap();
    http_raw(
        server,
        &format!(
            "POST /api/pair HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
    .await
}

#[tokio::test]
async fn with_a_pin_only_paired_remotes_may_control_and_the_screen_stays_open() {
    let dir = tempfile::tempdir().unwrap();
    {
        let database = sursum_server::db::Database::open(dir.path()).unwrap();
        let pin = sursum_server::auth::PinHash::new("2468").unwrap();
        database.set_setting(sursum_server::auth::PIN_SETTING, &pin).unwrap();
    }
    let server = start(dir.path(), "boot-1").await;
    let url = format!("ws://{}/ws/remote", server.addr);
    assert!(tokio_tungstenite::connect_async(&url).await.is_err(), "an unpaired remote must be refused");
    assert!(http_get(&server, "/api/songs?q=a").await.is_empty(), "the API is closed too");
    let session: sursum_protocol::SessionInfo =
        serde_json::from_str(&http_get(&server, "/api/session").await).unwrap();
    assert_eq!(session, sursum_protocol::SessionInfo { pin_required: true, paired: false, admin: false });

    assert!(pair_with(&server, "1111").await.starts_with("HTTP/1.1 401"));
    let response = pair_with(&server, "2468").await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let cookie = response
        .lines()
        .find_map(|line| line.strip_prefix("set-cookie: "))
        .and_then(|value| value.split(';').next())
        .expect("a cookie is set")
        .to_owned();
    assert!(response.to_lowercase().contains("httponly") && response.contains("SameSite=Strict"));

    let mut request =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(&url).unwrap();
    request.headers_mut().insert("cookie", cookie.parse().unwrap());
    assert!(tokio_tungstenite::connect_async(request).await.is_ok(), "a paired remote connects");
    // The screen never needs pairing.
    let _screen = connect(&server, Role::Screen, "screen").await;
    server.stop().await.unwrap();
}

async fn http_get_bytes(server: &RunningServer, path: &str) -> Vec<u8> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut stream = TcpStream::connect(server.addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.unwrap();
    let body_at = response.windows(4).position(|window| window == b"\r\n\r\n").unwrap() + 4;
    response.split_off(body_at)
}

#[tokio::test]
async fn library_file_moves_new_songs_to_another_device_after_a_dry_run() {
    use sursum_protocol::{MergeReport, SaveOutcome, SongDraft};
    let (dir_a, dir_b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let tulce = start(dir_a.path(), "boot-1").await;
    let gowarzewo = start(dir_b.path(), "boot-1").await;
    let draft = SongDraft {
        title: "Syntetyczna pieśń do scalenia".into(),
        lyrics: "[V1]\n Tylko w Tulcach".into(),
        ..SongDraft::default()
    };
    let saved: SaveOutcome = serde_json::from_str(
        &http_post(&tulce, "/api/songs/save", &serde_json::to_string(&draft).unwrap()).await,
    )
    .unwrap();
    assert!(matches!(saved, SaveOutcome::Saved { .. }), "{saved:?}");

    let bundle = http_get_bytes(&tulce, "/api/library/export").await;
    assert!(bundle.starts_with(b"PK"), "a ZIP file");

    let preview: MergeReport =
        serde_json::from_str(&http_post_bytes(&gowarzewo, "/api/library/merge?dry_run=true", &bundle).await)
            .unwrap();
    assert_eq!(preview.added, 1, "{preview:?}");
    assert!(preview.conflicts.is_empty(), "{preview:?}");
    let search = "/api/songs?q=do%20scalenia";
    let found: Vec<sursum_protocol::SongSummary> =
        serde_json::from_str(&http_get(&gowarzewo, search).await).unwrap();
    assert!(found.is_empty(), "a dry run writes nothing");

    let merged: MergeReport =
        serde_json::from_str(&http_post_bytes(&gowarzewo, "/api/library/merge", &bundle).await).unwrap();
    assert_eq!(merged, preview);
    let found: Vec<sursum_protocol::SongSummary> =
        serde_json::from_str(&http_get(&gowarzewo, search).await).unwrap();
    assert_eq!(found[0].title, "Syntetyczna pieśń do scalenia");

    let again: MergeReport =
        serde_json::from_str(&http_post_bytes(&gowarzewo, "/api/library/merge", &bundle).await).unwrap();
    assert_eq!((again.added, again.updated), (0, 0), "merging twice changes nothing: {again:?}");
    let rejected = http_post_bytes(&gowarzewo, "/api/library/merge", b"not a zip").await;
    assert!(serde_json::from_str::<MergeReport>(&rejected).is_err());
    tulce.stop().await.unwrap();
    gowarzewo.stop().await.unwrap();
}

#[tokio::test]
async fn calibration_changes_the_layout_at_once_and_survives_a_restart() {
    use sursum_protocol::{ErrorCode, ExclusionZone, Insets, MaskMode, Rect, ScreenProfile};
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut remote = connect(&server, Role::Remote, "admin").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    next_screen_state(&mut screen).await;

    let outcome = command(&mut remote, 1, Rev(u32::MAX), Command::Calibrate { on: true }).await;
    assert!(matches!(outcome, CmdOutcome::Applied { .. }), "{outcome:?}");
    assert!(next_screen_state(&mut screen).await.calibration.is_some());

    let pillar = ScreenProfile {
        safe_area: Insets::uniform(48),
        exclusions: vec![ExclusionZone {
            name: "słup".into(),
            rect: Rect::new(0, 0, 600, 1080),
            mask: MaskMode::Black,
        }],
    };
    command(&mut remote, 2, Rev(u32::MAX), Command::SetScreenProfile { profile: pillar.clone() }).await;
    let state = next_screen_state(&mut screen).await;
    assert!(state.layout.content.x >= 600, "text moves right of the pillar: {:?}", state.layout);
    assert_eq!(state.layout.masks, [Rect::new(0, 0, 600, 1080)]);
    assert_eq!(state.calibration.as_ref(), Some(&pillar), "the pattern follows the new profile");

    let covering = ScreenProfile {
        safe_area: Insets::uniform(0),
        exclusions: vec![ExclusionZone { name: "all".into(), rect: Rect::SCENE, mask: MaskMode::Black }],
    };
    let outcome =
        command(&mut remote, 3, Rev(u32::MAX), Command::SetScreenProfile { profile: covering }).await;
    assert_eq!(outcome, CmdOutcome::Rejected { code: ErrorCode::InvalidProfile });
    drop((remote, screen));
    server.stop().await.unwrap();

    let server = start(dir.path(), "boot-1").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    let state = next_screen_state(&mut screen).await;
    assert_eq!(state.layout.masks, [Rect::new(0, 0, 600, 1080)], "the profile was stored");
    assert!(state.calibration.is_none(), "calibration never survives a restart");
    server.stop().await.unwrap();
}

/// An HTTP request from a paired browser; returns the whole response.
async fn http_as(server: &RunningServer, cookie: &str, method: &str, path: &str, body: &str) -> String {
    http_raw(
        server,
        &format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nCookie: {cookie}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        ),
    )
    .await
}

fn cookie_of(response: &str) -> String {
    response
        .lines()
        .find_map(|line| line.strip_prefix("set-cookie: "))
        .and_then(|value| value.split(';').next())
        .expect("a cookie is set")
        .to_owned()
}

/// A remote socket with a device cookie, after `Welcome`.
async fn connect_as(server: &RunningServer, cookie: &str, client: &str) -> Socket {
    let url = format!("ws://{}/ws/remote", server.addr);
    let mut request =
        tokio_tungstenite::tungstenite::client::IntoClientRequest::into_client_request(&url).unwrap();
    request.headers_mut().insert("cookie", cookie.parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    let hello = ClientMsg::Hello {
        protocol: PROTOCOL_VERSION,
        role: Role::Remote,
        client_id: ClientId::from(client),
        build_id: "test".into(),
    };
    send(&mut socket, &hello).await;
    assert!(matches!(receive(&mut socket).await, ServerMsg::Welcome { .. }));
    socket
}

#[tokio::test]
async fn operators_need_admin_mode_for_setup_and_revoked_remotes_are_cut_off() {
    use sursum_protocol::{AdminRequest, DeviceView, ErrorCode, PairOutcome, SessionInfo};
    let dir = tempfile::tempdir().unwrap();
    {
        let database = sursum_server::db::Database::open(dir.path()).unwrap();
        let pin = sursum_server::auth::PinHash::new("2468").unwrap();
        database.set_setting(sursum_server::auth::PIN_SETTING, &pin).unwrap();
    }
    let server = start(dir.path(), "boot-1").await;
    let organist = cookie_of(&pair_with(&server, "2468").await);
    let sacristan = cookie_of(&pair_with(&server, "2468").await);
    let mut organist_socket = connect_as(&server, &organist, "organist").await;
    let mut sacristan_socket = connect_as(&server, &sacristan, "sacristan").await;

    // A paired remote operates the service but cannot change the setup.
    assert!(http_as(&server, &organist, "GET", "/api/devices", "").await.starts_with("HTTP/1.1 403"));
    let import = http_as(&server, &organist, "POST", "/api/import/opensong", "").await;
    assert!(import.starts_with("HTTP/1.1 403"), "{import}");
    let outcome = command(&mut organist_socket, 1, Rev(u32::MAX), Command::Calibrate { on: true }).await;
    assert_eq!(outcome, CmdOutcome::Rejected { code: ErrorCode::Forbidden });
    let outcome = command(&mut organist_socket, 2, Rev(u32::MAX), Command::Next).await;
    assert!(matches!(outcome, CmdOutcome::Applied { .. }), "{outcome:?}");

    // The PIN again unlocks admin mode on this device only.
    let wrong = serde_json::to_string(&AdminRequest { pin: "1111".into() }).unwrap();
    assert!(http_as(&server, &organist, "POST", "/api/admin", &wrong).await.starts_with("HTTP/1.1 401"));
    let right = serde_json::to_string(&AdminRequest { pin: "2468".into() }).unwrap();
    let unlocked = http_as(&server, &organist, "POST", "/api/admin", &right).await;
    assert!(unlocked.ends_with(&serde_json::to_string(&PairOutcome::Unlocked).unwrap()), "{unlocked}");
    let session = http_as(&server, &organist, "GET", "/api/session", "").await;
    let session: SessionInfo = serde_json::from_str(session.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert!(session.admin);
    assert!(http_as(&server, &sacristan, "GET", "/api/devices", "").await.starts_with("HTTP/1.1 403"));
    let outcome = command(&mut organist_socket, 3, Rev(u32::MAX), Command::Calibrate { on: false }).await;
    assert!(matches!(outcome, CmdOutcome::Applied { .. }), "{outcome:?}");

    let listing = http_as(&server, &organist, "GET", "/api/devices", "").await;
    let devices: Vec<DeviceView> = serde_json::from_str(listing.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices.iter().filter(|device| device.this_device).count(), 1);
    let other = devices.iter().find(|device| !device.this_device).unwrap();

    // Revoking the other tablet closes its connection and closes the API for it.
    let path = format!("/api/devices/{}/revoke", other.device_id);
    assert!(http_as(&server, &organist, "POST", &path, "").await.starts_with("HTTP/1.1 204"));
    let closed = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            match sacristan_socket.next().await {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                Some(Ok(_)) => {}
            }
        }
    })
    .await;
    assert!(closed.is_ok(), "the revoked remote's socket must close");
    assert!(http_as(&server, &sacristan, "GET", "/api/songs?q=a", "").await.starts_with("HTTP/1.1 401"));
    let outcome = command(&mut organist_socket, 4, Rev(u32::MAX), Command::Next).await;
    assert!(matches!(outcome, CmdOutcome::Applied { .. }), "the other remote keeps working: {outcome:?}");
    server.stop().await.unwrap();
}

#[tokio::test]
async fn uploaded_pictures_and_pauses_become_set_items_and_reach_the_screen() {
    use sursum_protocol::{AssetId, ErrorCode, SlideBody};
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let png = b"\x89PNG\r\n\x1a\nsynthetic picture bytes";
    let id: AssetId = serde_json::from_str(&http_post_bytes(&server, "/api/assets", png).await).unwrap();
    assert!(http_post_bytes(&server, "/api/assets", b"GIF89a").await.contains("not a PNG"));

    let mut remote = connect(&server, Role::Remote, "organist").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    next_screen_state(&mut screen).await;
    let add = Command::AddImage { title: "Plakat".into(), asset_id: id.clone() };
    assert!(matches!(command(&mut remote, 1, Rev(u32::MAX), add).await, CmdOutcome::Applied { .. }));
    let missing =
        Command::AddImage { title: "x".into(), asset_id: AssetId(format!("{}.png", "0".repeat(64))) };
    let outcome = command(&mut remote, 2, Rev(u32::MAX), missing).await;
    assert_eq!(outcome, CmdOutcome::Rejected { code: ErrorCode::NotFound });
    command(&mut remote, 3, Rev(u32::MAX), Command::AddBlank { title: String::new() }).await;
    let state = next_remote_state(&mut remote).await;
    let items = &state.active_set.as_ref().unwrap().items;
    let titles: Vec<&str> = items.iter().rev().take(2).map(|item| item.title.as_str()).collect();
    assert_eq!(titles, ["Przerwa", "Plakat"]);

    let number = u32::try_from(items.len() - 1).unwrap();
    command(&mut remote, 4, Rev(u32::MAX), Command::GoToItemNumber { number }).await;
    let live = loop {
        if let Some(live) = next_screen_state(&mut screen).await.live {
            break live;
        }
    };
    assert!(matches!(&live.deck.slides[0].body, SlideBody::Image { asset_id, .. } if asset_id == &id));

    let served = {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut stream = TcpStream::connect(server.addr).await.unwrap();
        let request =
            format!("GET /assets/{} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n", id.as_str());
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).await.unwrap();
        assert!(bytes.ends_with(png), "the stored bytes come back");
        String::from_utf8_lossy(&bytes).into_owned()
    };
    assert!(served.contains("content-type: image/png") && served.contains("immutable"), "{served}");
    assert!(
        http_raw(
            &server,
            "GET /assets/..%2Fsursum.db HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .await
        .starts_with("HTTP/1.1 404")
    );
    server.stop().await.unwrap();
}

/// A fake PJLink projector with a password: it greets with the spec's seed, checks the digest and
/// keeps its power state. Returns its address.
async fn fake_projector(password: &'static str) -> std::net::SocketAddr {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let power = std::sync::Arc::new(std::sync::Mutex::new('0'));
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else { return };
            let power = power.clone();
            tokio::spawn(async move {
                let (reader, mut writer) = stream.into_split();
                let mut reader = BufReader::new(reader);
                writer.write_all(b"PJLINK 1 498e4a67\r").await.unwrap();
                let mut line = Vec::new();
                if reader.read_until(b'\r', &mut line).await.unwrap_or(0) == 0 {
                    return;
                }
                let line = String::from_utf8(line).unwrap();
                let expected = sursum_core::display_power::auth_prefix("498e4a67", password);
                let Some(command) = line.strip_prefix(&expected) else {
                    writer.write_all(b"PJLINK ERRA\r").await.unwrap();
                    return;
                };
                let reply = match command.trim_end_matches('\r') {
                    "%1POWR 1" => {
                        *power.lock().unwrap() = '1';
                        "%1POWR=OK".to_owned()
                    }
                    "%1POWR 0" => {
                        *power.lock().unwrap() = '0';
                        "%1POWR=OK".to_owned()
                    }
                    "%1POWR ?" => format!("%1POWR={}", power.lock().unwrap()),
                    _ => "%1POWR=ERR1".to_owned(),
                };
                writer.write_all(format!("{reply}\r").as_bytes()).await.unwrap();
            });
        }
    });
    address
}

async fn start_with_projector(
    dir: &std::path::Path,
    password: &str,
    projector: std::net::SocketAddr,
) -> RunningServer {
    let mut config = Config::with_data_dir(dir);
    config.listen = "127.0.0.1:0".parse().unwrap();
    config.display_power = sursum_server::config::DisplayPowerConfig::Pjlink {
        host: projector.to_string(),
        password: Some(password.to_owned()),
    };
    sursum_server::start(config, &FixedPlatform(Some("boot-1".to_owned()))).await.unwrap()
}

async fn wait_for_power(socket: &mut Socket, wanted: &sursum_protocol::DisplayPowerState) {
    loop {
        if &next_remote_state(socket).await.display_power == wanted {
            return;
        }
    }
}

#[tokio::test]
async fn the_remote_switches_a_pjlink_projector_on_and_sees_its_state() {
    use sursum_protocol::{DisplayPowerState, ErrorCode, PowerAction};
    let projector = fake_projector("tajne").await;
    let dir = tempfile::tempdir().unwrap();
    let server = start_with_projector(dir.path(), "tajne", projector).await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    wait_for_power(&mut remote, &DisplayPowerState::Standby).await;
    let outcome = command(&mut remote, 1, Rev(0), Command::DisplayPower { action: PowerAction::On }).await;
    assert!(matches!(outcome, CmdOutcome::Applied { .. }), "{outcome:?}");
    wait_for_power(&mut remote, &DisplayPowerState::On).await;
    server.stop().await.unwrap();

    // A wrong password is reported, never retried blindly.
    let dir = tempfile::tempdir().unwrap();
    let server = start_with_projector(dir.path(), "zle", projector).await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    let unavailable = DisplayPowerState::Unavailable { reason: "auth_failed".into() };
    wait_for_power(&mut remote, &unavailable).await;
    let outcome =
        command(&mut remote, 1, Rev(0), Command::DisplayPower { action: PowerAction::Standby }).await;
    assert_eq!(outcome, CmdOutcome::Rejected { code: ErrorCode::DisplayAuthFailed });
    server.stop().await.unwrap();
}

#[tokio::test]
async fn without_power_control_the_remote_is_told_so() {
    use sursum_protocol::{DisplayPowerState, ErrorCode, PowerAction};
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let mut remote = connect(&server, Role::Remote, "organist").await;
    wait_for_power(&mut remote, &DisplayPowerState::Unavailable { reason: "not_configured".into() }).await;
    let outcome = command(&mut remote, 1, Rev(0), Command::DisplayPower { action: PowerAction::On }).await;
    assert_eq!(outcome, CmdOutcome::Rejected { code: ErrorCode::DisplayUnreachable });
    server.stop().await.unwrap();
}

#[tokio::test]
async fn the_parish_logo_is_stored_and_reaches_the_screen_after_a_restart() {
    use sursum_protocol::{AssetId, ErrorCode};
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    let png = b"\x89PNG\r\n\x1a\nsynthetic logo";
    let id: AssetId = serde_json::from_str(&http_post_bytes(&server, "/api/assets", png).await).unwrap();
    let mut remote = connect(&server, Role::Remote, "admin").await;
    let unknown = Command::SetLogo { asset_id: Some(AssetId(format!("{}.png", "1".repeat(64)))) };
    assert_eq!(
        command(&mut remote, 1, Rev(0), unknown).await,
        CmdOutcome::Rejected { code: ErrorCode::NotFound }
    );
    let set = Command::SetLogo { asset_id: Some(id.clone()) };
    assert!(matches!(command(&mut remote, 2, Rev(0), set).await, CmdOutcome::Applied { .. }));
    drop(remote);
    server.stop().await.unwrap();

    let server = start(dir.path(), "boot-2").await;
    let mut screen = connect(&server, Role::Screen, "screen").await;
    let state = next_screen_state(&mut screen).await;
    assert_eq!(state.logo, Some(id));
    server.stop().await.unwrap();
}

#[tokio::test]
async fn foreign_pages_cannot_upload_and_foreign_host_names_are_refused() {
    let dir = tempfile::tempdir().unwrap();
    let server = start(dir.path(), "boot-1").await;
    // A plain cross-site POST (no custom header, no preflight possible) is refused even without a PIN.
    let body = b"\x89PNG\r\n\x1a\nx";
    let request = format!(
        "POST /api/assets HTTP/1.1\r\nHost: localhost\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut raw = request.into_bytes();
    raw.extend_from_slice(body);
    let response = http_raw(&server, &String::from_utf8_lossy(&raw)).await;
    assert!(response.starts_with("HTTP/1.1 403"), "{response}");
    // DNS rebinding: an attacker's name pointing at the device.
    let rebound = http_raw(
        &server,
        "GET /api/library/export HTTP/1.1\r\nHost: evil.example.com\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(rebound.starts_with("HTTP/1.1 421"), "{rebound}");
    server.stop().await.unwrap();
}

#[tokio::test]
async fn a_screen_that_stays_away_gets_its_kiosk_restarted_and_remotes_know() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("kiosk-restarted");
    let mut config = Config::with_data_dir(dir.path());
    config.listen = "127.0.0.1:0".parse().unwrap();
    config.screen_guard = Some(sursum_server::screen_guard::ScreenGuardConfig {
        restart_command: vec!["sh".into(), "-c".into(), format!("touch '{}'", marker.display())],
        grace_s: 1,
    });
    let server = sursum_server::start(config, &FixedPlatform(Some("boot-1".to_owned()))).await.unwrap();
    let mut remote = connect(&server, Role::Remote, "organist").await;
    let screen = connect(&server, Role::Screen, "screen").await;
    while !next_remote_state(&mut remote).await.screen_connected {}
    drop(screen);
    while next_remote_state(&mut remote).await.screen_connected {}
    assert!(!marker.exists(), "not before the grace period");
    tokio::time::sleep(Duration::from_millis(1800)).await;
    assert!(marker.exists(), "the kiosk restart command ran");
    server.stop().await.unwrap();
}
