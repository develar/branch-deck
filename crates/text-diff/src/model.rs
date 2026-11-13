//! The types of `line_diff::model`, `line_diff::policy::WhitespacePolicy` and `line_diff::text::LineEndKind` as the
//! bindings of the app describe them (`specta::Type`): declared by the macros that declare them in `line-diff`, so
//! they are the same text. No value of them is made: a type of the app that holds a value of `line-diff` names its
//! description here (`#[specta(type = …)]`)

line_diff::diff_model!(#[derive(specta::Type)]; #[specta(optional)]);
line_diff::whitespace_policy!(#[derive(specta::Type)]);
line_diff::line_end_kind!(#[derive(specta::Type)]);
