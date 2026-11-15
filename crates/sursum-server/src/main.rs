// SPDX-License-Identifier: GPL-3.0-or-later
//! Command-line entry point of the sursum server.

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
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
