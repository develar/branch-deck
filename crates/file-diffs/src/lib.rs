//! The diffs of the files that the views of the app show: each changed file of a commit, a range or the working tree
//! as a typed `DiffFile`, compared by `text-diff`, and the texts of its blobs by id. No patch text: the views build
//! the context, the hunks and the expansion from the blocks and the texts.
//!
//! A crate of its own above `git-ops`: it joins the git reads with the comparison of `text-diff` and the declarations
//! of `code-symbols`, which the git plumbing of `git-ops` does not depend on.

pub mod accio;
pub mod blocks;
pub mod declarations;
mod merge;
pub mod model;
mod outside_place;
pub mod patch;
pub mod range_files;
mod scopes;
pub mod service;
pub mod syntax;
pub mod usages;
mod worktree;

#[cfg(test)]
mod accio_test;
#[cfg(test)]
mod blocks_test;
#[cfg(test)]
mod declarations_test;
#[cfg(test)]
mod fixture_test;
#[cfg(test)]
mod merge_test;
#[cfg(test)]
mod outside_place_test;
#[cfg(test)]
mod patch_test;
#[cfg(test)]
mod scopes_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod syntax_test;
#[cfg(test)]
mod usages_test;
