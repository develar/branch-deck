//! The wire model of the diffs of the app: what the backend sends as JSON and the views read, in `src/lib/bindings.ts`.
//! Branch Deck's own, over `line-diff`, which compares the texts and whose types are the values.
//!
//! - **The types of a diff** (`model`): `line-diff`'s, declared again here by the macros that declare them there
//!   (`line_diff::diff_model!`, `whitespace_policy!`, `line_end_kind!`) with `specta::Type`, so the bindings of the app
//!   describe them from one text: the same fields, names and documentation, and the same serde layout. A value is
//!   always `line-diff`'s; a type of the app that holds one names its description here
//!   (`#[specta(type = text_diff::model::TextDiff)]`). Only with the `specta` feature.
//! - **The merge of three texts** (`merge_model`): the blocks of a merge with their kind, the sides that changed them
//!   and their resolution, the wire model of a conflict; and its result on the bytes of the three texts (`compose`),
//!   which the sync commits when the words resolve it.

pub mod compose;
pub mod merge_model;
#[cfg(feature = "specta")]
pub mod model;

#[cfg(test)]
mod compose_test;
#[cfg(test)]
mod merge_model_test;
#[cfg(test)]
mod test_support;
