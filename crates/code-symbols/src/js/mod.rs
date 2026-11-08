//! TypeScript, TSX and JavaScript through psi-syntax's `js-syntax`: oxc's parser, the parse accio reads these languages
//! with too (accio decision `2026-11-08-rust-and-js-parse-entries`), whose tree is oxc's typed arena AST
//! (`Function`, `Class`, `VariableDeclarator`, `IdentifierReference`, `StaticMemberExpression`, …) and whose tokens
//! the parse keeps on request. `with_program` is the one call into the parser; `summary` makes the `FileSummary` by the
//! parser-neutral `summary_builder`, as the psi languages do.
//!
//! - **Three dialects** (`Dialect`): `.ts`, `.mts`, `.cts` are TypeScript without JSX, where `<T>x` is a type
//!   assertion; `.tsx` is TSX; `.js`, `.mjs`, `.cjs`, `.jsx` are JavaScript with JSX. The summary takes the dialect from
//!   the file's path (`dialect_of_path`), so its cache key carries it (`language::summary_key`).
//! - **The text** the parser reads is the file's bytes, a byte that is no UTF-8 a space at its own offset
//!   (`js_syntax::parse_text`); every span of the tree is an offset into the file, and names are cut from the file's
//!   own bytes (`Source::text`), where such a byte makes the name none.
//! - **Recovery**: a parse oxc gives up on is parsed again with the region of its error blanked, up to sixteen times
//!   (`js_syntax::recover`); the blanked bytes are the errors of the file (`Errors`), with the labels of the
//!   diagnostics of the parse that made the tree. A file no recovery saves has no tree: every byte is an error.
//! - **Bounded by the stack**: a parse and every walk of its tree run inside `with_program`, on the calling thread when
//!   the file's nesting fits `psi_builder::stack::STACK`, else on a thread of the stack it needs; past
//!   `js_syntax`'s limit there is no tree and the summary is `truncated`.

pub(crate) mod summary;

#[cfg(test)]
mod summary_test;

use crate::summary_builder::column_at;
use js_syntax::oxc_ast::ast::Program;
use js_syntax::oxc_parser::Token;
use js_syntax::oxc_span::Span;
use js_syntax::{Dialect, Parsed, Read};

/// The dialect of a file by its path: its extension's (`Dialect::of_extension`), TSX for a path without one of them,
/// the superset that reads every file of the other two
pub(crate) fn dialect_of_path(path: &str) -> Dialect {
  let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path);
  file_name
    .rsplit_once('.')
    .and_then(|(_, extension)| Dialect::of_extension(extension))
    .unwrap_or(Dialect::Tsx)
}

/// Parses `source` as `dialect`, with its tokens and the recovery of a fatal parse, and hands the parse to `read`; `None`
/// when the file nests deeper than `js_syntax` parses. The tree, every walk of it and its drop stay inside, on the
/// stack `js_syntax::with_program` chose
pub(crate) fn with_program<R: Send>(dialect: Dialect, source: &[u8], read: impl FnOnce(&Parsed<'_>) -> R + Send) -> Option<R> {
  let how = Read {
    recover: true,
    tokens: true,
    clock: || 0,
  };
  js_syntax::with_program(dialect, source, how, read).map(|(answer, _)| answer)
}

/// The bytes of a file and where its lines start: what a walk cuts names from and numbers lines by (1-based, a line
/// ends at `\n`, as the other parsers count them)
pub(crate) struct Source<'s> {
  pub(crate) bytes: &'s [u8],
  newlines: Vec<u32>,
}

impl<'s> Source<'s> {
  pub(crate) fn new(bytes: &'s [u8]) -> Self {
    let newlines = bytes
      .iter()
      .enumerate()
      .filter(|(_, byte)| **byte == b'\n')
      .map(|(index, _)| u32::try_from(index).unwrap_or(u32::MAX))
      .collect();
    Self { bytes, newlines }
  }

  /// The 1-based line of the byte at `offset`
  pub(crate) fn line(&self, offset: u32) -> u32 {
    u32::try_from(self.newlines.partition_point(|&it| it < offset) + 1).unwrap_or(u32::MAX)
  }

  /// The last line of a node at `span`: a node that ends with its line break ends on the line before
  pub(crate) fn end_line(&self, span: Span) -> u32 {
    crate::summary_builder::end_line(|it| self.line(u32::try_from(it).unwrap_or(u32::MAX)), self.bytes, span.start as usize, span.end as usize)
  }

  /// The UTF-16 column of the byte at `offset` in its line
  pub(crate) fn column(&self, offset: u32) -> u32 {
    column_at(self.bytes, offset as usize)
  }

  /// The text of `span` as the file has it; `None` when it is no UTF-8
  pub(crate) fn text(&self, span: Span) -> Option<&'s str> {
    std::str::from_utf8(self.bytes.get(span.start as usize..span.end as usize)?).ok()
  }
}

/// The errors of a parse: the regions a recovery blanked and the labels of the diagnostics of the parse that made the
/// tree, sorted, and the bytes they cover (`FileSummary::error_bytes`), a zero-width label one
pub(crate) struct Errors {
  pub(crate) ranges: Vec<(u32, u32)>,
  pub(crate) bytes: u32,
}

impl Errors {
  pub(crate) fn of(parsed: &Parsed<'_>, source: &[u8]) -> Self {
    let mut ranges = Vec::new();
    if parsed.recovered {
      // the text of the last parse is the file's with the regions of the earlier errors blanked
      let read = js_syntax::parse_text(source);
      let blanked = parsed.program.source_text.as_bytes();
      let mut start = None;
      for (index, (left, right)) in read.as_bytes().iter().zip(blanked).enumerate() {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        match (left == right, start) {
          (false, None) => start = Some(index),
          (true, Some(from)) => {
            ranges.push((from, index));
            start = None;
          }
          _ => {}
        }
      }
      if let Some(from) = start {
        ranges.push((from, u32::try_from(blanked.len()).unwrap_or(u32::MAX)));
      }
    }
    for diagnostic in &parsed.diagnostics {
      for label in &diagnostic.labels {
        ranges.push((label.offset(), label.offset().saturating_add(label.len())));
      }
    }
    ranges.sort_unstable();
    // the bytes under any range once, a zero-width one as one byte
    let mut bytes = 0u32;
    let mut covered = 0u32;
    for &(start, end) in &ranges {
      let end = end.max(start + 1);
      let from = start.max(covered);
      if end > from {
        bytes = bytes.saturating_add(end - from);
        covered = end;
      }
    }
    Self { ranges, bytes }
  }

  /// Whether an error lies in `start..end`, a zero-width one at its offset
  pub(crate) fn within(&self, start: u32, end: u32) -> bool {
    let first = self.ranges.partition_point(|it| it.0 < start);
    // a range that starts before `start` and reaches into it
    self.ranges[..first].iter().any(|it| it.1 > start) || self.ranges.get(first).is_some_and(|it| it.0 < end)
  }
}

/// The tokens of a parse in `start..end`, in source order
pub(crate) fn tokens_in(tokens: &[Token], start: u32, end: u32) -> &[Token] {
  let from = tokens.partition_point(|it| it.start() < start);
  let to = tokens.partition_point(|it| it.start() < end).max(from);
  &tokens[from..to]
}

/// The doc comments of a program by the start of the token each is attached to, with the first line of the doc
/// comments above that token: a `/**` comment, not `/**/` (a `///` one is a directive of the compiler)
pub(crate) fn doc_lines(program: &Program<'_>, source: &Source<'_>) -> Vec<(u32, u32)> {
  let mut docs: Vec<(u32, u32)> = program
    .comments
    .iter()
    .filter(|it| it.is_leading())
    .filter(|it| source.text(it.span).is_some_and(|text| text.starts_with("/**") && !text.starts_with("/**/")))
    .map(|it| (it.attached_to, source.line(it.span.start)))
    .collect();
  docs.sort_unstable();
  docs.dedup_by_key(|it| it.0);
  docs
}
