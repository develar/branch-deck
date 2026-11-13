//! What every walk of a tree writes its tokens with, whatever its parser (`Builder`): the tokens in the order of the
//! text with their lines and columns of the view, the nodes of the tree around each as its shape, the text of a
//! comment as it compares, and the hashes all of it is kept as.
//!
//! - **The hashes** are of this module alone, the same across runs, machines and compilers: FNV-1a over the bytes of a
//!   text, a multiply-xorshift mix (`MurmurHash3`'s finalizer) to combine. A kind of a token or a node is the hash of
//!   its name (`Kinds`), never of its number, so a parser that gains a kind moves no key.
//! - **The shape** of a token (`Shape`): the events of the tree between two tokens, a node opening, a node closing, a
//!   leaf without text (a missing token, an `INDENT`), an error the parser reported there, are cut after the last
//!   close of a node that was open at the first token. What is before the cut is the first token's `closes`, the rest
//!   the second's `opens`. So a statement carries its own nodes whole, wherever it stands: wrapped into a block, it
//!   has the same keys.
//! - **What must hold of the tokens** (`Builder::finish` answers `None` otherwise): they are in the order of the text,
//!   none empty, each on the boundaries of characters (Roslyn lexes UTF-16 units and may cut a character of two), and
//!   what stands between two of them is blank (`is_blank`). So a walk that left a piece of the text out, or a
//!   parser that skipped what no reader calls whitespace, gives no tokens instead of tokens that prove nothing, and
//!   texts with the same tokens are the same without their blanks by construction.
//! - **An irregular blank** (`Builder::irregular`): a blank the parser skips and its language does not call whitespace
//!   (oxc reads U+0085 and U+200B as the scanner of TypeScript does, an engine of JavaScript calls them a syntax
//!   error) is an error the parser did not report. It is part of what opens before the next token of the code, after
//!   the last one of what closes after it, so a text with it is another than the text without it, and a text of such
//!   blanks alone has no tokens.

use crate::{Token, TokenClass, Tokens, is_blank};
use std::ops::Range;

pub(crate) const SEED: u64 = 0x9e37_79b9_7f4a_7c15;
const OPEN: u64 = 1;
const CLOSE: u64 = 2;
const EMPTY: u64 = 3;
const CODE: u64 = 4;
const COMMENT: u64 = 5;
const INDENT: u64 = 6;
/// A punctuation mark with another right after it (`joint`)
const JOINT: u64 = 7;
/// An error the parser reported (`Builder::error`)
const ERROR: u64 = 8;
/// A blank the parser skipped that its language does not call whitespace (`Builder::irregular`)
const IRREGULAR: u64 = 9;

/// `hash` with `value` mixed in, in order: `mix(mix(h, a), b)` is not `mix(mix(h, b), a)`
pub(crate) const fn mix(hash: u64, value: u64) -> u64 {
  let mut mixed = (hash.rotate_left(23) ^ value).wrapping_mul(0xff51_afd7_ed55_8ccd);
  mixed ^= mixed >> 33;
  mixed = mixed.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
  mixed ^ (mixed >> 33)
}

/// The hash of a text: FNV-1a over its bytes, mixed with its length
pub(crate) fn hash_bytes(bytes: &[u8]) -> u64 {
  let mut hash = 0xcbf2_9ce4_8422_2325u64;
  for byte in bytes {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(0x0100_0000_01b3);
  }
  mix(hash, bytes.len() as u64)
}

/// The kind of a punctuation mark that another follows with nothing between them: another token than the mark alone
pub(crate) const fn joint(kind: u64) -> u64 {
  mix(kind, JOINT)
}

/// The hashes of the kinds of a grammar by their numbers, each made once from its name
#[derive(Default)]
pub(crate) struct Kinds(Vec<u64>);

impl Kinds {
  /// The hash of the kind `index`, whose name `name` gives
  pub(crate) fn of<N: AsRef<str>>(&mut self, index: usize, name: impl FnOnce() -> N) -> u64 {
    if index >= self.0.len() {
      self.0.resize(index + 1, 0);
    }
    if self.0[index] == 0 {
      // 0 marks a kind not yet named
      self.0[index] = hash_bytes(name().as_ref().as_bytes()).max(1);
    }
    self.0[index]
  }
}

/// The events of the tree since the last token (the module's "The shape")
struct Shape {
  /// The events up to the last close of a node that was open at the last token
  after: u64,
  /// The events since
  pending: u64,
  depth: u32,
  /// How many of the open nodes were open at the last token
  token_depth: u32,
}

impl Shape {
  const fn new() -> Self {
    Self {
      after: SEED,
      pending: SEED,
      depth: 0,
      token_depth: 0,
    }
  }

  const fn open(&mut self, kind: u64) {
    self.depth += 1;
    self.pending = mix(mix(self.pending, OPEN), kind);
  }

  const fn close(&mut self, kind: u64) {
    let event = mix(mix(self.pending, CLOSE), kind);
    if self.depth <= self.token_depth {
      self.after = mix(self.after, event);
      self.pending = SEED;
      self.token_depth = self.depth.saturating_sub(1);
    } else {
      self.pending = event;
    }
    self.depth = self.depth.saturating_sub(1);
  }

  const fn empty(&mut self, kind: u64) {
    self.pending = mix(mix(self.pending, EMPTY), kind);
  }

  const fn error(&mut self, message: u64) {
    self.pending = mix(mix(self.pending, ERROR), message);
  }

  /// A token: what closes after the token before it and what opens before this one
  const fn token(&mut self) -> (u64, u64) {
    let events = (self.after, self.pending);
    self.after = SEED;
    self.pending = SEED;
    self.token_depth = self.depth;
    events
  }
}

/// Writes the tokens of `text` as a walk of its tree names them, in the order of the text
pub(crate) struct Builder<'t> {
  text: &'t str,
  /// Where the cursor is: its byte, its line, its UTF-16 column in the line, the byte the line starts at
  pos: usize,
  line: u32,
  column: u32,
  line_start: usize,
  shape: Shape,
  /// The last token of the code: the next token, or the end, gives it what closes after it
  last_code: Option<usize>,
  /// The next token of the code starts a logical line of a grammar whose indentation is its syntax
  logical_line: bool,
  /// The errors the parser reported on the next comment (`comment_error`)
  comment_errors: u64,
  /// The byte after the last token: what stands between it and the next token is blank
  covered: usize,
  /// Whether a blank is irregular: skipped by the parser, no whitespace of its language (`irregular`)
  is_irregular: fn(char) -> bool,
  /// The irregular blanks since the last token of the code, in their order; `SEED` without one
  irregular: u64,
  /// A token did not hold what it must (the module's "What must hold of the tokens"): no tokens
  broken: bool,
  out: Tokens,
}

impl<'t> Builder<'t> {
  pub(crate) fn new(text: &'t str) -> Self {
    Self {
      text,
      pos: 0,
      line: 0,
      column: 0,
      line_start: 0,
      shape: Shape::new(),
      last_code: None,
      logical_line: false,
      comment_errors: SEED,
      covered: 0,
      is_irregular: |_| false,
      irregular: SEED,
      broken: false,
      out: Tokens {
        all: Vec::with_capacity(text.len() / 6),
        line_starts: vec![0],
        len: u32::try_from(text.len()).unwrap_or(u32::MAX),
      },
    }
  }

  /// The blanks the parser of this text skips between two tokens though its language calls them no whitespace: each
  /// is an error of the text at its place (the module's "An irregular blank")
  pub(crate) fn irregular(mut self, is_irregular: fn(char) -> bool) -> Self {
    self.is_irregular = is_irregular;
    self
  }

  /// A node of the tree opens
  pub(crate) const fn open(&mut self, kind: u64) {
    self.shape.open(kind);
  }

  /// The innermost open node closes
  pub(crate) const fn close(&mut self, kind: u64) {
    self.shape.close(kind);
  }

  /// A leaf of the tree without text: a token the parser missed, a mark of the lexer (`INDENT`, a statement break)
  pub(crate) const fn empty(&mut self, kind: u64) {
    self.shape.empty(kind);
  }

  /// An error the parser reported at this place of the walk, by the hash of its message: where the parser keeps its
  /// errors beside the tree, or names them by a message the tree does not hold, two texts of one tree are two programs
  /// (a line break before the `=>` of an arrow function is an error of JavaScript and changes no node)
  pub(crate) const fn error(&mut self, message: u64) {
    self.shape.error(message);
  }

  /// An error the parser reported on the comment that comes next, by the hash of its message: part of what that
  /// comment compares by, no code (a documentation comment of C# whose XML does not close)
  pub(crate) const fn comment_error(&mut self, message: u64) {
    self.comment_errors = mix(mix(self.comment_errors, ERROR), message);
  }

  /// The next token of the code starts a logical line whose indent is part of it (Starlark)
  pub(crate) const fn logical_line(&mut self) {
    self.logical_line = true;
  }

  /// A token of the code at `range`, which compares by its bytes
  pub(crate) fn code(&mut self, class: TokenClass, kind: u64, range: Range<usize>) {
    let text = hash_bytes(self.text.as_bytes().get(range.clone()).unwrap_or_default());
    self.code_as(class, kind, range, text);
  }

  /// A token of the code at `range` that compares by `text`, the hash of what its bytes mean
  pub(crate) fn code_as(&mut self, class: TokenClass, kind: u64, range: Range<usize>, text: u64) {
    let (closes, mut opens) = self.shape.token();
    self.finish_last(closes);
    if !self.take(&range) {
      return;
    }
    let irregular = std::mem::replace(&mut self.irregular, SEED);
    if irregular != SEED {
      opens = mix(opens, irregular);
    }
    let (line, start) = self.advance(range.start);
    let mut kind = mix(mix(SEED, CODE), kind);
    if std::mem::take(&mut self.logical_line) {
      kind = mix(mix(kind, INDENT), hash_bytes(self.indent()));
    }
    let (end_line, end) = self.end_of(&range);
    self.last_code = Some(self.out.all.len());
    // what closes after the token waits for the next one (`finish_last`)
    self.out.all.push(Token {
      class,
      line,
      start,
      end_line,
      end,
      byte_start: offset(range.start),
      byte_end: offset(range.end),
      kind,
      opens,
      closes: SEED,
      text,
    });
  }

  /// A comment at `range`, outside the tree: it compares by its text without the whitespace that ends its lines and
  /// without the indent of the line it starts on at the start of its other lines, and by the errors the parser
  /// reported on it (`comment_error`)
  pub(crate) fn comment(&mut self, range: Range<usize>) {
    // a lexer may end a line comment after the `\r` of its line end
    let bytes = self.text.as_bytes().get(range.clone()).unwrap_or_default();
    let range = range.start..range.start + bytes.trim_ascii_end().len();
    let errors = std::mem::replace(&mut self.comment_errors, SEED);
    if !self.take(&range) {
      return;
    }
    let (line, start) = self.advance(range.start);
    let indent = self.indent().len();
    let mut text = mix(mix(SEED, COMMENT), errors);
    for (index, row) in self.text.as_bytes()[range.clone()].split(|byte| *byte == b'\n').enumerate() {
      let row = if index == 0 {
        row
      } else {
        let shared = row.iter().take(indent).take_while(|byte| matches!(byte, b' ' | b'\t')).count();
        &row[shared..]
      };
      text = mix(text, hash_bytes(row.trim_ascii_end()));
    }
    let (end_line, end) = self.advance(range.end);
    self.out.all.push(Token {
      class: TokenClass::Comment,
      line,
      start,
      end_line,
      end,
      byte_start: offset(range.start),
      byte_end: offset(range.end),
      kind: mix(SEED, COMMENT),
      opens: SEED,
      closes: SEED,
      text,
    });
  }

  /// The tokens, once every node is closed; `None` when one of them did not hold what it must
  pub(crate) fn finish(mut self) -> Option<Tokens> {
    let (mut closes, _) = self.shape.token();
    let blank = self.blank_until(self.text.len());
    // an irregular blank after the last token of the code is of what closes after it; without any token of the code
    // it has no place
    if self.irregular != SEED {
      closes = mix(closes, self.irregular);
      self.broken |= self.last_code.is_none();
    }
    self.finish_last(closes);
    if self.broken || !blank {
      return None;
    }
    self.advance(self.text.len());
    self.out.all.shrink_to_fit();
    Some(self.out)
  }

  /// Takes `range` as the next token: whether it is one (the module's "What must hold of the tokens"). After a range
  /// that is none there are no tokens
  fn take(&mut self, range: &Range<usize>) -> bool {
    let held = range.start < range.end && self.text.is_char_boundary(range.end) && self.blank_until(range.start);
    if held {
      self.covered = range.end;
    } else {
      self.broken = true;
    }
    held
  }

  /// Whether what stands between the last token and the byte `to` is blank, `to` at or after that token and on the
  /// boundary of a character; its irregular blanks are kept for the next token of the code (`irregular`)
  fn blank_until(&mut self, to: usize) -> bool {
    let Some(between) = self.text.get(self.covered..to) else { return false };
    for character in between.chars() {
      if !is_blank(character) {
        return false;
      }
      if (self.is_irregular)(character) {
        self.irregular = mix(mix(self.irregular, IRREGULAR), u64::from(character));
      }
    }
    true
  }

  /// Gives the last token of the code what closes after it: its shape is whole
  fn finish_last(&mut self, closes: u64) {
    if let Some(token) = self.last_code.take().and_then(|index| self.out.all.get_mut(index)) {
      token.closes = closes;
    }
  }

  /// The whitespace the line of the cursor starts with
  fn indent(&self) -> &'t [u8] {
    let line = &self.text.as_bytes()[self.line_start..];
    &line[..line.iter().take_while(|byte| matches!(byte, b' ' | b'\t')).count()]
  }

  /// Moves the cursor to the end of the token at `range` and answers where the token ends: after its last character,
  /// and for a token whose bytes end with a line break (a line of a raw string of Kotlin, the text of a branch of
  /// `#if` not taken) at the end of the line that break ends, the last line the token has a byte in
  fn end_of(&mut self, range: &Range<usize>) -> (u32, u32) {
    let line_break = match &self.text.as_bytes()[range.clone()] {
      [.., b'\r', b'\n'] => 2,
      [.., b'\n' | b'\r'] => 1,
      _ => 0,
    };
    let end = self.advance(range.end - line_break);
    self.advance(range.end);
    end
  }

  /// Moves the cursor to the byte `to`, never back: its line and its UTF-16 column there. A line ends at `\n` and at a
  /// `\r` that no `\n` follows, as `buildLineTable` of the view splits them; a place inside a `\r\n` is the end of its
  /// line
  fn advance(&mut self, to: usize) -> (u32, u32) {
    let text = self.text.as_bytes();
    let to = to.min(text.len());
    while self.pos < to {
      let byte = text[self.pos];
      self.pos += 1;
      if byte == b'\n' || (byte == b'\r' && text.get(self.pos) != Some(&b'\n')) {
        self.line += 1;
        self.column = 0;
        self.line_start = self.pos;
        self.out.line_starts.push(offset(self.pos));
      } else {
        // the UTF-16 length of the character that starts here: a 4-byte sequence is a surrogate pair, a continuation
        // byte none, nor the `\r` of a `\r\n`, which is of the line break
        self.column += match byte {
          0x80..=0xbf | b'\r' => 0,
          0xf0.. => 2,
          _ => 1,
        };
      }
    }
    (self.line, self.column)
  }
}

fn offset(byte: usize) -> u32 {
  u32::try_from(byte).unwrap_or(u32::MAX)
}
