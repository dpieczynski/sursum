// SPDX-License-Identifier: GPL-3.0-or-later
//! The JSON wire format, pinned with snapshots. A screen may run old code until it reloads after
//! a server update, so any change here must be deliberate: review the snapshot diff and bump
//! `PROTOCOL_VERSION` if old clients would misread the new format.

use sursum_protocol::*;

fn sample_deck() -> Deck {
    Deck {
        deck_id: DeckId::from("0123456789abcdef"),
        fit_group: "0123456789abcdef".to_owned(),
        item_id: ItemId::from("item-1"),
        title: "Kiedy ranne wstają zorze".to_owned(),
        slides: vec![Slide {
            key: SlideKey::from("v1#0@0"),
            label: "v1".to_owned(),
            body: SlideBody::Lyrics { lines: vec!["Kiedy ranne wstają zorze,".to_owned()] },
        }],
    }
}

fn sample_screen() -> ScreenState {
    ScreenState {
        rev: Rev(7),
        live: Some(LiveSlide { deck: sample_deck(), slide_index: 0, gap: false }),
        upcoming: None,
        overlay: Overlay::Black,
        transition_ms: 250,
        layout: ScreenLayout {
            content: Rect::new(600, 48, 1272, 984),
            masks: vec![Rect::new(0, 0, 600, 1080)],
        },
        calibration: None,
        logo: Some(AssetId::from("0f0f.png")),
    }
}

fn sample_profile() -> ScreenProfile {
    ScreenProfile {
        safe_area: Insets::uniform(48),
        exclusions: vec![ExclusionZone {
            name: "Strefa 1".into(),
            rect: Rect::new(0, 0, 600, 1080),
            mask: MaskMode::Black,
        }],
    }
}

#[test]
fn client_messages() {
    let messages = vec![
        ClientMsg::Hello {
            protocol: PROTOCOL_VERSION,
            role: Role::Remote,
            client_id: ClientId::from("c1"),
            build_id: "b".into(),
        },
        ClientMsg::Cmd { seq: 3, base_rev: Rev(7), cmd: Command::Next },
        ClientMsg::Cmd {
            seq: 4,
            base_rev: Rev(7),
            cmd: Command::GoTo { item_id: ItemId::from("item-2"), slide_index: 1 },
        },
        ClientMsg::Cmd { seq: 5, base_rev: Rev(7), cmd: Command::SetOverlay { overlay: Overlay::Logo } },
        ClientMsg::Cmd {
            seq: 6,
            base_rev: Rev(7),
            cmd: Command::SetScreenProfile { profile: sample_profile() },
        },
        ClientMsg::Cmd { seq: 7, base_rev: Rev(7), cmd: Command::Calibrate { on: true } },
        ClientMsg::Cmd {
            seq: 8,
            base_rev: Rev(7),
            cmd: Command::RenameSet { set_id: SetId::from("set"), name: "Niedziela".into() },
        },
        ClientMsg::Cmd { seq: 9, base_rev: Rev(7), cmd: Command::DeleteSet { set_id: SetId::from("old") } },
        ClientMsg::Cmd { seq: 10, base_rev: Rev(7), cmd: Command::UndoDeleteSet },
        ClientMsg::Cmd {
            seq: 10,
            base_rev: Rev(7),
            cmd: Command::SetEndOfItem { mode: EndOfItem::Continue },
        },
        ClientMsg::Cmd { seq: 11, base_rev: Rev(7), cmd: Command::NameChurch { name: "Tulce".into() } },
        ClientMsg::Cmd {
            seq: 13,
            base_rev: Rev(7),
            cmd: Command::CopySet {
                set_id: SetId::from("set"),
                name: "Niedziela".into(),
                date: Some("2026-11-29".into()),
                church: None,
            },
        },
        ClientMsg::Cmd {
            seq: 15,
            base_rev: Rev(7),
            cmd: Command::KeepSetVersion {
                set_id: SetId::from("set"),
                version: "ab".repeat(32),
                add: vec![ItemId::from("item-2")],
            },
        },
        ClientMsg::Cmd {
            seq: 14,
            base_rev: Rev(7),
            cmd: Command::DateSet { set_id: SetId::from("set"), date: Some("2026-11-22T10:00".into()) },
        },
        ClientMsg::Cmd {
            seq: 12,
            base_rev: Rev(7),
            cmd: Command::UseChurch { church_id: ChurchId::from("church-0123456789abcdef") },
        },
        ClientMsg::Ping,
    ];
    insta::assert_json_snapshot!(messages);
}

#[test]
fn server_messages() {
    let messages = vec![
        ServerMsg::Welcome { protocol: PROTOCOL_VERSION, build_id: "b".into() },
        ServerMsg::Screen { state: sample_screen() },
        ServerMsg::CmdResult { seq: 3, outcome: CmdOutcome::Stale { rev: Rev(8) } },
        ServerMsg::CmdResult { seq: 4, outcome: CmdOutcome::Rejected { code: ErrorCode::NotFound } },
        ServerMsg::Pong,
    ];
    insta::assert_json_snapshot!(messages);
}

#[test]
fn remote_state() {
    let state = RemoteState {
        rev: Rev(7),
        program: sample_screen(),
        next: Some(LiveSlide { deck: sample_deck(), slide_index: 0, gap: true }),
        active_set: Some(SetView {
            set_id: SetId::from("set"),
            name: "Msza".into(),
            items: vec![SetItemView {
                item_id: ItemId::from("item-1"),
                number: 1,
                title: "Pieśń".into(),
                kind: SetItemKind::Song,
                song_id: Some(SongId::from("song-1")),
                song_number: Some("123".into()),
            }],
            date: Some("2026-11-22T10:00".into()),
            review: true,
        }),
        sets: vec![SetSummary {
            set_id: SetId::from("set"),
            name: "Msza".into(),
            items: 1,
            date: Some("2026-11-22T10:00".into()),
            review: true,
        }],
        can_undo: false,
        display_power: DisplayPowerState::Unavailable { reason: "not_configured".into() },
        screen_profile: sample_profile(),
        screen_connected: true,
        next_step: NextStep::Gap,
        end_of_item: EndOfItem::Gap,
        songs_to_review: 2,
        church: Some(ChurchView {
            church_id: ChurchId::from("church-0123456789abcdef"),
            name: "Tulce".into(),
        }),
        churches: vec![
            ChurchView { church_id: ChurchId::from("church-fedcba9876543210"), name: "Gowarzewo".into() },
            ChurchView { church_id: ChurchId::from("church-0123456789abcdef"), name: "Tulce".into() },
        ],
    };
    insta::assert_json_snapshot!(state);
}

#[test]
fn messages_round_trip() {
    let message = ServerMsg::Screen { state: sample_screen() };
    let json = serde_json::to_string(&message).unwrap();
    assert_eq!(serde_json::from_str::<ServerMsg>(&json).unwrap(), message);
}

#[test]
fn merge_report() {
    let report = sursum_protocol::MergeReport {
        added: 2,
        updated: 1,
        unchanged: 40,
        kept_local: 1,
        deleted: 1,
        conflicts: vec!["Pieśń zmieniona w obu kościołach".into()],
        sets_added: 1,
        sets_updated: 2,
        sets_to_review: vec!["Niedziela, suma".into()],
        other_church_sets: 3,
        pictures_added: 1,
    };
    insta::assert_json_snapshot!(report);
}

/// Pairing and first setup go over REST; old remotes must still read the session (new fields
/// default to false).
#[test]
fn pairing() {
    use sursum_protocol::{Installation, PairOutcome, SessionInfo};
    let session = SessionInfo {
        pin_required: false,
        paired: false,
        setup_needed: true,
        installation: Installation::Screen,
    };
    let paired = SessionInfo {
        pin_required: true,
        paired: true,
        setup_needed: false,
        installation: Installation::Screen,
    };
    let library = SessionInfo {
        pin_required: false,
        paired: true,
        setup_needed: false,
        installation: Installation::Library,
    };
    let outcomes = [
        PairOutcome::Paired,
        PairOutcome::WrongPin,
        PairOutcome::InvalidPin,
        PairOutcome::AlreadySet,
        PairOutcome::ChurchMissing,
        PairOutcome::Locked { retry_after_s: 30 },
    ];
    insta::assert_json_snapshot!((session, paired, library, outcomes));
    let old: SessionInfo = serde_json::from_str(r#"{"pin_required":true,"paired":false}"#).unwrap();
    assert!(!old.setup_needed && old.installation == Installation::Screen);
}

/// Updates go over REST. The remote that uploads an update reloads into the new build, so these
/// only have to stay readable within one release.
#[test]
fn update() {
    let status = UpdateStatus {
        build: "v0.1.0-34-g980afa5".into(),
        system: Some(SystemStatus {
            version: "v0.1.0-34-g980afa5".into(),
            on_trial: false,
            installing: false,
            target: None,
            last: Some(LastSystemUpdate::RolledBack {
                failed: "v0.1.0-40-g1a2b3c4".into(),
                database_restored: true,
            }),
            can_install: true,
        }),
    };
    let outcomes = [
        InstallOutcome::Installed { version: "v0.2.0".into() },
        InstallOutcome::Refused { reason: InstallRefusal::BadBundle },
    ];
    let committed = LastSystemUpdate::Committed { version: "v0.2.0".into() };
    insta::assert_json_snapshot!((status, committed, outcomes));
    let old: LastSystemUpdate = serde_json::from_str(r#"{"outcome":"rolled_back","failed":"v1"}"#).unwrap();
    assert_eq!(old, LastSystemUpdate::RolledBack { failed: "v1".into(), database_restored: false });
}

#[test]
fn backup() {
    let summary = BackupSummary {
        created_unix: 1_795_000_000,
        build: "0.2.0".into(),
        songs: 254,
        sets: 12,
        pictures: 3,
    };
    let outcomes = [
        RestoreOutcome::Restored { summary },
        RestoreOutcome::Refused { reason: RestoreRefusal::NewerVersion },
        RestoreOutcome::Refused { reason: RestoreRefusal::OlderVersion },
    ];
    insta::assert_json_snapshot!(outcomes);
}

#[test]
fn picture_cleanup() {
    insta::assert_json_snapshot!((
        PictureCleanupRequest { dry_run: true },
        PictureCleanup { pictures: 12, bytes: 36_700_160 }
    ));
}

/// The review of songs changed in two places (ADR 0025) goes over REST.
#[test]
fn song_review() {
    let summary = SongSummary {
        song_id: SongId::from("song-1"),
        title: "Pieśń".into(),
        number: Some("12".into()),
        first_line: "Pierwsza linia".into(),
        review: true,
    };
    let versions = SongVersions {
        song_id: SongId::from("song-1"),
        versions: vec![SongVersionView {
            version: "ab".repeat(32),
            shown: true,
            title: "Pieśń".into(),
            lyrics: "[V1]\nPierwsza linia".into(),
        }],
    };
    insta::assert_json_snapshot!((summary, versions, KeepVersion { version: "cd".repeat(32) }));
    let old: SongSummary =
        serde_json::from_str(r#"{"song_id":"s","title":"t","number":null,"first_line":"f"}"#).unwrap();
    assert!(!old.review);
}

/// A set changed in two places, as the comparison screen gets it.
#[test]
fn set_review() {
    let versions = SetVersions {
        set_id: SetId::from("set"),
        versions: vec![SetVersionView {
            version: "ab".repeat(32),
            shown: true,
            name: "Niedziela".into(),
            date: Some("2026-11-22".into()),
            items: vec![SetVersionItem {
                item_id: ItemId::from("item-1"),
                title: "Pieśń".into(),
                kind: SetItemKind::Song,
                missing_elsewhere: true,
            }],
        }],
    };
    insta::assert_json_snapshot!(versions);
}
