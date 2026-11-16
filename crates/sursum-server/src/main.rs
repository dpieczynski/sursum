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
    /// Verify a bundle against this build's key and make it the current release in DATA_DIR.
    Install { bundle: PathBuf },
    /// Make the previous release current again.
    Rollback,
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

    let mut config = Config::with_data_dir(&args.data_dir).load_file()?;
    if let Some(listen) = args.listen {
        config.listen = listen;
    }
    config.allowed_origins.extend(args.allow_origins);
    config.wasm_delay_ms = args.wasm_delay_ms;

    let server = sursum_server::start(config, &SystemPlatform).await?;
    println!("listening on http://{}", server.addr);
    tokio::signal::ctrl_c().await?;
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
        BundleTask::Install { bundle: path } => {
            let trusted = bundle::trusted_key().ok_or_else(|| {
                anyhow::anyhow!("this build has no SURSUM_BUNDLE_KEY and accepts no bundles")
            })?;
            let verified = bundle::verify(&std::fs::read(path)?, &trusted)?;
            let database = sursum_server::db::Database::open(data_dir)?;
            database.backup(data_dir, &format!("before-{}", verified.manifest.version))?;
            let release = bundle::install(&verified, data_dir)?;
            println!("installed {}; restart the service to run it", release.display());
        }
        BundleTask::Rollback => {
            let previous = bundle::rollback(data_dir)?;
            println!("current is {} again; restart the service", previous.display());
        }
    }
    Ok(())
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
