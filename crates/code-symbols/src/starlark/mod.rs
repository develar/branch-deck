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

use starlark_syntax::{FileKind, Parser, Tree};
use std::cell::RefCell;

thread_local! {
  /// The thread's parser: psi-syntax keeps every buffer of a parse in it and reuses them from one file to the next
  static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// The tree of a Starlark file from its bytes: the one Starlark parse of the crate. Every Bazel file kind shares one
/// grammar; the tree's offsets are the file's (`document_text` writes a byte order mark as spaces)
pub(crate) fn parse(source: &[u8], kind: FileKind) -> Tree {
  let text = starlark_syntax::document_text(source);
  PARSER.with_borrow_mut(|parser| parser.parse(&*text, kind))
}
