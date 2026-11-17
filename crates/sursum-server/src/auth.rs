// SPDX-License-Identifier: GPL-3.0-or-later
//! Who may control the screen: remotes paired with the device PIN.
//!
//! The PIN is stored as a salted SHA-256 hash. A short PIN is weak against offline guessing
//! whatever the hash, so the real protection is that the database never leaves the device and
//! that wrong attempts are rate-limited per address. A paired device keeps a random 256-bit token
//! in an `HttpOnly; SameSite=Strict` cookie; only its hash is stored. Until a PIN is set, nobody
//! may control the screen: the first remote sets the PIN and is paired by doing so (or the
//! installer runs `sursum-server set-pin`). Only an explicit development flag
//! (`--open-without-pin`) lets everyone in without a PIN.
//!
//! Two levels of rights: a paired device is an **operator** (navigation, sets, songs). Entering
//! the PIN again on a paired device unlocks **admin** mode for [`ADMIN_SESSION`] (import, merge,
//! calibration, device list); it lives only in memory, so a restart ends it too. Revoking a
//! device removes its token at once and wakes its open WebSocket sessions to close them.

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::watch;

/// Name of the cookie that carries a device token.
pub const COOKIE: &str = "sursum_device";
/// Settings key of the PIN hash.
pub const PIN_SETTING: &str = "pin";
/// Wrong attempts allowed before an address is locked out.
const FREE_ATTEMPTS: u32 = 5;
/// First lockout; it doubles with every further wrong attempt (capped).
const FIRST_LOCKOUT: Duration = Duration::from_secs(30);
const MAX_LOCKOUT: Duration = Duration::from_secs(15 * 60);
/// Wrong attempts allowed from all addresses together within [`GLOBAL_WINDOW`]; beyond that every
/// PIN entry waits. Per-address limits alone are weak: a Wi-Fi guest can change address at will.
/// Paired remotes are not affected (they do not enter the PIN to work).
const GLOBAL_FAILURES: usize = 30;
const GLOBAL_WINDOW: Duration = Duration::from_secs(10 * 60);
/// How long admin mode stays unlocked after entering the PIN.
pub const ADMIN_SESSION: Duration = Duration::from_secs(15 * 60);

/// The stored PIN: a random salt and SHA-256(salt ‖ PIN), both hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PinHash {
    pub salt: String,
    pub hash: String,
}

impl PinHash {
    /// Hashes a new PIN with a fresh salt.
    pub fn new(pin: &str) -> anyhow::Result<Self> {
        let salt = random_hex(16)?;
        let hash = salted_hash(&salt, pin);
        Ok(PinHash { salt, hash })
    }

    /// True if `pin` is the stored PIN.
    pub fn matches(&self, pin: &str) -> bool {
        salted_hash(&self.salt, pin.trim()) == self.hash
    }
}

/// A PIN must be 6 to 12 digits: still easy on a tablet keypad, but a million combinations, so
/// guessing it within the rate limits takes months rather than an evening.
pub fn valid_pin(pin: &str) -> bool {
    (6..=12).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit())
}

/// A new device token (64 hex characters) and the hash to store.
pub fn new_token() -> anyhow::Result<(String, String)> {
    let token = random_hex(32)?;
    let hash = token_hash(&token);
    Ok((token, hash))
}

/// SHA-256 of a token, hex.
pub fn token_hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// The `Set-Cookie` value pairing a browser (valid ten years; the device list can revoke it).
pub fn cookie_header(token: &str) -> String {
    format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age=315360000")
}

/// What a request may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rights {
    /// Not paired: may only see the pairing form.
    None,
    /// A paired remote: navigation, sets, songs.
    Operator,
    /// Admin mode unlocked (or the development open mode): import, merge, calibration, device
    /// list.
    Admin,
}

/// Access rules shared by all requests.
pub struct Access {
    pin: RwLock<Option<PinHash>>,
    /// Development and tests only: without a PIN everyone is admin.
    open_without_pin: bool,
    tokens: RwLock<HashSet<String>>,
    /// Token hash → end of its admin session.
    admin_until: Mutex<HashMap<String, Instant>>,
    attempts: Mutex<HashMap<Attempter, Attempts>>,
    /// Times of recent wrong attempts from any address, for [`GLOBAL_FAILURES`].
    recent_failures: Mutex<std::collections::VecDeque<Instant>>,
    /// Bumped on every revocation so open sessions re-check their device.
    revocations: watch::Sender<u64>,
}

/// Who is entering a PIN: an address (pairing) or a paired device (unlocking admin mode).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Attempter {
    Address(IpAddr),
    Device(String),
}

#[derive(Debug, Clone, Copy)]
struct Attempts {
    failures: u32,
    locked_until: Option<Instant>,
}

/// Result of checking a PIN attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinCheck {
    Correct,
    Wrong,
    Locked(Duration),
}

impl Access {
    /// Rules from the stored PIN and paired devices. `open_without_pin` is the development mode
    /// in which, as long as no PIN is set, every device is admin.
    pub fn new(pin: Option<PinHash>, token_hashes: Vec<String>, open_without_pin: bool) -> Self {
        Access {
            pin: RwLock::new(pin),
            open_without_pin,
            tokens: RwLock::new(token_hashes.into_iter().collect()),
            admin_until: Mutex::new(HashMap::new()),
            attempts: Mutex::new(HashMap::new()),
            recent_failures: Mutex::new(std::collections::VecDeque::new()),
            revocations: watch::Sender::new(0),
        }
    }

    /// What a request with these headers may do at `now`.
    pub fn rights(&self, headers: &HeaderMap, now: Instant) -> Rights {
        if !self.pin_required() {
            return if self.open_without_pin { Rights::Admin } else { Rights::None };
        }
        let Some(hash) = self.device(headers) else { return Rights::None };
        let admin =
            self.admin_until.lock().expect("not poisoned").get(&hash).is_some_and(|until| *until > now);
        if admin { Rights::Admin } else { Rights::Operator }
    }

    /// How long admin mode still lasts for the device sending these headers (`None` when it is
    /// off, or when the open development mode makes everyone admin for good).
    pub fn admin_remaining(&self, headers: &HeaderMap, now: Instant) -> Option<Duration> {
        let hash = self.device(headers)?;
        let until = *self.admin_until.lock().expect("not poisoned").get(&hash)?;
        until.checked_duration_since(now).filter(|left| !left.is_zero())
    }

    /// The token hash of the paired device sending these headers, if it is paired.
    pub fn device(&self, headers: &HeaderMap) -> Option<String> {
        let hash = token_hash(cookie_token(headers)?);
        self.tokens.read().expect("not poisoned").contains(&hash).then_some(hash)
    }

    /// Unlocks admin mode for a device (after its PIN was checked) until `now + ADMIN_SESSION`.
    pub fn grant_admin(&self, device: String, now: Instant) {
        self.admin_until.lock().expect("not poisoned").insert(device, now + ADMIN_SESSION);
    }

    /// Ends admin mode for a device.
    pub fn drop_admin(&self, device: &str) {
        self.admin_until.lock().expect("not poisoned").remove(device);
    }

    /// Forgets a device: its requests are refused from now on and its sessions close.
    pub fn revoke(&self, device: &str) {
        self.tokens.write().expect("not poisoned").remove(device);
        self.drop_admin(device);
        self.revocations.send_modify(|generation| *generation += 1);
    }

    /// Changes whenever a device is revoked.
    pub fn revocations(&self) -> watch::Receiver<u64> {
        self.revocations.subscribe()
    }

    /// True once a PIN is set.
    pub fn pin_required(&self) -> bool {
        self.pin.read().expect("not poisoned").is_some()
    }

    /// True while no PIN is set and the server is not in the open development mode: the first
    /// remote has to set the PIN.
    pub fn setup_needed(&self) -> bool {
        !self.pin_required() && !self.open_without_pin
    }

    /// Makes `pin` the PIN if none is set yet; false if one was set meanwhile (two remotes doing
    /// the first setup at once: only one wins).
    pub fn claim_pin(&self, pin: PinHash) -> bool {
        let mut stored = self.pin.write().expect("not poisoned");
        if stored.is_some() {
            return false;
        }
        *stored = Some(pin);
        true
    }

    /// Undoes [`Access::claim_pin`] when the PIN could not be saved.
    pub fn release_pin(&self) {
        *self.pin.write().expect("not poisoned") = None;
    }

    /// True if the request may control the screen (operator or admin).
    pub fn allows(&self, headers: &HeaderMap) -> bool {
        self.rights(headers, Instant::now()) >= Rights::Operator
    }

    /// Checks a PIN attempt from `address`, counting failures for the lockout.
    pub fn check_pin(&self, address: IpAddr, pin: &str, now: Instant) -> PinCheck {
        let mut recent = self.recent_failures.lock().expect("not poisoned");
        while recent.front().is_some_and(|time| now.saturating_duration_since(*time) >= GLOBAL_WINDOW) {
            recent.pop_front();
        }
        if recent.len() >= GLOBAL_FAILURES
            && let Some(oldest) = recent.front()
        {
            return PinCheck::Locked(GLOBAL_WINDOW.saturating_sub(now.saturating_duration_since(*oldest)));
        }
        let check = self.check(&Attempter::Address(address), pin, now);
        if check != PinCheck::Correct {
            recent.push_back(now);
        }
        check
    }

    /// Checks the PIN entered on a paired device to unlock admin mode. Counted per device and
    /// outside the global budget: guests guessing at the pairing form must not lock the parish's
    /// own tablets out of admin mode.
    pub fn check_admin_pin(&self, device: &str, pin: &str, now: Instant) -> PinCheck {
        self.check(&Attempter::Device(device.to_owned()), pin, now)
    }

    /// One attempt with the per-attempter lockout (doubling after [`FREE_ATTEMPTS`]).
    fn check(&self, attempter: &Attempter, pin: &str, now: Instant) -> PinCheck {
        let mut attempts = self.attempts.lock().expect("not poisoned");
        let entry = attempts.entry(attempter.clone()).or_insert(Attempts { failures: 0, locked_until: None });
        if let Some(until) = entry.locked_until.filter(|until| *until > now) {
            return PinCheck::Locked(until - now);
        }
        let correct =
            self.pin.read().expect("not poisoned").as_ref().is_some_and(|stored| stored.matches(pin));
        if correct {
            attempts.remove(attempter);
            return PinCheck::Correct;
        }
        entry.failures += 1;
        if entry.failures >= FREE_ATTEMPTS {
            let doublings = (entry.failures - FREE_ATTEMPTS).min(10);
            let lockout = (FIRST_LOCKOUT * 2_u32.pow(doublings)).min(MAX_LOCKOUT);
            entry.locked_until = Some(now + lockout);
            return PinCheck::Locked(lockout);
        }
        PinCheck::Wrong
    }

    /// Accepts a newly paired device's token hash.
    pub fn add_token(&self, hash: String) {
        self.tokens.write().expect("not poisoned").insert(hash);
    }
}

/// The device token from the `Cookie` header, if any.
fn cookie_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all("cookie")
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value)
}

fn salted_hash(salt: &str, pin: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(pin.as_bytes());
    hex(&hasher.finalize())
}

fn random_hex(bytes: usize) -> anyhow::Result<String> {
    let mut buffer = vec![0_u8; bytes];
    getrandom::fill(&mut buffer).map_err(|error| anyhow::anyhow!("no system randomness: {error}"))?;
    Ok(hex(&buffer))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(cookie: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", cookie.parse().unwrap());
        headers
    }

    #[test]
    fn pins_are_digits_and_hashes_match() {
        assert!(valid_pin("123456") && !valid_pin("12a456") && !valid_pin("1234"));
        let stored = PinHash::new("2468").unwrap();
        assert!(stored.matches("2468") && stored.matches(" 2468 ") && !stored.matches("2469"));
        assert_ne!(PinHash::new("2468").unwrap().salt, stored.salt, "salts are random");
    }

    #[test]
    fn without_a_pin_nobody_may_control_until_the_first_remote_sets_one() {
        let access = Access::new(None, vec![], false);
        assert!(access.setup_needed() && !access.allows(&HeaderMap::new()));
        assert!(access.claim_pin(PinHash::new("246813").unwrap()));
        assert!(!access.claim_pin(PinHash::new("135792").unwrap()), "only the first setup wins");
        assert!(!access.setup_needed() && access.pin_required());
        let open = Access::new(None, vec![], true);
        assert!(!open.setup_needed() && open.allows(&HeaderMap::new()), "development mode");
    }

    #[test]
    fn with_a_pin_only_paired_devices_may_control() {
        let (token, hash) = new_token().unwrap();
        let access = Access::new(Some(PinHash::new("1234").unwrap()), vec![], false);
        assert!(!access.allows(&headers(&format!("{COOKIE}={token}"))));
        access.add_token(hash);
        assert!(access.allows(&headers(&format!("other=1; {COOKIE}={token}"))));
        assert!(!access.allows(&headers(&format!("{COOKIE}=forged"))));
    }

    #[test]
    fn changing_addresses_does_not_escape_the_global_limit() {
        let access = Access::new(Some(PinHash::new("246813").unwrap()), vec![], false);
        let now = Instant::now();
        for host in 1..=GLOBAL_FAILURES {
            let address = IpAddr::from([192, 168, 1, u8::try_from(host).unwrap()]);
            assert_eq!(access.check_pin(address, "000000", now), PinCheck::Wrong);
        }
        let fresh: IpAddr = "192.168.1.200".parse().unwrap();
        // The parish's own paired tablet can still unlock admin mode.
        assert_eq!(access.check_admin_pin("tablet", "246813", now), PinCheck::Correct);
        assert!(
            matches!(access.check_pin(fresh, "246813", now), PinCheck::Locked(_)),
            "even the right PIN waits"
        );
        let later = now + GLOBAL_WINDOW;
        assert_eq!(access.check_pin(fresh, "246813", later), PinCheck::Correct);
    }

    #[test]
    fn admin_mode_is_per_device_expires_and_ends_on_revocation() {
        let (token, hash) = new_token().unwrap();
        let access = Access::new(Some(PinHash::new("1234").unwrap()), vec![hash.clone()], false);
        let request = headers(&format!("{COOKIE}={token}"));
        let now = Instant::now();
        assert_eq!(access.rights(&request, now), Rights::Operator);
        access.grant_admin(hash.clone(), now);
        assert_eq!(access.rights(&request, now + Duration::from_secs(60)), Rights::Admin);
        assert_eq!(access.rights(&request, now + ADMIN_SESSION), Rights::Operator, "expired");
        let minute = Duration::from_secs(60);
        assert_eq!(access.admin_remaining(&request, now + minute), Some(ADMIN_SESSION - minute));
        assert_eq!(access.admin_remaining(&request, now + ADMIN_SESSION), None);
        assert_eq!(access.rights(&headers("x=1"), now), Rights::None);

        let revocations = access.revocations();
        access.revoke(&hash);
        assert!(revocations.has_changed().unwrap());
        assert_eq!(access.rights(&request, now), Rights::None);
        assert_eq!(Access::new(None, vec![], true).rights(&HeaderMap::new(), now), Rights::Admin);
    }

    #[test]
    fn wrong_attempts_lock_the_address_out_with_doubling_time() {
        let access = Access::new(Some(PinHash::new("1234").unwrap()), vec![], false);
        let address: IpAddr = "192.168.1.20".parse().unwrap();
        let now = Instant::now();
        for _ in 0..FREE_ATTEMPTS - 1 {
            assert_eq!(access.check_pin(address, "0000", now), PinCheck::Wrong);
        }
        assert_eq!(access.check_pin(address, "0000", now), PinCheck::Locked(FIRST_LOCKOUT));
        // Even the right PIN is refused while locked.
        assert!(matches!(access.check_pin(address, "1234", now), PinCheck::Locked(_)));
        let later = now + FIRST_LOCKOUT + Duration::from_secs(1);
        assert_eq!(access.check_pin(address, "0000", later), PinCheck::Locked(FIRST_LOCKOUT * 2));
        let much_later = later + MAX_LOCKOUT;
        assert_eq!(access.check_pin(address, "1234", much_later), PinCheck::Correct);
        // Another address is not affected.
        assert_eq!(access.check_pin("192.168.1.21".parse().unwrap(), "1234", now), PinCheck::Correct);
    }
}
