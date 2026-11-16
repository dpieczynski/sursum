# 0017 — The remote: three views around a fixed live area

Status: Accepted (2026-11-17)

## Context
The first remote put everything on one screen: running the service, searching, editing the
set, importing, calibrating, pairing devices. On a phone the set list did not fit at all; on a
landscape tablet three to six items did. Notices and optional rows moved buttons under the
thumb, taps without a connection were lost silently, black and the logo were easy to miss,
"next" left a song without warning, and the song editor required OpenSong markup. Organists,
many not technical, have one hand free for a second during Mass.

## Decision
- Three views as tabs in the top bar (deliberately outside the thumb zone): **Msza** (running
  the service: what is on screen, the live item's slides by name and first line, the running
  order where a tap shows, the controls), **Zestaw** (preparing: sets, items, search, song card,
  editor; nothing here goes on screen by itself) and **Ustawienia** (screen, display, this device,
  song library, devices, about). The view is kept in the URL fragment.
- **Fixed regions**: only the set list (and the slide tiles) scroll; toasts are laid over the
  page; one problem banner line sits under the top bar. The controls never move.
- **Black is always one tap away**: on wide screens the stage and the controls stay beside every
  view; on narrow screens other views and the editor show a live strip (summary, cover, back,
  next). The cover button turns into "Pokaż tekst" while black or the logo is up.
- "Dalej" says what it will do ("→ 3 z 7", "→ przerwa", "→ 2. Tytuł", "Koniec zestawu").
- Without a connection the controls are disabled and commands are not queued.
- Controls can be mirrored for the left hand (per device, in the browser like the pedal keys).
- Admin items are always visible with a lock and ask for the PIN in place.
- Server changes that serve this: the gap after an item, showing lifts black, `next_step`, set
  rename/delete, song ids in set items (ADR 0004 amendment), and plain text in the editor.
- The "#" item-number keypad is removed: a visible set list does it in one tap.

## Consequences
More components, but each is small, and the e2e tests check the layout itself (rows visible at
360×800, 800×1280, 1280×800; controls inside the window and at least 56 px; positions unchanged
by state changes). `just remote-sizes` captures every tab at the three sizes for review.
