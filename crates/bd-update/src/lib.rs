//! The moves of the checked out branch and their undo: `brd update` (onto a newer baseline, the conflicts resolved by
//! words), `brd pick` (the commits of a lease onto a shared branch), `brd unapply` (a branch and its files out of HEAD),
//! `brd reapply` (an unapplied branch back on top), `brd drop` (commits and their files out of HEAD for good) and
//! `brd undo` (the token of a rewrite, an update, a pick, an unapply, a reapply or a drop). bd-cli parses the arguments and prints the outputs; the tests run brd through the whole Cli, with the
//! fixtures of bd-cli
//! - `update_command`: `brd update`, `--continue`, `--abort` and `--status`
//! - `pick_command`: `brd pick`
//! - `unapply_command`: `brd unapply` and `brd reapply`
//! - `drop_command`: `brd drop`
//! - `undo_command`: `brd undo`

pub mod drop_command;
pub mod pick_command;
pub mod unapply_command;
pub mod undo_command;
pub mod update_command;

#[cfg(test)]
mod drop_test;
#[cfg(test)]
mod error_code_test;
#[cfg(test)]
mod pick_test;
#[cfg(test)]
mod resolve_test;
#[cfg(test)]
mod unapply_test;
#[cfg(test)]
mod update_test;
