//! Whether a change is of whitespace alone (openspec/specs/branch/review-plan/spec.md, "Mechanical changes"), the one answer of
//! the files, the blocks and the declarations of a plan.
//!
//! - **A file of a grammar** (`code_tokens::grammar_of`) is compared by its tokens
//!   (`code_tokens::equal_by_tokens`, openspec/specs/commit/equal-by-tokens/spec.md): the same code and the
//!   same comments, so a space inside a string, a line break that ends a statement and an indent of Starlark are no
//!   whitespace, and a wrapped call, a line of Starlark continued inside its brackets, a byte order mark removed are.
//!   The rule the diffs mark a block of formatting by. `whitespace_only` asks it only of lines that are the same
//!   without their blanks (`code_tokens::same_without_blanks`), which lines equal by tokens always are.
//! - **A file of no grammar** keeps the rule of characters: the same text without its whitespace, never where the
//!   indentation has a meaning (`code_tokens::indentation_matters`: Python, YAML, a Makefile).
//! - **A file of a grammar without tokens** (nested too deeply, a script read only after a recovery) and a file whose
//!   two sides are not both of a grammar (a rename from `.txt` to `.kt`) have no whitespace change: nothing proves one.
//! - **The lines** are those of the diffs (`line_diff::text_lines::TextLines`): a line ends at `\n`, `\r\n` or a lone `\r`, as the engine
//!   that made the blocks and the hunks counts them and as the tokens do. The lines of a declaration are git's (a
//!   line ends at `\n` alone, as the summaries count them) and are translated first (`TextLines::of_git`): in a file
//!   with a lone `\r` the two counts differ, and the lines of one read by the other are other lines.
//! - **A range of lines** is asked with the code around it (`equal_by_tokens`): lines without a token are whitespace
//!   only where the line breaks they add or remove change no token of the lines around them.
//! - **Lines that moved** are asked without the code around them (`same_code`): the same tokens at another place of
//!   the file are a move, never whitespace. The plan folds it (`MechanicalKind::Moved`) only where the place of the
//!   declaration has no meaning (`moved`), and reads it as a change of behaviour elsewhere.
//!
//! The tokens of a blob are read once per grammar for the whole plan (`TextTokens`), when first asked: by
//! `whitespace_only` for a hunk that passed its check of blanks, and by the fingerprint of every unit of a source file
//! (`fingerprint_text`), which reads both sides of each such file. So a plan parses the two sides of every changed
//! source file that has a unit: about 0.1 s for the pairs of 141 files.

use crate::files::without_whitespace;
use code_tokens::{Equality, Token, TokenClass, Tokens, content, equal_by_tokens, same_without_blanks, tokens};
use code_tokens::{Grammar, grammar_of};
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

  /// Whether the lines `old` of the old side and `new` of the new one (lines of the diffs) are the same code
  /// wherever they stand: lines that moved, asked without the code around them, which is another at another place.
  /// The same content of their tokens (`code_tokens::content`: the tokens, comments too, and the tree
  /// between them, without what opens before the first and closes after the last), of at least one token: in a file
  /// of Starlark the indent of a line is in its tokens, so lines that moved to another nesting are not the same.
  /// Only the tokens tell: no for a file of no grammar, which has no declaration to ask it of, and for a side
  /// without tokens
  pub(crate) fn same_code(&self, old: Range<u32>, new: Range<u32>) -> bool {
    self.read().is_some_and(|(old_tokens, new_tokens)| {
      let (old_tokens, new_tokens) = (old_tokens.in_lines(old), new_tokens.in_lines(new));
      !old_tokens.is_empty() && !new_tokens.is_empty() && content(old_tokens) == content(new_tokens)
    })
  }

  /// The grammar both sides are read by; `None` for a file of no grammar and for sides of two grammars
  pub(crate) fn grammar(&self) -> Option<Grammar> {
    match self {
      Self::Grammar {
        old: Some((old, _)),
        new: Some((new, _)),
        ..
      } if old == new => Some(*old),
      _ => None,
    }
  }

  /// Whether the lines `lines` (of the diffs) of the new side hold no code: no token but comments. No without the
  /// tokens of both sides
  pub(crate) fn no_code(&self, lines: Range<u32>) -> bool {
    self.read().is_some_and(|(_, new_tokens)| new_tokens.in_lines(lines).iter().all(Token::is_comment))
  }

  /// Whether the code of the lines `lines` (of the diffs) of the new side holds no token `text`: none of its tokens
  /// that is neither a name, a keyword, a literal (a string, the text of a JSX element) nor a comment is that text.
  /// No without the tokens of both sides
  pub(crate) fn no_token(&self, lines: Range<u32>, text: &str) -> bool {
    let Self::Grammar { tokens, new: Some((_, blob)), .. } = self else {
      return false;
    };
    let Some(source) = tokens.texts.get(*blob) else { return false };
    self.read().is_some_and(|(_, new_tokens)| {
      let mut code = new_tokens.in_lines(lines).iter().filter(|token| token.class == TokenClass::Other);
      !code.any(|token| token.text_in(source) == text)
    })
  }

  /// The text of the lines `lines` (of the diffs) of the old side, or of the new one (`new_side`), as a fingerprint
  /// reads it: the content of their tokens (`code_tokens::content`), the tokens and the tree between them,
  /// the same after a change of whitespace alone and wherever the lines stand, another after a line break that ends a
  /// statement; `None` without the tokens of both sides, whose reader takes the characters
  pub(crate) fn fingerprint_text(&self, new_side: bool, lines: Range<u32>) -> Option<String> {
    let (old, new) = self.read()?;
    let tokens = if new_side { &new } else { &old };
    Some(format!("{:016x}", content(tokens.in_lines(lines))))
  }
}
