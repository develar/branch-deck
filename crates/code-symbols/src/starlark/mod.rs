//! Starlark, the language of Bazel's BUILD, `.bzl`, MODULE.bazel and WORKSPACE files, from psi-syntax's port of the
//! IntelliJ Bazel plugin's parser (`starlark-syntax`), whose tree is the plugin's PSI: its summary (`summary`) and its
//! highlighting (`highlight`). The rules of the plugin itself — what a top-level call declares, what a `load` binds —
//! are `starlark_syntax::psi`, which accio's extractor reads too, so neither repeats them.

pub(crate) mod highlight;
#[cfg(test)]
mod highlight_test;
pub(crate) mod navigate;
pub(crate) mod summary;
#[cfg(test)]
mod summary_test;
