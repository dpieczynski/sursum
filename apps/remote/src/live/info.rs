// SPDX-License-Identifier: GPL-3.0-or-later
//! What the remote says about the program, derived from the snapshot only (no decisions: the
//! server already decided what is live and what "next" does; this just puts it into words).

use sursum_protocol::{NextStep, Overlay, RemoteState, Slide, SlideBody};

use crate::messages::{Msg, pl, section_title};

/// What the congregation sees right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shows {
    Text,
    Picture,
    Gap,
    Black,
    Logo,
    Nothing,
}

impl Shows {
    /// The status chip text.
    pub fn label(self) -> &'static str {
        pl(match self {
            Shows::Text => Msg::ShowsText,
            Shows::Picture => Msg::ShowsPicture,
            Shows::Gap => Msg::ShowsGap,
            Shows::Black => Msg::ShowsBlack,
            Shows::Logo => Msg::ShowsLogo,
            Shows::Nothing => Msg::ShowsNothing,
        })
    }

    /// A one-word version for the narrow status chip.
    pub fn short_label(self) -> &'static str {
        pl(match self {
            Shows::Text => Msg::ShortText,
            Shows::Picture => Msg::ShortPicture,
            Shows::Gap => Msg::ShortGap,
            Shows::Black => Msg::ShortBlack,
            Shows::Logo => Msg::ShortLogo,
            Shows::Nothing => Msg::ShortNothing,
        })
    }

    /// True if the congregation sees words or a picture.
    pub fn is_content(self) -> bool {
        matches!(self, Shows::Text | Shows::Picture)
    }
}

/// What the congregation sees: an overlay wins over the live slide. The logo cover without an
/// uploaded logo is plain black on the screen, so it is called that.
pub fn screen_shows(state: &RemoteState) -> Shows {
    match state.program.overlay {
        Overlay::Black => return Shows::Black,
        Overlay::Logo if state.program.logo.is_none() => return Shows::Black,
        Overlay::Logo => return Shows::Logo,
        Overlay::None => {}
    }
    let Some(live) = &state.program.live else { return Shows::Nothing };
    if live.gap {
        return Shows::Gap;
    }
    match live.deck.slides.get(live.slide_index as usize).map(|slide| &slide.body) {
        Some(SlideBody::Image { .. }) => Shows::Picture,
        Some(SlideBody::Text { lines, .. }) if lines.is_empty() => Shows::Gap,
        _ => Shows::Text,
    }
}

/// The 1-based number of the live item in the active set, if it is there.
pub fn live_number(state: &RemoteState) -> Option<u32> {
    let item_id = &state.program.live.as_ref()?.deck.item_id;
    state.active_set.as_ref()?.items.iter().find(|item| &item.item_id == item_id).map(|item| item.number)
}

/// "3. Tytuł" for the live item (without the number if it is not in the set any more).
pub fn live_heading(state: &RemoteState) -> Option<String> {
    let live = state.program.live.as_ref()?;
    Some(match live_number(state) {
        Some(number) => format!("{number}. {}", live.deck.title),
        None => live.deck.title.clone(),
    })
}

/// "Zwrotka 2 · slajd 3 z 7", "Slajd 1 z 2" for pages without a section name (announcements,
/// whose tiles are just numbered), or the gap's description.
pub fn live_position(state: &RemoteState) -> Option<String> {
    let live = state.program.live.as_ref()?;
    if live.gap {
        return Some(pl(Msg::GapAfterItem).to_owned());
    }
    let slide = live.deck.slides.get(live.slide_index as usize)?;
    let count = live.deck.slides.len();
    let counter = format!("{} {} z {count}", pl(Msg::SlideOf), live.slide_index + 1);
    let numbered_page = slide.key.as_str().starts_with("t#");
    Some(match slide_name(slide).filter(|_| !numbered_page) {
        Some(name) => format!("{name} · {counter}"),
        None => capitalized(&counter),
    })
}

/// "slajd 1 z 2" → "Slajd 1 z 2".
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| first.to_uppercase().chain(chars).collect())
}

/// The tile label of a slide: "Zwrotka 1", "Refren (2)" for the second slide of a section,
/// "Obraz" for a picture; `None` for slides without a section (announcements).
pub fn slide_name(slide: &Slide) -> Option<String> {
    if matches!(slide.body, SlideBody::Image { .. }) {
        return Some(pl(Msg::Picture).to_owned());
    }
    if slide.label.is_empty() || slide.label == "–" {
        return None;
    }
    // Announcement pages: keys `t#0`, `t#1`, labels "1", "2".
    if slide.key.as_str().starts_with("t#") {
        return Some(format!("{} {}", pl(Msg::SlideN), slide.label));
    }
    let name = section_title(&slide.label);
    // Keys look like `c#1@0`: section, part within the section, occurrence in the order.
    let part = slide.key.as_str().split('#').nth(1).and_then(|rest| rest.split('@').next());
    match part.and_then(|part| part.parse::<u32>().ok()) {
        Some(part) if part > 0 => Some(format!("{name} ({})", part + 1)),
        _ => Some(name),
    }
}

/// The first line of a slide (for tiles), or its title.
pub fn slide_first_line(slide: &Slide) -> String {
    match &slide.body {
        SlideBody::Lyrics { lines } => lines.first().cloned().unwrap_or_default(),
        SlideBody::Text { title, lines } => {
            lines.first().cloned().or_else(|| title.clone()).unwrap_or_default()
        }
        SlideBody::Image { .. } => String::new(),
    }
}

/// How the caption under "Dalej" looks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Normal,
    /// The next press leaves this song.
    Warn,
}

/// The caption under "Dalej" and whether the button can do anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NextCaption {
    pub text: String,
    pub tone: Tone,
    pub enabled: bool,
}

/// What pressing "Dalej" will do, in words.
pub fn next_caption(state: &RemoteState) -> NextCaption {
    let caption = |text: String, tone| NextCaption { text, tone, enabled: true };
    let mut result = match state.next_step {
        NextStep::End => {
            return NextCaption { text: pl(Msg::EndOfSet).to_owned(), tone: Tone::Normal, enabled: false };
        }
        NextStep::Start => caption(pl(Msg::StartSet).to_owned(), Tone::Normal),
        NextStep::Gap => caption(pl(Msg::NextGap).to_owned(), Tone::Normal),
        NextStep::Slide => {
            let text = state
                .program
                .live
                .as_ref()
                .map(|live| format!("→ {} z {}", live.slide_index + 2, live.deck.slides.len()))
                .unwrap_or_default();
            caption(text, Tone::Normal)
        }
        NextStep::Item => caption(format!("→ {}", next_item_heading(state).unwrap_or_default()), Tone::Warn),
    };
    if state.program.overlay != Overlay::None {
        result.text = format!("{} · {}", result.text, pl(Msg::UnderCover));
    }
    result
}

/// "4. Tytuł" of the item "next" goes to.
fn next_item_heading(state: &RemoteState) -> Option<String> {
    let deck = &state.next.as_ref()?.deck;
    let number = state
        .active_set
        .as_ref()
        .and_then(|set| set.items.iter().find(|item| item.item_id == deck.item_id))
        .map(|item| item.number);
    Some(match number {
        Some(number) => format!("{number}. {}", deck.title),
        None => deck.title.clone(),
    })
}

/// Whether "Wstecz" can do anything: not before the first slide of the first item.
pub fn can_go_back(state: &RemoteState) -> bool {
    let Some(live) = &state.program.live else { return false };
    live.gap || live.slide_index > 0 || live_number(state).is_none_or(|number| number > 1)
}

/// Whether the previous and the next item exist.
pub fn item_steps(state: &RemoteState) -> (bool, bool) {
    let count = state.active_set.as_ref().map_or(0, |set| set.items.len() as u32);
    match live_number(state) {
        Some(number) => (number > 1, number < count),
        // Nothing live (or the live item was removed): "next item" starts the set.
        None => (false, count > 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sursum_protocol::*;

    fn slide(key: &str, label: &str, line: &str) -> Slide {
        Slide {
            key: SlideKey::from(key),
            label: label.into(),
            body: SlideBody::Lyrics { lines: vec![line.into()] },
        }
    }

    fn deck(item: &str, title: &str) -> Deck {
        Deck {
            deck_id: DeckId::from(item),
            fit_group: item.into(),
            item_id: ItemId::from(item),
            title: title.into(),
            slides: vec![slide("v1#0@0", "v1", "a"), slide("c#0@0", "c", "b"), slide("c#1@0", "c", "c")],
            background: Background::default(),
        }
    }

    fn state(live: Option<(&str, u32, bool)>, next_step: NextStep) -> RemoteState {
        let program = ScreenState {
            rev: Rev(1),
            live: live.map(|(item, slide_index, gap)| LiveSlide {
                deck: deck(item, "Barka"),
                slide_index,
                gap,
            }),
            upcoming: None,
            overlay: Overlay::None,
            transition_ms: 0,
            layout: ScreenLayout { content: Rect::SCENE, masks: vec![] },
            calibration: None,
            logo: None,
        };
        let item = |number: u32, id: &str| SetItemView {
            item_id: ItemId::from(id),
            number,
            title: id.into(),
            kind: SetItemKind::Song,
            song_id: None,
            song_number: None,
        };
        RemoteState {
            rev: Rev(1),
            program,
            next: Some(LiveSlide { deck: deck("i2", "Pan kiedyś stanął"), slide_index: 0, gap: false }),
            active_set: Some(SetView {
                set_id: SetId::from("s"),
                name: "Msza".into(),
                items: vec![item(1, "i1"), item(2, "i2")],
                date: None,
                review: false,
            }),
            sets: vec![],
            can_undo: false,
            display_power: DisplayPowerState::Unknown,
            screen_profile: ScreenProfile::default(),
            screen_connected: true,
            next_step,
            end_of_item: EndOfItem::Gap,
            songs_to_review: 0,
            church: None,
            churches: vec![],
        }
    }

    #[test]
    fn the_caption_says_what_next_does() {
        let on_slide = state(Some(("i1", 0, false)), NextStep::Slide);
        assert_eq!(next_caption(&on_slide).text, "→ 2 z 3");
        let leaving = state(Some(("i1", 2, true)), NextStep::Item);
        let caption = next_caption(&leaving);
        assert_eq!((caption.text.as_str(), caption.tone), ("→ 2. Pan kiedyś stanął", Tone::Warn));
        let end = next_caption(&state(Some(("i2", 2, true)), NextStep::End));
        assert!(!end.enabled && end.text == "Koniec zestawu");
        let mut covered = state(Some(("i1", 0, false)), NextStep::Slide);
        covered.program.overlay = Overlay::Black;
        assert_eq!(next_caption(&covered).text, "→ 2 z 3 · pod zasłoną");
    }

    #[test]
    fn what_the_congregation_sees() {
        assert_eq!(screen_shows(&state(None, NextStep::Start)), Shows::Nothing);
        assert_eq!(screen_shows(&state(Some(("i1", 0, false)), NextStep::Slide)), Shows::Text);
        assert_eq!(screen_shows(&state(Some(("i1", 2, true)), NextStep::Item)), Shows::Gap);
        let mut logo = state(Some(("i1", 0, false)), NextStep::Slide);
        logo.program.overlay = Overlay::Logo;
        assert_eq!(screen_shows(&logo), Shows::Black, "no logo uploaded: the screen is black");
        logo.program.logo = Some(AssetId::from("logo"));
        assert_eq!(screen_shows(&logo), Shows::Logo);
    }

    #[test]
    fn announcement_pages_are_counted_not_named_twice() {
        let mut page = state(Some(("i1", 0, false)), NextStep::Item);
        if let Some(live) = page.program.live.as_mut() {
            live.deck.slides = vec![slide("t#0", "1", "Zapraszamy")];
        }
        assert_eq!(live_position(&page).as_deref(), Some("Slajd 1 z 1"));
    }

    #[test]
    fn headings_and_positions() {
        let on_refrain = state(Some(("i1", 2, false)), NextStep::Item);
        assert_eq!(live_heading(&on_refrain).as_deref(), Some("1. Barka"));
        assert_eq!(live_position(&on_refrain).as_deref(), Some("Refren (2) · slajd 3 z 3"));
        assert_eq!(slide_name(&slide("v2#0@1", "v2", "x")).as_deref(), Some("Zwrotka 2"));
        assert_eq!(slide_name(&slide("t#1", "2", "x")).as_deref(), Some("Slajd 2"));
        assert_eq!(slide_name(&slide("blank#0", "–", "")), None);
    }

    #[test]
    fn steps_stop_at_the_ends_of_the_set() {
        let first = state(Some(("i1", 0, false)), NextStep::Slide);
        assert!(!can_go_back(&first));
        assert_eq!(item_steps(&first), (false, true));
        let second = state(Some(("i2", 0, false)), NextStep::Slide);
        assert!(can_go_back(&second));
        assert_eq!(item_steps(&second), (true, false));
        assert_eq!(item_steps(&state(None, NextStep::Start)), (false, true));
    }
}
