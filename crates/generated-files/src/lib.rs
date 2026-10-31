//! The files nobody writes by hand, one rule for `bd` and every review of the app:
//! - `names`: lock files, minified files, source maps and snapshots, by the name alone
//! - `attributes`: the files that the git attributes of a revision mark `linguist-generated` or `-diff`
//!
//! No policy of a view: what a caller leaves out, folds or counts is its own.

pub mod attributes;
pub mod names;

#[cfg(test)]
mod attributes_test;
#[cfg(test)]
mod names_test;
