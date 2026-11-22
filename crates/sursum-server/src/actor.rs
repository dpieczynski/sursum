// SPDX-License-Identifier: GPL-3.0-or-later
//! The state actor: the single owner of the server state (ADR 0005).
//!
//! It runs on its own thread (SQLite is synchronous), receives requests over a channel, applies
//! them one at a time, saves the program position and publishes whole snapshots through
//! `watch` channels. A slow client simply skips intermediate snapshots.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use anyhow::Context as _;
use sursum_core::layout;
use sursum_core::model::{ItemKind, ServiceSet, SetItem, Song};
use sursum_core::program::{Context, Live, Outcome, Program, Rejection};
use sursum_core::search::SearchIndex;
use sursum_core::slides::SlideRules;
use sursum_core::versions::VersionId;
use sursum_protocol::{
    ClientId, CmdOutcome, Command, Deck, DisplayPowerState, EndOfItem, ErrorCode, ItemId, LiveSlide, Overlay,
    Rect, RemoteState, Rev, SaveOutcome, ScreenLayout, ScreenProfile, ScreenState, SetId, SetItemView,
    SetView, SongDraft, SongId, SongSummary,
};
use tokio::sync::{mpsc, oneshot, watch};

use crate::config::Config;
use crate::db::{Base, Database, RuntimeRecord, SavedProgram, SongSave};
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

    /// Stores the PIN set from the first remote; false if it could not be stored.
    pub async fn set_pin(&self, pin: crate::auth::PinHash) -> bool {
        let (reply, stored) = oneshot::channel();
        if self.requests.send(Request::SetPin { pin, reply }).await.is_err() {
            return false;
        }
        stored.await.unwrap_or(false)
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

    /// Records that a remote's session ended (ends its calibration, if it was calibrating).
    pub async fn remote_gone(&self, client_id: ClientId) {
        let _ = self.requests.send(Request::RemoteGone { client_id }).await;
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
    pub async fn export_library(&self) -> Option<crate::library::Export> {
        let (reply, export) = oneshot::channel();
        self.requests.send(Request::Export { reply }).await.ok()?;
        export.await.ok().flatten()
    }

    /// Removes pictures nothing uses any more (or, with `dry_run`, counts them).
    pub async fn clean_up_pictures(&self, dry_run: bool) -> Option<sursum_protocol::PictureCleanup> {
        let (reply, cleanup) = oneshot::channel();
        self.requests.send(Request::CleanUpPictures { dry_run, reply }).await.ok()?;
        cleanup.await.ok().flatten()
    }

    /// Merges a `.sursum` file (or only reports what would happen, with `dry_run`).
    pub async fn merge_library(
        &self,
        incoming: crate::library::Incoming,
        dry_run: bool,
    ) -> Option<sursum_protocol::MergeReport> {
        let (reply, report) = oneshot::channel();
        self.requests.send(Request::Merge { incoming, dry_run, reply }).await.ok()?;
        report.await.ok().flatten()
    }

    /// Songs changed in two places that wait for review, for the list in the remote.
    pub async fn songs_in_review(&self) -> Vec<SongSummary> {
        let (reply, songs) = oneshot::channel();
        if self.requests.send(Request::SongsInReview { reply }).await.is_err() {
            return Vec::new();
        }
        songs.await.unwrap_or_default()
    }

    /// Every live version of a song, for the comparison screen.
    pub async fn song_versions(&self, song_id: SongId) -> Option<sursum_protocol::SongVersions> {
        let (reply, versions) = oneshot::channel();
        self.requests.send(Request::SongVersions { song_id, reply }).await.ok()?;
        versions.await.ok().flatten()
    }

    /// Every live version of a set in review, for the comparison screen.
    pub async fn set_versions(&self, set_id: SetId) -> Option<sursum_protocol::SetVersions> {
        let (reply, versions) = oneshot::channel();
        self.requests.send(Request::SetVersions { set_id, reply }).await.ok()?;
        versions.await.ok().flatten()
    }

    /// Keeps one version of a song in review and replaces the others.
    pub async fn keep_version(&self, song_id: SongId, version: String) -> SaveOutcome {
        let (reply, outcome) = oneshot::channel();
        let unavailable = SaveOutcome::Invalid { reason: "server_unavailable".to_owned() };
        if self.requests.send(Request::KeepVersion { song_id, version, reply }).await.is_err() {
            return unavailable;
        }
        outcome.await.unwrap_or(unavailable)
    }

    /// Saves an edited or new song on the version the editor loaded (ADR 0025).
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
    SongsInReview {
        reply: oneshot::Sender<Vec<SongSummary>>,
    },
    SongVersions {
        song_id: SongId,
        reply: oneshot::Sender<Option<sursum_protocol::SongVersions>>,
    },
    SetVersions {
        set_id: SetId,
        reply: oneshot::Sender<Option<sursum_protocol::SetVersions>>,
    },
    KeepVersion {
        song_id: SongId,
        version: String,
        reply: oneshot::Sender<SaveOutcome>,
    },
    Export {
        reply: oneshot::Sender<Option<crate::library::Export>>,
    },
    Merge {
        incoming: crate::library::Incoming,
        dry_run: bool,
        reply: oneshot::Sender<Option<sursum_protocol::MergeReport>>,
    },
    CleanUpPictures {
        dry_run: bool,
        reply: oneshot::Sender<Option<sursum_protocol::PictureCleanup>>,
    },
    SetPin {
        pin: crate::auth::PinHash,
        reply: oneshot::Sender<bool>,
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
    RemoteGone {
        client_id: ClientId,
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
    // On a device's very first start (only then) the library gets the demo songs and a demo
    // set with an id of its own: with a fixed id, two devices' demo sets would be one set.
    if database.setting::<bool>(STARTED_KEY).context("loading the first-start mark")?.is_none() {
        let demo_songs = demo::songs();
        for song in &demo_songs {
            database.save_song(song, Base::Current).context("storing demo songs")?;
        }
        database.save_set(&demo::set(&demo_songs, new_set_id())).context("storing the demo set")?;
        database.set_setting(STARTED_KEY, &true).context("marking the first start")?;
    }
    let churches = database.churches().context("loading the churches")?;
    let church = database.this_church().context("loading this device's church")?;
    let sets = database.load_sets().context("loading sets")?;
    let sets = sets.into_iter().filter(|set| shown_here(config.installation, church.as_ref(), set)).collect();
    let active_set: Option<SetId> = database.setting(ACTIVE_SET_KEY).context("loading the active set")?;
    let songs = database.load_songs().context("loading the song library")?;
    let reviews = database.songs_in_review().context("loading the songs in review")?.into_iter().collect();
    let set_reviews = database.sets_in_review().context("loading the sets in review")?.into_iter().collect();
    tracing::info!(songs = songs.len(), "song library loaded");
    let saved_profile: Option<ScreenProfile> =
        database.setting(SCREEN_PROFILE_KEY).context("loading the screen profile")?;
    let profile = layout::fit_to_scene(&saved_profile.unwrap_or(UNCALIBRATED));
    let logo: Option<sursum_protocol::AssetId> =
        database.setting::<Option<sursum_protocol::AssetId>>(LOGO_KEY).context("loading the logo")?.flatten();
    let end_of_item: EndOfItem =
        database.setting(END_OF_ITEM_KEY).context("loading the end-of-item setting")?.unwrap_or_default();
    let mut state = AppState {
        index: SearchIndex::build(&songs),
        data_dir: config.data_dir.clone(),
        songs: songs.into_iter().map(|song| (song.id.clone(), song)).collect(),
        reviews,
        set_reviews,
        churches,
        church,
        installation: config.installation,
        sets: SetStore::new(sets, active_set),
        program: Program::default(),
        rev: Rev(1),
        layout: layout_or_fallback(&profile),
        profile,
        calibrating_by: None,
        display_power: DisplayPowerState::Unknown,
        logo,
        screen_connected: false,
        end_of_item,
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
                    Request::SongsInReview { reply } => {
                        let _ = reply.send(state.songs_in_review());
                    }
                    Request::SongVersions { song_id, reply } => {
                        let _ = reply.send(state.song_versions(&song_id));
                    }
                    Request::SetVersions { set_id, reply } => {
                        let _ = reply.send(state.set_versions(&set_id));
                    }
                    Request::KeepVersion { song_id, version, reply } => {
                        let outcome = state.keep_version(&song_id, &version);
                        if matches!(outcome, SaveOutcome::Saved { .. }) {
                            screen_tx.send_replace(Arc::new(state.screen_state()));
                            remote_tx.send_replace(Arc::new(state.remote_state()));
                        }
                        let _ = reply.send(outcome);
                    }
                    Request::Export { reply } => {
                        let export = crate::library::collect(&state.database);
                        if let Err(error) = &export {
                            tracing::error!(?error, "could not export the library");
                        }
                        let _ = reply.send(export.ok());
                    }
                    Request::CleanUpPictures { dry_run, reply } => {
                        let in_use = state.pictures_in_use();
                        let cleanup = crate::assets::remove_unused(
                            &state.data_dir,
                            &in_use,
                            crate::assets::FRESH_UPLOAD,
                            dry_run,
                        );
                        if let Err(error) = &cleanup {
                            tracing::error!(?error, "could not clean up pictures");
                        }
                        let _ = reply.send(cleanup.ok());
                    }
                    Request::Merge { incoming, dry_run, reply } => {
                        let report = state.merge(&incoming, dry_run);
                        if !dry_run {
                            screen_tx.send_replace(Arc::new(state.screen_state()));
                            remote_tx.send_replace(Arc::new(state.remote_state()));
                        }
                        let _ = reply.send(report);
                    }
                    Request::RemoteGone { client_id } => {
                        // A remote that turned calibration on and went away (closed, reloaded,
                        // battery dead) must not leave the test card on the church screen.
                        if state.calibrating_by.as_ref() == Some(&client_id) {
                            state.calibrating_by = None;
                            state.rev = state.rev.next();
                            screen_tx.send_replace(Arc::new(state.screen_state()));
                            remote_tx.send_replace(Arc::new(state.remote_state()));
                        }
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
                    Request::SetPin { pin, reply } => {
                        let stored = state.database.set_setting(crate::auth::PIN_SETTING, &pin);
                        if let Err(error) = &stored {
                            tracing::error!(?error, "could not store the PIN");
                        } else {
                            tracing::info!("the PIN was set from a remote");
                        }
                        let _ = reply.send(stored.is_ok());
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
    /// Songs changed in two places that wait for review (ADR 0025).
    reviews: BTreeSet<SongId>,
    /// Sets changed in two places that wait for review (ADR 0025).
    set_reviews: BTreeSet<SetId>,
    /// Every church this device knows, by name (ADR 0025).
    churches: Vec<crate::db::Church>,
    /// The church this device serves; `None` until it is named.
    church: Option<sursum_protocol::ChurchId>,
    /// A device with a screen, or the desktop program's library (which shows every set).
    installation: sursum_protocol::Installation,
    index: SearchIndex,
    data_dir: std::path::PathBuf,
    sets: SetStore,
    program: Program,
    rev: Rev,
    layout: ScreenLayout,
    /// The profile `layout` was computed from (shown in the calibration panel).
    profile: ScreenProfile,
    /// The remote calibrating, while the screen shows the calibration pattern; never persisted.
    /// Calibration ends when that remote turns it off or disconnects.
    calibrating_by: Option<ClientId>,
    /// Last state reported by the display power task.
    display_power: DisplayPowerState,
    /// The parish logo for the logo overlay.
    logo: Option<sursum_protocol::AssetId>,
    /// True while a screen is connected.
    screen_connected: bool,
    /// What "next" does after an item's last slide (a device setting).
    end_of_item: EndOfItem,
    transition_ms: u32,
    rules: SlideRules,
    boot_id: Option<String>,
    database: Database,
}

impl AppState {
    /// Pictures that must stay: the logo, every picture in a set (also one whose deletion can
    /// still be undone) and what is on screen (a frozen copy may outlive its item).
    fn pictures_in_use(&self) -> std::collections::HashSet<sursum_protocol::AssetId> {
        let live = self.program.live.iter().flat_map(|live| &live.deck.slides);
        let on_screen = live.filter_map(|slide| match &slide.body {
            sursum_protocol::SlideBody::Image { asset_id, .. } => Some(asset_id),
            _ => None,
        });
        self.logo.iter().chain(self.sets.pictures()).chain(on_screen).cloned().collect()
    }

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
                let kind = ItemKind::Image { title: title.to_owned(), asset_id: asset_id.clone() };
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
            Command::NewSet { name, date } => checked_date(date.as_deref())
                .map(|date| Some(self.sets.create(new_set_id(), name, date, self.church.clone()).clone())),
            Command::DateSet { set_id, date } => checked_date(date.as_deref()).and_then(|date| {
                self.sets.set_date(set_id, date).cloned().ok_or(ErrorCode::NotFound).map(Some)
            }),
            Command::CopySet { set_id, name, date, church } => {
                self.copy_set(set_id, name, date.as_deref(), church)
            }
            Command::ActivateSet { set_id } => {
                if self.sets.activate(set_id) {
                    Ok(None)
                } else {
                    Err(ErrorCode::NotFound)
                }
            }
            Command::RenameSet { set_id, name } => {
                self.sets.rename(set_id, name).cloned().ok_or(ErrorCode::NotFound).map(Some)
            }
            Command::DeleteSet { set_id } => self.delete_set(set_id),
            Command::KeepSetVersion { set_id, version, add } => self.keep_set_version(set_id, version, add),
            Command::UndoDeleteSet => self.undo_delete_set(),
            Command::SetEndOfItem { mode } => {
                if let Err(error) = self.database.set_setting(END_OF_ITEM_KEY, mode) {
                    tracing::error!(?error, "could not store the end-of-item setting");
                }
                self.end_of_item = *mode;
                Ok(None)
            }
            Command::SetScreenProfile { profile } => self.set_screen_profile(profile),
            Command::DeleteSong { song_id } => match self.database.delete_song(song_id.as_str()) {
                Ok(true) => {
                    tracing::info!(%song_id, "song deleted");
                    self.reload_library();
                    Ok(None)
                }
                Ok(false) => Err(ErrorCode::NotFound),
                Err(error) => {
                    tracing::error!(?error, "could not delete a song");
                    Err(ErrorCode::NotFound)
                }
            },
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
                self.calibrating_by = on.then(|| actor.clone());
                Ok(None)
            }
            Command::NameChurch { name } => match self.database.name_church(name) {
                Ok(Some(_)) => {
                    self.reload_churches();
                    self.adopt_sets(None);
                    Ok(None)
                }
                Ok(None) => Err(ErrorCode::Unsupported),
                Err(error) => {
                    tracing::error!(?error, "could not name the church");
                    Err(ErrorCode::NotFound)
                }
            },
            Command::UseChurch { church_id } => match self.database.use_church(church_id) {
                Ok(true) => {
                    let left = self.church.clone();
                    self.reload_churches();
                    match self.installation {
                        sursum_protocol::Installation::Screen => self.adopt_sets(left.as_ref()),
                        // The desktop program prepares sets for any church: choosing one only
                        // changes which sets it lists and whose new sets are.
                        sursum_protocol::Installation::Library => self.reload_sets(),
                    }
                    Ok(None)
                }
                Ok(false) => Err(ErrorCode::NotFound),
                Err(error) => {
                    tracing::error!(?error, "could not change the church");
                    Err(ErrorCode::NotFound)
                }
            },
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

    /// Fits a new screen profile to the stage, validates, applies and stores it.
    fn set_screen_profile(&mut self, profile: &ScreenProfile) -> Result<Option<ServiceSet>, ErrorCode> {
        let profile = layout::fit_to_scene(profile);
        let layout = layout::screen_layout(&profile).map_err(|_| ErrorCode::InvalidProfile)?;
        if let Err(error) = self.database.set_setting(SCREEN_PROFILE_KEY, &profile) {
            tracing::error!(?error, "could not store the screen profile");
        }
        self.layout = layout;
        self.profile = profile;
        Ok(None)
    }

    /// Shows a song at once: its next occurrence in the active set if it is there (no
    /// duplicates), otherwise it is inserted right after the live item (or at the end).
    fn show_now(&mut self, actor: &ClientId, song_id: &SongId) -> (CmdOutcome, bool) {
        let anchor = self.program.live.as_ref().map(|live| live.item_id.clone());
        if let Some(item_id) = self.occurrence_of(song_id, anchor.as_ref()) {
            let command = Command::GoTo { item_id, slide_index: 0 };
            return self.run_program_command(actor, Rev(u32::MAX), &command);
        }
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

    /// The item of the active set showing `song_id`: the first one after `after` (the live item),
    /// else the first one at all.
    fn occurrence_of(&self, song_id: &SongId, after: Option<&ItemId>) -> Option<ItemId> {
        let set = self.sets.active()?;
        let shows =
            |item: &&SetItem| matches!(&item.kind, ItemKind::Song { song_id: id, .. } if id == song_id);
        let start = after.and_then(|id| set.position(id)).map_or(0, |position| position + 1);
        let later = set.items.iter().skip(start).find(shows);
        later.or_else(|| set.items.iter().find(shows)).map(|item| item.id.clone())
    }

    /// Deletes a set unless an item of it is on screen.
    fn delete_set(&mut self, set_id: &SetId) -> Result<Option<ServiceSet>, ErrorCode> {
        let set = self.sets.get(set_id).ok_or(ErrorCode::NotFound)?;
        let live = self.program.live.as_ref().map(|live| &live.item_id);
        if live.is_some_and(|item_id| set.position(item_id).is_some()) {
            return Err(ErrorCode::SetOnScreen);
        }
        if let Err(error) = self.database.delete_set(set_id.as_str()) {
            tracing::error!(?error, "could not delete a set");
            return Err(ErrorCode::NotFound);
        }
        self.sets.delete(set_id);
        self.set_reviews.remove(set_id);
        tracing::info!(%set_id, "set deleted");
        Ok(None)
    }

    fn undo_delete_set(&mut self) -> Result<Option<ServiceSet>, ErrorCode> {
        // Saving it again (by the caller) puts a live version on top of the deletion.
        let set = self.sets.undo_delete().cloned().ok_or(ErrorCode::NotFound)?;
        tracing::info!(set_id = %set.id, "set restored");
        Ok(Some(set))
    }

    /// "Nowy zestaw z tego": a new, independent set with the same items (new item ids), for this
    /// device's church; the desktop program may choose the church. It becomes active.
    fn copy_set(
        &mut self,
        set_id: &SetId,
        name: &str,
        date: Option<&str>,
        church: &Option<sursum_protocol::ChurchId>,
    ) -> Result<Option<ServiceSet>, ErrorCode> {
        let date = checked_date(date)?;
        let source = self.sets.get(set_id).ok_or(ErrorCode::NotFound)?;
        let church = match self.installation {
            sursum_protocol::Installation::Library => {
                church.clone().or_else(|| self.church.clone()).or_else(|| source.church.clone())
            }
            sursum_protocol::Installation::Screen => self.church.clone(),
        };
        let items =
            source.items.iter().map(|item| SetItem { id: new_item_id(), kind: item.kind.clone() }).collect();
        let name = if name.trim().is_empty() { source.name.clone() } else { name.trim().to_owned() };
        let copy = ServiceSet { id: new_set_id(), name, items, date, church };
        Ok(Some(self.sets.add(copy).clone()))
    }

    /// The live versions of a set and the set each holds, the shown one first.
    fn set_version_bodies(&self, set_id: &SetId) -> Option<Vec<(VersionId, bool, ServiceSet)>> {
        let heads = self.database.set_heads(set_id).ok()?;
        let shown = heads.current()?.id().clone();
        let mut versions: Vec<(VersionId, bool, ServiceSet)> = heads
            .versions()
            .iter()
            .filter_map(|version| {
                let set = crate::db::set_from_body(set_id, version.live_body()?).ok()?;
                Some((version.id().clone(), version.id() == &shown, set))
            })
            .collect();
        versions.sort_by_key(|(_, shown, _)| !shown);
        Some(versions)
    }

    /// Every live version of a set this device shows, for the comparison screen.
    fn set_versions(&self, set_id: &SetId) -> Option<sursum_protocol::SetVersions> {
        self.sets.get(set_id)?;
        let versions = self.set_version_bodies(set_id)?;
        let sets: Vec<ServiceSet> = versions.iter().map(|(_, _, set)| set.clone()).collect();
        let views = versions
            .iter()
            .map(|(version, shown, set)| {
                let missing = sursum_core::set_review::items_missing_elsewhere(set, &sets);
                let items = set
                    .items
                    .iter()
                    .zip(missing)
                    .map(|(item, missing_elsewhere)| sursum_protocol::SetVersionItem {
                        item_id: item.id.clone(),
                        title: self.item_title(item),
                        kind: self.item_kind(item),
                        missing_elsewhere,
                    })
                    .collect();
                sursum_protocol::SetVersionView {
                    version: version.as_str().to_owned(),
                    shown: *shown,
                    name: set.name.clone(),
                    date: set.date.clone(),
                    items,
                }
            })
            .collect();
        Some(sursum_protocol::SetVersions { set_id: set_id.clone(), versions: views })
    }

    /// The set review's "Zostaw tę": one version, with the items chosen from the others, replaces
    /// every version. Like an edit from another remote, it never changes the program (ADR 0005).
    fn keep_set_version(
        &mut self,
        set_id: &SetId,
        version: &str,
        add: &[ItemId],
    ) -> Result<Option<ServiceSet>, ErrorCode> {
        self.sets.get(set_id).ok_or(ErrorCode::NotFound)?;
        let keep = VersionId::parse(version).ok_or(ErrorCode::NotFound)?;
        let versions = self.set_version_bodies(set_id).ok_or(ErrorCode::NotFound)?;
        let (kept, others): (Vec<_>, Vec<_>) = versions.into_iter().partition(|(id, _, _)| id == &keep);
        let (_, _, kept) = kept.into_iter().next().ok_or(ErrorCode::NotFound)?;
        let others: Vec<ServiceSet> = others.into_iter().map(|(_, _, set)| set).collect();
        let result = sursum_core::set_review::keep_with_items(&kept, &others, add);
        if let Err(error) = self.database.resolve_set(&result) {
            tracing::error!(?error, "could not keep a set version");
            return Err(ErrorCode::NotFound);
        }
        tracing::info!(%set_id, "set reviewed");
        self.reload_sets();
        Ok(None)
    }

    /// After the device's church changed: sets made before it was named, or for the church it
    /// left, become this church's sets (a version each), then the list is read again.
    fn adopt_sets(&mut self, left: Option<&sursum_protocol::ChurchId>) {
        let Some(church) = self.church.clone() else { return };
        let sets = match self.database.load_sets() {
            Ok(sets) => sets,
            Err(error) => return tracing::error!(?error, "could not read the sets"),
        };
        for mut set in sets {
            if set.church.is_none() || set.church.as_ref() == left {
                set.church = Some(church.clone());
                if let Err(error) = self.database.save_set(&set) {
                    tracing::error!(?error, "could not move a set to this church");
                }
            }
        }
        self.reload_sets();
    }

    /// Reads the sets this device shows again, keeping the active one if it is still there.
    fn reload_sets(&mut self) {
        match self.database.sets_in_review() {
            Ok(reviews) => self.set_reviews = reviews.into_iter().collect(),
            Err(error) => tracing::error!(?error, "could not read the sets in review"),
        }
        match self.database.load_sets() {
            Ok(sets) => {
                let shown =
                    sets.into_iter().filter(|set| shown_here(self.installation, self.church.as_ref(), set));
                self.sets = SetStore::new(shown.collect(), self.sets.active_id().cloned());
            }
            Err(error) => tracing::error!(?error, "could not read the sets"),
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
        self.program.item_removed(item_id, index, &set);
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
        let context = Context { set: self.sets.active(), deck_for: &deck_for, end_of_item: self.end_of_item };
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
        incoming: &crate::library::Incoming,
        dry_run: bool,
    ) -> Option<sursum_protocol::MergeReport> {
        if !dry_run && let Err(error) = self.database.backup(&self.data_dir, "before-merge") {
            tracing::error!(?error, "could not back up before merging");
        }
        let (installation, church) = (self.installation, self.church.clone());
        let ours = move |set: &ServiceSet| shown_here(installation, church.as_ref(), set);
        let report = crate::library::merge(&mut self.database, incoming, dry_run, &ours);
        if let Err(error) = &report {
            tracing::error!(?error, "merging failed");
        }
        if !dry_run {
            self.reload_library();
            self.reload_churches();
            self.reload_sets();
        }
        report.ok()
    }

    /// Re-reads the churches and which one this device serves.
    fn reload_churches(&mut self) {
        match (self.database.churches(), self.database.this_church()) {
            (Ok(churches), Ok(church)) => {
                self.churches = churches;
                self.church = church;
            }
            (Err(error), _) | (_, Err(error)) => tracing::error!(?error, "could not read the churches"),
        }
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
        match self.database.songs_in_review() {
            Ok(reviews) => self.reviews = reviews.into_iter().collect(),
            Err(error) => tracing::error!(?error, "could not read the songs in review"),
        }
        self.rev = self.rev.next();
    }

    /// Imports songs after backing up the database, then reloads the library and the index.
    fn import(&mut self, library: &sursum_formats::opensong::LibraryImport) -> sursum_protocol::ImportReport {
        if let Err(error) = self.database.backup(&self.data_dir, "before-import") {
            tracing::error!(?error, "could not back up before importing");
        }
        let report = match crate::import::store(&mut self.database, library, self.church.as_ref()) {
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
        let version = self.database.song_version(song_id.as_str()).ok().flatten()?;
        Some(editor::draft_of(song, &version))
    }

    /// Saves a draft as a new version built on the one the editor loaded (ADR 0025); refused if
    /// that version was replaced meanwhile. The program keeps its frozen deck until the next
    /// navigation (ADR 0005).
    fn save_song(&mut self, draft: &SongDraft) -> SaveOutcome {
        let loaded = match (&draft.song_id, &draft.base_version) {
            (Some(_), Some(version)) => match VersionId::parse(version) {
                Some(version) => Some(version),
                None => return SaveOutcome::Invalid { reason: "bad_version".to_owned() },
            },
            (Some(_), None) => return SaveOutcome::Invalid { reason: "bad_version".to_owned() },
            (None, _) => None,
        };
        let id = draft.song_id.clone().unwrap_or_else(|| SongId(format!("song-{}", uuid::Uuid::new_v4())));
        let (song, _) = editor::song_from_draft(draft, id.clone(), self.songs.get(&id));
        if let Some(reason) = editor::validation_error(&song) {
            return SaveOutcome::Invalid { reason: reason.to_owned() };
        }
        let base = loaded.as_ref().map_or(Base::Current, Base::Loaded);
        let version = match self.database.save_song(&song, base) {
            Ok(SongSave::Created(version) | SongSave::Changed(version) | SongSave::Unchanged(version)) => {
                version
            }
            Ok(SongSave::Outdated) => return SaveOutcome::Conflict,
            Err(error) => return SaveOutcome::Invalid { reason: format!("database: {error}") },
        };
        self.reload_library();
        SaveOutcome::Saved { song_id: id, version: version.as_str().to_owned() }
    }

    /// The songs in review, by title.
    fn songs_in_review(&self) -> Vec<SongSummary> {
        let mut songs: Vec<SongSummary> = self
            .reviews
            .iter()
            .filter_map(|id| self.songs.get(id))
            .map(|song| song_summary(song, true))
            .collect();
        songs.sort_by(|a, b| a.title.cmp(&b.title));
        songs
    }

    /// Every live version of a song, the shown one first, in the editor's text form.
    fn song_versions(&self, song_id: &SongId) -> Option<sursum_protocol::SongVersions> {
        let heads = self.database.song_heads(song_id).ok()?;
        let shown = heads.current()?.id().clone();
        let mut versions: Vec<sursum_protocol::SongVersionView> = heads
            .versions()
            .iter()
            .filter_map(|version| {
                let song = crate::db::song_from_body(song_id, version.live_body()?).ok()?;
                let draft = editor::draft_of(&song, version.id());
                Some(sursum_protocol::SongVersionView {
                    version: version.id().as_str().to_owned(),
                    shown: version.id() == &shown,
                    title: draft.title,
                    lyrics: draft.lyrics,
                })
            })
            .collect();
        versions.sort_by_key(|version| !version.shown);
        Some(sursum_protocol::SongVersions { song_id: song_id.clone(), versions })
    }

    /// The review's "Zostaw tę": one version replaces the others.
    fn keep_version(&mut self, song_id: &SongId, version: &str) -> SaveOutcome {
        let Some(keep) = VersionId::parse(version) else {
            return SaveOutcome::Invalid { reason: "bad_version".to_owned() };
        };
        match self.database.keep_song_version(song_id, &keep) {
            Ok(Some(resolved)) => {
                self.reload_library();
                SaveOutcome::Saved { song_id: song_id.clone(), version: resolved.as_str().to_owned() }
            }
            Ok(None) => SaveOutcome::Conflict,
            Err(error) => SaveOutcome::Invalid { reason: format!("database: {error}") },
        }
    }

    fn search(&self, query: &str, limit: usize) -> Vec<SongSummary> {
        self.index
            .query(query, limit)
            .into_iter()
            .filter_map(|hit| self.songs.get(&hit.song_id))
            .map(|song| song_summary(song, self.reviews.contains(&song.id)))
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
        self.program.live = Some(Live { gap: saved.gap, ..Live::at(item.id, deck, slide_index) });
        tracing::info!(same_boot, "restored the program position");
    }

    fn save_program(&self) {
        let live = self.program.live.as_ref();
        let saved = SavedProgram {
            set_id: self.sets.active().map(|set| set.id.clone()),
            item_id: live.map(|live| live.item_id.clone()),
            slide_key: live.map(|live| live.deck.slides[live.slide_index].key.clone()),
            overlay: self.program.overlay,
            gap: live.is_some_and(|live| live.gap),
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
            transition_ms: self.transition_ms,
            layout: self.layout.clone(),
            calibration: self.calibrating_by.as_ref().map(|_| self.profile.clone()),
            logo: self.logo.clone(),
        }
    }

    fn remote_state(&self) -> RemoteState {
        let (songs, rules) = (&self.songs, self.rules);
        let deck_for = |item: &SetItem| compile_item(songs, rules, item);
        let context = Context { set: self.sets.active(), deck_for: &deck_for, end_of_item: self.end_of_item };
        RemoteState {
            rev: self.rev,
            program: self.screen_state(),
            next: self.program.peek_next(&context).as_ref().map(live_slide),
            active_set: self.sets.active().map(|set| self.set_view(set)),
            sets: self.sets.summaries(&self.set_reviews),
            can_undo: self.sets.can_undo(),
            display_power: self.display_power.clone(),
            screen_profile: self.profile.clone(),
            screen_connected: self.screen_connected,
            next_step: self.program.next_step(&context),
            end_of_item: self.end_of_item,
            songs_to_review: u32::try_from(self.reviews.len()).unwrap_or(u32::MAX),
            church: self
                .church
                .as_ref()
                .and_then(|id| self.churches.iter().find(|church| &church.id == id))
                .map(church_view),
            churches: self.churches.iter().map(church_view).collect(),
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
                kind: self.item_kind(item),
                song_id: match &item.kind {
                    ItemKind::Song { song_id, .. } => Some(song_id.clone()),
                    _ => None,
                },
                song_number: match &item.kind {
                    ItemKind::Song { song_id, .. } => self
                        .songs
                        .get(song_id)
                        .and_then(|song| song.songbooks.first())
                        .map(|entry| entry.entry.clone()),
                    _ => None,
                },
            })
            .collect();
        SetView {
            set_id: set.id.clone(),
            name: set.name.clone(),
            items,
            date: set.date.clone(),
            review: self.set_reviews.contains(&set.id),
        }
    }

    fn item_kind(&self, item: &SetItem) -> sursum_protocol::SetItemKind {
        use sursum_protocol::SetItemKind;
        match &item.kind {
            ItemKind::Song { song_id, .. } if self.songs.contains_key(song_id) => SetItemKind::Song,
            ItemKind::Song { .. } => SetItemKind::MissingSong,
            ItemKind::Text { .. } => SetItemKind::Text,
            ItemKind::Image { .. } => SetItemKind::Image,
            ItemKind::Blank { .. } => SetItemKind::Pause,
        }
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
fn church_view(church: &crate::db::Church) -> sursum_protocol::ChurchView {
    sursum_protocol::ChurchView { church_id: church.id.clone(), name: church.name.clone() }
}

fn song_summary(song: &Song, review: bool) -> SongSummary {
    let first_line =
        song.verses.iter().flat_map(|verse| &verse.blocks).flat_map(|block| block.texts()).next();
    SongSummary {
        song_id: song.id.clone(),
        title: song.title().to_owned(),
        number: song.songbooks.first().map(|entry| entry.entry.clone()),
        first_line: first_line.unwrap_or_default().to_owned(),
        review,
    }
}

/// A set date from the remote, checked; `Err(InvalidDate)` for anything else than
/// `YYYY-MM-DD[THH:MM]`. Empty text clears the date.
fn checked_date(date: Option<&str>) -> Result<Option<String>, ErrorCode> {
    match date.map(str::trim).filter(|date| !date.is_empty()) {
        None => Ok(None),
        Some(date) if sursum_core::model::is_set_date(date) => Ok(Some(date.to_owned())),
        Some(_) => Err(ErrorCode::InvalidDate),
    }
}

/// True if this device shows `set`: its church's sets and those made before a church was named
/// (ADR 0025). The desktop program shows the sets of the church chosen in its Zestaw view, or
/// every set until one is chosen.
fn shown_here(
    installation: sursum_protocol::Installation,
    church: Option<&sursum_protocol::ChurchId>,
    set: &ServiceSet,
) -> bool {
    let nothing_chosen = installation == sursum_protocol::Installation::Library && church.is_none();
    nothing_chosen || set.church.is_none() || set.church.as_ref() == church
}

/// A new, globally unique set id (sets travel between devices).
fn new_set_id() -> SetId {
    SetId(format!("set-{}", uuid::Uuid::new_v4()))
}

/// A new, globally unique item id (sets travel between devices).
fn new_item_id() -> ItemId {
    ItemId(format!("item-{}", uuid::Uuid::new_v4()))
}

/// The settings key holding the active set.
const ACTIVE_SET_KEY: &str = "active_set";
/// Settings key of what "next" does after an item's last slide.
const END_OF_ITEM_KEY: &str = "end_of_item";
/// Settings key of the parish logo (an asset id).
const LOGO_KEY: &str = "logo";
/// Settings key of the screen profile calibrated from a remote (its only source, ADR 0008).
const SCREEN_PROFILE_KEY: &str = "screen_profile";
/// Settings key marking that the device has started once (the demo set is made only then).
const STARTED_KEY: &str = "started";
/// The screen until the calibration panel saves a profile: 48 px margins, no exclusion zones.
const UNCALIBRATED: ScreenProfile =
    ScreenProfile { safe_area: sursum_protocol::Insets::uniform(48), exclusions: Vec::new() };

/// Compiles the deck for a set item.
fn compile_item(songs: &HashMap<SongId, Song>, rules: SlideRules, item: &SetItem) -> Option<Deck> {
    sursum_core::slides::compile_item(item, |song_id| songs.get(song_id), rules)
}

fn live_slide(live: &Live) -> LiveSlide {
    LiveSlide { deck: live.deck.clone(), slide_index: live.slide_index as u32, gap: live.gap }
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
