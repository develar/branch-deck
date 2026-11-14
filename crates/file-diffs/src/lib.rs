//! The diffs of the files that the views of the app show: each changed file of a commit, a range or the working tree
//! as a typed `DiffFile`, compared by `line-diff` and marked by `diff-marks`, and the texts of its blobs by id. No
//! patch text: the views build the context, the hunks and the expansion from the blocks and the texts.
//!
//! A crate of its own above `git-ops`: it joins the git reads with the comparison of `line-diff`, the marks of
//! `diff-marks` and the declarations of `code-symbols`, which the git plumbing of `git-ops` does not depend on, and
//! answers them in the wire model of `text-diff`, which describes the diffs and merges them.
//!
//! It owns the versions of what the object cache keeps of the crates of psi-syntax: the revision the workspace links
//! them by (`PSI_SYNTAX_REVISION`) and the version of the summaries (`summaries_version`), which every reader and
//! writer of the `blob_summary` store keys it by.

pub mod accio;
pub mod blocks;
pub mod collapse;
pub mod declarations;
pub mod marks;
pub mod marks_measure;
mod merge;
pub mod model;
mod outside_place;
pub mod patch;
pub mod range_files;
pub mod reading;
mod scopes;
pub mod service;
pub mod syntax;
pub mod usages;
mod worktree;

/// The revision of psi-syntax, whose `kotlin-syntax`, `java-syntax` and `csharp-syntax` read Kotlin, Java and C#, whose
/// `starlark-syntax` reads Starlark, whose `js-syntax` reads TypeScript and JavaScript and whose `rust-syntax` reads Rust:
/// the `rev` of develar/psi-syntax (the public mirror of accio's `psi-syntax/`) of the workspace's dependencies, the
/// one of every crate of psi-syntax the workspace builds but `lmdb-env`: `code-symbols`, whose summaries are keyed by
/// it (`summaries_version`), `code-tokens`, whose parsers are of the same revision and whose version of the tokens is
/// keyed by it (`code_tokens::tokens_version`), `line-diff` and `diff-marks`
/// (`the_psi_syntax_revision_is_the_one_cargo_builds`). A new revision makes the stored diffs (`file_diff`) and
/// summaries (`blob_summary`) again; the highlighting is kept in memory only
pub const PSI_SYNTAX_REVISION: &str = "a26787b231d3321689a1171a9a0ee38424067336";

/// The version of the summaries of this build (`code_symbols::extract::extractor_version` of `PSI_SYNTAX_REVISION`): the
/// one the `blob_summary` store is read and written by, so a summary of other rules or other parsers is made again
pub fn summaries_version() -> u32 {
  code_symbols::extract::extractor_version(PSI_SYNTAX_REVISION)
}

#[cfg(test)]
mod accio_test;
#[cfg(test)]
mod blocks_test;
#[cfg(test)]
mod collapse_test;
#[cfg(test)]
mod declarations_test;
#[cfg(test)]
mod fixture_test;
#[cfg(test)]
mod lib_test;
#[cfg(test)]
mod marks_measure_test;
#[cfg(test)]
mod marks_test;
#[cfg(test)]
mod merge_test;
#[cfg(test)]
mod outside_place_test;
#[cfg(test)]
mod patch_test;
#[cfg(test)]
mod reading_test;
#[cfg(test)]
mod scopes_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod stored_layout_test;
#[cfg(test)]
mod syntax_test;
#[cfg(test)]
mod usages_test;
