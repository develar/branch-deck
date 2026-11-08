//! The summary of a file (`summarize`), by the parser of its language, each on the parser-neutral `summary_builder`:
//! Kotlin, Java and C# by a walk of their PSI and Roslyn's tree (`kotlin::summary`, `java::summary`,
//! `csharp::summary`), Starlark by the Bazel plugin's (`starlark::summary`), TypeScript, TSX and JavaScript by a walk of
//! oxc's tree (`js::summary`), Rust by a walk of rust-analyzer's (`rust::summary`). The size limits and the version of
//! the rules are those of every language.
//!
//! - **Bounded**: files over `MAX_BYTES`, `MAX_LINES` or `MAX_AVERAGE_LINE` (minified) are not parsed, a `truncated`
//!   summary. psi-syntax parses any text within the limits, a construct past its nesting limit as an error element;
//!   oxc and rust-analyzer any text they can recover, and none nested deeper than `js_syntax` and `rust_syntax` read
//!   (`truncated`).
//! - `parse_language` is the tree-sitter parse within a budget that the highlighting of Rust and TypeScript and the
//!   navigation of TypeScript still read.

use crate::language::Language;
use crate::summary::FileSummary;
use crate::summary_builder::empty_summary;
use std::cell::RefCell;
use std::ops::ControlFlow;
use tree_sitter::{ParseOptions, ParseState, Parser, Tree};

/// The version of the extraction rules; bump it on any rule change, so that cached summaries are made again
pub const RULES_VERSION: u32 = 26;
/// Files above this size are not parsed
pub const MAX_BYTES: usize = 1 << 20;
/// Files with more lines are not parsed
pub const MAX_LINES: u32 = 20_000;
/// Files whose average line is longer are minified or generated, and not parsed
pub const MAX_AVERAGE_LINE: u32 = 500;

/// The summary of one file, at `path` (which names the kind of a Bazel file, `BUILD` declares targets and a `.bzl` does
/// not, and the dialect of a script file), within the limits of size, by the parser of its language; never fails, a
/// file that cannot be summarized gives a `truncated` summary
pub fn summarize(language: Language, path: &str, source: &[u8]) -> FileSummary {
  #[cfg(not(any(feature = "starlark", feature = "typescript")))]
  let _ = path;
  let mut summary = empty_summary(language, source);
  let lines = summary.lines;
  if !language.is_enabled() || source.len() > MAX_BYTES || lines > MAX_LINES || summary.total_bytes > lines.saturating_mul(MAX_AVERAGE_LINE) {
    return summary;
  }
  let read = match language {
    #[cfg(feature = "kotlin")]
    Language::Kotlin => crate::kotlin::summary::summarize(source, &mut summary),
    #[cfg(feature = "java")]
    Language::Java => crate::java::summary::summarize(source, &mut summary),
    #[cfg(feature = "csharp")]
    Language::CSharp => crate::csharp::summary::summarize(source, &mut summary),
    #[cfg(feature = "starlark")]
    Language::Starlark => crate::starlark::summary::summarize(source, starlark_syntax::FileKind::from_path(path).unwrap_or_default(), &mut summary),
    #[cfg(feature = "typescript")]
    Language::TypeScript => crate::js::summary::summarize(crate::js::dialect_of_path(path), source, &mut summary),
    #[cfg(feature = "rust")]
    Language::Rust => crate::rust::summary::summarize(source, &mut summary),
    #[cfg(not(all(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark", feature = "typescript", feature = "rust")))]
    _ => false,
  };
  summary.truncated = !read;
  summary
}

/// The revision of psi-syntax, whose `kotlin-syntax`, `java-syntax` and `csharp-syntax` read Kotlin, Java and C#, whose
/// `starlark-syntax` reads Starlark, whose `js-syntax` reads TypeScript and JavaScript and whose `rust-syntax` reads Rust:
/// the `rev` of develar/psi-syntax (the public mirror of accio's `psi-syntax/`) of the psi-syntax dependencies in
/// `Cargo.toml`
/// (`the_psi_syntax_revision_is_the_one_cargo_builds`)
pub(crate) const PSI_SYNTAX_REVISION: &str = "2eaa382792a4f2bd0318dc13518bf8e066a9506b";

/// A stable hash of the rules and the parsers of this build: a cached summary is valid only for the same value. FNV-1a
/// over `RULES_VERSION` and, for each enabled language, the revision of psi-syntax that reads it, so it is the same
/// across runs and compilers.
pub fn extractor_version() -> u32 {
  let mut hash = Fnv::new();
  hash.write(&RULES_VERSION.to_le_bytes());
  for language in Language::ALL.into_iter().filter(|it| it.is_enabled()) {
    hash.write(&[language.id()]);
    hash.write(PSI_SYNTAX_REVISION.as_bytes());
  }
  hash.finish()
}

struct Fnv(u32);

impl Fnv {
  const fn new() -> Self {
    Self(0x811c_9dc5)
  }

  fn write(&mut self, bytes: &[u8]) {
    for byte in bytes {
      self.0 ^= u32::from(*byte);
      self.0 = self.0.wrapping_mul(0x0100_0193);
    }
  }

  const fn finish(&self) -> u32 {
    self.0
  }
}

/// A tree-sitter parse that stops after `budget` progress callbacks, on one parser per thread; the highlighting and the
/// navigation parse with it too
pub(crate) fn parse_language(language: &tree_sitter::Language, source: &[u8], budget: u32) -> Option<Tree> {
  thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
  }
  PARSER.with_borrow_mut(|parser| {
    parser.set_language(language).ok()?;
    let mut calls = 0u32;
    let mut progress = |_: &ParseState| {
      calls += 1;
      if calls > budget { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
    };
    let options = ParseOptions::new().progress_callback(&mut progress);
    let tree = parser.parse_with_options(&mut |offset, _| source.get(offset..).unwrap_or_default(), None, Some(options));
    if tree.is_none() {
      // a cancelled parse keeps its state to resume; the next file must start afresh
      parser.reset();
    }
    tree
  })
}
