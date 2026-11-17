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
        background: Background::default(),
    }
}

fn sample_screen() -> ScreenState {
    ScreenState {
        rev: Rev(7),
        live: Some(LiveSlide { deck: sample_deck(), slide_index: 0, gap: false }),
        upcoming: None,
        overlay: Overlay::Black,
        theme: Theme::default(),
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
        ClientMsg::Ping,
        ClientMsg::Log { message: "font missing".into() },
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
        sections: vec![SectionJump { label: "v1".into(), slide_index: 0 }],
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
        }),
        sets: vec![SetSummary { set_id: SetId::from("set"), name: "Msza".into(), items: 1 }],
        can_undo: false,
        display_power: DisplayPowerState::Unavailable { reason: "not_configured".into() },
        screen_profile: sample_profile(),
        screen_connected: true,
        next_step: NextStep::Gap,
        end_of_item: EndOfItem::Gap,
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
    };
    insta::assert_json_snapshot!(report);
}

/// Pairing and first setup go over REST; old remotes must still read the session (new fields
/// default to false).
#[test]
fn pairing() {
    use sursum_protocol::{PairOutcome, SessionInfo};
    let session = SessionInfo {
        pin_required: false,
        paired: false,
        admin: false,
        setup_needed: true,
        admin_remaining_s: None,
    };
    let unlocked = SessionInfo {
        pin_required: true,
        paired: true,
        admin: true,
        setup_needed: false,
        admin_remaining_s: Some(840),
    };
    let outcomes = [
        PairOutcome::Paired,
        PairOutcome::Unlocked,
        PairOutcome::WrongPin,
        PairOutcome::InvalidPin,
        PairOutcome::AlreadySet,
        PairOutcome::Locked { retry_after_s: 30 },
    ];
    insta::assert_json_snapshot!((session, unlocked, outcomes));
    let old: SessionInfo = serde_json::from_str(r#"{"pin_required":true,"paired":false}"#).unwrap();
    assert!(!old.admin && !old.setup_needed);
}

/// Updates go over REST. The remote that uploads a bundle reloads into the new build, so these
/// only have to stay readable within one release.
#[test]
fn update() {
    let status = UpdateStatus {
        build: "0.2.0".into(),
        release: Some("0.2.0".into()),
        on_trial: true,
        last: Some(LastUpdate::RolledBack {
            failed: "0.3.0".into(),
            restored: "0.2.0".into(),
            database_restored: true,
        }),
        can_install: true,
        system: Some(SystemStatus {
            version: "v0.1.0-34-g980afa5".into(),
            on_trial: false,
            installing: false,
            target: None,
            last: Some(LastSystemUpdate::RolledBack { failed: "v0.1.0-40-g1a2b3c4".into() }),
            can_install: true,
        }),
    };
    let outcomes = [
        InstallOutcome::Installed { version: "0.3.0".into() },
        InstallOutcome::Refused { reason: InstallRefusal::BadBundle },
    ];
    insta::assert_json_snapshot!((status, LastUpdate::Committed { version: "0.2.0".into() }, outcomes));
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
