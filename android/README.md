<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# Android launcher (spike)

A small Android app that opens the remote from the sursum device in a full-screen WebView. It
keeps the screen on, has its own icon, and shows no address bar. The tablet needs no CA and the
device needs no HTTPS (ADR 0007). The app has no domain logic (hard rule 1): it finds the
device, loads `http://<device>/remote/` and stays out of the way. It is a spike. Whether it
replaces plan §11.2's rejection of "option E" waits for the tablet checklist below and then an
ADR.

## What it does

- **One activity, plain Java, no libraries.** It uses only the platform's own classes: no
  AndroidX and no Kotlin standard library. The release APK is about 20 KB.
- **Finds the device by DNS-SD.** `NsdManager` looks for `_sursum._tcp`, which the image
  announces through Avahi (`image/layer/sursum-device.d/customize.overlay/etc/avahi/services/sursum.service`).
  It connects to the first device it finds and remembers the last address that worked. As a
  fallback the operator can type an address (`192.168.1.20`, `sursum.local`, or a pasted URL).
- **By name first, IP second.** The pairing cookie belongs to the host the remote was loaded
  from. Loaded from an IP address, a tablet would have to pair again whenever DHCP gives the
  device a new one. So a found device is loaded as `<service name>.local` (Avahi announces the
  service under the host name, so `sursum.local`), and only if that name does not resolve on
  the tablet, from the IP that discovery reported. Whether a WebView resolves `.local` names
  depends on the Android version; that is checklist item 2 (on Android 17 it does not).
- **The pairing moves with the device (ADR 0028).** After the remote loads, the app reads the
  server's identity (`GET /api/identity`, a random id the device keeps for good). Once the
  tablet is paired there, it remembers that address and identity as the pairing's home, apart
  from the last loaded address, so visiting an unpaired page (another device, or an impostor)
  never loses track of the real pairing. When the device turns up at another IPv4 address
  (discovery, a typed address, or the IP behind a name that did not load), the app checks the
  identity there. If it matches, the app asks the device to prove that it holds this tablet's
  pairing (`POST /api/identity/proof`: an HMAC keyed with the token hash only the real device
  stores, over a fresh nonce and the address the question reached). Only then does it copy the
  pairing cookie to the new address with `CookieManager` and load the remote, without the PIN.
  The old address loses the cookie if it was an IPv4 address (DHCP may give it to another
  machine); a name keeps it, and a name the tablet is paired under is always tried first, so
  tablets that resolve `sursum.local` keep that path. A host that announces `_sursum._tcp` and copies the identity cannot answer,
  and relaying the question to the real device gives an answer for the real device's address,
  which the app refuses. Then, and for any other device, the app connects as before (the PIN
  form for an unpaired tablet). The check is `PairingMove.java`; the format is in
  `docs/protocol.md`.
- **Find screen.** While no remote has loaded, a find screen (Polish) covers the WebView. When
  a load fails, it says so and tries the same address again every 5 s. It also connects to any
  device that discovery reports.
- **Screen and navigation.** `FLAG_KEEP_SCREEN_ON` keeps the screen awake; the web Wake Lock
  API is not used. The app runs in immersive full screen; a swipe shows the system bars for a
  moment. Back moves between the remote's views (its URL fragments) and never leaves the app.
  An Escape key the page did not handle is not turned into Back.
- **Survives tablet events.** The activity is never recreated, so turning the tablet, pairing a
  keyboard or pedal, or a theme change does not reload the remote. If the WebView's renderer
  dies, the app makes a new WebView instead of crashing.
- **File fields and downloads.** The remote's file fields (logo, OpenSong import, library
  merge, backup restore, system update) open the system file picker. Library exports and
  backups go to Downloads through `DownloadManager`, with the pairing cookie attached.
- **Pairing data stays on the tablet.** There is no backup and no device-to-device transfer, so
  the pairing cookie never reaches another tablet.

## Build and install

```sh
just android            # target/android/sursum-debug.apk (debug-signed), also runs Android lint
just android-install    # adb install -r on a tablet connected over USB (or `adb connect`)
```

The build runs in a container (`android/build.Containerfile`: Debian trixie, OpenJDK 21, the
SDK command-line tools with platform 37 and build tools 36.0.0, Gradle 9.9.2; AGP 9.5.3), so
the only thing installed on the host is podman. The first build downloads about 1 GB. Gradle's
cache and the debug signing key are kept in `~/.cache/sursum-android`. Keep that folder: a
tablet refuses to update the app over one signed with a different key, and uninstalling the
app deletes its pairing. Building accepts the Android SDK licence (`sdkmanager --licenses`).

Debug builds allow `chrome://inspect` from a PC over USB. That is how the remote's console can
be read on the tablet.

Versions: compileSdk 37, targetSdk 36, minSdk 24 (Android 7.0). Not targeting 37 yet:
apps that target Android 17 need the `ACCESS_LOCAL_NETWORK` runtime permission to reach the
device. Below 37, the `INTERNET` permission grants it implicitly
([developer.android.com](https://developer.android.com/privacy-and-security/local-network-permission)).

## Release key, Google Play and developer verification

Parish tablets get only release builds. Decided 2026-11-27: the app is published on **Google
Play** from the maintainer's full, identity-verified developer account (this replaces the free
limited distribution account chosen on 2026-11-26). The same account registers the package for
Google's developer verification (worldwide in 2027), so an APK installed outside Play keeps
working too.

```sh
just android-key ~/sursum-keys/android-release.p12    # once; asks for a password
just android-fingerprint ~/sursum-keys/android-release.p12
SURSUM_ANDROID_KEYSTORE=~/sursum-keys/android-release.p12 \
SURSUM_ANDROID_KEYSTORE_PASSWORD=… just android-release   # target/android/sursum.apk and .aab
```

- **One key for both channels.** Play App Signing is mandatory for new apps. Play signs with
  *our* release key, uploaded once, rather than a key Google generates. A tablet can then move
  between the Play build and an APK installed by hand (GitHub release, adb) without
  uninstalling, which would delete its pairing. It also keeps signed builds outside Play
  possible.
- **For now the release key is the upload key too.** `just android-release` signs the APK and
  the AAB with the one key. A separate upload key only helps once releases are built in CI.
  Then CI gets a new upload key (Play Console's upload key reset), and the release key stays
  offline.
- Keep the key file and its password offline as well (for example on a USB stick in the parish
  office). Google holds a copy for Play, but APKs installed outside Play need ours. The key
  never goes into the repository.
- A tablet that has the debug build must uninstall it before the release build goes on, which
  deletes its pairing. That is expected once, after the test.

### Uploading the release key to Play App Signing (once)

When creating the app in Play Console, choose **Export and upload a key from Java keystore**.
`keytool`'s PKCS#12 file counts as a Java keystore here. Download the PEPK tool (`pepk.jar`)
and the encryption public key (`encryption_public_key.pem`) from that page into an empty folder,
for example `~/pepk`. Then run PEPK in the builder container. The host needs no Java, and PEPK
sees the key read-only and with no network:

```sh
podman build -q -t localhost/sursum-android-builder -f android/build.Containerfile android
podman run --rm -it --network=none --userns=keep-id --security-opt label=disable \
  -v ~/sursum-keys/android-release.p12:/keys/release.p12:ro \
  -v ~/pepk:/work -w /work \
  localhost/sursum-android-builder \
  java -jar pepk.jar --keystore=/keys/release.p12 --alias=sursum \
    --output=/work/sursum-signing-key.zip --include-cert --rsa-aes-encryption \
    --encryption-key-path=/work/encryption_public_key.pem
```

PEPK asks for the store password and the key password. Both are the one password `just
android-key` set. If Play Console shows a newer set of flags, use its flags inside the same
wrapper. Upload `sursum-signing-key.zip`, then delete it.

Afterwards check that **Test and release → App integrity → App signing** shows, as the app
signing key certificate, the same SHA-256 as `just android-fingerprint`. Also check that
Play Console's developer verification section lists `io.github.dpieczynski.sursum` with that
fingerprint.

Open points: which Play track the parishes use (internal or closed testing needs a Google
account on each tablet; production first needs the 12-tester, 14-day closed test that new
personal accounts must run), and the launcher ADR, which should record this route.

## Checked in an emulator (2026-11-26)

Android 16 tablet image (API 36, WebView 133). The server was this checkout's release build,
reached at 10.0.2.2:18080.

| Check | Result |
|---|---|
| Find screen at first start (no device on the network) | shown, Polish, immersive (no system bars) |
| Typed address | the remote loads over plain HTTP; PIN set, the tablet paired |
| On-screen keyboard over a remote field | the page shrinks above the keyboard |
| Back | closes the keyboard; Zestaw → Msza; on Msza it stays in the app |
| Escape (hardware key) | does not change the view |
| PageDown (what a pedal sends) | reaches the remote, the program advances |
| Force stop, start again | opens the remote at once, still paired (cookie and address kept) |
| Server restarted while the remote is open | the remote reconnects on its own (WebView untouched) |
| Server down at app start, then started | find screen "Nie udało się połączyć…", remote shown 3 s after the server came back |
| `document.fullscreenEnabled` | `false`, so the remote hides its own full-screen toggle (the app is already full screen) |
| `navigator.vibrate` | is a function; returned `false` when called without a tap and with no vibrator in the emulator, so this is not settled |

Not checked: DNS-SD discovery (the emulator's network has no multicast) and the Avahi file on
a real device.

## Checked on a phone (2026-11-27)

Google Pixel 10 Pro, Android 17, Android System WebView 157.0.8103.52, on home Wi-Fi. The app
came from Google Play's internal testing track (Play App Signing). The opt-in link first showed
"Item not found"; the store page appeared after a while and the install worked. The device was
simulated by this checkout's release server on a laptop (`--listen 0.0.0.0:18080`, the starter
library), announced with `avahi-publish -s sursum _sursum._tcp 18080` and a `sursum.local`
address record, as the Pi does.

| Check (checklist item) | Result |
|---|---|
| Discovery (1) | found without typing |
| PIN and pairing | PIN set from the phone, paired |
| Controls | the remote's controls work |
| Screen stays on (3) | the screen did not dim |
| The name, `sursum.local` (2) | **does not resolve**: the app remembered `192.168.5.40:18080`, so loading by name failed and it fell back to the IP from discovery. Pairing is therefore tied to the IP |
| Vibration (11) | works: taps vibrate |
| Android 17 with targetSdk 36 | reaches the device on the local network without `ACCESS_LOCAL_NETWORK` |

Still open: everything else on the checklist, and a real tablet. The pairing move (ADR 0028)
was built after this test and is not yet checked on the phone; the steps are below.

### Checking the pairing move with the laptop simulator

Cookies belong to a host, not to a port, so a new port on the same address is **not** a new
address: the phone would simply send the cookie it has. The simulated device must change its
IP address. The laptop gets two extra addresses on its Wi-Fi interface for that, and each
"device" announces its own mDNS host name so discovery reports exactly the address it listens
on (the laptop's own host name would announce all of them).

Pick the interface and its address with `ip -br addr` (below `wlan0` and A = `192.168.5.40`),
and two free addresses in the same network, outside the router's DHCP pool (B =
`192.168.5.77`, C = `192.168.5.78`; `ping -c1 -W1` must not answer). Run the three commands of a
step in three terminals, and stop all three (Ctrl+C) and swipe the app away before the next
step; only one `sursum` service may be announced at a time.

0. `just build`, `sudo ip addr add 192.168.5.77/24 dev wlan0` and
   `sudo ip addr add 192.168.5.78/24 dev wlan0`. Install the debug app (`just android-install`;
   clear its data first if it was installed before).
1. The device at A:
   `target/release/sursum-server --data-dir ~/sursum-sim --listen 192.168.5.40:18080`,
   `avahi-publish -a -R sim-a.local 192.168.5.40` and
   `avahi-publish -s -H sim-a.local sursum _sursum._tcp 18080`.
2. Start the app: it finds the device, set the PIN and pair. Wait until the remote shows, then
   swipe the app away (that is when it notes where the pairing lives).
   `curl http://192.168.5.40:18080/api/identity` shows the identity.
3. The device moves to B: the same three commands with `--listen 192.168.5.77:18080`,
   `avahi-publish -a -R sim-b.local 192.168.5.77` and `-H sim-b.local`.
4. Start the app. Expected: "Nie udało się połączyć z 192.168.5.40:18080", then within a few
   seconds the remote at 192.168.5.77:18080, **paired, without the PIN**.
5. An impostor at C with the same identity but without the pairings:
   `rm -rf ~/sursum-impostor && cp -r ~/sursum-sim ~/sursum-impostor && sqlite3 ~/sursum-impostor/sursum.db 'DELETE FROM devices'`,
   then `--data-dir ~/sursum-impostor --listen 192.168.5.78:18080`, `sim-c.local` for
   192.168.5.78. Start the app. Expected: the impostor's PIN form, **not paired** (it answers
   the proof with 404, so the cookie is not copied). Do not enter the PIN.
6. The real device again, now at A (a third address after B): step 1's commands. Start the app.
   Expected: "Nie udało się połączyć z 192.168.5.78:18080", then the remote at A, **paired,
   without the PIN**: visiting the impostor did not make the app forget the pairing at B.
7. Typed address and a late device: stop everything, start the app (find screen), type
   `192.168.5.77:18080` and tap Połącz; it keeps retrying. Then start step 3's server (the
   `avahi-publish` commands are not needed). Expected: within 5 s of the server starting, the
   remote opens paired, without the PIN.
8. Clean up: `sudo ip addr del 192.168.5.77/24 dev wlan0` and
   `sudo ip addr del 192.168.5.78/24 dev wlan0`.

The name-first path cannot be checked on a phone that does not resolve `.local` names; on a
tablet that does (checklist item 2), item 10 covers it.

## Checklist for one parish tablet

Install the image with the Avahi file (`just image`, or a system update built from this
branch), or keep the current image and type the address. Note the tablet's model and its
Android and WebView versions (Settings → Apps → Android System WebView).

1. **Discovery.** Start the app with the tablet on the device's network. Is the device found
   without typing? How long does it take? If it is not found, check `avahi-browse -rt
   _sursum._tcp` from a Linux laptop on the same network. Also check whether the address field
   works as a fallback.
2. **The name, `sursum.local`.** This decides how pairing survives a new IP address. After
   item 1, the address field on the find screen (swipe the app away and start it with the Pi
   off to see it) shows what the app remembered: `sursum.local` means the name worked, an IP
   means it fell back. Also type `sursum.local` by hand on the find screen and tap Połącz.
   Optionally, with `chrome://inspect` from a PC, `location.host` in the console shows it too.
   Note the result with the Android version. If the name does not resolve, pairing still
   survives a new address through the move of item 10, so this decides only which path the
   tablet takes.
3. **Screen stays on.** Leave the remote open, untouched, longer than the tablet's screen
   timeout (on battery and while charging). The screen must not dim or lock.
4. **Immersive mode.** No status bar or navigation bar. A swipe from the edge shows them
   briefly and they hide again. Turn the tablet: the layout follows and the remote does not
   reload (it stays on the same view). On a tablet with a camera cutout, nothing important is
   hidden behind it.
5. **Back.** The back gesture or button moves between Msza, Zestaw and Ustawienia, then does
   nothing. It never closes the app. Home and the recent-apps switcher still work.
6. **Pairing survives a restart.** Pair with the PIN, then swipe the app away in recent apps
   (or Settings → Apps → sursum → Force stop), and also reboot the tablet. The remote must open
   paired, without the PIN.
7. **Foot pedal.** Pair the Bluetooth pedal while the remote is open. The remote must not
   reload. Pedal presses move the slides (also after the tablet slept and woke), and learn mode
   in Ustawienia → To urządzenie → Pedał i klawisze works. Note whether the on-screen keyboard
   still appears when a text field is tapped with the pedal connected (roadmap M5).
8. **Device reboots.** With the remote open, reboot the Pi (Ustawienia, or pull the power). The
   remote shows its own "Brak połączenia" banner and reconnects by itself, without reloading
   the page or showing the find screen. Note the seconds from the device's screen showing the
   logo to the remote being usable.
9. **Device unreachable.** Turn the Pi off, then close and start the app. The find screen says
   "Nie udało się połączyć z …" and keeps trying. Turn the Pi on: the remote should appear by
   itself. Also try the tablet on the wrong Wi-Fi.
10. **New IP address.** Pair the tablet, close the app, give the Pi another address (change its
    DHCP reservation in the router and reboot the Pi), then start the app. The find screen shows
    "Nie udało się połączyć z <old address>", discovery finds the device at the new one and
    the remote opens **without asking for the PIN** (by name if item 2 showed `sursum.local`,
    otherwise by moving the pairing). Afterwards the find screen's address field (swipe the app
    away with the Pi off) shows the new address. Also try typing the new IP address by hand on
    the find screen with discovery off (another network segment, or before Avahi announces):
    the remote must open paired too.
11. **Vibration.** Ustawienia → To urządzenie: is "Wibracja przy dotknięciu" offered? With it
   on, does a tap on Dalej vibrate?
12. **Files.** Ustawienia: download the library export and a backup. Do they land in Downloads
    with their names (`biblioteka.sursum`, `….sursum-backup`)? Upload a logo picture with the
    file picker. Optionally pick a `.sursum-system` file and cancel at the confirmation.
13. **Text size.** Set Android's font size to the largest. The remote must look the same as
    before (the app pins the WebView's text zoom to 100 %).
14. **Overnight.** Leave the tablet on the charger with the remote open overnight. In the
    morning: is it still connected, and did the battery or charger behave?

Report what failed and how, with the tablet model and the Android and WebView versions.
