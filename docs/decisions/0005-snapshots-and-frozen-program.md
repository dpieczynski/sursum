# 0005 — Snapshot protocol, frozen program, `Stale` rule

Status: Accepted (2026-11-15)

## Context
Several remotes edit the set during mass while the screen must not change until the operator
navigates. State is small.

## Decision
- One state actor applies all commands in a single order; snapshots of whole domains are fanned
  out with `tokio::sync::watch`. No patches, no event log.
- Going live stores a **frozen copy** of the compiled deck in the program; set/song edits never
  touch it. A ghost position handles removal of the live item.
- Relative navigation carries `baseRev` and is rejected as `Stale` if another client changed the
  program meanwhile (same client may press repeatedly). Absolute commands always apply. Song
  edits use optimistic locking on `revision`.

## Consequences
Simple, idempotent reconnect (full snapshot). Two operators pressing "next" together advance
once. Details in [../protocol.md](../protocol.md).
