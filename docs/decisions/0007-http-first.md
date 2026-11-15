# 0007 — Plain HTTP in the MVP; optional device CA later

Status: Accepted (2026-11-15)

## Context
Service Worker, PWA installation and Screen Wake Lock need a secure context. The LAN has no
internet, so public certificates are impractical (renewals, shortening lifetimes).

## Decision
The MVP uses plain HTTP. The remote must not depend on secure-context APIs (feature detection
via `isSecureContext`). Tablets stay awake through device settings (stay awake while charging /
long screen timeout). Optional later (M6): a name-constrained CA generated on the device, with a
`/setup` page explaining installation on Android and Windows. The Chrome flag
`unsafely-treat-insecure-origin-as-secure` is a documented per-tablet workaround (verify it
persists across updates).

## Consequences
WebSocket, keyboard events (pedals), fullscreen and vibration work without HTTPS. In the Windows
pilot, `http://localhost` is already a secure context.
