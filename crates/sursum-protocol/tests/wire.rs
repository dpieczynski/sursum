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
        live: Some(LiveSlide { deck: sample_deck(), slide_index: 0 }),
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
            name: "słup".into(),
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
        next: None,
        sections: vec![SectionJump { label: "v1".into(), slide_index: 0 }],
        active_set: Some(SetView {
            set_id: SetId::from("set"),
            name: "Msza".into(),
            items: vec![SetItemView { item_id: ItemId::from("item-1"), number: 1, title: "Pieśń".into() }],
        }),
        sets: vec![SetSummary { set_id: SetId::from("set"), name: "Msza".into(), items: 1 }],
        can_undo: false,
        display_power: DisplayPowerState::Unavailable { reason: "not_configured".into() },
        screen_profile: sample_profile(),
        screen_connected: true,
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
