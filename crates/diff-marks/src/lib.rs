//! The marks of a diff (openspec/specs/commit/quiet-changes/spec.md; docs/design/diff-view.md, "Marks"): what a reader
//! of a change may pass and what a view labels. The runs of lines that changed in their formatting, their comments or
//! a renamed name alone (`marks`, `marks_runs`, the blocks the comparison cut out of one change `marks_joined`), the names a change renamed within a file (`marks_renames`), the lines
//! that moved, and the line the reading of each block starts at; what a pair of blobs leaves to the other files of its
//! change (`leftovers`) and what those files say of each other (`marks_subject`).
//!
//! A pure function of two texts and their diff (`line-diff`), read by their tokens (`code-tokens`): no git, no cache,
//! no IO. What it needs of a reader is asked through a trait or handed in as a value, so Branch Deck and another reader
//! of code (accio) can share it:
//!
//! - **The bindings of a text** (`bindings::Bindings`): the names at many places of one parse, for the renames. Branch
//!   Deck answers them by `code-symbols` (`file_diffs::bindings`).
//! - **The answers of the index of a repository** (`index`): the contract of `accio renames` and `accio moves`, which
//!   the marks of a subject decide by, and the mark of a free reorder (`reorder`) the latter gives.
//! - **The tokens of a side**: a closure the reader keeps a cache behind (`marks::marks_of`).
//!
//! Limits by work, never by time: the same pair has the same marks on every machine and under every load.

pub mod bindings;
pub mod index;
pub mod leftovers;
pub mod marks;
pub mod marks_joined;
pub mod marks_renames;
pub mod marks_runs;
pub mod marks_subject;
pub mod model;
pub mod reorder;

#[cfg(test)]
mod index_test;
