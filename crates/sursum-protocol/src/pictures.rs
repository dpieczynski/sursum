// SPDX-License-Identifier: GPL-3.0-or-later
//! Removing pictures nothing uses any more (admin, on request): uploaded posters stay on the
//! device after their item or set is gone, and only the operator knows when they are done.

use serde::{Deserialize, Serialize};

/// `POST /api/assets/cleanup`: count first (`dry_run`), then remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PictureCleanupRequest {
    pub dry_run: bool,
}

/// Pictures that are (or, in a dry run, would be) removed. Pictures in a set, the logo, what is
/// on screen and fresh uploads are always kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PictureCleanup {
    pub pictures: u32,
    pub bytes: u64,
}
