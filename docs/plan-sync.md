# Implementation plan: synchronizing songs and sets (Y1)

Written 2026-11-19, revised the same day. The decisions are recorded in
[ADR 0025](decisions/0025-versioned-songs-and-sets.md); this file is the implementation plan.
Where the two differ, the ADR wins. ⚠ marks something not yet verified. Progress is tracked in
the roadmap's section Y1.

## 1. Why the present merge has to go

`.sursum` files carry songs only, and a song changed in two places becomes a second song
("(kopia ze scalenia)"). A throwaway test on 2026-11-19 (two databases, one song corrected in
both, files carried both ways) showed what that does to two churches that sync with each other:

- each church ends with three songs: the original and two copies;
- the original keeps a different text in each church for good; no later sync changes that;
- every further correction of that song adds another copy on the other side.

Two more defects found while reading the code:

- **A text typed back is taken for the old one.** History is a list of content hashes, so a
  song changed from X to Y and back to X has the hash of its first version again; a device
  still on Y then keeps Y, although X is the newer decision (`library.rs`, `decide`).
- **Importing from OpenSong again overwrites corrections made in sursum.** The import stores
  whatever differs from the current text as a new revision (`import.rs` → `upsert_song`), so an
  old OpenSong file brings the old text back.

The root cause is the same in all three: identity and history are not kept apart. Merging must
add **versions** to an object that keeps its identity, never add objects.

## 2. Decisions (maintainer, 2026-11-19)

1. **More changes win.** When a song or a set was changed in two places, the version with more
   changes in its history becomes the current one, on every device alike.
2. **Sets belong to a church.** Songs are shared by the parish; a set is made for one church.
3. **One person manages a church's sets** (the organist), so a set is never edited by two
   people at once. Sets therefore use the same whole-version rule as songs; merging a set item
   by item is not built. (One person on two computers, or the organist at home and "Pokaż
   teraz" in the church, can still change a set in both places; then decision 1 applies.)
4. **Every device stores everything and shows its own.** A church device keeps the other
   church's sets without showing them, so any file is complete and can be carried onward.
5. **A set has a date** chosen by the organist; the list of sets is ordered by it.
6. **The word stays "Zestaw"** in the remote, the guide and the code (`ServiceSet`); the tab
   for running the service stays "Msza".
7. **"Połącz pieśni"** (declaring two songs to be one) is a second step (section 12).
8. **No migration of existing databases.** Nobody uses the program yet, so the new schema starts
   from scratch and databases and files of earlier builds are not carried over (section 9.1).
9. **"Nowy zestaw z tego"** (a new set copied from an existing one) is part of this work
   (section 5.1).
10. **A deletion never beats a change made elsewhere** without knowing of it: the song or set
    stays with the change, even if the deleting side had more changes (rule 3).
11. **A set that lost is reviewed like a song**: it stays in the set as another version, the set
    is marked "do przejrzenia" and has a comparison screen (section 3.5).
12. **A second OpenSong import does not undo corrections made in sursum** (section 7).
13. **Every file carries all pictures of all sets**; a limit by set date comes later if the files
    grow too large.
14. **The church is named together with the PIN** on a device's first start (section 4).

## 3. The mechanism

One mechanism for songs, sets and churches. It uses no clock (a Pi without internet may have the
wrong time) and no device identity (a restored backup or a copied data folder must not confuse
it).

### 3.1 Terms

- A **document** is a song, a set or a church. It has an id that never changes.
- A **version** is one saved state of a document: its **body** (the document without its id, as
  bytes, or "deleted") and its **ancestors**, the ids of every version it replaces, directly or
  not. The body leaves the id out so two songs with the same text have the same body (the
  later "Połącz pieśni" relies on that).
- The **version id** is SHA-256 over the body's bytes and the sorted ancestor ids. The document
  id is not part of it. So the same text saved on top of the same history has the same id on
  every device, and a text typed back is a new version (its ancestors differ).
- The **number of changes** of a version is the number of its ancestors.
- The **heads** of a document are the versions no other known version lists as an ancestor.
  Usually there is one.

### 3.2 Rules

| # | Rule |
|---|---|
| 1 | Saving creates a version whose ancestors are the version it was made from and every deleted head, plus all of theirs. A review replaces all the versions it showed. |
| 2 | Merging two devices' knowledge of a document = all versions either knows, minus those that are an ancestor of another. What is left are the heads. |
| 3 | The **current** version is chosen among the heads: a live one before a deleted one, then more changes, then the greater version id. |
| 4 | The other heads stay in the document as "other versions". Nothing is thrown away. |
| 5 | Deleting is saving a "deleted" version that replaces every head the device knows (one deletes the song, not one of its texts). Bringing back is saving a live version on top of it. |
| 6 | A file carries, for every document, all its heads with their bodies and ancestor ids. |

What follows from them:

- **Same result everywhere.** Rule 2 is a union followed by "keep the newest"; it gives the
  same heads whoever merges, in whatever order, however often. Rule 3 depends only on the
  heads. Two devices that have seen the same files show the same thing.
- **No duplicates.** A merge adds versions to a document that exists, or documents the device
  did not have. It never makes a second document for something it already has.
- **An old file changes nothing.** Everything in it is an ancestor of what the device has.
- **A loser never comes back.** A version that replaces the current one has more changes than
  it, and a deletion replaces all heads (rule 5), so the other versions matter only to people,
  not to the outcome.
- **A deletion that lost goes away on the next save** (rule 1): it needs no review, and it does
  not stay in the document for good.
- **Rule 3, "a live one before a deleted one"**, is the one exception to "more changes win": a
  deletion never beats a text someone changed in the meantime without knowing of the deletion.
  It is today's behavior. Without it a corrected song could vanish with nothing on screen to
  say so (decision 10). Whoever still wants it gone deletes it again; that deletion knows the
  change and wins everywhere.

### 3.3 Examples

| Situation | Result |
|---|---|
| Home corrects a song; the church did not touch it | the church gets the correction (one head) |
| The church still has the old text and its file reaches home | nothing changes at home |
| Corrected at home (2 changes) and in the church (1 change) | one song; the home text is current everywhere; the church text is its "other version"; the song is on the "Do przejrzenia" list |
| Same, and somebody then picks or edits a text in the review | a version replacing both; it spreads like any correction and the mark goes away everywhere |
| Both places typed the same correction (same build of the program) | the same version id: nothing to merge; with different builds, two versions with equal text, which is not put on the review list |
| Deleted in the church, corrected at home | the song stays, with the correction |
| Deleted in both | deleted |
| Set changed at home after its file left, and in the church during Mass ("Pokaż teraz") | the set with more changes is current everywhere; the other version stays in it and the set is "do przejrzenia" |
| Both churches import the same OpenSong folder | the same songs and versions: nothing to merge |
| The churches' OpenSong folders differ in a song | one song with two versions, on the review list (today: a copy per song) |

### 3.4 Review (songs and sets)

A song with two or more live heads whose bodies differ is "do przejrzenia": a mark in search
results and the song card, and a list in the Zestaw (and Pieśni) view. Opening it shows the
versions side by side: **Zostaw tę**, **Użyj tamtej**, or edit. Saving from there replaces all
the versions shown (rule 1). Reviewing is optional: until then every device shows the same
current text.

Saving from the ordinary editor replaces only the version the editor loaded. If another head
arrived meanwhile it stays, and the song is still on the list.

Sets work the same way (decision 11): a set with two or more live heads whose bodies differ is
"do przejrzenia". The mark shows in the list of sets ("Zmień zestaw") and as a line at the top of
the Zestaw view of that set, never in the Msza view. The comparison shows both versions' items
side by side and offers **Zostaw tę**, **Użyj tamtej**, and **Dodaj** next to each item that only
the other version has (it goes in after the same neighbour, or at the end). Closing the review
saves one version replacing both; the mark goes away everywhere once the file travels.

### 3.5 Sets

A set's body: name, date (section 5), church, and the items in order. Every command that
changes the set (add, move, remove, undo, rename, set the date) saves a version, so "more
changes" counts the organist's actions. Like the song editor, a command replaces only the
current version: the other version of a set that is "do przejrzenia" stays until it is reviewed
(3.4), so editing the set during Mass never silently drops what came from home. Deleting the
set replaces every version (rule 5).

Merging never changes which set is active on a device and never changes the program: the live
deck is a frozen copy (ADR 0005), so a merge that replaces the active set behaves like an edit
from another remote. If a merge deletes the active set, the list's first set becomes active and
the program keeps its ghost position.

### 3.6 Why not something simpler or something stronger

- **Timestamps ("the later edit wins")**: the church devices have no reliable clock.
- **Per-device counters (version vectors)**: break when a backup is restored on a second device
  or a data folder is copied, because two devices then share an identity.
- **Copies for conflicts** (today): section 1.
- **Merging sets item by item**: keeps both sides' additions automatically, but needs a marker
  on every item and its removal; not needed while one person manages a church's sets, and the
  review's **Dodaj** covers the rare case by hand. Item ids stay stable, so it can be added later
  without a new file format.

## 4. Churches

- A **church** is a document with a name ("Tulce"). Churches cannot be deleted, only renamed.
- **Named with the PIN** (decision 14): the first-start form ("Ustaw PIN") also asks for the
  church's name. A device with a screen keeps "this device's church" as a local setting and
  lists, activates and creates sets of that church only. The name can be changed later in
  Ustawienia → Kościół.
- **The church's id comes from the name typed at first start** (lower case, without Polish
  letters and spaces at the ends, as the search folds text), not from chance. A device set up
  again (new SD card, no backup) where someone types "Tulce" again is the same church as before,
  so its sets and the old ones are one list, with no question to answer. A later rename changes
  the name, not the id.
- **When the name was typed differently** ("Tulec"): Ustawienia → Kościół lists
  the churches known from loaded files; choosing one moves this device and the sets it made to
  that church. A church with no sets is not offered anywhere.
- Restoring a device backup carries the setting, as it carries everything else.
- The desktop program has no church of its own. Its Zestaw view has a church switcher and
  creates a set for the church shown. It knows the churches from loaded files, and can create
  one (same naming rule) for someone who starts at home before any device exists.
- Sets of other churches are stored and travel in files, and are not shown on a church device
  (decision 4).

## 5. Set dates and the list

- A set has an optional date and time of day, chosen by the organist ("2026-11-22", "10:00"); no
  device clock is involved.
- The list: sets without a date first (drafts), then by date, newest on top. The same order on
  every device. No "today", because the Pi may not know what day it is.
- A new set asks for the date together with the name.

### 5.1 "Nowy zestaw z tego"

The usual way to start next Sunday's set: take last Sunday's and change what differs.

- Offered on every set in the list ("Zmień zestaw") and in the Zestaw view of the set itself.
- Asks for the name (the original's, to edit) and the date (empty, to fill in), like "Nowy
  zestaw".
- The copy is a **new set**: a new id, new item ids, no ancestors. It is not a version of the
  original, so later merges never mix the two, and the original stays as it was.
- Items are copied as they are: songs by id with their order override, announcements with their
  text, pictures by their asset id (no file is copied), pauses.
- Church: on a church device its own. In the desktop program the church shown, with a choice of
  the other one, so a set from Tulce can be the start of one for Gowarzewo.
- The copy becomes the active set, as a new set does today; the program on screen does not
  change (ADR 0005).
- Server: one command, e.g. `CopySet { set_id, name, date, church }` (the church is ignored on a
  church device); the copy is built on the server, the remote only asks.

## 6. What travels and what stays

**In the file** (`.sursum`, format 2, a ZIP): `manifest.json`, `songs.jsonl`, `sets.jsonl`,
`churches.jsonl` (one document with its heads per line), and `assets/<id>` for every picture a
head of a live set uses. Pictures are named by the hash of their content, so the same poster
uploaded in two places is one file and merging pictures is "add the missing ones".

**Never in the file**: PIN and paired remotes, the active set, the program, the screen profile,
display power, the gap setting, the logo, this device's church, the import bookkeeping
(section 7).

- Format 1 files are refused with "ten plik pochodzi ze starszej wersji programu"; nothing
  translates them (decision 8).
- A body is stored and passed on as the bytes it arrived in, never written out again, so a
  version id stays valid when a later build adds a field to `Song`.
- The reader computes every version id itself and trusts no id in the file (as today).
- The whole-device backup (`.sursum-backup`, ADR 0018) stays as it is: it also holds the
  device's settings and replaces rather than merges.

## 7. OpenSong import under the same rules

Built in Y1.4 (2026-11-20). One rule replaced the earlier idea of looking for the file's text in
a song's history: a device that received a song from the other church's file has only its heads,
not the texts of older versions, so it could not recognise an old text by its content.

- The version an import makes for a text is built on the version the previous import of that
  song made **on this device** (table `imported_versions`, never in a file); if there was none,
  it is a first version with no ancestors. A first version has the same id on every device, so
  a church that never imported a song still recognises the other church's import of the same
  text: it is an ancestor of what that church sent, and nothing happens.
- The same text as the previous import, or a version already known: nothing happens. An old
  folder never undoes a correction made in sursum, and never brings back a deleted song.
- A new text on a song untouched in sursum since the last import: the song shows it (updated).
- A new text on a song corrected in sursum meanwhile: both stay, rule 3 picks, and the song is on
  the review list; the import report names it (`ImportReport.to_review`).
- A church that never imported a song and whose folder has a text nobody imported before: the
  text is a first version next to what the other church sent; it has fewer changes, so the other
  church's text stays shown and the song waits for review.
- Imported sets: the set id also depends on the device's church, so the same OpenSong set
  imported in both churches is two sets, one per church.

## 8. In the remote

- Ustawienia → Baza pieśni becomes **Synchronizacja**: **Zapisz plik** and **Wczytaj plik**.
  One kind of file, any direction, any number of times.
- Loading shows a report before anything is written, then applies it: songs added, updated,
  deleted, changed in both places (with the title and which text stays); sets of this church
  added, updated, changed in both places (with what the other version held); pictures added;
  one line for other churches' sets.
- "Do przejrzenia": songs in the search and the song card, sets in the list of sets; the two
  comparison screens (3.4).
- The first-start form: PIN twice and the church's name.
- Zestaw view: a date next to the name; in the desktop program the same view with the church
  switcher and without anything that shows on a screen.
- Ustawienia → To urządzenie: the church's name, and the choice of a known church (section 4).

All texts through `Msg`; no logic in the browser: the server sends the review list, the report
and the list order.

## 9. Code

- `sursum-core`: a small pure module (versions, heads, merge, current, version id) used for
  songs, sets and churches, with property tests of the laws in section 10.
- `sursum-server`: one `versions` table (document kind and id, version id, ancestors, body or
  "deleted", head or not) instead of `songs`, `song_history`, `song_known_hashes`, `sets` and
  `set_items`. Built for songs in Y1.1 (`db/songs.rs`); search and backups read it through
  `load_songs`; sets move to it in Y1.6.
  Old versions of songs stay in the database as local history; old versions of sets are dropped
  (every command makes one).
- `library.rs`: export and merge are rewritten on the module above; `decide`, the conflict copy
  and the known-hashes bookkeeping go away.
- The merge upload is streamed to disk like the backup restore (today it is held in memory
  with a 256 MB cap; files with pictures will be larger).
- `sursum-protocol`: the report, review list, set date, church and `CopySet` types; format-2
  snapshots.

### 9.1 No migration (decision 8)

- The schema list in `db.rs` starts over: the tables of section 9 and the ones that stay
  (settings, runtime state, devices) are migration 1 of the new list. Nothing reads the old
  tables.
- A database written by an earlier build is not converted and not mistaken for a damaged one:
  the server refuses to start with a message to move the data folder aside. Built in Y1.1:
  schema numbers of this line start above 100 (`SCHEMA_BASE`), so 1 to 100 means an earlier
  build; that is reported as its own error, never as damage, so the automatic restore of the
  newest backup does not run (every backup has the old schema too). Old releases, in turn, see
  the new numbers as "newer" and refuse.
- Device backups (`.sursum-backup`) of earlier builds are refused the same way.
- The pilot Pi and the development machines start with an empty data folder. The pilot Pi is
  reflashed for ADR 0022 anyway.
- The demo set gets a random id and is created only on a device's very first start (today it has
  a fixed id and comes back whenever no set exists; with a fixed id two devices' demo sets would
  be one document in two churches).

## 10. Tests

- Property tests in `sursum-core`: merging is the same in any order, grouping and repetition;
  the current version is the same on devices that have seen the same files; an ancestor never
  replaces its descendant; a document is never duplicated; random edits, deletions and merges
  among three devices end in one state once files went round.
- Server tests: the two-church scenario of section 1 (one song each, same text after one round);
  the text typed back; delete against edit; a stale file; the OpenSong cases of section 7; a set
  changed in both places, edited during Mass and then reviewed; a merge while the set is active
  and an item is live; two devices named "Tulce" being one church.
- e2e: load a file in the remote and see the report; review a song and a set (with **Dodaj**); a
  set made in library mode appears in the church after loading the file; the list order by
  date; "Nowy zestaw z tego"; the first-start form with the church's name.

## 11. Work packages, in order

Each is one pull request.

| # | Package | Depends on |
|---|---|---|
| Y1.0 | ADR 0025 from section 2 and 3; roadmap section ✓ | PR #2 merged |
| Y1.1 | Core module with property tests; the new schema and refusing old databases (section 9.1); songs saved and loaded through it; editor saves on the loaded version; file format 2 for songs with merge and report; the section 1 scenario as a test; conflict copies gone ✓ | Y1.0 |
| Y1.2 | Streamed merge upload (done with Y1.6) | Y1.1 |
| Y1.3 | Review list and comparison screen (keep one version; editing comes after keeping) ✓ | Y1.1 |
| Y1.4 | OpenSong import under section 7 ✓ | Y1.1 |
| Y1.5 | Churches: the document, the id from the name, the first-start form, the device setting and the choice of a known church ✓ (the section is Ustawienia → Kościół, not To urządzenie: that one is about the tablet) | Y1.1 |
| Y1.6a | Sets as versions: every command saves one; date and list order; "Nowy zestaw z tego" (5.1); church-bound sets; the demo set once ✓ | Y1.5 |
| Y1.6b | Sets and pictures in the file; report lines; streamed merge upload | Y1.6a |
| Y1.6c | Set review and its comparison screen | Y1.6b |
| Y1.7 | Zestaw view in the desktop program with the church switcher; "Przywróć z kopii" hidden there (it cannot work without a service manager) | Y1.6 |
| Y1.8 | Operator guide: Synchronizacja, review, sets at home, dates, "Nowy zestaw z tego"; new screenshots | Y1.3, Y1.7 |

## 12. Second step: "Połącz pieśni"

Not planned in detail. The first step must not block it, and does not: version ids do not
contain the song id, so two songs' versions can be pooled.

- The merge report names **certain duplicates** (two songs whose current versions are the same
  bytes) and **possible ones** (same title after folding, or same number in a songbook).
- "Połącz" records that two ids are one song (a pair that only ever gets added and travels in
  the file). The smaller id stays; the other becomes an alias that sets keep working with; the
  texts become versions of one song, so rule 3 and the review apply.
- "To różne pieśni" is remembered the same way, so no device asks again.

## 13. Questions answered

All answered by the maintainer on 2026-11-19 and recorded as decisions 8 to 14 in section 2: no
migration; "Nowy zestaw z tego" now; a deletion never beats a change made elsewhere; a set that
lost is reviewed like a song; a second OpenSong import keeps sursum's corrections; all pictures
travel, a limit later; the church is named with the PIN. Questions that come up while building
go to the maintainer before the package that needs the answer.

## 14. Risks

- The new schema replaces five tables at once; nothing old is converted, so whoever has data
  from an earlier build loses it (nobody does yet, decision 8).
- "More changes win" is arbitrary about quality: thirty small saves beat one important
  correction. The review lists for songs and sets are what make that safe.
- A church name typed differently on a new device makes a second church until someone chooses
  the right one in Ustawienia (section 4).
- Files grow with every poster (decision 13); the limit by set date is the planned answer.
- Every device must be updated before the first format-2 file is loaded, and the Windows
  program has no update mechanism: people replace the file by hand.
