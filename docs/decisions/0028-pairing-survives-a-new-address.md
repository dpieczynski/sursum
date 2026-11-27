# 0028 — A tablet's pairing survives a new device address

Status: Proposed (2026-11-27)

## Context
The pairing cookie (`sursum_device`, `HttpOnly; SameSite=Strict`, ADR 0016) belongs to the
host the remote was loaded from. The Android launcher (`android/`, spike) therefore loads a
device it found by DNS-SD as `sursum.local` first. On a Pixel 10 Pro with Android 17 and
WebView 157 (2026-11-27) `.local` names do not resolve in the WebView, so the launcher fell back
to the IP address and the pairing became tied to it: when the parish router gives the Pi another
address, every tablet must enter the PIN again.

The Avahi service file is the same on every Pi (one image), so the announcement alone cannot
say which device it is. The pairing token gives full control of the screen, and the parish Wi-Fi
is shared with guests: anything that moves the cookie to a new address must not hand it to a
host that merely announces `_sursum._tcp`.

## Decision
- **Server identity.** Each server creates a random `ServerId` (`server-<uuid>`) at its first
  start and keeps it in the `settings` table. It survives restarts and system updates (the
  database is shared by both A/B slots). It is treated like the PIN and the paired remotes: a
  device backup leaves it out and restoring keeps the device's own (ADR 0018). Tokens do not
  travel with a backup either, so an identity that did would only let tablets believe a
  restored-onto device is "theirs" while their tokens are unknown there.
- **Public identity over HTTP.** `GET /api/identity` answers `{ server_id }`. The launcher reads
  it from the address it loaded the remote from and remembers it with that address. An HTTP
  endpoint rather than a TXT record: the Avahi file stays static, nothing has to rewrite it at
  boot, and the laptop simulator (`avahi-publish` plus a server) behaves exactly like the Pi.
- **Proof before the move.** When the launcher finds the device again at another IPv4 address
  (discovery or a typed address) and the identity there matches, it asks
  `POST /api/identity/proof { key, nonce }`. `key` names the pairing (SHA-256 of a prefix and
  the token hash); the server answers HMAC-SHA256, keyed with the token hash it stores, over its
  identity, its own address as the request reached it (the TCP socket's local address) and the
  nonce. Only if the proof checks out and is for the address the launcher connected to does it
  copy the cookie to the new origin with Android's `CookieManager` and load the remote.
  Otherwise it connects as it always did (the operator sees the PIN form). The exact format is
  in `docs/protocol.md`.
- Once the copy is in place, an old IPv4 address of another host loses the cookie (DHCP may give
  it to another machine). A name keeps it, and the same host is never touched (cookies ignore
  the port).
- The launcher remembers where its pairing lives (address and server identity) separately from
  the last address it loaded, and updates it only when it holds a cookie for the loaded
  address. Loading an unpaired page, such as another device or an impostor, therefore never
  strands the pairing.
- The name-first path (`sursum.local`) stays for tablets where the name resolves: a name the
  tablet is paired under is always tried first, and only if it does not load does the fallback
  to the IP address move the pairing.
- Operators are advised to give the Pi a DHCP reservation in each church's router anyway
  (operator guide).

## Consequences
- A rogue host that copies the public identity cannot answer the proof (it lacks the token hash)
  and gets nothing it could not get before this change: the launcher then behaves as before and
  shows the rogue's page without a cookie. Relaying the question to the real device does not
  help: the answer names the real device's address, which the launcher refuses for any other.
- The proof does not defend against an attacker who can take over the device's current or old
  IP address on the LAN (ARP spoofing, or claiming the old address after a DHCP change). Such an
  attacker can already read the cookie from plain HTTP traffic (ADR 0007) whatever the launcher
  does; moving the cookie, and removing it from the old address, does not widen that.
- Whoever holds the device's database (it never leaves the device; backups strip the devices
  table) could answer proofs; that is the same trust as the token hashes themselves.
- HMAC-SHA256 is written out in the server over the `sha2` crate already used (ten lines,
  checked against RFC 4231) instead of adding the `hmac` crate; the launcher uses the platform's
  `javax.crypto.Mac`.
- The HTTP router's connect info now carries the local address as well as the peer
  (`http::Connection`).
- Because the proof names the server's own socket address, anything that rewrites addresses
  fails closed: NAT, port forwarding, a reverse proxy, or the Android emulator's `10.0.2.2`
  mapping. The launcher then shows the PIN form, as it did before this change. The device sits
  directly on the parish LAN, so none of these apply there.
- Only IPv4 targets are moved to: what DHCP changes and what discovery reports first. A tablet
  that reaches the device only over IPv6 must pair again after a change, as before.
- The launcher stores the server identity next to the last address; the privacy policy says so.
