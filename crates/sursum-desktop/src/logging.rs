// SPDX-License-Identifier: GPL-3.0-or-later
//! Log lines go to standard error and, once this copy owns the data folder, to `sursum.log`
//! there. On Windows the program has no console, so the file is the only place a problem can be
//! read afterwards. The previous run's file is kept as `sursum.previous.log`; nothing grows
//! without bound.

use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

const LOG_FILE: &str = "sursum.log";
const PREVIOUS_LOG_FILE: &str = "sursum.previous.log";

/// The log file, shared by every thread that logs; empty until [`LogFile::open`].
#[derive(Clone, Default)]
pub struct LogFile(Arc<Mutex<Option<(File, PathBuf)>>>);

impl LogFile {
    /// Makes this the global log writer (`RUST_LOG` filters, `info` by default).
    pub fn install(&self) {
        let writer = self.clone();
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
            )
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .init();
    }

    /// Starts a new log file in `data_dir`, keeping the previous run's. Only the copy that owns
    /// the data folder may call this, or it would move the running copy's file away.
    pub fn open(&self, data_dir: &Path) -> io::Result<()> {
        let path = data_dir.join(LOG_FILE);
        if path.exists() {
            std::fs::rename(&path, data_dir.join(PREVIOUS_LOG_FILE))?;
        }
        let file = File::create(&path)?;
        *self.0.lock().unwrap_or_else(PoisonError::into_inner) = Some((file, path));
        Ok(())
    }

    /// Where the log is written, once it is.
    pub fn path(&self) -> Option<PathBuf> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner).as_ref().map(|(_, path)| path.clone())
    }
}

impl Write for LogFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        // Standard error may be closed (no console on Windows); the file matters more.
        let _ = io::stderr().write_all(bytes);
        if let Some((file, _)) = self.0.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            file.write_all(bytes)?;
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Some((file, _)) = self.0.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_run_keeps_the_previous_log_only() {
        let dir = tempfile::tempdir().unwrap();
        for run in ["first", "second", "third"] {
            let mut log = LogFile::default();
            log.open(dir.path()).unwrap();
            log.write_all(run.as_bytes()).unwrap();
        }
        let read = |name| std::fs::read_to_string(dir.path().join(name)).unwrap();
        assert_eq!(read(LOG_FILE), "third");
        assert_eq!(read(PREVIOUS_LOG_FILE), "second");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 2);
    }
}
