// SPDX-License-Identifier: GPL-3.0-or-later
//! The program: what the congregation sees, and how operator commands change it.
//!
//! The live deck is a **frozen copy** (ADR 0005). Editing the set or a song never changes what
//! is on screen; only navigation does. When the operator navigates inside an item whose song
//! was edited meanwhile, the fresh deck is used and the current slide is found again by its
//! [`SlideKey`].
//!
//! After an item's last slide, "next" can first show a **gap** (an empty slide) before the next
//! item, so the congregation does not see the next song's words before it starts. The gap is
//! part of the content, not an overlay: black and logo stay the operator's own tools.
//!
//! Commands that name what to show (`GoTo`, `GoToItemNumber`, `GoToSlide`) lift black or the
//! logo; relative steps keep them, so a slide can be prepared under black (ADR 0004).

use sursum_protocol::{ClientId, Command, Deck, EndOfItem, ItemId, NextStep, Overlay, Rev};

use crate::model::{ItemKind, ServiceSet, SetItem};

/// The slide on screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    pub item_id: ItemId,
    pub deck: Deck,
    /// The slide shown, or (during the gap) the last slide the gap follows.
    pub slide_index: usize,
    /// True during the gap after the item's last slide: the screen shows an empty slide.
    pub gap: bool,
}

impl Live {
    /// A slide of an item, not in the gap.
    pub fn at(item_id: ItemId, deck: Deck, slide_index: usize) -> Live {
        Live { item_id, deck, slide_index, gap: false }
    }
}

/// What the congregation sees, plus who changed it last.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Program {
    pub live: Option<Live>,
    pub overlay: Overlay,
    /// Revision at which the program last changed (base for the `Stale` rule).
    pub updated_rev: Rev,
    /// Client that made the last change; may keep pressing "next" without being stale.
    pub last_actor: Option<ClientId>,
    /// Where the live item was when someone removed it from the set (the "ghost position"):
    /// the screen keeps showing it, and "next" continues with the item that followed it.
    pub removed_at: Option<usize>,
}

/// Where the live item is in the set, for next/previous item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Anchor {
    At(usize),
    RemovedAt(usize),
}

/// Why a command was not applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    #[error("another operator changed the program since this command was issued")]
    Stale,
    #[error("the requested item or slide does not exist")]
    NotFound,
    #[error("there is no active set")]
    NoActiveSet,
    #[error("this command changes the set and is applied by the caller, not by the program")]
    NotAProgramCommand,
}

/// Inputs a command needs besides the program itself.
pub struct Context<'a> {
    /// The active set, if any.
    pub set: Option<&'a ServiceSet>,
    /// Compiles the deck for a set item (the program does not know about songs or rules).
    pub deck_for: &'a dyn Fn(&SetItem) -> Option<Deck>,
    /// What "next" does after an item's last slide.
    pub end_of_item: EndOfItem,
}

/// Result of a successfully applied command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Changed,
    /// The command was valid but there was nothing to do (e.g. "next" on the last slide).
    Unchanged,
}

impl Program {
    /// The `Stale` rule (ADR 0005): a relative command is accepted if the operator saw the
    /// current program, or if they made the last change themselves (fast repeated presses).
    pub fn accepts(&self, command: &Command, base_rev: Rev, actor: &ClientId) -> bool {
        !command.is_relative() || base_rev >= self.updated_rev || self.last_actor.as_ref() == Some(actor)
    }

    /// Applies a command, checking the `Stale` rule first. On change, records `new_rev` and the
    /// actor.
    pub fn execute(
        &mut self,
        command: &Command,
        base_rev: Rev,
        actor: &ClientId,
        new_rev: Rev,
        context: &Context<'_>,
    ) -> Result<Outcome, Rejection> {
        if !self.accepts(command, base_rev, actor) {
            return Err(Rejection::Stale);
        }
        let before = (self.live.clone(), self.overlay);
        self.apply(command, context)?;
        if (self.live.clone(), self.overlay) == before {
            return Ok(Outcome::Unchanged);
        }
        self.updated_rev = new_rev;
        self.last_actor = Some(actor.clone());
        Ok(Outcome::Changed)
    }

    /// Applies a command without the `Stale` check or bookkeeping.
    pub fn apply(&mut self, command: &Command, context: &Context<'_>) -> Result<(), Rejection> {
        match command {
            Command::Next => self.next(context),
            Command::Prev => self.prev(context),
            Command::NextItem => self.step_item(context, 1),
            Command::PrevItem => self.step_item(context, -1),
            Command::GoTo { item_id, slide_index } => {
                self.go_to(context, item_id, *slide_index as usize)?;
                self.overlay = Overlay::None;
                Ok(())
            }
            Command::GoToItemNumber { number } => {
                self.go_to_number(context, *number)?;
                self.overlay = Overlay::None;
                Ok(())
            }
            Command::GoToSlide { slide_index } => {
                self.go_to_slide(*slide_index as usize)?;
                self.overlay = Overlay::None;
                Ok(())
            }
            Command::SetOverlay { overlay } => {
                self.overlay = *overlay;
                Ok(())
            }
            Command::ToggleBlack => {
                self.overlay = if self.overlay == Overlay::Black { Overlay::None } else { Overlay::Black };
                Ok(())
            }
            // Set changes: the server edits the set first (see the state actor).
            Command::ShowNow { .. }
            | Command::AddSong { .. }
            | Command::AddText { .. }
            | Command::AddImage { .. }
            | Command::AddBlank { .. }
            | Command::RemoveItem { .. }
            | Command::MoveItem { .. }
            | Command::UndoRemove
            | Command::NewSet { .. }
            | Command::ActivateSet { .. }
            | Command::RenameSet { .. }
            | Command::DeleteSet { .. }
            | Command::UndoDeleteSet
            | Command::SetEndOfItem { .. }
            | Command::SetScreenProfile { .. }
            | Command::Calibrate { .. }
            | Command::DisplayPower { .. }
            | Command::SetLogo { .. }
            | Command::DeleteSong { .. }
            | Command::NameChurch { .. }
            | Command::UseChurch { .. } => Err(Rejection::NotAProgramCommand),
        }
    }

    /// What "next" would show, without changing anything (for the remote's preview).
    pub fn peek_next(&self, context: &Context<'_>) -> Option<Live> {
        let mut probe = self.clone();
        probe.next(context).ok()?;
        probe.live.filter(|live| Some(live) != self.live.as_ref())
    }

    /// What kind of step "next" takes from here, so the remote can announce it.
    pub fn next_step(&self, context: &Context<'_>) -> NextStep {
        let Some(live) = &self.live else {
            let has_items = context.set.is_some_and(|set| !set.items.is_empty());
            return if has_items { NextStep::Start } else { NextStep::End };
        };
        match self.peek_next(context) {
            None => NextStep::End,
            Some(next) if next.gap => NextStep::Gap,
            Some(next) if next.item_id != live.item_id => NextStep::Item,
            Some(_) => NextStep::Slide,
        }
    }

    /// Tells the program that an item was removed from the set at `index`. If it was the live
    /// item, the screen keeps it (frozen) and navigation continues from that position.
    pub fn item_removed(&mut self, item_id: &ItemId, index: usize) {
        if self.live.as_ref().is_some_and(|live| &live.item_id == item_id) {
            self.removed_at = Some(index);
        }
    }

    fn next(&mut self, context: &Context<'_>) -> Result<(), Rejection> {
        let Some(live) = self.refreshed_live(context) else {
            return self.show_item_at(context, 0, SlidePick::First);
        };
        if live.gap {
            self.live = Some(live);
            return self.step_item(context, 1);
        }
        if live.slide_index + 1 < live.deck.slides.len() {
            self.live = Some(Live { slide_index: live.slide_index + 1, ..live });
            return Ok(());
        }
        let gap = context.end_of_item == EndOfItem::Gap && !self.is_pause(context, &live.item_id);
        self.live = Some(Live { gap, ..live });
        if gap { Ok(()) } else { self.step_item(context, 1) }
    }

    /// True if the item is a pause: it is an empty slide already, so no gap follows it.
    fn is_pause(&self, context: &Context<'_>, item_id: &ItemId) -> bool {
        context
            .set
            .and_then(|set| set.items.iter().find(|item| &item.id == item_id))
            .is_some_and(|item| matches!(item.kind, ItemKind::Blank { .. }))
    }

    fn prev(&mut self, context: &Context<'_>) -> Result<(), Rejection> {
        let Some(live) = self.refreshed_live(context) else {
            return Ok(());
        };
        if live.gap {
            self.live = Some(Live { gap: false, ..live });
            return Ok(());
        }
        if live.slide_index > 0 {
            self.live = Some(Live { slide_index: live.slide_index - 1, ..live });
            return Ok(());
        }
        let previous = match self.anchor(context) {
            Some(Anchor::At(position) | Anchor::RemovedAt(position)) => position.checked_sub(1),
            None => None,
        };
        self.live = Some(live);
        match previous {
            Some(previous) => self.show_item_at(context, previous, SlidePick::Last),
            None => Ok(()),
        }
    }

    fn step_item(&mut self, context: &Context<'_>, delta: isize) -> Result<(), Rejection> {
        let set = context.set.ok_or(Rejection::NoActiveSet)?;
        let target = match self.anchor(context) {
            Some(Anchor::At(position)) => position.checked_add_signed(delta),
            // The item now at the removed item's place is the one that followed it.
            Some(Anchor::RemovedAt(position)) if delta > 0 => Some(position),
            Some(Anchor::RemovedAt(position)) => position.checked_sub(1),
            None if delta > 0 => Some(0),
            None => None,
        };
        match target {
            Some(index) if index < set.items.len() => self.show_item_at(context, index, SlidePick::First),
            _ => Ok(()),
        }
    }

    fn go_to(
        &mut self,
        context: &Context<'_>,
        item_id: &ItemId,
        slide_index: usize,
    ) -> Result<(), Rejection> {
        let set = context.set.ok_or(Rejection::NoActiveSet)?;
        let position = set.position(item_id).ok_or(Rejection::NotFound)?;
        self.show_item_at(context, position, SlidePick::Index(slide_index))
    }

    fn go_to_number(&mut self, context: &Context<'_>, number: u32) -> Result<(), Rejection> {
        let set = context.set.ok_or(Rejection::NoActiveSet)?;
        let index = (number as usize)
            .checked_sub(1)
            .filter(|index| *index < set.items.len())
            .ok_or(Rejection::NotFound)?;
        self.show_item_at(context, index, SlidePick::First)
    }

    fn go_to_slide(&mut self, slide_index: usize) -> Result<(), Rejection> {
        let live = self.live.as_mut().ok_or(Rejection::NotFound)?;
        if slide_index >= live.deck.slides.len() {
            return Err(Rejection::NotFound);
        }
        live.slide_index = slide_index;
        live.gap = false;
        Ok(())
    }

    /// Shows the item at `index` of the active set.
    fn show_item_at(
        &mut self,
        context: &Context<'_>,
        index: usize,
        pick: SlidePick,
    ) -> Result<(), Rejection> {
        let set = context.set.ok_or(Rejection::NoActiveSet)?;
        let item = set.items.get(index).ok_or(Rejection::NotFound)?;
        let deck = (context.deck_for)(item).ok_or(Rejection::NotFound)?;
        if deck.slides.is_empty() {
            return Err(Rejection::NotFound);
        }
        let last = deck.slides.len() - 1;
        let slide_index = match pick {
            SlidePick::First => 0,
            SlidePick::Last => last,
            SlidePick::Index(index) => index.min(last),
        };
        self.live = Some(Live::at(item.id.clone(), deck, slide_index));
        self.removed_at = None;
        Ok(())
    }

    /// Position of the live item in the active set.
    fn anchor(&self, context: &Context<'_>) -> Option<Anchor> {
        let live = self.live.as_ref()?;
        match context.set?.position(&live.item_id) {
            Some(position) => Some(Anchor::At(position)),
            None => self.removed_at.map(Anchor::RemovedAt),
        }
    }

    /// The live slide, switched to a freshly compiled deck if the item's content changed.
    /// The current slide is found again by key; if it vanished, the index is clamped.
    fn refreshed_live(&self, context: &Context<'_>) -> Option<Live> {
        let live = self.live.clone()?;
        let fresh = context
            .set
            .and_then(|set| set.items.iter().find(|item| item.id == live.item_id))
            .and_then(|item| (context.deck_for)(item))
            .filter(|deck| !deck.slides.is_empty());
        let Some(fresh) = fresh else {
            return Some(live);
        };
        if fresh.deck_id == live.deck.deck_id {
            return Some(live);
        }
        let current_key = &live.deck.slides[live.slide_index].key;
        let slide_index = fresh
            .slides
            .iter()
            .position(|slide| &slide.key == current_key)
            .unwrap_or(live.slide_index.min(fresh.slides.len() - 1));
        Some(Live { item_id: live.item_id, deck: fresh, slide_index, gap: live.gap })
    }
}

/// Which slide to show when entering an item.
#[derive(Debug, Clone, Copy)]
enum SlidePick {
    First,
    Last,
    Index(usize),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ItemKind, Song, Verse};
    use crate::slides::{SlideRules, compile_item, compile_song};
    use sursum_protocol::{SetId, SongId};

    fn song(id: &str) -> Song {
        Song {
            id: SongId::from(id),
            titles: vec![id.to_owned()],
            authors: vec![],
            songbooks: vec![],
            copyright: None,
            ccli: None,
            themes: vec![],
            extra: Default::default(),
            verse_order: vec![],
            verses: vec![
                Verse::simple("v1", &["a"]),
                Verse::simple("c", &["r"]),
                Verse::simple("v2", &["b"]),
            ],
        }
    }

    fn set() -> ServiceSet {
        let item = |id: &str, song_id: &str| SetItem {
            id: ItemId::from(id),
            kind: ItemKind::Song { song_id: SongId::from(song_id), order_override: None },
        };
        ServiceSet {
            id: SetId::from("set"),
            name: "Msza".to_owned(),
            items: vec![
                item("i1", "s1"),
                item("i2", "s2"),
                SetItem {
                    id: ItemId::from("i3"),
                    kind: ItemKind::Text { title: "Ogł.".to_owned(), slides: vec!["x".to_owned()] },
                },
            ],
        }
    }

    fn deck_for(item: &SetItem) -> Option<Deck> {
        match &item.kind {
            ItemKind::Song { song_id, .. } => {
                Some(compile_song(&song(song_id.as_str()), &item.id, None, SlideRules::default()))
            }
            _ => compile_item(item, |_| None, SlideRules::default()),
        }
    }

    // Tests read better with commands passed by value.
    #[allow(clippy::needless_pass_by_value)]
    fn run(program: &mut Program, set: &ServiceSet, command: Command) -> Result<(), Rejection> {
        program.apply(
            &command,
            &Context { set: Some(set), deck_for: &deck_for, end_of_item: EndOfItem::Continue },
        )
    }

    fn position(program: &Program) -> (String, usize) {
        let live = program.live.as_ref().expect("live");
        (live.item_id.0.clone(), live.slide_index)
    }

    #[test]
    fn next_walks_through_slides_and_items() {
        let set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::Next).unwrap();
        assert_eq!(position(&program), ("i1".into(), 0));
        for _ in 0..2 {
            run(&mut program, &set, Command::Next).unwrap();
        }
        assert_eq!(position(&program), ("i1".into(), 2));
        run(&mut program, &set, Command::Next).unwrap();
        assert_eq!(position(&program), ("i2".into(), 0));
    }

    #[test]
    fn next_on_the_last_slide_of_the_set_does_nothing() {
        let set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::GoToItemNumber { number: 3 }).unwrap();
        let before = program.clone();
        run(&mut program, &set, Command::Next).unwrap();
        assert_eq!(program, before);
    }

    #[test]
    fn prev_from_first_slide_goes_to_last_slide_of_previous_item() {
        let set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::GoToItemNumber { number: 2 }).unwrap();
        run(&mut program, &set, Command::Prev).unwrap();
        assert_eq!(position(&program), ("i1".into(), 2));
    }

    #[test]
    fn item_steps_land_on_first_slides() {
        let set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::NextItem).unwrap();
        run(&mut program, &set, Command::GoToSlide { slide_index: 2 }).unwrap();
        run(&mut program, &set, Command::NextItem).unwrap();
        assert_eq!(position(&program), ("i2".into(), 0));
        run(&mut program, &set, Command::PrevItem).unwrap();
        assert_eq!(position(&program), ("i1".into(), 0));
    }

    #[test]
    fn invalid_targets_are_rejected() {
        let set = set();
        let mut program = Program::default();
        assert_eq!(run(&mut program, &set, Command::GoToItemNumber { number: 0 }), Err(Rejection::NotFound));
        assert_eq!(run(&mut program, &set, Command::GoToItemNumber { number: 9 }), Err(Rejection::NotFound));
        assert_eq!(run(&mut program, &set, Command::GoToSlide { slide_index: 0 }), Err(Rejection::NotFound));
        let command = Command::GoTo { item_id: ItemId::from("nope"), slide_index: 0 };
        assert_eq!(run(&mut program, &set, command), Err(Rejection::NotFound));
    }

    #[test]
    fn no_set_means_no_navigation() {
        let mut program = Program::default();
        let context = Context { set: None, deck_for: &deck_for, end_of_item: EndOfItem::Continue };
        assert_eq!(program.apply(&Command::NextItem, &context), Err(Rejection::NoActiveSet));
    }

    #[test]
    fn toggle_black_switches_between_black_and_none() {
        let set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::ToggleBlack).unwrap();
        assert_eq!(program.overlay, Overlay::Black);
        run(&mut program, &set, Command::ToggleBlack).unwrap();
        assert_eq!(program.overlay, Overlay::None);
    }

    fn with_gap(program: &mut Program, set: &ServiceSet, command: &Command) {
        program
            .apply(command, &Context { set: Some(set), deck_for: &deck_for, end_of_item: EndOfItem::Gap })
            .unwrap();
    }

    fn gap_context(set: &ServiceSet) -> Context<'_> {
        Context { set: Some(set), deck_for: &deck_for, end_of_item: EndOfItem::Gap }
    }

    #[test]
    fn a_gap_follows_the_last_slide_before_the_next_item() {
        let set = set();
        let mut program = Program::default();
        with_gap(&mut program, &set, &Command::GoTo { item_id: ItemId::from("i1"), slide_index: 2 });
        assert_eq!(program.next_step(&gap_context(&set)), NextStep::Gap);
        with_gap(&mut program, &set, &Command::Next);
        let live = program.live.as_ref().unwrap();
        assert!(live.gap && live.item_id.as_str() == "i1" && live.slide_index == 2, "{live:?}");
        assert_eq!(program.next_step(&gap_context(&set)), NextStep::Item);
        with_gap(&mut program, &set, &Command::Prev);
        assert_eq!(position(&program), ("i1".into(), 2), "back to the last slide");
        assert!(!program.live.as_ref().unwrap().gap);
        with_gap(&mut program, &set, &Command::Next);
        with_gap(&mut program, &set, &Command::Next);
        assert_eq!(position(&program), ("i2".into(), 0));
        assert!(!program.live.as_ref().unwrap().gap);
    }

    #[test]
    fn the_last_item_of_the_set_ends_with_a_gap_and_then_nothing() {
        let set = set();
        let mut program = Program::default();
        with_gap(&mut program, &set, &Command::GoToItemNumber { number: 3 });
        with_gap(&mut program, &set, &Command::Next);
        assert!(program.live.as_ref().unwrap().gap);
        assert_eq!(program.next_step(&gap_context(&set)), NextStep::End);
        let before = program.clone();
        with_gap(&mut program, &set, &Command::Next);
        assert_eq!(program, before);
    }

    #[test]
    fn no_gap_after_a_pause_item_and_item_steps_skip_the_gap() {
        let mut set = set();
        set.items
            .insert(1, SetItem { id: ItemId::from("p"), kind: ItemKind::Blank { title: "Przerwa".into() } });
        let mut program = Program::default();
        with_gap(&mut program, &set, &Command::GoToItemNumber { number: 2 });
        assert_eq!(program.next_step(&gap_context(&set)), NextStep::Item, "a pause is empty already");
        with_gap(&mut program, &set, &Command::Next);
        assert_eq!(position(&program), ("i2".into(), 0));
        with_gap(&mut program, &set, &Command::GoTo { item_id: ItemId::from("i1"), slide_index: 2 });
        with_gap(&mut program, &set, &Command::NextItem);
        assert_eq!(position(&program), ("p".into(), 0), "the next-item button never stops in a gap");
    }

    #[test]
    fn next_step_names_what_comes() {
        let set = set();
        let mut program = Program::default();
        assert_eq!(program.next_step(&gap_context(&set)), NextStep::Start);
        with_gap(&mut program, &set, &Command::Next);
        assert_eq!(program.next_step(&gap_context(&set)), NextStep::Slide);
        let empty = ServiceSet { items: vec![], ..set.clone() };
        assert_eq!(Program::default().next_step(&gap_context(&empty)), NextStep::End);
    }

    #[test]
    fn choosing_what_to_show_lifts_black_and_the_logo_but_steps_do_not() {
        let set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::Next).unwrap();
        for jump in [
            Command::GoToItemNumber { number: 2 },
            Command::GoTo { item_id: ItemId::from("i1"), slide_index: 1 },
            Command::GoToSlide { slide_index: 0 },
        ] {
            run(&mut program, &set, Command::SetOverlay { overlay: Overlay::Logo }).unwrap();
            run(&mut program, &set, jump.clone()).unwrap();
            assert_eq!(program.overlay, Overlay::None, "{jump:?}");
        }
        for step in [Command::Next, Command::Prev, Command::NextItem, Command::PrevItem] {
            run(&mut program, &set, Command::SetOverlay { overlay: Overlay::Black }).unwrap();
            run(&mut program, &set, step.clone()).unwrap();
            assert_eq!(program.overlay, Overlay::Black, "{step:?}");
        }
    }

    #[test]
    fn navigation_under_black_keeps_the_overlay() {
        let set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::SetOverlay { overlay: Overlay::Black }).unwrap();
        run(&mut program, &set, Command::Next).unwrap();
        assert_eq!(program.overlay, Overlay::Black);
        assert!(program.live.is_some());
    }

    #[test]
    fn stale_rule_blocks_other_operators_but_not_the_last_actor() {
        let set = set();
        let context = Context { set: Some(&set), deck_for: &deck_for, end_of_item: EndOfItem::Continue };
        let (alice, bob) = (ClientId::from("alice"), ClientId::from("bob"));
        let mut program = Program::default();
        let first = program.execute(&Command::Next, Rev(0), &alice, Rev(1), &context);
        assert_eq!(first, Ok(Outcome::Changed));
        // Bob saw revision 0 and pressed "next" at the same moment: stale.
        assert_eq!(program.execute(&Command::Next, Rev(0), &bob, Rev(2), &context), Err(Rejection::Stale));
        // Alice presses again quickly, still based on revision 0: accepted.
        assert_eq!(program.execute(&Command::Next, Rev(0), &alice, Rev(2), &context), Ok(Outcome::Changed));
        // Absolute commands are never stale.
        let black = Command::SetOverlay { overlay: Overlay::Black };
        assert_eq!(program.execute(&black, Rev(0), &bob, Rev(3), &context), Ok(Outcome::Changed));
    }

    #[test]
    fn edits_never_change_the_frozen_deck_until_navigation() {
        let mut set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::GoTo { item_id: ItemId::from("i1"), slide_index: 1 }).unwrap();
        let frozen = program.clone();
        // Removing and reordering items does not touch the program.
        set.items.swap(0, 1);
        set.items.pop();
        assert_eq!(program, frozen);
        run(&mut program, &set, Command::Next).unwrap();
        assert_eq!(position(&program), ("i1".into(), 2));
    }

    #[test]
    fn edited_song_is_picked_up_on_next_navigation_by_slide_key() {
        let set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::GoTo { item_id: ItemId::from("i1"), slide_index: 1 }).unwrap();
        // The song gets a new first verse; the chorus moves from index 1 to index 2.
        let edited = |item: &SetItem| {
            let mut song = song("s1");
            song.verses.insert(0, Verse::simple("v0", &["new"]));
            Some(compile_song(&song, &item.id, None, SlideRules::default()))
        };
        program
            .apply(
                &Command::Next,
                &Context { set: Some(&set), deck_for: &edited, end_of_item: EndOfItem::Continue },
            )
            .unwrap();
        let live = program.live.as_ref().unwrap();
        assert_eq!(live.deck.slides[live.slide_index].key.as_str(), "v2#0@0");
    }

    #[test]
    fn removing_the_live_item_keeps_it_and_next_continues_with_the_following_one() {
        let mut set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::GoToItemNumber { number: 2 }).unwrap();
        let index = set.position(&ItemId::from("i2")).unwrap();
        set.items.remove(index);
        program.item_removed(&ItemId::from("i2"), index);
        assert_eq!(position(&program), ("i2".into(), 0), "the screen keeps the removed item");
        run(&mut program, &set, Command::NextItem).unwrap();
        assert_eq!(position(&program), ("i3".into(), 0));
    }

    #[test]
    fn previous_from_a_removed_item_goes_to_the_one_before_it() {
        let mut set = set();
        let mut program = Program::default();
        run(&mut program, &set, Command::GoToItemNumber { number: 2 }).unwrap();
        set.items.remove(1);
        program.item_removed(&ItemId::from("i2"), 1);
        run(&mut program, &set, Command::Prev).unwrap();
        assert_eq!(position(&program), ("i1".into(), 2));
    }

    #[test]
    fn peek_next_does_not_mutate() {
        let set = set();
        let context = Context { set: Some(&set), deck_for: &deck_for, end_of_item: EndOfItem::Continue };
        let mut program = Program::default();
        run(&mut program, &set, Command::GoToItemNumber { number: 1 }).unwrap();
        let before = program.clone();
        let next = program.peek_next(&context).unwrap();
        assert_eq!(next.slide_index, 1);
        assert_eq!(program, before);
    }
}
