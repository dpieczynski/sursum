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
