//! Rust through psi-syntax's `rust-syntax`: rust-analyzer's own lexer and parser (`ra_ap_syntax`), the parse accio
//! reads Rust with too (its decision `2026-11-08-rust-and-js-parse-entries`), whose tree is rowan's lossless syntax tree
//! (`FN`, `PATH_SEGMENT`, `NAME_REF`, …): every token a leaf, trivia (whitespace, comments) included, an item's outer
//! attributes and doc comments inside its node. `summary` makes the `FileSummary` by one walk over it, on the
//! parser-neutral `summary_builder`.
//!
//! - **One parse entry**: the text, the edition (`rust_syntax::EDITION`, 2021) and the tree are `rust_syntax`'s; this
//!   module names the parser's types only through its re-exports (`rust_syntax::ra_ap_syntax`).
//! - **The stack**: rust-analyzer's parser recurses without a depth limit, and the drop of a rowan tree recurses as deep
//!   as the tree, so every parse, every walk of the tree and its drop run inside `rust_syntax::guarded`, which runs a
//!   deep file on a thread with a stack for it; a file nested deeper than any code (`rust_syntax::nesting::MAX_LEVELS`)
//!   gives no tree, a `truncated` summary. The threads that summarize have 2 MiB (trial-check's workers, tokio's
//!   blocking threads).
//! - **Offsets**: `rust_syntax::utf8_text` keeps every offset of the file (a byte that is no UTF-8 and a byte order mark
//!   are spaces, a CRLF's `\r` a `\n`), so a walk slices that text by the tree's ranges and counts lines and columns on
//!   the file's own bytes.

pub(crate) mod summary;

#[cfg(test)]
mod summary_test;

use rust_syntax::ra_ap_syntax::{SyntaxKind, SyntaxNode, TextRange};

/// The 1-based line of each byte offset of a file: rowan's tree gives offsets, a summary records lines
pub(crate) struct Lines(Vec<usize>);

impl Lines {
  pub(crate) fn new(source: &[u8]) -> Self {
    Self(source.iter().enumerate().filter(|(_, byte)| **byte == b'\n').map(|(offset, _)| offset).collect())
  }

  /// The line of `offset`: one more than the line ends before it
  pub(crate) fn line(&self, offset: usize) -> u32 {
    u32::try_from(self.0.partition_point(|&end| end < offset) + 1).unwrap_or(u32::MAX)
  }
}

/// The byte range of a node or a token
pub(crate) fn span(range: TextRange) -> std::ops::Range<usize> {
  usize::from(range.start())..usize::from(range.end())
}

/// The names of Rust's primitive types, which no file declares: a path of one of them alone in a type is none of the
/// file's references, nor is one among the tokens of a macro, as tree-sitter's grammar read both
pub(crate) const PRIMITIVES: &[&str] = &[
  "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str", "u8", "u16", "u32", "u64", "u128", "usize",
];

/// Where a declaration's own text starts, with the outer attributes and doc comments above it that rust-analyzer keeps
/// in its node, without the plain comments before them (which the parser attaches too): its first line
pub(crate) fn leading_start(node: &SyntaxNode) -> usize {
  node
    .children_with_tokens()
    .find(|it| !matches!(it.kind(), SyntaxKind::WHITESPACE | SyntaxKind::COMMENT))
    .map_or_else(|| usize::from(node.text_range().start()), |it| usize::from(it.text_range().start()))
}

/// Whether a comment's text is a doc comment of the item after it: `///` and `/**`, not `////` or `/**/`
pub(crate) fn is_outer_doc(text: &str) -> bool {
  (text.starts_with("/**") && !text.starts_with("/**/")) || (text.starts_with("///") && !text.starts_with("////"))
}
