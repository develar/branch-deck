//! The wire model of the diffs of the app: what the backend sends as JSON and the views read, in `src/lib/bindings.ts`,
//! and what the `file_diff` store keeps. Branch Deck's own, over `line-diff`, which compares the texts, and
//! `diff-marks`, which marks their diff: their values are theirs, their description here.
//!
//! - **The model of a diff** (`model`): the types of the app, mapped field by field to the values of `line_diff::model`
//!   and (with the `marks` feature) `diff_marks::model`, and the scopes and the blocks of the imports alone the
//!   file-diffs service adds. `specta::Type` with the `specta` feature.
//! - **The merge of three texts** (`merge_model`): the blocks of a merge with their kind, the sides that changed them
//!   and their resolution, the wire model of a conflict; and its result on the bytes of the three texts (`compose`),
//!   which the sync commits when the words resolve it.

pub mod compose;
pub mod merge_model;
pub mod model;

#[cfg(test)]
mod compose_test;
#[cfg(test)]
mod merge_model_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod test_support;
