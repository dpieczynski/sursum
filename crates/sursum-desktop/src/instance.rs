// SPDX-License-Identifier: GPL-3.0-or-later
//! One copy of the program at a time: two servers on one database would each believe they own
//! it. The lock is a file in the data folder, held for the whole run; the operating system
//! releases it when the process ends, also after a crash.

use std::fs::{File, TryLockError};
use std::path::Path;

/// Name of the lock file in the data folder.
const LOCK_FILE: &str = "sursum.lock";

/// The lock of the data folder; dropping it lets another copy start.
#[derive(Debug)]
pub struct Instance {
    _file: File,
}

impl Instance {
    /// Locks the data folder for this copy; `None` if another copy holds it.
    pub fn claim(data_dir: &Path) -> std::io::Result<Option<Instance>> {
        let file = File::options().create(true).truncate(false).write(true).open(data_dir.join(LOCK_FILE))?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Instance { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(error)) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_copy_is_refused_until_the_first_ends() {
        let dir = tempfile::tempdir().unwrap();
        let first = Instance::claim(dir.path()).unwrap().expect("the first copy gets the lock");
        assert!(Instance::claim(dir.path()).unwrap().is_none(), "the second copy must wait");
        drop(first);
        assert!(Instance::claim(dir.path()).unwrap().is_some(), "free again after the first ends");
    }
}
