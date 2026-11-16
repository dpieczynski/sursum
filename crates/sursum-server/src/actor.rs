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
use sursum_core::slides::SlideRules;
use sursum_protocol::{
    ClientId, CmdOutcome, Command, Deck, DisplayPowerState, ErrorCode, ItemId, LiveSlide, Overlay, Rect,
    RemoteState, Rev, SaveOutcome, ScreenLayout, ScreenProfile, ScreenState, SetId, SetItemView, SetView,
    SongDraft, SongId, SongSummary, Theme,
};
use tokio::sync::{mpsc, oneshot, watch};

use crate::config::Config;
use crate::db::{Database, RuntimeRecord, SavedProgram, Upsert};
use crate::demo;
use crate::editor;
use crate::platform::Platform;
use crate::sets::SetStore;

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

    /// The editor's text form of a stored song.
    pub async fn draft(&self, song_id: SongId) -> Option<SongDraft> {
        let (reply, draft) = oneshot::channel();
        self.requests.send(Request::Draft { song_id, reply }).await.ok()?;
        draft.await.ok().flatten()
    }

    /// Stores imported songs (after a backup) and reloads the library.
    pub async fn import(
        &self,
        library: sursum_formats::opensong::LibraryImport,
    ) -> sursum_protocol::ImportReport {
        let (reply, report) = oneshot::channel();
        if self.requests.send(Request::Import { library, reply }).await.is_err() {
            return sursum_protocol::ImportReport::default();
        }
        report.await.unwrap_or_default()
    }

    /// Records a newly paired device; false if it could not be stored.
    pub async fn add_device(&self, name: String, token_hash: String) -> bool {
        let (reply, stored) = oneshot::channel();
        if self.requests.send(Request::AddDevice { name, token_hash, reply }).await.is_err() {
            return false;
        }
        stored.await.unwrap_or(false)
    }

    /// Records whether a screen is connected (from the screen guard).
    pub async fn screen_connected(&self, connected: bool) {
        let _ = self.requests.send(Request::ScreenConnected { connected }).await;
    }

    /// True if the state actor answers (for the systemd watchdog).
    pub async fn alive(&self) -> bool {
        let (reply, answer) = oneshot::channel();
        self.requests.send(Request::Ping { reply }).await.is_ok() && answer.await.is_ok()
    }

    /// Records the display's power state (from the display power task).
    pub async fn display_power_changed(&self, state: DisplayPowerState) {
        let _ = self.requests.send(Request::DisplayPower { state }).await;
    }

    /// Paired devices: `(id, name, token_hash, paired_at)`.
    pub async fn devices(&self) -> Vec<(String, String, String, String)> {
        let (reply, devices) = oneshot::channel();
        if self.requests.send(Request::Devices { reply }).await.is_err() {
            return Vec::new();
        }
        devices.await.unwrap_or_default()
    }

    /// Revokes a device; returns its token hash so access can be withdrawn at once.
    pub async fn revoke_device(&self, device_id: String) -> Option<String> {
        let (reply, hash) = oneshot::channel();
        self.requests.send(Request::RevokeDevice { device_id, reply }).await.ok()?;
        hash.await.ok().flatten()
    }

    /// The whole library as a `.sursum` file.
    pub async fn export_library(&self) -> Option<Vec<u8>> {
        let (reply, bytes) = oneshot::channel();
        self.requests.send(Request::Export { reply }).await.ok()?;
        bytes.await.ok().flatten()
    }

    /// Merges a `.sursum` file (or only reports what would happen, with `dry_run`).
    pub async fn merge_library(
        &self,
        songs: Vec<crate::library::BundleSong>,
        dry_run: bool,
    ) -> Option<sursum_protocol::MergeReport> {
        let (reply, report) = oneshot::channel();
        self.requests.send(Request::Merge { songs, dry_run, reply }).await.ok()?;
        report.await.ok().flatten()
    }

    /// Saves an edited or new song (optimistic locking on the revision).
    pub async fn save_song(&self, draft: SongDraft) -> SaveOutcome {
        let (reply, outcome) = oneshot::channel();
        let unavailable = SaveOutcome::Invalid { reason: "server_unavailable".to_owned() };
        if self.requests.send(Request::SaveSong { draft, reply }).await.is_err() {
            return unavailable;
        }
        outcome.await.unwrap_or(unavailable)
    }
}

enum Request {
    Command {
        actor: ClientId,
        base_rev: Rev,
        command: Command,
        reply: oneshot::Sender<CmdOutcome>,
    },
    Search {
        query: String,
        limit: usize,
        reply: oneshot::Sender<Vec<SongSummary>>,
    },
    Draft {
        song_id: SongId,
        reply: oneshot::Sender<Option<SongDraft>>,
    },
    SaveSong {
        draft: SongDraft,
        reply: oneshot::Sender<SaveOutcome>,
    },
    Export {
        reply: oneshot::Sender<Option<Vec<u8>>>,
    },
    Merge {
        songs: Vec<crate::library::BundleSong>,
        dry_run: bool,
        reply: oneshot::Sender<Option<sursum_protocol::MergeReport>>,
    },
    AddDevice {
        name: String,
        token_hash: String,
        reply: oneshot::Sender<bool>,
    },
    DisplayPower {
        state: DisplayPowerState,
    },
    Ping {
        reply: oneshot::Sender<()>,
    },
    ScreenConnected {
        connected: bool,
    },
    Devices {
        reply: oneshot::Sender<Vec<(String, String, String, String)>>,
    },
    RevokeDevice {
        device_id: String,
        reply: oneshot::Sender<Option<String>>,
    },
    Import {
        library: sursum_formats::opensong::LibraryImport,
        reply: oneshot::Sender<sursum_protocol::ImportReport>,
    },
}

/// Starts the actor thread with restored state.
pub fn spawn(
    config: &Config,
    mut database: Database,
    platform: &dyn Platform,
) -> anyhow::Result<StateHandle> {
    // On the very first start the library gets the demo songs and a demo set.
    let mut sets = database.load_sets().context("loading sets")?;
    if sets.is_empty() {
        let demo_songs = demo::songs();
        for song in &demo_songs {
            database.upsert_song(song, "demo").context("storing demo songs")?;
        }
        let demo_set = demo::set(&demo_songs);
        database.save_set(&demo_set).context("storing the demo set")?;
        sets.push(demo_set);
    }
    let active_set: Option<SetId> = database.setting(ACTIVE_SET_KEY).context("loading the active set")?;
    let songs = database.load_songs().context("loading the song library")?;
    tracing::info!(songs = songs.len(), "song library loaded");
    let saved_profile: Option<ScreenProfile> =
        database.setting(SCREEN_PROFILE_KEY).context("loading the screen profile")?;
    if saved_profile.is_some() {
        tracing::info!("using the screen profile calibrated from a remote (config.toml's is ignored)");
    }
    let profile = saved_profile.unwrap_or_else(|| config.screen_profile.clone());
    let logo: Option<sursum_protocol::AssetId> =
        database.setting::<Option<sursum_protocol::AssetId>>(LOGO_KEY).context("loading the logo")?.flatten();
    let mut state = AppState {
        index: SearchIndex::build(&songs),
        data_dir: config.data_dir.clone(),
        songs: songs.into_iter().map(|song| (song.id.clone(), song)).collect(),
        sets: SetStore::new(sets, active_set),
        program: Program::default(),
        rev: Rev(1),
        theme: Theme::default(),
        layout: layout_or_fallback(&profile),
        profile,
        calibrating: false,
        display_power: DisplayPowerState::Unknown,
        logo,
        screen_connected: false,
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
            let _exit_on_panic = ExitIfActorPanics;
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
                    Request::Draft { song_id, reply } => {
                        let _ = reply.send(state.draft(&song_id));
                    }
                    Request::Export { reply } => {
                        let bytes = crate::library::export(&state.database);
                        if let Err(error) = &bytes {
                            tracing::error!(?error, "could not export the library");
                        }
                        let _ = reply.send(bytes.ok());
                    }
                    Request::Merge { songs, dry_run, reply } => {
                        let report = state.merge(&songs, dry_run);
                        if !dry_run {
                            screen_tx.send_replace(Arc::new(state.screen_state()));
                            remote_tx.send_replace(Arc::new(state.remote_state()));
                        }
                        let _ = reply.send(report);
                    }
                    Request::ScreenConnected { connected } => {
                        state.screen_connected = connected;
                        state.rev = state.rev.next();
                        remote_tx.send_replace(Arc::new(state.remote_state()));
                    }
                    Request::Ping { reply } => {
                        let _ = reply.send(());
                    }
                    Request::DisplayPower { state: power } => {
                        state.display_power = power;
                        state.rev = state.rev.next();
                        remote_tx.send_replace(Arc::new(state.remote_state()));
                    }
                    Request::AddDevice { name, token_hash, reply } => {
                        let id = format!("device-{}", uuid::Uuid::new_v4());
                        let stored = state.database.add_device(&id, &name, &token_hash);
                        if let Err(error) = &stored {
                            tracing::error!(?error, "could not store a paired device");
                        }
                        tracing::info!(device = %name, "a remote was paired");
                        let _ = reply.send(stored.is_ok());
                    }
                    Request::Devices { reply } => {
                        let devices = state.database.devices();
                        if let Err(error) = &devices {
                            tracing::error!(?error, "could not list devices");
                        }
                        let _ = reply.send(devices.unwrap_or_default());
                    }
                    Request::RevokeDevice { device_id, reply } => {
                        let hash = state.database.revoke_device(&device_id);
                        match &hash {
                            Ok(Some(_)) => tracing::info!(%device_id, "a remote was revoked"),
                            Ok(None) => {}
                            Err(error) => tracing::error!(?error, "could not revoke a device"),
                        }
                        let _ = reply.send(hash.ok().flatten());
                    }
                    Request::Import { library, reply } => {
                        let report = state.import(&library);
                        screen_tx.send_replace(Arc::new(state.screen_state()));
                        remote_tx.send_replace(Arc::new(state.remote_state()));
                        let _ = reply.send(report);
                    }
                    Request::SaveSong { draft, reply } => {
                        let outcome = state.save_song(&draft);
                        if matches!(outcome, SaveOutcome::Saved { .. }) {
                            screen_tx.send_replace(Arc::new(state.screen_state()));
                            remote_tx.send_replace(Arc::new(state.remote_state()));
                        }
                        let _ = reply.send(outcome);
                    }
                }
            }
        })
        .context("starting the state actor thread")?;

    Ok(StateHandle { requests, screen: screen_rx, remote: remote_rx })
}

/// Ends the process if the state actor panics. Without its actor the server would keep running
/// but answer nothing; exiting lets the service manager restart it, and the screen gets its slide
/// back (same boot: full restore, ADR 0003).
struct ExitIfActorPanics;

impl Drop for ExitIfActorPanics {
    fn drop(&mut self) {
        if std::thread::panicking() {
            tracing::error!("the state actor panicked; exiting so the server is restarted");
            std::process::exit(70);
        }
    }
}

/// Everything the server knows, owned by the actor thread.
struct AppState {
    songs: HashMap<SongId, Song>,
    index: SearchIndex,
    data_dir: std::path::PathBuf,
    sets: SetStore,
    program: Program,
    rev: Rev,
    theme: Theme,
    layout: ScreenLayout,
    /// The profile `layout` was computed from (shown in the calibration panel).
    profile: ScreenProfile,
    /// True while the screen shows the calibration pattern; never persisted.
    calibrating: bool,
    /// Last state reported by the display power task.
    display_power: DisplayPowerState,
    /// The parish logo for the logo overlay.
    logo: Option<sursum_protocol::AssetId>,
    /// True while a screen is connected.
    screen_connected: bool,
    transition_ms: u32,
    rules: SlideRules,
    boot_id: Option<String>,
    database: Database,
}

impl AppState {
    /// Applies one command; returns its outcome and whether the state changed.
    fn handle_command(&mut self, actor: &ClientId, base_rev: Rev, command: &Command) -> (CmdOutcome, bool) {
        let result = match command {
            Command::ShowNow { song_id } => return self.show_now(actor, song_id),
            Command::AddSong { song_id } => self.add_song(song_id),
            Command::AddText { title, text } => self.add_text(title, text),
            Command::AddImage { title, asset_id } => {
                if !crate::assets::exists(&self.data_dir, asset_id) {
                    return (CmdOutcome::Rejected { code: ErrorCode::NotFound }, false);
                }
                let title = if title.trim().is_empty() { "Obraz" } else { title.trim() };
                let kind = ItemKind::Image {
                    title: title.to_owned(),
                    asset_id: asset_id.clone(),
                    fit: sursum_protocol::ImageFit::Contain,
                };
                self.sets.insert(None, new_item_id(), kind).cloned().ok_or(ErrorCode::NoActiveSet).map(Some)
            }
            Command::AddBlank { title } => {
                let title = if title.trim().is_empty() { "Przerwa" } else { title.trim() };
                let kind = ItemKind::Blank { title: title.to_owned() };
                self.sets.insert(None, new_item_id(), kind).cloned().ok_or(ErrorCode::NoActiveSet).map(Some)
            }
            Command::RemoveItem { item_id } => self.remove_item(item_id),
            Command::MoveItem { item_id, to_index } => self.move_item(item_id, *to_index as usize),
            Command::UndoRemove => self.sets.undo_remove().cloned().ok_or(ErrorCode::NotFound).map(Some),
            Command::NewSet { name } => {
                let set = self.sets.create(SetId(format!("set-{}", uuid::Uuid::new_v4())), name).clone();
                Ok(Some(set))
            }
            Command::ActivateSet { set_id } => {
                if self.sets.activate(set_id) {
                    Ok(None)
                } else {
                    Err(ErrorCode::NotFound)
                }
            }
            Command::SetScreenProfile { profile } => self.set_screen_profile(profile),
            Command::SetLogo { asset_id } => {
                if asset_id.as_ref().is_some_and(|id| !crate::assets::exists(&self.data_dir, id)) {
                    return (CmdOutcome::Rejected { code: ErrorCode::NotFound }, false);
                }
                if let Err(error) = self.database.set_setting(LOGO_KEY, asset_id) {
                    tracing::error!(?error, "could not store the logo");
                }
                self.logo.clone_from(asset_id);
                Ok(None)
            }
            Command::Calibrate { on } => {
                self.calibrating = *on;
                Ok(None)
            }
            _ => return self.run_program_command(actor, base_rev, command),
        };
        match result {
            Ok(changed_set) => {
                if let Some(set) = changed_set {
                    self.save_set(&set);
                }
                self.save_active_set();
                self.rev = self.rev.next();
                (CmdOutcome::Applied { rev: self.rev }, true)
            }
            Err(code) => (CmdOutcome::Rejected { code }, false),
        }
    }

    /// Validates, applies and stores a new screen profile.
    fn set_screen_profile(&mut self, profile: &ScreenProfile) -> Result<Option<ServiceSet>, ErrorCode> {
        let layout = layout::screen_layout(profile).map_err(|_| ErrorCode::InvalidProfile)?;
        if let Err(error) = self.database.set_setting(SCREEN_PROFILE_KEY, profile) {
            tracing::error!(?error, "could not store the screen profile");
        }
        self.layout = layout;
        self.profile = profile.clone();
        Ok(None)
    }

    /// Inserts a song right after the live item (or at the end if nothing is live) and shows it.
    fn show_now(&mut self, actor: &ClientId, song_id: &SongId) -> (CmdOutcome, bool) {
        let anchor = self.program.live.as_ref().map(|live| live.item_id.clone());
        match self.insert_song(song_id, anchor.as_ref()) {
            Ok((item_id, set)) => {
                self.save_set(&set);
                // An explicit choice of the operator: never stale.
                let (outcome, _) = self.run_program_command(
                    actor,
                    Rev(u32::MAX),
                    &Command::GoTo { item_id, slide_index: 0 },
                );
                (outcome, true)
            }
            Err(code) => (CmdOutcome::Rejected { code }, false),
        }
    }

    fn add_song(&mut self, song_id: &SongId) -> Result<Option<ServiceSet>, ErrorCode> {
        self.insert_song(song_id, None).map(|(_, set)| Some(set))
    }

    /// Adds announcement slides; pages are separated by an empty line.
    fn add_text(&mut self, title: &str, text: &str) -> Result<Option<ServiceSet>, ErrorCode> {
        let slides: Vec<String> =
            text.split("\n\n").map(str::trim).filter(|page| !page.is_empty()).map(str::to_owned).collect();
        if slides.is_empty() {
            return Err(ErrorCode::NotFound);
        }
        let kind = ItemKind::Text { title: title.trim().to_owned(), slides };
        self.sets.insert(None, new_item_id(), kind).cloned().ok_or(ErrorCode::NoActiveSet).map(Some)
    }

    fn remove_item(&mut self, item_id: &ItemId) -> Result<Option<ServiceSet>, ErrorCode> {
        let (index, set) = self.sets.remove(item_id).ok_or(ErrorCode::NotFound)?;
        let set = set.clone();
        self.program.item_removed(item_id, index);
        Ok(Some(set))
    }

    fn move_item(&mut self, item_id: &ItemId, to_index: usize) -> Result<Option<ServiceSet>, ErrorCode> {
        self.sets.move_item(item_id, to_index).cloned().ok_or(ErrorCode::NotFound).map(Some)
    }

    /// Inserts a song into the active set after `anchor` (or at the end).
    fn insert_song(
        &mut self,
        song_id: &SongId,
        anchor: Option<&ItemId>,
    ) -> Result<(ItemId, ServiceSet), ErrorCode> {
        if !self.songs.contains_key(song_id) {
            return Err(ErrorCode::NotFound);
        }
        let item_id = new_item_id();
        let kind = ItemKind::Song { song_id: song_id.clone(), order_override: None };
        let set = self.sets.insert(anchor, item_id.clone(), kind).ok_or(ErrorCode::NoActiveSet)?;
        Ok((item_id, set.clone()))
    }

    fn save_set(&mut self, set: &ServiceSet) {
        if let Err(error) = self.database.save_set(set) {
            tracing::error!(?error, "could not save the set");
        }
    }

    fn save_active_set(&self) {
        if let Err(error) = self.database.set_setting(ACTIVE_SET_KEY, &self.sets.active_id()) {
            tracing::error!(?error, "could not save the active set");
        }
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
        let context = Context { set: self.sets.active(), deck_for: &deck_for };
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

    /// Merges a library file after backing up the database, then reloads the library.
    fn merge(
        &mut self,
        songs: &[crate::library::BundleSong],
        dry_run: bool,
    ) -> Option<sursum_protocol::MergeReport> {
        if !dry_run && let Err(error) = self.database.backup(&self.data_dir, "before-merge") {
            tracing::error!(?error, "could not back up before merging");
        }
        let report = crate::library::merge(&mut self.database, songs, dry_run);
        if let Err(error) = &report {
            tracing::error!(?error, "merging failed");
        }
        if !dry_run {
            self.reload_library();
        }
        report.ok()
    }

    /// Re-reads all songs from the database and rebuilds the search index.
    fn reload_library(&mut self) {
        match self.database.load_songs() {
            Ok(songs) => {
                self.index = SearchIndex::build(&songs);
                self.songs = songs.into_iter().map(|song| (song.id.clone(), song)).collect();
            }
            Err(error) => tracing::error!(?error, "could not reload the song library"),
        }
        self.rev = self.rev.next();
    }

    /// Imports songs after backing up the database, then reloads the library and the index.
    fn import(&mut self, library: &sursum_formats::opensong::LibraryImport) -> sursum_protocol::ImportReport {
        if let Err(error) = self.database.backup(&self.data_dir, "before-import") {
            tracing::error!(?error, "could not back up before importing");
        }
        let report = match crate::import::store(&mut self.database, library) {
            Ok((report, sets)) => {
                for set in sets {
                    self.sets.upsert(set);
                }
                report
            }
            Err(error) => {
                sursum_protocol::ImportReport { problems: vec![error.to_string()], ..Default::default() }
            }
        };
        self.reload_library();
        report
    }

    fn draft(&self, song_id: &SongId) -> Option<SongDraft> {
        let song = self.songs.get(song_id)?;
        let revision = self.database.song_revision(song_id.as_str()).ok().flatten()?;
        Some(editor::draft_of(song, u32::try_from(revision).unwrap_or(u32::MAX)))
    }

    /// Saves a draft. An existing song is only overwritten if nobody saved it since the editor
    /// opened it; the program keeps its frozen deck until the next navigation (ADR 0005).
    fn save_song(&mut self, draft: &SongDraft) -> SaveOutcome {
        let current = match &draft.song_id {
            Some(id) => match self.database.song_revision(id.as_str()) {
                Ok(Some(revision)) => Some(u32::try_from(revision).unwrap_or(u32::MAX)),
                Ok(None) => return SaveOutcome::Invalid { reason: "not_found".to_owned() },
                Err(error) => return SaveOutcome::Invalid { reason: format!("database: {error}") },
            },
            None => None,
        };
        if let (Some(current), Some(base)) = (current, draft.base_revision)
            && current != base
        {
            return SaveOutcome::Conflict { current_revision: current };
        }
        let id = draft.song_id.clone().unwrap_or_else(|| SongId(format!("song-{}", uuid::Uuid::new_v4())));
        let (song, _) = editor::song_from_draft(draft, id.clone(), self.songs.get(&id));
        if let Some(reason) = editor::validation_error(&song) {
            return SaveOutcome::Invalid { reason: reason.to_owned() };
        }
        let revision = match self.database.upsert_song(&song, "editor") {
            Ok(Upsert::Inserted) => 1,
            Ok(Upsert::Updated { revision }) => u32::try_from(revision).unwrap_or(u32::MAX),
            Ok(Upsert::Unchanged) => current.unwrap_or(1),
            Err(error) => return SaveOutcome::Invalid { reason: format!("database: {error}") },
        };
        self.index.upsert(&song);
        self.songs.insert(id.clone(), song);
        self.rev = self.rev.next();
        SaveOutcome::Saved { song_id: id, revision }
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
        // The live item belongs to the set that was active when it went live.
        let item = saved.set_id.as_ref().and_then(|set_id| self.sets.get(set_id)).and_then(|set| {
            let item_id = saved.item_id.as_ref()?;
            set.items.iter().find(|item| &item.id == item_id).cloned()
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
            set_id: self.sets.active().map(|set| set.id.clone()),
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
            calibration: self.calibrating.then(|| self.profile.clone()),
            logo: self.logo.clone(),
        }
    }

    fn remote_state(&self) -> RemoteState {
        let (songs, rules) = (&self.songs, self.rules);
        let deck_for = |item: &SetItem| compile_item(songs, rules, item);
        let context = Context { set: self.sets.active(), deck_for: &deck_for };
        RemoteState {
            rev: self.rev,
            program: self.screen_state(),
            next: self.program.peek_next(&context).as_ref().map(live_slide),
            sections: self.program.sections(),
            active_set: self.sets.active().map(|set| self.set_view(set)),
            sets: self.sets.summaries(),
            can_undo: self.sets.can_undo(),
            display_power: self.display_power.clone(),
            screen_profile: self.profile.clone(),
            screen_connected: self.screen_connected,
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
            other => other.own_title().unwrap_or_default().to_owned(),
        }
    }

    /// The deck of the item after the live one, so the screen can fit it in advance.
    fn upcoming_deck(&self) -> Option<Deck> {
        let set = self.sets.active()?;
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

/// A new, globally unique item id (sets are shared between devices later).
fn new_item_id() -> ItemId {
    ItemId(format!("item-{}", uuid::Uuid::new_v4()))
}

/// The settings key holding the active set.
const ACTIVE_SET_KEY: &str = "active_set";
/// Settings key of the parish logo (an asset id).
const LOGO_KEY: &str = "logo";
/// Settings key of a screen profile calibrated from a remote; it wins over `config.toml`.
const SCREEN_PROFILE_KEY: &str = "screen_profile";

/// Compiles the deck for a set item.
fn compile_item(songs: &HashMap<SongId, Song>, rules: SlideRules, item: &SetItem) -> Option<Deck> {
    sursum_core::slides::compile_item(item, |song_id| songs.get(song_id), rules)
}

fn live_slide(live: &Live) -> LiveSlide {
    LiveSlide { deck: live.deck.clone(), slide_index: live.slide_index as u32 }
}

/// The layout for a profile, or the plain safe area if the profile is invalid (logged).
fn layout_or_fallback(profile: &ScreenProfile) -> ScreenLayout {
    match layout::screen_layout(profile) {
        Ok(layout) => layout,
        Err(error) => {
            tracing::error!(%error, "invalid screen profile; ignoring exclusion zones");
            let without_zones = ScreenProfile { exclusions: Vec::new(), ..profile.clone() };
            let content = layout::content_region(&without_zones).unwrap_or(Rect::SCENE);
            ScreenLayout { content, masks: Vec::new() }
        }
    }
}
