//! The bodies of the reads: no lock, no effect, each the git of one core in a blocking step (`run_blocking`), or a rule
//! of the grammar of what the user types
//!
//! - `repository`, `commits`, `blobs`, `grammar`: a repository, its commits and their files, the blobs of the diff
//!   views, what the user types.
//! - `diffs`: the diffs and the merges of the views on the channels of their commands, the file lists of the ranges.
//! - `symbols`: the symbol searches of a window in a review.
//! - `review_plan`: the plan of a review in its window (experimental).
//! - `conflict_fix`: the search of the fix of the conflicts.

pub mod blobs;
pub mod commits;
pub mod conflict_fix;
pub mod diffs;
pub mod grammar;
pub mod repository;
pub mod review_plan;
pub mod symbols;

#[cfg(test)]
mod conflict_fix_test;
