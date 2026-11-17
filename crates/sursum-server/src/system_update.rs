// SPDX-License-Identifier: GPL-3.0-or-later
//! Signed system updates (`.sursum-system`, ADR 0019): replacing the read-only system of an imaged
//! device (kiosk, Chromium, cage, kernel) from the remote, without the internet and without
//! reflashing, with the Raspberry Pi firmware's `tryboot` as the safety net.
//!
//! A file is a ZIP with:
//! - `manifest.json`: format, version (the image's build id) and the size and SHA-256 of each image;
//! - `manifest.sig`: the Ed25519 signature of exactly those manifest bytes (the application
//!   bundles' key, [`crate::bundle`]);
//! - `boot.img` and `system.img`: the raw boot (vfat) and system (erofs) partition images.
//!
//! The flow, from the remote's upload to the next boot:
//! 1. The server (unprivileged) receives the file into `system-update/` in the data directory,
//!    [`verify`]s it, renames it to `ready.sursum-system` and asks for the install by creating
//!    [`REQUEST`].
//! 2. A root unit runs this binary's copy in the read-only system (so a compromised server
//!    cannot plant a system): [`apply`] verifies again, writes both images to the **other**
//!    slot, records the trial and points `[tryboot]` at that slot; the unit then reboots with
//!    `0 tryboot`.
//! 3. At the next boot, a root unit checks the trial ([`trial_state`]): booted back into the old
//!    slot means the new one failed (rolled back); booted into the new one, it waits until the
//!    server and the kiosk are healthy and [`commit`]s by making that slot the default, or
//!    reboots, which brings the old slot back (tryboot lasts one boot only).

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, ensure};
use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use sursum_protocol::{LastSystemUpdate, SystemStatus};

use crate::bundle::{from_hex, hex, valid_version};

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
const TRIAL: &str = "trial.json";
const OUTCOME: &str = "outcome.json";
const MANIFEST: &str = "manifest.json";
const SIGNATURE: &str = "manifest.sig";
const BOOT: &str = "boot.img";
const SYSTEM: &str = "system.img";

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

/// The trial of a newly written system, read at the next boot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Trial {
    version: String,
    /// PARTLABEL of the boot partition the new system was written to, e.g. `boot_b`.
    slot: String,
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
        let (active, _) = slots.boot_partition("active")?;
        let (other, label) = slots.boot_partition("other")?;
        write_images(
            &ready,
            &manifest,
            &slots.device_of("other", "boot"),
            &slots.device_of("other", "system"),
        )?;
        write_json(&dir, TRIAL, &Trial { version: manifest.version.clone(), slot: label })?;
        slots.write_autoboot(active, other)?;
        Ok(manifest.version)
    })();
    let _ = std::fs::remove_file(&ready);
    if result.is_err() {
        let failed = version.unwrap_or_else(|| "the uploaded update".into());
        write_json(&dir, OUTCOME, &LastSystemUpdate::RolledBack { failed })?;
    }
    result
}

/// What the boot-time check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrialState {
    /// No system update is on trial.
    None,
    /// The device started the old slot again: the new system did not come up healthy.
    RolledBack(String),
    /// The new system runs on trial; [`commit`] once it is healthy.
    OnTrial(String),
}

/// Root, at boot: is a new system on trial, and did the device start it? A start of the old slot
/// is recorded as rolled back.
pub fn trial_state(data_dir: &Path, slots: &Slots) -> anyhow::Result<TrialState> {
    let dir = data_dir.join(DIR);
    let Some(trial) = read_json::<Trial>(&dir, TRIAL)? else { return Ok(TrialState::None) };
    let (_, active) = slots.boot_partition("active")?;
    if active == trial.slot {
        return Ok(TrialState::OnTrial(trial.version));
    }
    write_json(&dir, OUTCOME, &LastSystemUpdate::RolledBack { failed: trial.version.clone() })?;
    std::fs::remove_file(dir.join(TRIAL))?;
    Ok(TrialState::RolledBack(trial.version))
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
        Some(SystemStatus {
            version,
            on_trial,
            installing,
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

    /// Verifies a received upload and hands it to the root unit. Returns the new version.
    pub fn stage(&self, data_dir: &Path, incoming: &Path) -> Result<String, SystemUpdateError> {
        let failed = |error: std::io::Error| SystemUpdateError::Failed(error.into());
        let key = self.key.as_ref().ok_or_else(|| SystemUpdateError::Failed(anyhow::anyhow!("no key")))?;
        let manifest = verify(incoming, key).map_err(SystemUpdateError::BadFile)?;
        if self.version.as_deref() == Some(manifest.version.as_str()) {
            return Err(SystemUpdateError::AlreadyCurrent);
        }
        std::fs::rename(incoming, data_dir.join(DIR).join(READY)).map_err(failed)?;
        std::fs::write(&self.request, b"").map_err(failed)?;
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

fn write_json(dir: &Path, name: &str, value: &impl Serialize) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let staged = dir.join(format!(".{name}.new"));
    let mut file = std::fs::File::create(&staged)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    std::fs::rename(&staged, dir.join(name))?;
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
        crate::bundle::signing_key(&crate::bundle::new_key().unwrap().0).unwrap()
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
        assert!(!updates.status(data.path()).unwrap().can_install, "one update at a time");

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
        assert_eq!(trial_state(data.path(), &device.slots).unwrap(), TrialState::RolledBack("v2".into()));
        let status = updates.status(data.path()).unwrap();
        assert_eq!(status.last, Some(LastSystemUpdate::RolledBack { failed: "v2".into() }));
        assert!(status.can_install && !status.on_trial);
        assert!(device.autoboot().contains("boot_partition=2\n"), "slot A stays the default");
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
