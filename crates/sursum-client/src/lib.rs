// SPDX-License-Identifier: GPL-3.0-or-later
//! WebSocket client shared by the screen and the remote.
//!
//! It connects to `/ws/<role>` on the page's own host, introduces itself with `Hello`, passes
//! every server message to a callback and keeps the connection alive:
//! - reconnects after a drop with exponential backoff (250 ms → 5 s, plus jitter);
//! - sends `Ping` every 5 s and reconnects at once if nothing arrived for 15 s (a dead Wi-Fi
//!   link does not always close the socket, and a closing handshake over it can take minutes);
//! - gives up on a connection attempt that has not opened within 10 s and tries again.
//!
//! While disconnected the apps simply keep showing their last state.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use gloo_timers::callback::{Interval, Timeout};
use leptos::prelude::*;
use sursum_protocol::{ClientId, ClientMsg, PROTOCOL_VERSION, Role, ServerMsg};
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

/// Build identifier compiled into the app; compared with the server's in `Welcome`.
pub const BUILD_ID: &str = match option_env!("SURSUM_BUILD_ID") {
    Some(id) => id,
    None => env!("CARGO_PKG_VERSION"),
};

const PING_INTERVAL_MS: u32 = 5_000;
const SILENCE_LIMIT_MS: f64 = 15_000.0;
const CONNECT_LIMIT_MS: f64 = 10_000.0;
const BACKOFF_START_MS: u32 = 250;
const BACKOFF_MAX_MS: u32 = 5_000;

/// Connection state, for a status indicator in the remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Connected,
    Disconnected,
}

/// A JavaScript event callback kept alive as long as its socket.
type EventHandler = Closure<dyn FnMut(web_sys::Event)>;

/// A live connection. Cheap to clone; all clones share one socket.
#[derive(Clone)]
pub struct Client {
    inner: Rc<Inner>,
}

struct Inner {
    role: Role,
    client_id: ClientId,
    on_message: Box<dyn Fn(ServerMsg)>,
    status: RwSignal<Status>,
    socket: RefCell<Option<web_sys::WebSocket>>,
    handlers: RefCell<Vec<EventHandler>>,
    failed_attempts: Cell<u32>,
    last_heard_ms: Cell<f64>,
    /// When the current socket was created, to give up on one that never opens.
    opened_ms: Cell<f64>,
    heartbeat: RefCell<Option<Interval>>,
}

impl Client {
    /// Connects as `role` and calls `on_message` for every message from the server.
    pub fn connect(role: Role, on_message: impl Fn(ServerMsg) + 'static) -> Client {
        let inner = Rc::new(Inner {
            role,
            client_id: client_id(),
            on_message: Box::new(on_message),
            status: RwSignal::new(Status::Connecting),
            socket: RefCell::new(None),
            handlers: RefCell::new(Vec::new()),
            failed_attempts: Cell::new(0),
            last_heard_ms: Cell::new(now_ms()),
            opened_ms: Cell::new(now_ms()),
            heartbeat: RefCell::new(None),
        });
        open(&inner);
        let weak = Rc::downgrade(&inner);
        *inner.heartbeat.borrow_mut() = Some(Interval::new(PING_INTERVAL_MS, move || heartbeat(&weak)));
        Client { inner }
    }

    /// Sends a message; returns false while disconnected (the message is dropped).
    pub fn send(&self, message: &ClientMsg) -> bool {
        send(&self.inner, message)
    }

    /// The connection status signal.
    pub fn status(&self) -> RwSignal<Status> {
        self.inner.status
    }

    /// This client's identity (stable across reloads on the same device).
    pub fn client_id(&self) -> ClientId {
        self.inner.client_id.clone()
    }
}

/// True if the server runs another protocol or build; the page should reload.
pub fn needs_reload(message: &ServerMsg) -> bool {
    matches!(message, ServerMsg::Welcome { protocol, build_id } if *protocol != PROTOCOL_VERSION || build_id != BUILD_ID)
}

fn open(inner: &Rc<Inner>) {
    let Some(url) = socket_url(inner.role) else { return };
    let Ok(socket) = web_sys::WebSocket::new(&url) else {
        schedule_reconnect(inner);
        return;
    };
    inner.status.set(Status::Connecting);
    inner.opened_ms.set(now_ms());
    let weak = Rc::downgrade(inner);
    let on_open = handler(&weak, |inner, _| {
        inner.failed_attempts.set(0);
        inner.last_heard_ms.set(now_ms());
        inner.status.set(Status::Connected);
        let hello = ClientMsg::Hello {
            protocol: PROTOCOL_VERSION,
            role: inner.role,
            client_id: inner.client_id.clone(),
            build_id: BUILD_ID.to_owned(),
        };
        send(inner, &hello);
    });
    let on_message = handler(&weak, |inner, event| {
        inner.last_heard_ms.set(now_ms());
        let Some(text) = event.dyn_ref::<web_sys::MessageEvent>().and_then(|event| event.data().as_string())
        else {
            return;
        };
        match serde_json::from_str::<ServerMsg>(&text) {
            Ok(message) => (inner.on_message)(message),
            Err(error) => {
                web_sys::console::warn_1(&format!("ignoring a malformed server message: {error}").into());
            }
        }
    });
    let on_close = handler(&weak, |inner, _| {
        inner.status.set(Status::Disconnected);
        schedule_reconnect(inner);
    });
    socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
    socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    socket.set_onclose(Some(on_close.as_ref().unchecked_ref()));
    detach_old_socket(inner);
    *inner.socket.borrow_mut() = Some(socket);
    *inner.handlers.borrow_mut() = vec![on_open, on_message, on_close];
}

/// Removes handlers from the previous socket so its late events cannot interfere.
fn detach_old_socket(inner: &Inner) {
    if let Some(old) = inner.socket.borrow_mut().take() {
        old.set_onopen(None);
        old.set_onmessage(None);
        old.set_onclose(None);
        let _ = old.close();
    }
}

fn handler(
    weak: &Weak<Inner>,
    body: impl Fn(&Rc<Inner>, web_sys::Event) + 'static,
) -> Closure<dyn FnMut(web_sys::Event)> {
    let weak = weak.clone();
    Closure::new(move |event: web_sys::Event| {
        if let Some(inner) = weak.upgrade() {
            body(&inner, event);
        }
    })
}

fn schedule_reconnect(inner: &Rc<Inner>) {
    let attempts = inner.failed_attempts.get();
    inner.failed_attempts.set(attempts.saturating_add(1));
    let delay = backoff_ms(attempts) + (js_sys::Math::random() * 250.0) as u32;
    let weak = Rc::downgrade(inner);
    Timeout::new(delay, move || {
        if let Some(inner) = weak.upgrade() {
            open(&inner);
        }
    })
    .forget();
}

/// Exponential backoff without jitter: 250, 500, 1000, … capped at 5000 ms.
pub fn backoff_ms(failed_attempts: u32) -> u32 {
    BACKOFF_START_MS.saturating_mul(1 << failed_attempts.min(10)).min(BACKOFF_MAX_MS)
}

fn heartbeat(weak: &Weak<Inner>) {
    let Some(inner) = weak.upgrade() else { return };
    let now = now_ms();
    match inner.status.get_untracked() {
        // The link is dead without a close event: open a new socket now (the old one is
        // detached and closed), so the status never claims a connection that is gone.
        Status::Connected if now - inner.last_heard_ms.get() > SILENCE_LIMIT_MS => open(&inner),
        Status::Connected => {
            send(&inner, &ClientMsg::Ping);
        }
        // An attempt that hangs (e.g. its packets are lost) would otherwise never time out.
        Status::Connecting if now - inner.opened_ms.get() > CONNECT_LIMIT_MS => open(&inner),
        Status::Connecting | Status::Disconnected => {}
    }
}

fn send(inner: &Inner, message: &ClientMsg) -> bool {
    let socket = inner.socket.borrow();
    let Some(socket) = socket.as_ref().filter(|socket| socket.ready_state() == web_sys::WebSocket::OPEN)
    else {
        return false;
    };
    let json = serde_json::to_string(message).expect("protocol types always serialize");
    socket.send_with_str(&json).is_ok()
}

/// `ws://<host>/ws/<role>` (or `wss://` on HTTPS pages).
fn socket_url(role: Role) -> Option<String> {
    let location = web_sys::window()?.location();
    let scheme = if location.protocol().ok()? == "https:" { "wss" } else { "ws" };
    let path = match role {
        Role::Screen => "screen",
        Role::Remote => "remote",
    };
    Some(format!("{scheme}://{}/ws/{path}", location.host().ok()?))
}

/// A random identity, kept in `localStorage` so a device keeps it across reloads.
fn client_id() -> ClientId {
    const KEY: &str = "sursum.client_id";
    let storage = web_sys::window().and_then(|window| window.local_storage().ok().flatten());
    if let Some(existing) = storage.as_ref().and_then(|storage| storage.get_item(KEY).ok().flatten()) {
        return ClientId(existing);
    }
    let id: String = (0..16).map(|_| format!("{:x}", (js_sys::Math::random() * 16.0) as u8)).collect();
    if let Some(storage) = storage {
        let _ = storage.set_item(KEY, &id);
    }
    ClientId(id)
}

fn now_ms() -> f64 {
    js_sys::Date::now()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_doubles_and_caps() {
        let delays: Vec<u32> = (0..8).map(backoff_ms).collect();
        assert_eq!(delays, [250, 500, 1000, 2000, 4000, 5000, 5000, 5000]);
    }
}
