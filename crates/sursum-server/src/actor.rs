// SPDX-License-Identifier: GPL-3.0-or-later
//! The state actor: the single owner of the server state (ADR 0005).
//!
//! It runs on its own thread (SQLite is synchronous), receives requests over a channel, applies
//! them one at a time, saves the program position and publishes whole snapshots through
//! `watch` channels. A slow client simply skips intermediate snapshots.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context as _;
use sursum_core::layout;
use sursum_core::model::{ItemKind, ServiceSet, SetItem, Song};
use sursum_core::program::{Context, Live, Outcome, Program, Rejection};
use sursum_core::search::SearchIndex;
use sursum_core::slides::{SlideRules, compile_song, compile_text};
use sursum_protocol::{
    ClientId, CmdOutcome, Command, Deck, DisplayPowerState, ErrorCode, ItemId, LiveSlide, Overlay, Rect,
    RemoteState, Rev, ScreenLayout, ScreenState, SetItemView, SetView, SongId, SongSummary, Theme,
};
use tokio::sync::{mpsc, oneshot, watch};

use crate::config::Config;
use crate::db::{Database, RuntimeRecord, SavedProgram};
use crate::demo;
use crate::platform::Platform;

/// Handle to the actor, cheap to clone; one per WebSocket session.
#[derive(Clone)]
pub struct StateHandle {
    requests: mpsc::Sender<Request>,
    /// Latest screen snapshot.
    pub screen: watch::Receiver<Arc<ScreenState>>,
    /// Latest remote snapshot.
    pub remote: watch::Receiver<Arc<RemoteState>>,
}

impl StateHandle {
    /// Sends an operator command and waits for its outcome.
    pub async fn command(&self, actor: ClientId, base_rev: Rev, command: Command) -> CmdOutcome {
        let (reply, outcome) = oneshot::channel();
        let request = Request::Command { actor, base_rev, command, reply };
        if self.requests.send(request).await.is_err() {
            return CmdOutcome::Rejected { code: ErrorCode::Unsupported };
        }
        outcome.await.unwrap_or(CmdOutcome::Rejected { code: ErrorCode::Unsupported })
    }

    /// Searches the song library (ADR 0006).
    pub async fn search(&self, query: String, limit: usize) -> Vec<SongSummary> {
        let (reply, results) = oneshot::channel();
        if self.requests.send(Request::Search { query, limit, reply }).await.is_err() {
            return Vec::new();
        }
        results.await.unwrap_or_default()
    }
}

enum Request {
    Command { actor: ClientId, base_rev: Rev, command: Command, reply: oneshot::Sender<CmdOutcome> },
    Search { query: String, limit: usize, reply: oneshot::Sender<Vec<SongSummary>> },
}

/// Starts the actor thread with restored state.
pub fn spawn(
    config: &Config,
    mut database: Database,
    platform: &dyn Platform,
) -> anyhow::Result<StateHandle> {
    // The demo songs stay in the library until real sets exist (milestone M3).
    let demo_songs = demo::songs();
    for song in &demo_songs {
        database.upsert_song(song, "demo").context("storing demo songs")?;
    }
    let songs = database.load_songs().context("loading the song library")?;
    tracing::info!(songs = songs.len(), "song library loaded");
    let mut state = AppState {
        index: SearchIndex::build(&songs),
        songs: songs.into_iter().map(|song| (song.id.clone(), song)).collect(),
        set: Some(demo::set(&demo_songs)),
        next_item: 1,
        program: Program::default(),
        rev: Rev(1),
        theme: Theme::default(),
        layout: layout_or_fallback(config),
        transition_ms: config.transition_ms,
        rules: SlideRules::default(),
        boot_id: platform.boot_id(),
        database,
    };
    let record = state.database.load_runtime().context("loading runtime state")?;
    state.restore(record);

    let (screen_tx, screen_rx) = watch::channel(Arc::new(state.screen_state()));
    let (remote_tx, remote_rx) = watch::channel(Arc::new(state.remote_state()));
    let (requests, mut inbox) = mpsc::channel::<Request>(64);

    std::thread::Builder::new()
        .name("state-actor".to_owned())
        .spawn(move || {
            while let Some(request) = inbox.blocking_recv() {
                match request {
                    Request::Command { actor, base_rev, command, reply } => {
                        let (outcome, changed) = state.handle_command(&actor, base_rev, &command);
                        if changed {
                            screen_tx.send_replace(Arc::new(state.screen_state()));
                            remote_tx.send_replace(Arc::new(state.remote_state()));
                        }
                        let _ = reply.send(outcome);
                    }
                    Request::Search { query, limit, reply } => {
                        let _ = reply.send(state.search(&query, limit));
                    }
                }
            }
        })
        .context("starting the state actor thread")?;

    Ok(StateHandle { requests, screen: screen_rx, remote: remote_rx })
}

/// Everything the server knows, owned by the actor thread.
struct AppState {
    songs: HashMap<SongId, Song>,
    index: SearchIndex,
    set: Option<ServiceSet>,
    /// Counter for ids of items added during this run (sets are persisted from M3).
    next_item: u32,
    program: Program,
    rev: Rev,
    theme: Theme,
    layout: ScreenLayout,
    transition_ms: u32,
    rules: SlideRules,
    boot_id: Option<String>,
    database: Database,
}

impl AppState {
    /// Applies one command; returns its outcome and whether the state changed.
    fn handle_command(&mut self, actor: &ClientId, base_rev: Rev, command: &Command) -> (CmdOutcome, bool) {
        match command {
            Command::AddSong { song_id } => match self.insert_song(song_id, None) {
                Ok(_) => {
                    self.rev = self.rev.next();
                    (CmdOutcome::Applied { rev: self.rev }, true)
                }
                Err(code) => (CmdOutcome::Rejected { code }, false),
            },
            Command::ShowNow { song_id } => {
                // Right after the live item (or at the end if nothing is live), then on screen.
                let anchor = self.program.live.as_ref().map(|live| live.item_id.clone());
                match self.insert_song(song_id, anchor.as_ref()) {
                    Ok(item_id) => {
                        let show = Command::GoTo { item_id, slide_index: 0 };
                        // An explicit choice of the operator: never stale.
                        let (outcome, _) = self.run_program_command(actor, Rev(u32::MAX), &show);
                        (outcome, true)
                    }
                    Err(code) => (CmdOutcome::Rejected { code }, false),
                }
            }
            _ => self.run_program_command(actor, base_rev, command),
        }
    }

    /// Inserts a song into the active set after `anchor` (or at the end); returns the new item.
    fn insert_song(&mut self, song_id: &SongId, anchor: Option<&ItemId>) -> Result<ItemId, ErrorCode> {
        if !self.songs.contains_key(song_id) {
            return Err(ErrorCode::NotFound);
        }
        let set = self.set.as_mut().ok_or(ErrorCode::NoActiveSet)?;
        let item_id = ItemId(format!("added-{}", self.next_item));
        self.next_item += 1;
        let kind = ItemKind::Song { song_id: song_id.clone(), order_override: None };
        set.insert_after(anchor, SetItem { id: item_id.clone(), kind });
        Ok(item_id)
    }

    /// Runs a command that only changes the program (navigation, overlay).
    fn run_program_command(
        &mut self,
        actor: &ClientId,
        base_rev: Rev,
        command: &Command,
    ) -> (CmdOutcome, bool) {
        let new_rev = self.rev.next();
        let (songs, rules) = (&self.songs, self.rules);
        let deck_for = |item: &SetItem| compile_item(songs, rules, item);
        let context = Context { set: self.set.as_ref(), deck_for: &deck_for };
        match self.program.execute(command, base_rev, actor, new_rev, &context) {
            Ok(Outcome::Changed) => {
                self.rev = new_rev;
                self.save_program();
                (CmdOutcome::Applied { rev: self.rev }, true)
            }
            Ok(Outcome::Unchanged) => (CmdOutcome::Applied { rev: self.rev }, false),
            Err(Rejection::Stale) => (CmdOutcome::Stale { rev: self.rev }, false),
            Err(Rejection::NotFound) => (CmdOutcome::Rejected { code: ErrorCode::NotFound }, false),
            Err(Rejection::NoActiveSet) => (CmdOutcome::Rejected { code: ErrorCode::NoActiveSet }, false),
            Err(Rejection::NotAProgramCommand) => {
                (CmdOutcome::Rejected { code: ErrorCode::Unsupported }, false)
            }
        }
    }

    fn search(&self, query: &str, limit: usize) -> Vec<SongSummary> {
        self.index
            .query(query, limit)
            .into_iter()
            .filter_map(|hit| self.songs.get(&hit.song_id))
            .map(song_summary)
            .collect()
    }

    /// Restores the program position saved before the last stop (ADR 0003): after a process
    /// restart in the same boot everything comes back; after a reboot the position comes back
    /// under the logo.
    fn restore(&mut self, record: Option<RuntimeRecord>) {
        let Some(record) = record else { return };
        let same_boot = self.boot_id.is_some() && record.boot_id == self.boot_id;
        self.program.overlay = if same_boot { record.program.overlay } else { Overlay::Logo };
        let saved = record.program;
        let set_matches = self.set.as_ref().is_some_and(|set| Some(&set.id) == saved.set_id.as_ref());
        let item =
            saved.item_id.as_ref().filter(|_| set_matches).and_then(|item_id| {
                self.set.as_ref()?.items.iter().find(|item| &item.id == item_id).cloned()
            });
        let Some(item) = item else { return };
        let Some(deck) = compile_item(&self.songs, self.rules, &item).filter(|deck| !deck.slides.is_empty())
        else {
            return;
        };
        let slide_index = saved
            .slide_key
            .as_ref()
            .and_then(|key| deck.slides.iter().position(|slide| &slide.key == key))
            .unwrap_or(0);
        self.program.live = Some(Live { item_id: item.id, deck, slide_index });
        tracing::info!(same_boot, "restored the program position");
    }

    fn save_program(&self) {
        let live = self.program.live.as_ref();
        let saved = SavedProgram {
            set_id: self.set.as_ref().map(|set| set.id.clone()),
            item_id: live.map(|live| live.item_id.clone()),
            slide_key: live.map(|live| live.deck.slides[live.slide_index].key.clone()),
            overlay: self.program.overlay,
        };
        if let Err(error) = self.database.save_runtime(&saved, self.boot_id.as_deref()) {
            tracing::error!(?error, "could not save the program position");
        }
    }

    fn screen_state(&self) -> ScreenState {
        ScreenState {
            rev: self.rev,
            live: self.program.live.as_ref().map(live_slide),
            upcoming: self.upcoming_deck(),
            overlay: self.program.overlay,
            theme: self.theme.clone(),
            transition_ms: self.transition_ms,
            layout: self.layout.clone(),
        }
    }

    fn remote_state(&self) -> RemoteState {
        let (songs, rules) = (&self.songs, self.rules);
        let deck_for = |item: &SetItem| compile_item(songs, rules, item);
        let context = Context { set: self.set.as_ref(), deck_for: &deck_for };
        RemoteState {
            rev: self.rev,
            program: self.screen_state(),
            next: self.program.peek_next(&context).as_ref().map(live_slide),
            sections: self.program.sections(),
            active_set: self.set.as_ref().map(|set| self.set_view(set)),
            display_power: DisplayPowerState::Unavailable { reason: "not_configured".to_owned() },
        }
    }

    fn set_view(&self, set: &ServiceSet) -> SetView {
        let items = set
            .items
            .iter()
            .enumerate()
            .map(|(index, item)| SetItemView {
                item_id: item.id.clone(),
                number: index as u32 + 1,
                title: self.item_title(item),
            })
            .collect();
        SetView { set_id: set.id.clone(), name: set.name.clone(), items }
    }

    fn item_title(&self, item: &SetItem) -> String {
        match &item.kind {
            ItemKind::Song { song_id, .. } => {
                self.songs.get(song_id).map(|song| song.title().to_owned()).unwrap_or_default()
            }
            ItemKind::Text { title, .. } => title.clone(),
        }
    }

    /// The deck of the item after the live one, so the screen can fit it in advance.
    fn upcoming_deck(&self) -> Option<Deck> {
        let set = self.set.as_ref()?;
        let next_index = match &self.program.live {
            Some(live) => set.position(&live.item_id)? + 1,
            None => 0,
        };
        compile_item(&self.songs, self.rules, set.items.get(next_index)?)
    }
}

/// How a song is listed in search results.
fn song_summary(song: &Song) -> SongSummary {
    let first_line =
        song.verses.iter().flat_map(|verse| &verse.blocks).flat_map(|block| block.texts()).next();
    SongSummary {
        song_id: song.id.clone(),
        title: song.title().to_owned(),
        number: song.songbooks.first().map(|entry| entry.entry.clone()),
        first_line: first_line.unwrap_or_default().to_owned(),
    }
}

/// Compiles the deck for a set item.
fn compile_item(songs: &HashMap<SongId, Song>, rules: SlideRules, item: &SetItem) -> Option<Deck> {
    match &item.kind {
        ItemKind::Song { song_id, order_override } => {
            Some(compile_song(songs.get(song_id)?, &item.id, order_override.as_deref(), rules))
        }
        ItemKind::Text { title, slides } => Some(compile_text(&item.id, title, slides)),
    }
}

fn live_slide(live: &Live) -> LiveSlide {
    LiveSlide { deck: live.deck.clone(), slide_index: live.slide_index as u32 }
}

/// The configured layout, or the plain safe area if the profile is invalid (logged).
fn layout_or_fallback(config: &Config) -> ScreenLayout {
    match layout::screen_layout(&config.screen_profile) {
        Ok(layout) => layout,
        Err(error) => {
            tracing::error!(%error, "invalid screen profile; ignoring exclusion zones");
            let without_zones =
                sursum_protocol::ScreenProfile { exclusions: Vec::new(), ..config.screen_profile.clone() };
            let content = layout::content_region(&without_zones).unwrap_or(Rect::SCENE);
            ScreenLayout { content, masks: Vec::new() }
        }
    }
}
