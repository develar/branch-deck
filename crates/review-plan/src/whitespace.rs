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
//!   the file are a move, never whitespace. The plan folds it (`MechanicalKind::Moved`) only where the index of the
//!   repository says the place of the declaration has no meaning (`units::FreeOrder`), and reads it as a change of
//!   behaviour elsewhere.
//! - **Lines that moved to another file** are compared by the same content, read of each side alone (`code`, with its
//!   grammar): a declaration removed from a file and added to another with the same code says so (`moved_across`); the
//!   same but the modifiers of its reach says that too (`code_but_reach`). The `impl` blocks of Rust, which no summary
//!   declares, are read of the tokens (`impls`): a member moves with its header, and the lines of one the commits
//!   emptied go with its members.
//!
//! The tokens of a blob are read once per grammar for the whole plan (`TextTokens`), when first asked: by
//! `whitespace_only` for a hunk that passed its check of blanks, and by the fingerprint of every unit of a source file
//! (`fingerprint_text`), which reads both sides of each such file. So a plan parses the two sides of every changed
//! source file that has a unit: about 0.1 s for the pairs of 141 files. A file the commits add or delete is read only
//! where a move between files may name it (`moved_across`, "Cost").

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

/// The code of lines of a side (`Sides::code`): its grammar and the content of its tokens, equal for the same code in
/// any file of that grammar
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Code {
  grammar: Grammar,
  content: u64,
}

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

  /// The code of the lines `lines` (of the diffs) of the old side, or of the new one (`new_side`), wherever they stand
  /// and in whichever file: the grammar of the side and the content of their tokens (`code_tokens::content`), as
  /// `same_code` compares them, of a token at least that is no comment. `None` for a side of no grammar, without its
  /// blob or its tokens. Read of that side alone: a file the commits add or delete has its code too
  pub(crate) fn code(&self, new_side: bool, lines: Range<u32>) -> Option<Code> {
    let Self::Grammar { tokens, old, new } = self else { return None };
    let (grammar, blob) = if new_side { (*new)? } else { (*old)? };
    let read = tokens.of(grammar, blob)?;
    let run = read.in_lines(lines);
    run.iter().any(|token| !token.is_comment()).then(|| Code { grammar, content: content(run) })
  }

  /// The code of `lines` as `code` reads it, but without the modifiers of visibility (`visibility_modifiers`): the same
  /// for `fn helper` and `pub(crate) fn helper`, for `glob: String` and `pub(crate) glob: String`, and another for any
  /// other change. The rest of the tree stays between the tokens (what closes after each), but what opens before the
  /// first token of the code of each line: the node a modifier stands in opens there, before the modifier where it has
  /// one and before the next token where it has none. `None` where `code` is, and for a grammar of no such modifiers
  /// (TypeScript's `export` is no modifier of a declaration's reach but of its file, Starlark has none)
  pub(crate) fn code_but_reach(&self, new_side: bool, lines: Range<u32>) -> Option<Code> {
    let Self::Grammar { tokens, old, new } = self else { return None };
    let (grammar, blob) = if new_side { (*new)? } else { (*old)? };
    let words = visibility_words(grammar)?;
    let (read, text) = (tokens.of(grammar, blob)?, tokens.texts.get(blob)?);
    let run = read.in_lines(lines);
    let dropped = visibility_modifiers(run, text, grammar, words);
    let kept: Vec<&Token> = run.iter().enumerate().filter(|(index, _)| !dropped[*index]).map(|(_, token)| token).collect();
    let code = |token: &&Token| !token.is_comment();
    let (first, last) = (kept.iter().position(code)?, kept.iter().rposition(code)?);
    let mut hash = LOOSE_SEED;
    let mut line_started: Option<u32> = None;
    for (index, token) in kept.iter().enumerate() {
      hash = mix(mix(hash, token.kind), token.text);
      let first_of_line = !token.is_comment() && line_started != Some(token.line);
      if !token.is_comment() {
        line_started = Some(token.line);
      }
      if index != first && !first_of_line {
        hash = mix(hash, token.opens);
      }
      if index != last {
        hash = mix(hash, token.closes);
      }
    }
    Some(Code { grammar, content: hash })
  }

  /// The code of the line `line` (of the diffs) of the old side, or of the new one (`new_side`), where the line is whole
  /// items of its own: its brackets close on it, and its code ends with `;` (a call of a macro at the top of a file,
  /// `answer_metadata!(SearchAnswer, …);`). `None` for any other line, and as `code` is
  pub(crate) fn item_line_code(&self, new_side: bool, line: u32) -> Option<Code> {
    let Self::Grammar { tokens, old, new } = self else { return None };
    let (grammar, blob) = if new_side { (*new)? } else { (*old)? };
    let (read, text) = (tokens.of(grammar, blob)?, tokens.texts.get(blob)?);
    let run = read.in_lines(line..line + 1);
    // a token over several lines (a string, a block comment) makes the line no item of its own
    if run.iter().any(|token| token.line != line || token.end_line != line) {
      return None;
    }
    let mut depth = 0_i32;
    let mut last = None;
    for token in run.iter().filter(|token| !token.is_comment()) {
      let word = token.text_in(text);
      match word {
        "(" | "[" | "{" => depth += 1,
        ")" | "]" | "}" => depth -= 1,
        _ => {}
      }
      if depth < 0 {
        return None;
      }
      last = Some(word);
    }
    (depth == 0 && last == Some(";")).then(|| self.code(new_side, line..line + 1)).flatten()
  }

  /// The `impl` blocks of Rust of the old side, or of the new one (`new_side`), each with its header and its closing
  /// brace, in the order of the text; none for another grammar and a side without tokens. Read of the tokens: the
  /// summaries declare no `impl` (`code_symbols`, "impl blocks declare nothing themselves")
  pub(crate) fn impls(&self, new_side: bool) -> Vec<ImplBlock> {
    let Self::Grammar { tokens, old, new } = self else { return Vec::new() };
    let Some((grammar, blob)) = (if new_side { *new } else { *old }) else {
      return Vec::new();
    };
    if grammar != Grammar::Rust {
      return Vec::new();
    }
    let (Some(read), Some(text)) = (tokens.of(grammar, blob), tokens.texts.get(blob)) else {
      return Vec::new();
    };
    impl_blocks(read.all(), text)
  }

  /// Whether the lines `lines` (of the diffs) of the new side hold no code: no token but comments. No without the
  /// tokens of both sides
  pub(crate) fn no_code(&self, lines: Range<u32>) -> bool {
    self.read().is_some_and(|(_, new_tokens)| new_tokens.in_lines(lines).iter().all(Token::is_comment))
  }

  /// Whether the lines `lines` (of the diffs) of the old side, or of the new one (`new_side`), hold no code: no token
  /// but comments, or none. No for a side without tokens
  pub(crate) fn no_code_on(&self, new_side: bool, lines: Range<u32>) -> bool {
    let Self::Grammar { tokens, old, new } = self else { return false };
    let Some((grammar, blob)) = (if new_side { *new } else { *old }) else { return false };
    tokens.of(grammar, blob).is_some_and(|read| read.in_lines(lines).iter().all(Token::is_comment))
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

/// An `impl` block of Rust (`Sides::impls`): its header, from its first token (an attribute of it, `unsafe`) to its
/// opening brace, and its closing brace, by the lines of the diffs (from 0)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ImplBlock {
  /// The lines of the header, the end excluded: its last line holds the opening brace
  pub(crate) header: (u32, u32),
  /// The line of the closing brace
  pub(crate) close: u32,
  /// The content of the tokens of the header (`code_tokens::content`), comments aside: what a member of it moved with
  pub(crate) code: u64,
}

impl ImplBlock {
  /// Whether `lines` (of the diffs) stand inside its braces
  pub(crate) const fn holds(&self, lines: &Range<u32>) -> bool {
    self.header.1 <= lines.start + 1 && lines.end <= self.close + 1 && lines.start < lines.end
  }
}

/// The impl blocks of `all`, the tokens of `text` (`Sides::impls`): each pair of braces whose opening one ends an item
/// that is an `impl`, its attributes, `unsafe` and `default` before the keyword; a function that returns `impl Trait`
/// is none
fn impl_blocks(all: &[Token], text: &str) -> Vec<ImplBlock> {
  let is = |token: &Token, word: &str| !token.is_comment() && token.text_in(text) == word;
  let mut open: Vec<usize> = Vec::new();
  let mut blocks: Vec<ImplBlock> = Vec::new();
  for (index, token) in all.iter().enumerate() {
    if is(token, "{") {
      open.push(index);
    } else if is(token, "}")
      && let Some(opening) = open.pop()
      && let Some(start) = impl_header_start(all, text, opening)
    {
      let code: Vec<Token> = all[start..=opening].iter().filter(|token| !token.is_comment()).copied().collect();
      blocks.push(ImplBlock {
        header: (all[start].line, all[opening].end_line + 1),
        close: token.line,
        code: content(&code),
      });
    }
  }
  blocks.sort_by_key(|block| block.header);
  blocks
}

/// The first token of the item whose opening brace is `all[opening]` where that item is an `impl`: the item starts
/// after the `;`, `{` or `}` before it outside brackets, its attributes (`#[…]`) and the words `unsafe` and `default`
/// stand before `impl`
fn impl_header_start(all: &[Token], text: &str, opening: usize) -> Option<usize> {
  let word = |index: usize| all[index].text_in(text);
  // the start of the item: after the token that ends the one before it, outside brackets
  let mut depth = 0_i32;
  let mut start = 0;
  for index in (0..opening).rev() {
    if all[index].is_comment() {
      continue;
    }
    match word(index) {
      ")" | "]" => depth += 1,
      "(" | "[" => depth -= 1,
      ";" | "{" | "}" if depth == 0 => {
        start = index + 1;
        break;
      }
      _ => {}
    }
  }
  let code: Vec<usize> = (start..opening).filter(|&index| !all[index].is_comment()).collect();
  let mut at = 0;
  // the attributes of the item, `#[…]` and `#![…]`
  while code.get(at).is_some_and(|&index| word(index) == "#") {
    at += 1;
    if code.get(at).is_some_and(|&index| word(index) == "!") {
      at += 1;
    }
    let mut depth = 0_i32;
    while let Some(&index) = code.get(at) {
      at += 1;
      match word(index) {
        "[" => depth += 1,
        "]" => {
          depth -= 1;
          if depth == 0 {
            break;
          }
        }
        _ => {}
      }
    }
  }
  while code.get(at).is_some_and(|&index| matches!(word(index), "unsafe" | "default")) {
    at += 1;
  }
  let keyword = *code.get(at)?;
  if word(keyword) != "impl" || all[keyword].class != TokenClass::Keyword {
    return None;
  }
  // its first token: the first of its code, after the comments right above it, a doc comment of it
  let mut first = code.first().copied().unwrap_or(keyword);
  while first > start && all[first - 1].is_comment() && all[first - 1].end_line + 1 >= all[first].line {
    first -= 1;
  }
  Some(first)
}

/// The words of the modifiers of the reach of a declaration in `grammar`; `None` for a grammar of none
/// (`Sides::code_but_reach`)
const fn visibility_words(grammar: Grammar) -> Option<&'static [&'static str]> {
  match grammar {
    Grammar::Rust => Some(&["pub"]),
    Grammar::Kotlin | Grammar::KotlinScript => Some(&["public", "private", "protected", "internal"]),
    Grammar::Java => Some(&["public", "private", "protected"]),
    Grammar::CSharp => Some(&["public", "private", "protected", "internal", "file"]),
    _ => None,
  }
}

/// Per token of `run`, whether it is a modifier of a reach: a keyword of `words`, and in Rust the brackets after `pub`
/// that hold `crate`, `self`, `super` or `in …` (`pub(crate)`, `pub(in a::b)`), never those of a tuple field
/// (`pub (u8, u8)`)
fn visibility_modifiers(run: &[Token], text: &str, grammar: Grammar, words: &[&str]) -> Vec<bool> {
  let mut dropped = vec![false; run.len()];
  let word = |index: usize| run.get(index).filter(|token| !token.is_comment()).map(|token| token.text_in(text));
  let mut index = 0;
  while index < run.len() {
    let token = &run[index];
    if token.class != TokenClass::Keyword || !words.contains(&token.text_in(text)) {
      index += 1;
      continue;
    }
    dropped[index] = true;
    index += 1;
    if grammar == Grammar::Rust && word(index) == Some("(") && matches!(word(index + 1), Some("crate" | "self" | "super" | "in")) {
      while index < run.len() {
        dropped[index] = true;
        index += 1;
        if word(index - 1) == Some(")") {
          break;
        }
      }
    }
  }
  dropped
}

/// The seed of `Sides::code_but_reach`
const LOOSE_SEED: u64 = 0x2545_f491_4f6c_dd1d;

/// Two hashes as one, in order (`MurmurHash3`'s finalizer over their mix)
const fn mix(hash: u64, value: u64) -> u64 {
  let mut mixed = (hash ^ value.rotate_left(29)).wrapping_mul(0xff51_afd7_ed55_8ccd);
  mixed ^= mixed >> 33;
  mixed = mixed.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
  mixed ^ (mixed >> 33)
}
