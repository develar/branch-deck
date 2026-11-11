//! Whether a change is of whitespace alone (openspec/specs/branch/review-plan/spec.md, "Mechanical changes"), the one answer of
//! the files, the blocks and the declarations of a plan.
//!
//! - **A file of a grammar** (`code_symbols::highlight::grammar_of`) is compared by its tokens
//!   (`code_symbols::tokens::equal_by_tokens`, openspec/specs/commit/equal-by-tokens/spec.md): the same code and the
//!   same comments, so a space inside a string, a line break that ends a statement and an indent of Starlark are no
//!   whitespace, and a wrapped call, a line of Starlark continued inside its brackets, a byte order mark removed are.
//!   The rule the diffs mark a block of formatting by. `whitespace_only` asks it only of lines that are the same
//!   without their blanks (`code_symbols::tokens::same_without_blanks`), which lines equal by tokens always are.
//! - **A file of no grammar** keeps the rule of characters: the same text without its whitespace, never where the
//!   indentation has a meaning (`code_symbols::tokens::indentation_matters`: Python, YAML, a Makefile).
//! - **A file of a grammar without tokens** (nested too deeply, a script read only after a recovery) and a file whose
//!   two sides are not both of a grammar (a rename from `.txt` to `.kt`) have no whitespace change: nothing proves one.
//! - **The lines** are those of the diffs (`TextLines`): a line ends at `\n`, `\r\n` or a lone `\r`, as the engine
//!   that made the blocks and the hunks counts them and as the tokens do. The lines of a declaration are git's (a
//!   line ends at `\n` alone, as the summaries count them) and are translated first (`TextLines::of_git`): in a file
//!   with a lone `\r` the two counts differ, and the lines of one read by the other are other lines.
//! - **A range of lines** is asked with the code around it (`equal_by_tokens`): lines without a token are whitespace
//!   only where the line breaks they add or remove change no token of the lines around them.
//!
//! The tokens of a blob are read once per grammar for the whole plan (`TextTokens`), when first asked: by
//! `whitespace_only` for a hunk that passed its check of blanks, and by the fingerprint of every unit of a source file
//! (`fingerprint_text`), which reads both sides of each such file. So a plan parses the two sides of every changed
//! source file that has a unit: about 0.1 s for the pairs of 141 files.

use crate::files::without_whitespace;
use code_symbols::highlight::{Grammar, grammar_of};
use code_symbols::tokens::{Equality, Tokens, content, equal_by_tokens, same_without_blanks, tokens};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

/// The tokens of a blob read as a grammar; `None` when it has none
type Read<'t> = HashMap<(&'t str, Grammar), Option<Rc<Tokens>>>;

/// The tokens of the texts of a plan (by blob), each read once per grammar, when first asked
pub(crate) struct TextTokens<'t> {
  texts: &'t HashMap<String, String>,
  read: RefCell<Read<'t>>,
}

/// A side of a change of a file of a grammar: the grammar of its path and its blob
type Side<'a> = Option<(Grammar, &'a str)>;

/// The two sides of a change of a file, as they are compared
pub(crate) enum Sides<'a, 't> {
  /// Neither side is of a grammar: compared by characters
  Characters,
  /// Both sides are of a grammar: compared by their tokens, read when first asked; a side without a blob has none
  Grammar { tokens: &'a TextTokens<'t>, old: Side<'a>, new: Side<'a> },
  /// Only one side is of a grammar: never the same
  Unknown,
}

/// The lines of a text as the diffs count them: a line ends at `\n`, `\r\n` or a lone `\r`
/// (`text_diff::apply::split_lines`), so the text has one more line than line ends
pub(crate) struct TextLines<'t> {
  /// Each line without its line end
  lines: Vec<&'t str>,
  /// The lines a lone `\r` ends: line ends of the diffs, none of git
  lone_cr: Vec<u32>,
}

impl<'t> TextLines<'t> {
  pub(crate) fn of(text: &'t str) -> Self {
    let (mut lines, mut lone_cr) = (Vec::new(), Vec::new());
    let mut rest = text;
    while let Some(end) = rest.find(['\n', '\r']) {
      let line_end = if rest[end..].starts_with("\r\n") {
        2
      } else {
        if rest[end..].starts_with('\r') {
          lone_cr.push(u32::try_from(lines.len()).unwrap_or(u32::MAX));
        }
        1
      };
      lines.push(&rest[..end]);
      rest = &rest[end + line_end..];
    }
    lines.push(rest);
    Self { lines, lone_cr }
  }

  /// Every line
  pub(crate) fn all(&self) -> &[&'t str] {
    &self.lines
  }

  /// The lines `lines` (from 0, the end excluded), as far as the text has them
  pub(crate) fn text(&self, lines: &Range<u32>) -> &[&'t str] {
    let (start, end) = ((lines.start as usize).min(self.lines.len()), (lines.end as usize).min(self.lines.len()));
    &self.lines[start..end.max(start)]
  }

  /// The lines that are `lines` as git counts them (a line ends at `\n` alone; from 0, the end excluded): the same
  /// lines unless a lone `\r` ends one
  pub(crate) fn of_git(&self, lines: Range<u32>) -> Range<u32> {
    // the line that starts the line `line` of git: each lone `\r` before it ends one more line here
    let at = |line: u32| {
      let mut view = line;
      for &ended in &self.lone_cr {
        if ended >= view {
          break;
        }
        view += 1;
      }
      view
    };
    let start = at(lines.start);
    start..at(lines.end).max(start)
  }
}

impl<'t> TextTokens<'t> {
  pub(crate) fn new(texts: &'t HashMap<String, String>) -> Self {
    Self {
      texts,
      read: RefCell::new(HashMap::new()),
    }
  }

  /// The tokens of the text of `blob` read as `grammar`; `None` without the text or without tokens
  fn of(&self, grammar: Grammar, blob: &str) -> Option<Rc<Tokens>> {
    let (id, text) = self.texts.get_key_value(blob)?;
    self
      .read
      .borrow_mut()
      .entry((id.as_str(), grammar))
      .or_insert_with(|| tokens(grammar, text).map(Rc::new))
      .clone()
  }

  /// The sides of a change from `old_blob` at `old_path` to `new_blob` at `new_path`
  pub(crate) fn sides<'a>(&'a self, old_path: &str, old_blob: Option<&'a str>, new_path: &str, new_blob: Option<&'a str>) -> Sides<'a, 't> {
    match (grammar_of(old_path), grammar_of(new_path)) {
      (None, None) => Sides::Characters,
      (Some(old), Some(new)) => Sides::Grammar {
        tokens: self,
        old: old_blob.map(|blob| (old, blob)),
        new: new_blob.map(|blob| (new, blob)),
      },
      _ => Sides::Unknown,
    }
  }
}

impl Sides<'_, '_> {
  /// The tokens of both sides; `None` for a file of no grammar and when a side has none
  fn read(&self) -> Option<(Rc<Tokens>, Rc<Tokens>)> {
    let Self::Grammar { tokens, old, new } = self else { return None };
    let side = |side: &Side<'_>| side.and_then(|(grammar, blob)| tokens.of(grammar, blob));
    Some((side(old)?, side(new)?))
  }

  /// Whether the lines `old` of the old side and `new` of the new one (lines of the diffs, from 0, the end
  /// excluded), whose text is `old_text` and `new_text`, differ in whitespace alone: changed lines between lines that
  /// did not change. A file of no grammar: the same text without its whitespace, unless its indentation has a
  /// meaning (`indented`). A file of a grammar: equal by tokens with the code around the lines, asked only of lines
  /// that are the same without their blanks, which lines equal by tokens are
  pub(crate) fn whitespace_only(&self, old: Range<u32>, new: Range<u32>, indented: bool, old_text: &[&str], new_text: &[&str]) -> bool {
    match self {
      Self::Characters => !indented && without_whitespace(old_text) == without_whitespace(new_text),
      Self::Grammar { .. } => {
        same_without_blanks(old_text.iter().copied(), new_text.iter().copied())
          && self
            .read()
            .is_some_and(|(old_tokens, new_tokens)| equal_by_tokens(&old_tokens, old, &new_tokens, new) == Equality::Same)
      }
      Self::Unknown => false,
    }
  }

  /// The text of the lines `lines` (of the diffs) of the old side, or of the new one (`new_side`), as a fingerprint
  /// reads it: the content of their tokens (`code_symbols::tokens::content`), the tokens and the tree between them,
  /// the same after a change of whitespace alone and wherever the lines stand, another after a line break that ends a
  /// statement; `None` without the tokens of both sides, whose reader takes the characters
  pub(crate) fn fingerprint_text(&self, new_side: bool, lines: Range<u32>) -> Option<String> {
    let (old, new) = self.read()?;
    let tokens = if new_side { &new } else { &old };
    Some(format!("{:016x}", content(tokens.in_lines(lines))))
  }
}
