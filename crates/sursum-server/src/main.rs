// SPDX-License-Identifier: GPL-3.0-or-later
//! Command-line entry point of the sursum server.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use sursum_server::{BUILD_ID, Config, SystemPlatform};

const LICENSE_NOTICE: &str = "sursum — offline church lyrics display
Copyright (C) 2026 Dominik Pieczyński and contributors

This program is free software: you can redistribute it and/or modify it under the terms of the
GNU General Public License as published by the Free Software Foundation, either version 3 of the
License, or (at your option) any later version. It is distributed WITHOUT ANY WARRANTY; see the
GNU General Public License for details: https://www.gnu.org/licenses/gpl-3.0.html
Source code: https://github.com/dpieczynski/sursum";

/// sursum server: holds all state and serves the screen and the remotes.
#[derive(Debug, Parser)]
#[command(version = BUILD_ID)]
struct Args {
    /// Directory for the database and `config.toml`.
    #[arg(long, default_value = "sursum-data")]
    data_dir: PathBuf,
    /// Address to listen on (overrides `config.toml`).
    #[arg(long)]
    listen: Option<SocketAddr>,
    /// Extra allowed WebSocket `Origin` (repeatable), e.g. a Trunk dev server.
    #[arg(long = "allow-origin")]
    allow_origins: Vec<String>,
    /// Print the license notice and exit.
    #[arg(long)]
    license: bool,
    /// Development only: while no PIN is set, let every device on the local network in as admin
    /// (without it, the first remote must set the PIN).
    #[arg(long)]
    open_without_pin: bool,
    /// Test-only: delay serving WebAssembly (needs the `e2e-hooks` feature).
    #[arg(long, default_value_t = 0, hide = true)]
    wasm_delay_ms: u64,
    #[command(subcommand)]
    command: Option<Task>,
}

/// One-off tasks instead of running the server.
#[derive(Debug, Subcommand)]
enum Task {
    /// Set the PIN that remotes must enter to pair (6-12 digits), then exit.
    SetPin {
        /// The new PIN.
        pin: String,
    },
    /// Import (or update) songs from an OpenSong `Songs` folder (and `Sets` next to it), then exit.
    ImportOpensong {
        /// The OpenSong `Songs` folder.
        songs_dir: PathBuf,
        /// Name of the songbook that OpenSong hymn numbers refer to.
        #[arg(long, default_value = "OpenSong")]
        songbook: String,
    },
    /// Signed application bundles (`.sursum-app`, ADR 0013).
    #[command(subcommand)]
    Bundle(BundleTask),
    /// Signed system updates (`.sursum-system`, ADR 0019).
    #[command(subcommand)]
    System(SystemTask),
}

/// Making and installing system updates; `apply` and `finish-trial` run as root on the device,
/// from the read-only system's copy of this binary.
#[derive(Debug, Subcommand)]
enum SystemTask {
    /// Sign an image build's partition images into a `.sursum-system`.
    Create {
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        version: String,
        /// The boot partition image (`boot.vfat` of the image build).
        #[arg(long)]
        boot: PathBuf,
        /// The system partition image (`system.erofs` of the image build).
        #[arg(long)]
        system: PathBuf,
        output: PathBuf,
    },
    /// Check a `.sursum-system` against this build's key.
    Verify { update: PathBuf },
    /// Write the update the server staged in DATA_DIR to the other slot and point tryboot at it
    /// (the caller then reboots with "0 tryboot").
    Apply,
    /// At boot: end a system update's trial. Back in the old slot it is recorded as rolled back;
    /// in the new one, wait until the server and the kiosk are healthy and make it the default,
    /// or reboot into the old slot.
    FinishTrial {
        /// The server's address for /healthz.
        #[arg(long, default_value = "127.0.0.1:80")]
        server: String,
    },
}

/// Making, installing and rolling back application bundles.
#[derive(Debug, Subcommand)]
enum BundleTask {
    /// Create a signing key: the secret goes to KEY_FILE (keep it out of the repository), the
    /// public key is printed for `SURSUM_BUNDLE_KEY` at build time.
    Keygen { key_file: PathBuf },
    /// Sign a server binary into a bundle.
    Create {
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        version: String,
        binary: PathBuf,
        output: PathBuf,
    },
    /// Check a bundle against this build's key without installing it (e.g. that a release was
    /// signed with the secret matching the public key devices trust).
    Verify { bundle: PathBuf },
    /// Verify a bundle against this build's key and make it the current release in DATA_DIR, on
    /// trial until it has run healthily (restart the service afterwards).
    Install { bundle: PathBuf },
    /// Make the previous release current again.
    Rollback,
    /// Run by systemd before each start, with the previous release's binary: count the starts
    /// of a release on trial and go back to the previous one when it keeps failing.
    Guard,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.license {
        println!("{LICENSE_NOTICE}");
        return Ok(());
    }
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    if let Some(Task::SetPin { pin }) = &args.command {
        anyhow::ensure!(sursum_server::auth::valid_pin(pin), "the PIN must be 6 to 12 digits");
        let database = sursum_server::db::Database::open(&args.data_dir)?;
        database.set_setting(sursum_server::auth::PIN_SETTING, &sursum_server::auth::PinHash::new(pin)?)?;
        println!("PIN set. Remotes must now pair with it; restart the server if it is running.");
        return Ok(());
    }
    if let Some(Task::ImportOpensong { songs_dir, songbook }) = &args.command {
        let mut database = sursum_server::db::Database::open(&args.data_dir)?;
        database.backup(&args.data_dir, "before-import")?;
        let report = sursum_server::import::import_opensong(&mut database, songs_dir, songbook)?;
        for line in &report.problems {
            eprintln!("{line}");
        }
        println!(
            "imported: {} new, {} updated, {} unchanged, {} failed, {} sets",
            report.inserted, report.updated, report.unchanged, report.failed, report.sets
        );
        return Ok(());
    }

    if let Some(Task::Bundle(task)) = &args.command {
        return run_bundle_task(task, &args.data_dir);
    }
    if let Some(Task::System(task)) = &args.command {
        return run_system_task(task, &args.data_dir);
    }

    // The device image's defaults (screen guard, display power); the unit names the file, so an
    // older binary after an application rollback simply ignores it.
    let system_config = std::env::var_os("SURSUM_SYSTEM_CONFIG").map(PathBuf::from);
    let mut config = Config::with_data_dir(&args.data_dir).load_files(system_config.as_deref())?;
    if let Some(listen) = args.listen {
        config.listen = listen;
    }
    config.allowed_origins.extend(args.allow_origins);
    config.wasm_delay_ms = args.wasm_delay_ms;
    config.open_without_pin = args.open_without_pin;

    let server = sursum_server::start(config, &SystemPlatform).await?;
    println!("listening on http://{}", server.addr);
    tokio::select! {
        signal = tokio::signal::ctrl_c() => signal?,
        () = server.restart_requested() => tracing::info!("exiting so the service starts the update"),
    }
    tracing::info!("shutting down");
    server.stop().await
}

fn run_bundle_task(task: &BundleTask, data_dir: &std::path::Path) -> anyhow::Result<()> {
    use sursum_server::bundle;
    match task {
        BundleTask::Keygen { key_file } => {
            let (secret, public) = bundle::new_key()?;
            write_secret(key_file, &secret)?;
            println!("SURSUM_BUNDLE_KEY={public}");
        }
        BundleTask::Create { key, version, binary, output } => {
            let key = bundle::signing_key(&std::fs::read_to_string(key)?)?;
            let bytes = bundle::create(&std::fs::read(binary)?, version, BUILD_ID, &key)?;
            std::fs::write(output, bytes)?;
            println!("wrote {}", output.display());
        }
        BundleTask::Verify { bundle: path } => {
            let trusted = bundle::trusted_key().ok_or_else(|| {
                anyhow::anyhow!("this build has no SURSUM_BUNDLE_KEY and accepts no bundles")
            })?;
            let verified = bundle::verify(&std::fs::read(path)?, &trusted)?;
            println!("{} is genuine: version {}", path.display(), verified.manifest.version);
        }
        BundleTask::Install { bundle: path } => {
            let trusted = bundle::trusted_key().ok_or_else(|| {
                anyhow::anyhow!("this build has no SURSUM_BUNDLE_KEY and accepts no bundles")
            })?;
            // Backs up through a plain connection: the server may be running, and nothing must
            // migrate or "repair" the database under it.
            let version = sursum_server::update::install(data_dir, &std::fs::read(path)?, &trusted)?;
            println!("installed {version} on trial; restart the service to run it");
        }
        BundleTask::Rollback => {
            let schema = sursum_server::db::Database::schema_version(data_dir)?;
            let previous = bundle::rollback(data_dir, schema)?;
            println!("current is {} again; restart the service", previous.display());
        }
        BundleTask::Guard => guard(data_dir)?,
    }
    Ok(())
}

fn run_system_task(task: &SystemTask, data_dir: &std::path::Path) -> anyhow::Result<()> {
    use sursum_server::system_update::{self, Slots, TrialState};
    let trusted = || {
        sursum_server::bundle::trusted_key()
            .ok_or_else(|| anyhow::anyhow!("this build has no SURSUM_BUNDLE_KEY and accepts no updates"))
    };
    match task {
        SystemTask::Create { key, version, boot, system, output } => {
            let key = sursum_server::bundle::signing_key(&std::fs::read_to_string(key)?)?;
            let manifest =
                system_update::create(boot, system, version, &key, std::fs::File::create(output)?)?;
            println!(
                "wrote {} (boot {} bytes, system {} bytes)",
                output.display(),
                manifest.boot_bytes,
                manifest.system_bytes
            );
        }
        SystemTask::Verify { update } => {
            let manifest = system_update::verify(update, &trusted()?)?;
            println!("{} is genuine: system {}", update.display(), manifest.version);
        }
        SystemTask::Apply => {
            let version = system_update::apply(data_dir, &trusted()?, &Slots::device())?;
            println!("system {version} written to the other slot; reboot with \"0 tryboot\"");
        }
        SystemTask::FinishTrial { server } => match system_update::trial_state(data_dir, &Slots::device())? {
            TrialState::None => {}
            TrialState::RolledBack(version) => {
                println!("system {version} did not come up; running the previous one");
            }
            TrialState::OnTrial(version) => {
                if healthy_for_a_while(server) {
                    system_update::commit(data_dir, &Slots::device())?;
                    println!("system {version} is healthy and now the default");
                } else {
                    println!("system {version} is not healthy; rebooting into the previous system");
                    std::process::Command::new("systemctl").arg("reboot").status()?;
                }
            }
        },
    }
    Ok(())
}

/// Up to three minutes for the server to answer /healthz and the kiosk to run, then both must
/// stay so for 30 s (six checks 5 s apart).
fn healthy_for_a_while(server: &str) -> bool {
    let healthy = || {
        let answers = (|| -> std::io::Result<bool> {
            use std::io::{Read as _, Write as _};
            let address = server.parse().map_err(std::io::Error::other)?;
            let mut stream =
                std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(3))?;
            stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
            stream.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
            let mut answer = String::new();
            stream.read_to_string(&mut answer)?;
            Ok(answer.starts_with("HTTP/1.0 200") || answer.starts_with("HTTP/1.1 200"))
        })()
        .unwrap_or(false);
        let kiosk = std::process::Command::new("systemctl")
            .args(["is-active", "--quiet", "sursum-kiosk.service"])
            .status()
            .is_ok_and(|status| status.success());
        answers && kiosk
    };
    let mut in_a_row = 0;
    for _ in 0..36 {
        std::thread::sleep(std::time::Duration::from_secs(5));
        in_a_row = if healthy() { in_a_row + 1 } else { 0 };
        if in_a_row >= 6 {
            return true;
        }
    }
    false
}

#[cfg(unix)]
fn guard(data_dir: &std::path::Path) -> anyhow::Result<()> {
    use sursum_server::update::{Guarded, MAX_STARTS};
    match sursum_server::update::guard(data_dir)? {
        Guarded::NoTrial => {}
        Guarded::Start(start) => println!("release on trial: start {start} of {MAX_STARTS}"),
        Guarded::RolledBack(outcome) => println!("the release on trial kept failing: {outcome:?}"),
        Guarded::NothingToRestore => println!("the release on trial kept failing; no previous release"),
    }
    Ok(())
}

#[cfg(not(unix))]
fn guard(_: &std::path::Path) -> anyhow::Result<()> {
    anyhow::bail!("releases are only supported on the Linux device")
}

/// Writes a secret readable only by its owner.
fn write_secret(path: &std::path::Path, secret: &str) -> anyhow::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    use std::io::Write as _;
    options.open(path)?.write_all(secret.as_bytes())?;
    Ok(())
}
