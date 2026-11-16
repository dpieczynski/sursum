// SPDX-License-Identifier: GPL-3.0-or-later
//! Who may control the screen: remotes paired with the device PIN.
//!
//! The PIN is stored as a salted SHA-256 hash. A short PIN is weak against offline guessing
//! whatever the hash, so the real protection is that the database never leaves the device and
//! that wrong attempts are rate-limited per address. A paired device keeps a random 256-bit token
//! in an `HttpOnly; SameSite=Strict` cookie; only its hash is stored. Until a PIN is set, control
//! is open (first setup and development).

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::{Mutex, RwLock};
use std::time::{Duration, Instant};

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Name of the cookie that carries a device token.
pub const COOKIE: &str = "sursum_device";
/// Settings key of the PIN hash.
pub const PIN_SETTING: &str = "pin";
/// Wrong attempts allowed before an address is locked out.
const FREE_ATTEMPTS: u32 = 5;
/// First lockout; it doubles with every further wrong attempt (capped).
const FIRST_LOCKOUT: Duration = Duration::from_secs(30);
const MAX_LOCKOUT: Duration = Duration::from_secs(15 * 60);

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

/// A PIN must be 4 to 12 digits (easy on a tablet keypad).
pub fn valid_pin(pin: &str) -> bool {
    (4..=12).contains(&pin.len()) && pin.chars().all(|c| c.is_ascii_digit())
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

/// Access rules shared by all requests.
pub struct Access {
    pin: RwLock<Option<PinHash>>,
    tokens: RwLock<HashSet<String>>,
    attempts: Mutex<HashMap<IpAddr, Attempts>>,
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
    /// Rules from the stored PIN and paired devices.
    pub fn new(pin: Option<PinHash>, token_hashes: Vec<String>) -> Self {
        Access {
            pin: RwLock::new(pin),
            tokens: RwLock::new(token_hashes.into_iter().collect()),
            attempts: Mutex::new(HashMap::new()),
        }
    }

    /// True once a PIN is set.
    pub fn pin_required(&self) -> bool {
        self.pin.read().expect("not poisoned").is_some()
    }

    /// True if the request may control the screen.
    pub fn allows(&self, headers: &HeaderMap) -> bool {
        if !self.pin_required() {
            return true;
        }
        cookie_token(headers)
            .is_some_and(|token| self.tokens.read().expect("not poisoned").contains(&token_hash(token)))
    }

    /// Checks a PIN attempt from `address`, counting failures for the lockout.
    pub fn check_pin(&self, address: IpAddr, pin: &str, now: Instant) -> PinCheck {
        let mut attempts = self.attempts.lock().expect("not poisoned");
        let entry = attempts.entry(address).or_insert(Attempts { failures: 0, locked_until: None });
        if let Some(until) = entry.locked_until.filter(|until| *until > now) {
            return PinCheck::Locked(until - now);
        }
        let correct =
            self.pin.read().expect("not poisoned").as_ref().is_some_and(|stored| stored.matches(pin));
        if correct {
            attempts.remove(&address);
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
        assert!(valid_pin("1234") && !valid_pin("12a4") && !valid_pin("123"));
        let stored = PinHash::new("2468").unwrap();
        assert!(stored.matches("2468") && stored.matches(" 2468 ") && !stored.matches("2469"));
        assert_ne!(PinHash::new("2468").unwrap().salt, stored.salt, "salts are random");
    }

    #[test]
    fn without_a_pin_everyone_may_control() {
        assert!(Access::new(None, vec![]).allows(&HeaderMap::new()));
    }

    #[test]
    fn with_a_pin_only_paired_devices_may_control() {
        let (token, hash) = new_token().unwrap();
        let access = Access::new(Some(PinHash::new("1234").unwrap()), vec![]);
        assert!(!access.allows(&headers(&format!("{COOKIE}={token}"))));
        access.add_token(hash);
        assert!(access.allows(&headers(&format!("other=1; {COOKIE}={token}"))));
        assert!(!access.allows(&headers(&format!("{COOKIE}=forged"))));
    }

    #[test]
    fn wrong_attempts_lock_the_address_out_with_doubling_time() {
        let access = Access::new(Some(PinHash::new("1234").unwrap()), vec![]);
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
