// SPDX-License-Identifier: GPL-3.0-or-later
//! Pure domain logic of sursum.
//!
//! Nothing here performs I/O, spawns tasks or reads the clock: every function takes its inputs
//! as parameters and returns a value. That keeps the rules of the system (how songs become
//! slides, how navigation works, where text may be drawn) easy to read and to test.

pub mod churches;
pub mod display_power;
pub mod layout;
pub mod model;
pub mod program;
pub mod search;
pub mod set_review;
pub mod slides;
pub mod versions;
