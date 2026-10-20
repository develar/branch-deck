//! The moves of the checked out branch and their undo: `bd update` (onto a newer baseline, the conflicts resolved by
//! words), `bd pick` (the commits of a lease onto a shared branch) and `bd undo` (the token of a rewrite, an update or a
//! pick). bd-cli parses the arguments and prints the outputs; the tests run bd through the whole Cli, with the fixtures
//! of bd-cli
//! - `update_command`: `bd update`, `--continue`, `--abort` and `--status`
//! - `pick_command`: `bd pick`
//! - `undo_command`: `bd undo`

pub mod pick_command;
pub mod undo_command;
pub mod update_command;

#[cfg(test)]
mod pick_test;
#[cfg(test)]
mod resolve_test;
#[cfg(test)]
mod update_test;
