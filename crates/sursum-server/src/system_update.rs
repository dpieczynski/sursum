// SPDX-License-Identifier: GPL-3.0-or-later
//! Signed system updates (`.sursum-system`, ADR 0019): replacing the read-only system of an imaged
//! device from the remote, without the internet and without reflashing, with the Raspberry Pi
//! firmware's `tryboot` as the safety net. Since ADR 0022 this is the only update path: the
//! server itself is part of the system (`/usr/lib/sursum/sursum-server`), so one update brings a
//! new server, kiosk, browser and kernel together, and a device has one version.
//!
//! A file is a ZIP with:
//! - `manifest.json`: format, version (the image's build id) and the size and SHA-256 of each image;
//! - `manifest.sig`: the Ed25519 signature of exactly those manifest bytes ([`crate::signing`]);
//! - `boot.img` and `system.img`: the raw boot (vfat) and system (erofs) partition images.
//!
//! The flow, from the remote's upload to the next boot:
//! 1. The server (unprivileged) receives the file into `system-update/` in the data directory,
//!    [`verify`]s it, copies the database (`system-update/before-<version>.db`, outside the
//!    rotating `backups/`, so a crash-looping trial cannot push it out) and notes its schema,
//!    renames the file to `ready.sursum-system` and asks for the install by creating [`REQUEST`].
//! 2. A root unit runs the read-only system's server (so a compromised server process cannot
//!    plant a system): [`apply`] verifies again, writes both images to the **other** slot,
//!    records the trial and points `[tryboot]` at that slot; the unit then reboots with
//!    `0 tryboot`.
//! 3. At the next boot, before the server starts, [`trial_state`] tells whether the device came
//!    back to the old slot: then the new system failed, and [`end_failed_trial`] puts the database
//!    copy back if the new server had raised the schema (the old one cannot open it) and records
//!    the rollback. In the new slot, a root unit waits after the server's start until the server
//!    and the kiosk are healthy and [`commit`]s by making that slot the default, or reboots,
//!    which brings the old slot back (tryboot lasts one boot only).

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, ensure};
use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sursum_protocol::{LastSystemUpdate, SystemStatus};

use crate::db::Database;
use crate::signing::{from_hex, hex, valid_version};

/// The running system's version, written into the image at build time.
pub const SYSTEM_RELEASE: &str = "/etc/sursum/system-release";
/// Created by the server to ask the root unit to install the ready update.
pub const REQUEST: &str = "/run/sursum/system-update";
/// Largest `.sursum-system` accepted (two slots' worth of partitions).
pub const MAX_SYSTEM_BYTES: u64 = 4 * 1024 * 1024 * 1024;

const FORMAT: u32 = 1;
const DIR: &str = "system-update";
const INCOMING: &str = "incoming.part";
const READY: &str = "ready.sursum-system";
/// What the server noted about the ready update ([`Ready`]), written next to it.
const READY_NOTE: &str = "ready.json";
const TRIAL: &str = "trial.json";
const OUTCOME: &str = "outcome.json";
const MANIFEST: &str = "manifest.json";
const SIGNATURE: &str = "manifest.sig";
const BOOT: &str = "boot.img";
const SYSTEM: &str = "system.img";
/// Databases a failed system had migrated are kept here after the rollback, never deleted.
const ROLLED_BACK_DIR: &str = "rolled-back";

/// What a system update says about itself (signed).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SystemManifest {
    pub format: u32,
    pub version: String,
    pub boot_bytes: u64,
    pub boot_sha256: String,
    pub system_bytes: u64,
    pub system_sha256: String,
}

/// Why an uploaded system update is not installed.
#[derive(Debug, thiserror::Error)]
pub enum SystemUpdateError {
    #[error("not a sursum system update, damaged, or not signed with this device's key: {0:#}")]
    BadFile(anyhow::Error),
    #[error("this system already runs")]
    AlreadyCurrent,
    #[error("{0:#}")]
    Failed(anyhow::Error),
}

/// What the server noted when it staged an update, for the trial [`apply`] records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Ready {
    version: String,
    /// The database copy taken just before the update, relative to the data directory.
    backup: Option<PathBuf>,
    /// The database schema the running system's server uses.
    schema_before: i64,
}

/// The trial of a newly written system, read at the next boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Trial {
    version: String,
    /// PARTLABEL of the boot partition the new system was written to, e.g. `boot_b`.
    slot: String,
    /// The database copy taken before the update, relative to the data directory.
    backup: Option<PathBuf>,
    /// The schema before the update: a rollback puts the copy back if the new server raised it.
    schema_before: i64,
}

/// Where the device keeps its A/B slots; [`Slots::device`] on a Pi, fake trees in tests.
#[derive(Debug, Clone)]
pub struct Slots {
    /// `/dev/disk/by-slot` with `active/` and `other/`, each holding `boot` and `system`.
    pub by_slot: PathBuf,
    /// `/sys/class/block`, for partition numbers and labels.
    pub sys_block: PathBuf,
    /// The mounted `bootconfig` partition holding `autoboot.txt`.
    pub bootfs: PathBuf,
}

impl Slots {
    /// The real paths on a device built with rpi-image-gen's `image-rota` layout.
    pub fn device() -> Self {
        Slots {
            by_slot: "/dev/disk/by-slot".into(),
            sys_block: "/sys/class/block".into(),
            bootfs: "/bootfs".into(),
        }
    }

    /// The device node of `which` (`active` or `other`) and `part` (`boot` or `system`).
    fn device_of(&self, which: &str, part: &str) -> PathBuf {
        self.by_slot.join(which).join(part)
    }

    /// Partition number and label of a slot's boot partition.
    fn boot_partition(&self, which: &str) -> anyhow::Result<(u32, String)> {
        let device = std::fs::canonicalize(self.device_of(which, "boot"))
            .with_context(|| format!("resolving the {which} boot partition"))?;
        let name = device.file_name().context("no device name")?.to_string_lossy().into_owned();
        let sys = self.sys_block.join(&name);
        let number = std::fs::read_to_string(sys.join("partition"))?.trim().parse()?;
        let uevent = std::fs::read_to_string(sys.join("uevent"))?;
        let label = uevent
            .lines()
            .find_map(|line| line.strip_prefix("PARTNAME="))
            .with_context(|| format!("{name} has no partition label"))?;
        Ok((number, label.to_owned()))
    }

    /// Makes `default` the partition the firmware starts, and `tryboot` the one a
    /// `reboot "0 tryboot"` starts once. Written atomically (the file is tiny).
    fn write_autoboot(&self, default: u32, tryboot: u32) -> anyhow::Result<()> {
        let text =
            format!("[all]\ntryboot_a_b=1\nboot_partition={default}\n[tryboot]\nboot_partition={tryboot}\n");
        let staged = self.bootfs.join("autoboot.txt.new");
        let mut file = std::fs::File::create(&staged)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        std::fs::rename(&staged, self.bootfs.join("autoboot.txt"))?;
        if let Ok(dir) = std::fs::File::open(&self.bootfs) {
            let _ = dir.sync_all();
        }
        Ok(())
    }
}

/// Writes a `.sursum-system` from the raw partition images of an image build.
pub fn create(
    boot: &Path,
    system: &Path,
    version: &str,
    key: &SigningKey,
    out: impl Write + Seek,
) -> anyhow::Result<SystemManifest> {
    ensure!(valid_version(version), "invalid version {version:?} (letters, digits, '.', '+', '-')");
    let (boot_bytes, boot_sha256) = digest_file(boot)?;
    let (system_bytes, system_sha256) = digest_file(system)?;
    let manifest = SystemManifest {
        format: FORMAT,
        version: version.to_owned(),
        boot_bytes,
        boot_sha256,
        system_bytes,
        system_sha256,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let signature = key.sign(&manifest_bytes);
    let mut zip = zip::ZipWriter::new(out);
    let deflated =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(MANIFEST, deflated)?;
    zip.write_all(&manifest_bytes)?;
    zip.start_file(SIGNATURE, deflated)?;
    zip.write_all(hex(&signature.to_bytes()).as_bytes())?;
    // The boot partition is mostly empty; the erofs system image is compressed already.
    zip.start_file(BOOT, deflated.large_file(true))?;
    std::io::copy(&mut std::fs::File::open(boot)?, &mut zip)?;
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    zip.start_file(SYSTEM, stored)?;
    std::io::copy(&mut std::fs::File::open(system)?, &mut zip)?;
    zip.finish()?;
    Ok(manifest)
}

/// Checks the signature with `trusted`, then streams both images to check their sizes and hashes.
pub fn verify(path: &Path, trusted: &VerifyingKey) -> anyhow::Result<SystemManifest> {
    use crate::limits::read_capped;
    let mut archive =
        zip::ZipArchive::new(std::fs::File::open(path)?).context("not a .sursum-system file")?;
    let manifest_bytes =
        read_capped(archive.by_name(MANIFEST).context("no manifest")?, 64 * 1024, "manifest")?;
    let signature_hex = read_capped(archive.by_name(SIGNATURE).context("no signature")?, 1024, "signature")?;
    let signature = std::str::from_utf8(&signature_hex)
        .ok()
        .and_then(|text| from_hex::<64>(text.trim()))
        .context("malformed signature")?;
    trusted
        .verify_strict(&manifest_bytes, &Signature::from_bytes(&signature))
        .context("the system update is not signed with this device's key")?;
    // Only now, with a genuine manifest, is anything else read.
    let manifest: SystemManifest = serde_json::from_slice(&manifest_bytes)?;
    ensure!(manifest.format == FORMAT, "unsupported system update format {}", manifest.format);
    ensure!(valid_version(&manifest.version), "invalid version in the manifest");
    for (entry, bytes, sha256) in [
        (BOOT, manifest.boot_bytes, &manifest.boot_sha256),
        (SYSTEM, manifest.system_bytes, &manifest.system_sha256),
    ] {
        let mut reader = archive.by_name(entry).with_context(|| format!("no {entry}"))?;
        let (read, digest) = digest(&mut (&mut reader).take(bytes + 1), std::io::sink())?;
        ensure!(read == bytes && &digest == sha256, "{entry} does not match the manifest");
    }
    Ok(manifest)
}

/// Writes both images of a verified update to `boot` and `system` (the other slot's partitions),
/// checking each write against the manifest.
fn write_images(path: &Path, manifest: &SystemManifest, boot: &Path, system: &Path) -> anyhow::Result<()> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path)?)?;
    for (entry, device, bytes, sha256) in [
        (BOOT, boot, manifest.boot_bytes, &manifest.boot_sha256),
        (SYSTEM, system, manifest.system_bytes, &manifest.system_sha256),
    ] {
        let mut target = std::fs::OpenOptions::new()
            .write(true)
            .open(device)
            .with_context(|| format!("opening {}", device.display()))?;
        let room = target.seek(std::io::SeekFrom::End(0))?;
        // A regular file (tests) has no fixed size; a partition must hold the image.
        ensure!(
            room == 0 || room >= bytes,
            "{entry} ({bytes} bytes) does not fit {} ({room})",
            device.display()
        );
        target.seek(std::io::SeekFrom::Start(0))?;
        let mut reader = archive.by_name(entry)?;
        let (written, digest) = digest(&mut (&mut reader).take(bytes + 1), &mut target)?;
        ensure!(written == bytes && &digest == sha256, "{entry} changed while it was written");
        target.sync_all()?;
    }
    Ok(())
}

/// Root, from the system-update unit: verifies the ready update with this binary's key, writes
/// it to the other slot, records the trial and points `[tryboot]` at that slot. The caller
/// reboots with `0 tryboot`. A failure is recorded as rolled back and leaves the system as it was.
pub fn apply(data_dir: &Path, trusted: &VerifyingKey, slots: &Slots) -> anyhow::Result<String> {
    let dir = data_dir.join(DIR);
    let ready = dir.join(READY);
    let mut version = None;
    let result = (|| {
        let manifest = verify(&ready, trusted)?;
        version = Some(manifest.version.clone());
        let note = read_json::<Ready>(&dir, READY_NOTE)?.context("the staged update has no note")?;
        ensure!(note.version == manifest.version, "the staged update's note names another version");
        let (active, _) = slots.boot_partition("active")?;
        let (other, label) = slots.boot_partition("other")?;
        write_images(
            &ready,
            &manifest,
            &slots.device_of("other", "boot"),
            &slots.device_of("other", "system"),
        )?;
        let trial = Trial {
            version: manifest.version.clone(),
            slot: label,
            backup: note.backup,
            schema_before: note.schema_before,
        };
        write_json(&dir, TRIAL, &trial)?;
        slots.write_autoboot(active, other)?;
        Ok(manifest.version)
    })();
    let _ = std::fs::remove_file(&ready);
    let _ = std::fs::remove_file(dir.join(READY_NOTE));
    if result.is_err() {
        // The update did not go on trial: no trial may be left to be ended at the next boot,
        // and the database copy is not needed.
        let _ = std::fs::remove_file(dir.join(TRIAL));
        let _ = remove_database_copies(data_dir);
        let failed = version.unwrap_or_else(|| "the uploaded update".into());
        write_json(&dir, OUTCOME, &LastSystemUpdate::RolledBack { failed, database_restored: false })?;
    }
    result
}

/// What the boot-time check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrialState {
    /// No system update is on trial.
    None,
    /// The device started the old slot again: the new system did not come up healthy (or did not
    /// boot at all). [`end_failed_trial`] before the server starts.
    BootedBack(String),
    /// The new system runs on trial; [`commit`] once it is healthy.
    OnTrial(String),
}

/// At boot: is a new system on trial, and did the device start it? Changes nothing.
pub fn trial_state(data_dir: &Path, slots: &Slots) -> anyhow::Result<TrialState> {
    let dir = data_dir.join(DIR);
    let Some(trial) = read_json::<Trial>(&dir, TRIAL)? else { return Ok(TrialState::None) };
    let (_, active) = slots.boot_partition("active")?;
    Ok(if active == trial.slot {
        TrialState::OnTrial(trial.version)
    } else {
        TrialState::BootedBack(trial.version)
    })
}

/// Before the server starts, back in the old slot after a failed trial (ADR 0022): if the new
/// system's server had raised the database schema, the old server could not open it, so the
/// migrated database is moved to `rolled-back/` and the copy taken before the update is put back
/// (changes made during the trial are lost; the remote says so). Records the rollback and ends
/// the trial. Runs as the server's user, so every file it writes stays the server's.
pub fn end_failed_trial(data_dir: &Path) -> anyhow::Result<LastSystemUpdate> {
    let dir = data_dir.join(DIR);
    let trial = read_json::<Trial>(&dir, TRIAL)?.context("no system update on trial")?;
    let database_restored = restore_database_if_migrated(data_dir, &trial)?;
    let outcome = LastSystemUpdate::RolledBack { failed: trial.version.clone(), database_restored };
    write_json(&dir, OUTCOME, &outcome)?;
    std::fs::remove_file(dir.join(TRIAL))?;
    // Only space is at stake: a copy left behind is removed with the next update.
    let _ = remove_database_copies(data_dir);
    Ok(outcome)
}

/// Puts the pre-update copy back if the schema is newer than before the update; true if it did.
fn restore_database_if_migrated(data_dir: &Path, trial: &Trial) -> anyhow::Result<bool> {
    if Database::schema_version(data_dir)? <= trial.schema_before {
        return Ok(false);
    }
    let backup = trial
        .backup
        .as_ref()
        .map(|backup| data_dir.join(backup))
        .filter(|backup| backup.is_file())
        .context("the new system changed the database and its pre-update copy is gone")?;
    let aside = data_dir.join(ROLLED_BACK_DIR);
    std::fs::create_dir_all(&aside)?;
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    for suffix in ["", "-wal", "-shm"] {
        let file = data_dir.join(format!("{}{suffix}", crate::db::DATABASE_FILE));
        if file.exists() {
            std::fs::rename(&file, aside.join(format!("{stamp}-{}{suffix}", crate::db::DATABASE_FILE)))?;
        }
    }
    let staged = data_dir.join(format!(".{}.restoring", crate::db::DATABASE_FILE));
    std::fs::copy(&backup, &staged)?;
    std::fs::File::open(&staged)?.sync_all()?;
    std::fs::rename(&staged, data_dir.join(crate::db::DATABASE_FILE))?;
    crate::durable::sync_dir(data_dir)?;
    Ok(true)
}

/// Root, once the system on trial is healthy: makes its slot the one the firmware starts.
pub fn commit(data_dir: &Path, slots: &Slots) -> anyhow::Result<String> {
    let dir = data_dir.join(DIR);
    let trial = read_json::<Trial>(&dir, TRIAL)?.context("no system update on trial")?;
    let (active, label) = slots.boot_partition("active")?;
    ensure!(label == trial.slot, "the system on trial is not running");
    let (other, _) = slots.boot_partition("other")?;
    slots.write_autoboot(active, other)?;
    write_json(&dir, OUTCOME, &LastSystemUpdate::Committed { version: trial.version.clone() })?;
    std::fs::remove_file(dir.join(TRIAL))?;
    let _ = remove_database_copies(data_dir);
    Ok(trial.version)
}

/// The server's side: whether system updates can be installed here, and the uploaded file's fate.
#[derive(Clone)]
pub struct SystemUpdates {
    pub key: Option<VerifyingKey>,
    /// The running system's version, or `None` off an imaged device (no [`SYSTEM_RELEASE`]).
    pub version: Option<String>,
    pub supervised: bool,
    pub request: PathBuf,
    /// The device's slots, to tell an update about to restart from one running on trial.
    pub slots: Slots,
}

impl SystemUpdates {
    /// For a server that runs on an imaged device if its `release` file (normally
    /// [`SYSTEM_RELEASE`]) says so; `request` is normally [`REQUEST`].
    pub fn detect(key: Option<VerifyingKey>, supervised: bool, release: &Path, request: &Path) -> Self {
        let version = std::fs::read_to_string(release).ok().map(|text| text.trim().to_owned());
        SystemUpdates {
            key,
            version: version.filter(|version| !version.is_empty()),
            supervised,
            request: request.to_owned(),
            slots: Slots::device(),
        }
    }

    /// The remote's view; `None` off an imaged device.
    pub fn status(&self, data_dir: &Path) -> Option<SystemStatus> {
        let version = self.version.clone()?;
        let dir = data_dir.join(DIR);
        let trial = read_json::<Trial>(&dir, TRIAL).ok().flatten();
        // Reading the by-slot links needs no privileges.
        let active = self.slots.boot_partition("active").ok().map(|(_, label)| label);
        let on_trial = trial.as_ref().is_some_and(|trial| active.as_deref() == Some(trial.slot.as_str()));
        let installing = dir.join(READY).exists() || (trial.is_some() && !on_trial);
        let target = match &trial {
            Some(trial) => Some(trial.version.clone()),
            None => read_json::<Ready>(&dir, READY_NOTE).ok().flatten().map(|note| note.version),
        };
        Some(SystemStatus {
            version,
            on_trial,
            installing,
            target: target.filter(|_| installing || on_trial),
            last: read_json(&dir, OUTCOME).ok().flatten(),
            can_install: self.key.is_some() && self.supervised && !installing && !on_trial,
        })
    }

    /// Where an upload is received (in the data directory, which has the room).
    pub fn incoming(&self, data_dir: &Path) -> std::io::Result<PathBuf> {
        let dir = data_dir.join(DIR);
        std::fs::create_dir_all(&dir)?;
        Ok(dir.join(INCOMING))
    }

    /// Verifies a received upload, copies the database and hands the update to the root unit.
    /// Returns the new version. The copy goes through a plain connection (no migration, no
    /// repair), so it is safe while the server runs.
    pub fn stage(&self, data_dir: &Path, incoming: &Path) -> Result<String, SystemUpdateError> {
        let key = self.key.as_ref().ok_or_else(|| SystemUpdateError::Failed(anyhow::anyhow!("no key")))?;
        let manifest = verify(incoming, key).map_err(SystemUpdateError::BadFile)?;
        if self.version.as_deref() == Some(manifest.version.as_str()) {
            return Err(SystemUpdateError::AlreadyCurrent);
        }
        let dir = data_dir.join(DIR);
        let backup = Path::new(DIR).join(format!("before-{}.db", manifest.version));
        let staged = (|| -> anyhow::Result<()> {
            remove_database_copies(data_dir)?;
            let copied = Database::copy_to(data_dir, &data_dir.join(&backup))?;
            let note = Ready {
                version: manifest.version.clone(),
                backup: copied.then(|| backup.clone()),
                schema_before: Database::schema_version(data_dir)?,
            };
            write_json(&dir, READY_NOTE, &note)?;
            std::fs::rename(incoming, dir.join(READY))?;
            std::fs::write(&self.request, b"")?;
            Ok(())
        })();
        if let Err(error) = staged {
            // Nothing half-staged may stay: a ready file without its request would count as
            // "installing" forever and block every later update.
            let _ = std::fs::remove_file(dir.join(READY));
            let _ = std::fs::remove_file(dir.join(READY_NOTE));
            let _ = remove_database_copies(data_dir);
            return Err(SystemUpdateError::Failed(error));
        }
        Ok(manifest.version)
    }
}

fn digest_file(path: &Path) -> anyhow::Result<(u64, String)> {
    let mut file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    digest(&mut file, std::io::sink()).map_err(Into::into)
}

/// Copies `reader` to `writer` and returns the byte count and the SHA-256 in hex.
fn digest(reader: &mut impl Read, mut writer: impl Write) -> std::io::Result<(u64, String)> {
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    let mut total = 0;
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        writer.write_all(&buffer[..read])?;
        total += read as u64;
    }
    Ok((total, hex(&hasher.finalize())))
}

/// Writes a note atomically and durably. The temporary name is unique: root (`apply`, `commit`)
/// and the server's user (`stage`, `end_failed_trial`) write here, and a leftover of one must not
/// stop the other (a root-owned file cannot be truncated by the server's user).
fn write_json(dir: &Path, name: &str, value: &impl Serialize) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let staged = dir.join(format!(".{name}.{}.new", uuid::Uuid::new_v4().simple()));
    let written = (|| -> anyhow::Result<()> {
        let mut file = std::fs::File::create_new(&staged)?;
        file.write_all(&serde_json::to_vec_pretty(value)?)?;
        file.sync_all()?;
        std::fs::rename(&staged, dir.join(name))?;
        // The rename itself must survive a power cut (only devices with the image run this).
        if cfg!(unix) {
            std::fs::File::open(dir)?.sync_all()?;
        }
        Ok(())
    })();
    if written.is_err() {
        let _ = std::fs::remove_file(&staged);
    }
    written
}

/// Removes the pre-update database copies in `system-update/` (and unfinished ones): once an
/// update is committed or rolled back, nothing needs them.
fn remove_database_copies(data_dir: &Path) -> std::io::Result<()> {
    let dir = data_dir.join(DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with("before-") && (name.ends_with(".db") || name.ends_with(".db.part")) {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn read_json<T: serde::de::DeserializeOwned>(dir: &Path, name: &str) -> anyhow::Result<Option<T>> {
    match std::fs::read(dir.join(name)) {
        Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn key() -> SigningKey {
        crate::signing::signing_key(&crate::signing::new_key().unwrap().0).unwrap()
    }

    /// A fake device: two slots of partitions as plain files, their sysfs entries, `bootfs`.
    struct Device {
        root: tempfile::TempDir,
        slots: Slots,
    }

    impl Device {
        fn new(active: char) -> Self {
            let root = tempfile::tempdir().unwrap();
            let path = root.path();
            let slots = Slots {
                by_slot: path.join("by-slot"),
                sys_block: path.join("sys"),
                bootfs: path.join("bootfs"),
            };
            std::fs::create_dir_all(&slots.bootfs).unwrap();
            for (slot, boot, system) in [('a', 2, 4), ('b', 3, 5)] {
                for (part, number) in [("boot", boot), ("system", system)] {
                    let name = format!("p{number}");
                    let mut old = b"old".to_vec();
                    old.resize(8192, 0);
                    std::fs::write(path.join(&name), old).unwrap();
                    let sys = slots.sys_block.join(&name);
                    std::fs::create_dir_all(&sys).unwrap();
                    std::fs::write(sys.join("partition"), format!("{number}\n")).unwrap();
                    std::fs::write(sys.join("uevent"), format!("DEVNAME={name}\nPARTNAME={part}_{slot}\n"))
                        .unwrap();
                    let which = if slot == active { "active" } else { "other" };
                    std::fs::create_dir_all(slots.by_slot.join(which)).unwrap();
                    std::os::unix::fs::symlink(path.join(&name), slots.by_slot.join(which).join(part))
                        .unwrap();
                }
            }
            Device { root, slots }
        }

        /// The start of a partition, as long as `len`.
        fn partition(&self, number: u32, len: usize) -> Vec<u8> {
            std::fs::read(self.root.path().join(format!("p{number}"))).unwrap()[..len].to_vec()
        }

        fn autoboot(&self) -> String {
            std::fs::read_to_string(self.slots.bootfs.join("autoboot.txt")).unwrap()
        }

        /// Restarts "into" the given slot.
        fn boot_into(&mut self, slot: char) {
            std::fs::remove_dir_all(&self.slots.by_slot).unwrap();
            for (s, boot, system) in [('a', 2, 4), ('b', 3, 5)] {
                let which = if s == slot { "active" } else { "other" };
                std::fs::create_dir_all(self.slots.by_slot.join(which)).unwrap();
                for (part, number) in [("boot", boot), ("system", system)] {
                    let target = self.root.path().join(format!("p{number}"));
                    std::os::unix::fs::symlink(target, self.slots.by_slot.join(which).join(part)).unwrap();
                }
            }
        }
    }

    fn update(dir: &Path, key: &SigningKey, version: &str) -> PathBuf {
        let boot = dir.join("boot.vfat");
        let system = dir.join("system.erofs");
        std::fs::write(&boot, vec![0x42; 3000]).unwrap();
        std::fs::write(&system, b"new system").unwrap();
        let path = dir.join("update.sursum-system");
        create(&boot, &system, version, key, std::fs::File::create(&path).unwrap()).unwrap();
        path
    }

    fn uploaded(data: &Path, file: &Path, key: &SigningKey, slots: &Slots) -> SystemUpdates {
        let updates = SystemUpdates {
            key: Some(key.verifying_key()),
            version: Some("v1".into()),
            supervised: true,
            request: data.join("request"),
            slots: slots.clone(),
        };
        let incoming = updates.incoming(data).unwrap();
        std::fs::copy(file, &incoming).unwrap();
        updates
    }

    #[test]
    fn an_update_goes_to_the_other_slot_on_trial_and_is_committed_there() {
        let key = key();
        let data = tempfile::tempdir().unwrap();
        let mut device = Device::new('a');
        let file = update(data.path(), &key, "v2");
        let updates = uploaded(data.path(), &file, &key, &device.slots);
        let incoming = updates.incoming(data.path()).unwrap();
        assert_eq!(updates.stage(data.path(), &incoming).unwrap(), "v2");
        assert!(data.path().join("request").exists(), "the root unit is asked");
        let status = updates.status(data.path()).unwrap();
        assert!(!status.can_install, "one update at a time");
        assert_eq!(status.target.as_deref(), Some("v2"), "a remote that connects now can follow it");

        assert_eq!(apply(data.path(), &key.verifying_key(), &device.slots).unwrap(), "v2");
        assert_eq!(device.partition(3, 3000), vec![0x42; 3000], "other boot written");
        assert_eq!(device.partition(5, 10), b"new system");
        assert_eq!(device.partition(4, 3), b"old", "the running system is untouched");
        assert!(device.autoboot().contains("boot_partition=2\n[tryboot]\nboot_partition=3"));
        let status = updates.status(data.path()).unwrap();
        assert!(status.installing && !status.on_trial, "written, the restart is still to come");

        device.boot_into('b');
        let status = updates.status(data.path()).unwrap();
        assert!(status.on_trial && !status.installing && !status.can_install);
        assert_eq!(status.target.as_deref(), Some("v2"));
        assert_eq!(trial_state(data.path(), &device.slots).unwrap(), TrialState::OnTrial("v2".into()));
        assert_eq!(commit(data.path(), &device.slots).unwrap(), "v2");
        assert!(device.autoboot().contains("boot_partition=3\n[tryboot]\nboot_partition=2"));
        let status = updates.status(data.path()).unwrap();
        assert_eq!(status.last, Some(LastSystemUpdate::Committed { version: "v2".into() }));
        assert!(!status.on_trial);
        assert_eq!(trial_state(data.path(), &device.slots).unwrap(), TrialState::None);
    }

    #[test]
    fn a_new_system_that_never_got_healthy_is_rolled_back_by_starting_the_old_slot() {
        let key = key();
        let data = tempfile::tempdir().unwrap();
        let device = Device::new('a');
        let file = update(data.path(), &key, "v2");
        let updates = uploaded(data.path(), &file, &key, &device.slots);
        updates.stage(data.path(), &updates.incoming(data.path()).unwrap()).unwrap();
        apply(data.path(), &key.verifying_key(), &device.slots).unwrap();
        // The trial boot failed: the firmware started slot A again.
        assert_eq!(trial_state(data.path(), &device.slots).unwrap(), TrialState::BootedBack("v2".into()));
        let rolled_back = LastSystemUpdate::RolledBack { failed: "v2".into(), database_restored: false };
        assert_eq!(end_failed_trial(data.path()).unwrap(), rolled_back);
        assert_eq!(trial_state(data.path(), &device.slots).unwrap(), TrialState::None);
        let status = updates.status(data.path()).unwrap();
        assert_eq!(status.last, Some(rolled_back));
        assert!(status.can_install && !status.on_trial);
        assert!(device.autoboot().contains("boot_partition=2\n"), "slot A stays the default");
    }

    #[test]
    fn a_rollback_puts_back_the_database_the_failed_system_migrated() {
        let key = key();
        let data = tempfile::tempdir().unwrap();
        let mut device = Device::new('a');
        let database = Database::open(data.path()).unwrap();
        database.set_setting("marker", &"before").unwrap();
        drop(database);
        let file = update(data.path(), &key, "v2");
        let updates = uploaded(data.path(), &file, &key, &device.slots);
        updates.stage(data.path(), &updates.incoming(data.path()).unwrap()).unwrap();
        apply(data.path(), &key.verifying_key(), &device.slots).unwrap();

        // The new system's server changes data and migrates the schema, then crash-loops: every
        // start writes a startup backup, far more than the rotation keeps.
        device.boot_into('b');
        let database = Database::open(data.path()).unwrap();
        database.set_setting("marker", &"during the trial").unwrap();
        for start in 0..crate::db::BACKUPS_KEPT + 3 {
            database.backup(data.path(), &format!("startup{start:02}")).unwrap();
        }
        drop(database);
        let schema = Database::supported_schema();
        rusqlite::Connection::open(data.path().join(crate::db::DATABASE_FILE))
            .unwrap()
            .pragma_update(None, "user_version", schema + 1)
            .unwrap();
        device.boot_into('a');

        assert_eq!(trial_state(data.path(), &device.slots).unwrap(), TrialState::BootedBack("v2".into()));
        let outcome = end_failed_trial(data.path()).unwrap();
        assert_eq!(outcome, LastSystemUpdate::RolledBack { failed: "v2".into(), database_restored: true });
        assert_eq!(Database::schema_version(data.path()).unwrap(), schema);
        let database = Database::open(data.path()).unwrap();
        assert_eq!(database.setting::<String>("marker").unwrap().as_deref(), Some("before"));
        let kept = std::fs::read_dir(data.path().join(ROLLED_BACK_DIR))
            .unwrap()
            .filter(|entry| entry.as_ref().unwrap().file_name().to_string_lossy().ends_with("-sursum.db"))
            .count();
        assert_eq!(kept, 1, "the migrated database is kept aside");
        assert!(!data.path().join(DIR).join("before-v2.db").exists(), "the copy is not needed any more");
    }

    #[test]
    fn a_stage_that_cannot_ask_for_the_install_leaves_nothing_behind() {
        let key = key();
        let data = tempfile::tempdir().unwrap();
        let device = Device::new('a');
        drop(Database::open(data.path()).unwrap());
        let file = update(data.path(), &key, "v2");
        let mut updates = uploaded(data.path(), &file, &key, &device.slots);
        updates.request = data.path().join("missing-run-dir").join("system-update");
        let incoming = updates.incoming(data.path()).unwrap();
        assert!(matches!(updates.stage(data.path(), &incoming), Err(SystemUpdateError::Failed(_))));
        let status = updates.status(data.path()).unwrap();
        assert!(status.can_install && !status.installing, "a failed stage does not block the next one");
        assert!(!data.path().join(DIR).join("before-v2.db").exists());
    }

    #[test]
    fn foreign_damaged_or_current_updates_are_refused_and_change_nothing() {
        let key = key();
        let data = tempfile::tempdir().unwrap();
        let device = Device::new('a');

        let foreign = update(data.path(), &self::key(), "v2");
        let updates = uploaded(data.path(), &foreign, &key, &device.slots);
        let incoming = updates.incoming(data.path()).unwrap();
        assert!(matches!(updates.stage(data.path(), &incoming), Err(SystemUpdateError::BadFile(_))));

        let current = update(data.path(), &key, "v1");
        let updates = uploaded(data.path(), &current, &key, &device.slots);
        assert!(matches!(updates.stage(data.path(), &incoming), Err(SystemUpdateError::AlreadyCurrent)));

        // A file whose system image was swapped after signing.
        let genuine = update(data.path(), &key, "v3");
        let mut archive = zip::ZipArchive::new(std::fs::File::open(&genuine).unwrap()).unwrap();
        let tampered = data.path().join("tampered.sursum-system");
        let mut zip = zip::ZipWriter::new(std::fs::File::create(&tampered).unwrap());
        for name in [MANIFEST, SIGNATURE, BOOT] {
            zip.raw_copy_file(archive.by_name(name).unwrap()).unwrap();
        }
        zip.start_file(SYSTEM, zip::write::SimpleFileOptions::default()).unwrap();
        zip.write_all(b"evil system").unwrap();
        zip.finish().unwrap();
        let updates = uploaded(data.path(), &tampered, &key, &device.slots);
        assert!(matches!(updates.stage(data.path(), &incoming), Err(SystemUpdateError::BadFile(_))));

        assert!(!data.path().join("request").exists());
        assert_eq!(device.partition(3, 3), b"old");
        assert!(!device.slots.bootfs.join("autoboot.txt").exists());
        assert!(apply(data.path(), &key.verifying_key(), &device.slots).is_err(), "nothing is ready");
    }
}
