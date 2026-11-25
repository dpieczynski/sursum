# 0025 — Songs and sets keep their identity and get versions; more changes win

Status: Accepted (2026-11-19); built 2026-11-20, see the roadmap section Y1 and
[plan-sync.md](../plan-sync.md). Supersedes the merge rules of the `.sursum` file (roadmap M3,
`library.rs`): conflict copies, the list of known hashes and the songs-only file.

## Context
The two churches of the parish, and people preparing songs at home with the desktop program
(ADR 0020), exchange `.sursum` files. Today a file carries songs only, and a song changed in two
places becomes a second song ("(kopia ze scalenia)"). A test on 2026-11-19 carried files both
ways between two databases with one song corrected in each: both ended with three songs, the
original stayed different in each church for good, and every later correction added another
copy. Two more defects: history is a list of content hashes, so a text typed back looks like
the old one and loses; and a second OpenSong import overwrites corrections made in sursum.

The maintainer wants the library and the sets to be managed as simply as possible and merged
without duplicates, with the two churches syncing in any direction.

## Decision
1. **Documents and versions.** A song, a set and a church are documents with an id that never
   changes. Every save creates a version: its body (the document without its id, as the bytes
   it was saved in; or "deleted") and its ancestors (every version it replaces, directly or
   not). The version id is SHA-256 over the body and the sorted ancestor ids.
2. **Merging** is the union of both sides' versions, minus every version that is another's
   ancestor; what remains are the heads. The result does not depend on direction, order or
   repetition, and a merge never creates a document the device already has.
3. **The current version** among the heads: a live one before a deleted one (a deletion never
   beats a change made elsewhere without knowing of it), then the one with **more changes**
   (more ancestors), then the greater version id. No clock and no device identity are used.
4. **Nothing is thrown away.** Other live heads stay as "other versions"; a song or set with
   two live heads whose bodies differ is "do przejrzenia", with a comparison screen (for sets
   also adding single items from the other version). Reviewing saves one version replacing both.
5. **Saving** replaces the version the user worked from and every deleted head. **Deleting**
   replaces every head. So an ordinary edit (also during Mass) never silently drops another
   live version, and a deletion that lost needs no review.
6. **Sets** belong to a church, have an optional date set by the organist (the list is ordered
   by it, never by a device clock), and every command that changes a set saves a version. One
   person manages a church's sets, so sets are versioned as a whole, not item by item.
   "Nowy zestaw z tego" makes a new, independent set from an existing one.
7. **Churches** are named together with the PIN on a device's first start; the church's id comes
   from the folded name, so a device set up again under the same name is the same church. A
   device shows and edits only its church's sets but stores and passes on all of them.
8. **One file**: `.sursum` format 2 carries songs, sets, churches and every picture the sets use.
   It never carries the PIN, paired remotes, the active set, the program, the screen profile,
   display power, the logo or the device's church. Format 1 is refused.
9. **OpenSong import** follows the same rules: a file's text the device already knows changes
   nothing; a new text is a version on top of what the import last brought for that song.
10. **No migration**: nobody uses the program yet, so the schema starts over and databases,
    backups and files of earlier builds are refused with a clear message (never mistaken for a
    damaged database).

## Consequences
- Two churches that have exchanged files show the same text of every song and the same sets,
  and loading any file again, in any direction, changes nothing.
- "More changes win" says nothing about quality: many small saves beat one important
  correction. The review lists are what make that acceptable.
- Ancestor lists grow with every save; a set changed by hundreds of commands carries hundreds of
  ids in each head (about 64 bytes each). Acceptable; compaction can come later without
  changing the rules.
- Every device must be updated before the first format-2 file is loaded; the Windows program
  is replaced by hand.
- "Połącz pieśni" (two song ids declared one song) is a later step; version ids do not contain
  the document id, so it fits the same model.

## Rejected
- **Timestamps**: the church devices have no reliable clock.
- **Per-device counters (version vectors)**: two devices share an identity after a backup is
  restored or a data folder copied.
- **Conflict copies** (today): they multiply songs instead of converging.
- **Merging sets item by item**: not needed with one organist per church; the review's "Dodaj"
  covers the rare case. Item ids stay stable, so it remains possible.
