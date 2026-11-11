//! The tokens of one file as its parser read them, and whether two pieces of two texts are the same code
//! (openspec/specs/commit/equal-by-tokens/spec.md; docs/design/change-review.md, "Equal by tokens"): what tells a
//! change of formatting, which no reader has to read, from a change of the code. Of every grammar of
//! `highlight::Grammar`; a file of no grammar has no tokens, and its reader keeps a rule of characters.
//!
//! - **The proof**: two texts with the same tokens parse to the same tree, so they are the same program. A token
//!   therefore is a leaf of the tree the parser made, never a role of the highlighting (`LineRuns` holds only the
//!   changes of style, `> >` and `>>` are one run there) and never the lexer's own token where the parser decides what
//!   a token is: psi-syntax's builders join `?.`, `?:`, `!!` (Kotlin) and `>>`, `>>>`, `>>=` (Java, C#) from single
//!   characters only when nothing stands between them and remap the soft keywords, oxc lexes a `/` as a division or a
//!   regular expression and a `}` as the end of a block or of a template's hole by the parse, rust-analyzer glues `>>`
//!   and `=>` where the grammar reads them. So `a - -b` and `a--b`, `> >` and `>>` are different tokens.
//! - **The tree is in the token** (`Token::shape`): each token carries the nodes of the tree that open before it
//!   (`opens`) and those that close after it (`closes`), by their names, as far as the token before and the one after.
//!   A sequence of tokens with their shapes is the tree written out, so equal tokens are equal trees by construction,
//!   whatever the parser reads from the whitespace: the end of a statement at a line break in Kotlin, JavaScript and
//!   Starlark (`return⏎foo()` closes the `return` before `foo`, `return foo()` does not), a label only without a space
//!   (`return@label x`), a lambda after a name only on its line (`foo⏎{ … }` is no call). No rule here names a
//!   grammar's line breaks or brackets: a line break inside `()` changes no node and is formatting, one that ends a
//!   statement changes the nodes around it and is not.
//! - **The errors are in the tokens** too: every error the parser reported is an event of the tree at its place, by
//!   its message. oxc and Roslyn keep their errors in a list beside the tree, and a line break can be an error that
//!   changes no node (before the `=>` of an arrow function, after a `throw`; no space before the file name of a
//!   `#line` with a span); an error node of PSI has a message its kind does not say. So two texts with the same tokens
//!   are the same tree with the same errors: broken alike. An error on a comment (a documentation comment of C# whose
//!   XML does not close) is part of what the comment compares by, not of the code.
//! - **Text**: a token compares by its bytes. A string, a character, a text block, the text of a template and of an
//!   interpolated string, a regular expression are tokens of their own, so the whitespace inside them is content. Only
//!   the text of a JSX element compares as JSX reads it (`js::tokens::jsx_text`): the lines trimmed and joined, a text
//!   of whitespace with a line break nothing at all, the spaces inside a line kept.
//! - **Comments** are tokens of the class `Comment`, outside the tree: one compares by its text without the
//!   whitespace that ends its lines and without the indent of the line it starts on at the start of its other lines, so
//!   a block comment that moved with its code is the same comment, and `//go:build` and `// go:build` are not.
//! - **Where the indentation is the syntax** (Starlark: `indentation_matters`), the indent of each logical line is
//!   part of its first token, beside the blocks the parser made of the indents, which are in the shapes: a tab is not
//!   a number of spaces to every reader of such a file.
//! - **Rust's macros** read tokens the parser leaves alone: inside a token tree a punctuation mark with another right
//!   after it is another token than one before a space (`Spacing::Joint`), as a macro sees them.
//! - **Blanks** (`is_blank`): what stands between two tokens is whitespace or a character a parser skips as such (a
//!   byte order mark, a left-to-right mark of Rust, the Ctrl-Z of C#, the zero width space oxc skips), checked for
//!   every text (`build::Builder`). So texts with the same tokens are the same text without its blanks
//!   (`same_without_blanks`), what a reader asks before it reads tokens. A blank a parser skips though its language
//!   calls it no whitespace (U+0085 and U+200B of JavaScript and TypeScript: a syntax error to an engine, to esbuild
//!   and to Babel) is an error of the text at its place, in the tokens as any other.
//! - **A range of lines** (`equal_by_tokens`) is the same only when the code around it reads the same too. A line
//!   without a token can change the tokens of the lines around it, which no range of changed lines holds: a blank
//!   line after the `\` that continues a line of Starlark ends the statement, one before a `#!` makes code of it. So
//!   the token of the code before the range must close the same nodes, and the one after it must be the same token
//!   with the same nodes opening before it (`same_before`, `same_after`). A lexer reads forward: what stands before a
//!   token can change what it is, what stands after it only what closes after it.
//! - **What has no tokens** (`tokens` answers `None`, `equal_by_tokens` of such a text is `Different`): a text over
//!   `MAX_BYTES`; one nested deeper than its parser reads (psi-syntax's error "Code is too deeply nested" and Roslyn's
//!   "insufficient stack", whose rest is the lexer's tokens or one token; rust-analyzer's and oxc's stack); a script
//!   file oxc read only after a recovery, whose blanked regions have no tokens; a text whose tokens are not what
//!   `build::Builder` asks of them (a C# text whose tree dropped a token or cut a character in two). Any other
//!   syntax error is in the tokens, so two texts with the same tokens are broken alike.
//!
//! A pure function of the text, like the highlighting, with its own parse. `Tokens` holds no text and no tree, so a
//! reader keeps it per blob and grammar (`tokens_version` for a stored one).

pub(crate) mod build;
#[cfg(any(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark"))]
pub(crate) mod psi;

use crate::highlight::{Grammar, grammar_of};
use std::ops::Range;

/// The version of the rules of the tokens; bump it on any rule change, so that stored tokens and what was derived from
/// them are made again
pub const RULES_VERSION: u32 = 1;
/// Texts above this size have no tokens: `extract::MAX_BYTES`, the limit of the summaries
pub const MAX_BYTES: usize = crate::extract::MAX_BYTES;

/// What a token is to a reader of a change; the grammar's own kind of it is in its `shape`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum TokenClass {
  /// A comment, a doc comment whole: no code
  Comment,
  /// An identifier: what a rename changes
  Name,
  Keyword,
  /// A string, a character, a text of a template, a regular expression, the text of a JSX element: its whitespace is
  /// content
  Literal,
  /// A number, an operator, a bracket, a separator
  Other,
}

/// One token of a text. The lines are those of the view (`\n`, `\r\n`, a lone `\r`; from 0), the columns UTF-16 code
/// units in the line, as the runs of the highlighting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
  pub class: TokenClass,
  /// The line it starts on
  pub line: u32,
  /// Its start in that line
  pub start: u32,
  /// The last line it has a byte in: another one for a block comment, a raw string, a template
  pub end_line: u32,
  /// Its end in that line. A token whose bytes end with a line break (a line of a raw string of Kotlin, the text of a
  /// branch of `#if` not taken) ends where the line of that break does, so it is in every line it names
  pub end: u32,
  /// Its bytes in the text
  pub byte_start: u32,
  pub byte_end: u32,
  /// What the token is to its grammar beside its bytes: the kind the parser gave it, and where the indentation is the
  /// syntax the indent of the logical line it starts. Of a comment, one constant
  pub kind: u64,
  /// The tree before it: the nodes that open since the token before it, the errors the parser reported there and the
  /// blanks before it that are an error of its language. Of a comment, one constant
  pub opens: u64,
  /// The tree after it: the nodes that close before the next token. Of a comment, one constant
  pub closes: u64,
  /// Its text as it compares: the hash of its bytes, of a comment and of the text of a JSX element the hash of what
  /// they read as. The same wherever the token stands
  pub text: u64,
}

impl Token {
  /// Where the token stands in the tree: its kind, what opens before it and what closes after it. Two names of one
  /// shape and two texts are a rename's candidates
  pub const fn shape(&self) -> u64 {
    build::mix(build::mix(self.kind, self.opens), self.closes)
  }

  /// What the token is: its shape and its text. Two tokens are the same token when their keys are equal
  pub const fn key(&self) -> u64 {
    build::mix(self.shape(), self.text)
  }

  /// Whether `other` is the same token: the same shape and the same text
  pub const fn same(&self, other: &Self) -> bool {
    self.kind == other.kind && self.opens == other.opens && self.closes == other.closes && self.text == other.text
  }

  /// Its bytes in the text
  pub const fn bytes(&self) -> Range<usize> {
    self.byte_start as usize..self.byte_end as usize
  }

  /// Its characters in the text it was read from
  pub fn text_in<'t>(&self, text: &'t str) -> &'t str {
    text.get(self.bytes()).unwrap_or_default()
  }

  /// Its columns on `line`, which is `line_length` UTF-16 code units long: a token over several lines takes each of
  /// them to its end; `None` for a line it is not on. Empty for a token that is the line break of `line` alone
  pub fn columns_on(&self, line: u32, line_length: u32) -> Option<Range<u32>> {
    (self.line..=self.end_line).contains(&line).then(|| {
      let start = if line == self.line { self.start } else { 0 };
      let end = if line == self.end_line { self.end } else { line_length };
      start..end.max(start)
    })
  }

  pub const fn is_comment(&self) -> bool {
    matches!(self.class, TokenClass::Comment)
  }
}

/// The tokens of a text in its order, comments among them; whitespace is no token
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Tokens {
  pub(crate) all: Vec<Token>,
  /// The byte each line of the view starts at
  pub(crate) line_starts: Vec<u32>,
  /// The length of the text in bytes
  pub(crate) len: u32,
}

impl Tokens {
  /// Every token
  pub fn all(&self) -> &[Token] {
    &self.all
  }

  /// The lines of the text as the view counts them: one more than its line ends
  pub fn line_count(&self) -> u32 {
    u32::try_from(self.line_starts.len()).unwrap_or(u32::MAX)
  }

  /// The bytes of `lines` (from 0, the end excluded), their line ends included
  pub fn line_bytes(&self, lines: Range<u32>) -> Range<u32> {
    let at = |line: u32| self.line_starts.get(line as usize).copied().unwrap_or(self.len);
    let start = at(lines.start);
    start..at(lines.end).max(start)
  }

  /// The tokens of `lines` (from 0, the end excluded): every token with a byte in them, so a comment or a string over
  /// several lines is a token of each of its lines; of no lines, the token that runs across that place
  pub fn in_lines(&self, lines: Range<u32>) -> &[Token] {
    &self.all[self.places_in_lines(lines)]
  }

  /// The places in `all` of the tokens of `lines` (`in_lines`): the tokens before them and after them are `all` up
  /// to the start and from the end
  pub fn places_in_lines(&self, lines: Range<u32>) -> Range<usize> {
    self.places_in_bytes(self.line_bytes(lines))
  }

  /// The tokens with a byte in `bytes`; of an empty range, the token that holds its place inside
  pub fn in_bytes(&self, bytes: Range<u32>) -> &[Token] {
    &self.all[self.places_in_bytes(bytes)]
  }

  /// The places in `all` of the tokens of `bytes` (`in_bytes`)
  fn places_in_bytes(&self, bytes: Range<u32>) -> Range<usize> {
    let (start, end) = if bytes.start < bytes.end {
      (bytes.start, bytes.end)
    } else {
      (bytes.start, bytes.start)
    };
    let from = self.all.partition_point(|token| token.byte_end <= start);
    let to = self.all.partition_point(|token| token.byte_start < end).max(from);
    from..to
  }

  /// The same tokens with another `Token::text` at some places of `all`: what a reader compares that takes two
  /// spellings for one name (the places of a rename, which get one text on both sides). Nothing else of a token
  /// changes, so two such tokens are the same only where the tree around them is; a place past the tokens is left out
  #[must_use]
  pub fn with_texts(&self, texts: impl IntoIterator<Item = (usize, u64)>) -> Self {
    let mut tokens = self.clone();
    for (place, text) in texts {
      if let Some(token) = tokens.all.get_mut(place) {
        token.text = text;
      }
    }
    tokens
  }

  /// The place in `all` of the token that starts at `column` of `line` (a line of the view, a UTF-16 column), if one does
  pub fn place_at(&self, line: u32, column: u32) -> Option<usize> {
    let start = self.line_starts.get(line as usize).copied()?;
    let from = self.all.partition_point(|token| token.byte_end <= start);
    self.all[from..]
      .iter()
      .take_while(|token| token.line <= line)
      .position(|token| token.line == line && token.start == column)
      .map(|index| from + index)
  }

  /// The heap bytes it holds, for the weight of a cache entry
  pub const fn heap_bytes(&self) -> usize {
    self.all.capacity() * size_of::<Token>() + self.line_starts.capacity() * size_of::<u32>()
  }
}

/// Whether two pieces of two texts are the same
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Equality {
  /// The same code and the same comments: what differs, if anything, is the formatting
  Same,
  /// The same code, other comments
  CodeOnly,
  Different,
}

/// Whether `old_lines` of `old` and `new_lines` of `new` (lines of the view, from 0, the end excluded) are equal by
/// tokens: the one rule of "only the formatting changed", of a range of changed lines between lines that did not
/// change (a block of a diff). The tokens of the lines (`Tokens::in_lines`) are the same (`compare`) and the code
/// around them reads the same (`same_before`, `same_after`), else `Different`: lines without a token are the same
/// only when the line breaks they add or remove change no token around them. A range whose neighbours changed too (a
/// part of a block, a declaration paired with itself elsewhere) is asked by `compare`, and by `same_before` and
/// `same_after` where it ends at lines that did not change
pub fn equal_by_tokens(old: &Tokens, old_lines: Range<u32>, new: &Tokens, new_lines: Range<u32>) -> Equality {
  let (old_at, new_at) = (old.places_in_lines(old_lines), new.places_in_lines(new_lines));
  let around = same_before(&old.all[..old_at.start], &new.all[..new_at.start]) && same_after(&old.all[old_at.end..], &new.all[new_at.end..]);
  if around { compare(&old.all[old_at], &new.all[new_at]) } else { Equality::Different }
}

/// Whether the code before two runs of tokens, each given by the tokens before it, ends alike: the last token of
/// the code closes the same nodes before the run, or neither side has one. What that token is its own lines say:
/// whitespace after a token changes only what closes after it
pub fn same_before(old: &[Token], new: &[Token]) -> bool {
  let closes = |tokens: &[Token]| tokens.iter().rev().find(|token| !token.is_comment()).map(|token| token.closes);
  closes(old) == closes(new)
}

/// Whether the code after two runs of tokens, each given by the tokens after it, starts alike: the first token of
/// the code is the same token (its kind and its text) with the same nodes opening before it, or neither side has
/// one. What closes after it the lines after it say. The comments between are not asked: a comment is outside the
/// tree, and what a lexer reads as one by its place alone (a `#!` at the start of a file) is code elsewhere
pub fn same_after(old: &[Token], new: &[Token]) -> bool {
  let first = |tokens: &[Token]| tokens.iter().find(|token| !token.is_comment()).map(|token| (token.kind, token.opens, token.text));
  first(old) == first(new)
}

/// Whether two runs of tokens are equal: the same tokens in the same order, or the same without their comments. Two
/// empty runs are the same: lines of whitespace alone, as far as the code around them reads the same (`same_before`,
/// `same_after`), which `equal_by_tokens` asks and this does not
pub fn compare(old: &[Token], new: &[Token]) -> Equality {
  if old.len() == new.len() && old.iter().zip(new).all(|(old, new)| old.same(new)) {
    return Equality::Same;
  }
  let (mut old, mut new) = (old.iter().filter(|token| !token.is_comment()), new.iter().filter(|token| !token.is_comment()));
  loop {
    match (old.next(), new.next()) {
      (None, None) => return Equality::CodeOnly,
      (Some(old), Some(new)) if old.same(new) => {}
      _ => return Equality::Different,
    }
  }
}

/// What a run of tokens is wherever it stands, as one hash: each token by its kind and its text, and the tree between
/// the tokens, without what opens before the first token of the code and what closes after the last. Those two the
/// code around the run decides: a declaration that becomes the last of its file closes the file after its last token.
/// Two runs that `compare` as `Same` have the same content, and two that differ between their ends (a line break that
/// ends a statement, a space inside a string) have not: the identity of a change across two states of a file
pub fn content(run: &[Token]) -> u64 {
  let code = |token: &Token| !token.is_comment();
  let (first, last) = (run.iter().position(code), run.iter().rposition(code));
  let mut hash = build::SEED;
  for (index, token) in run.iter().enumerate() {
    hash = build::mix(build::mix(hash, token.kind), token.text);
    if Some(index) != first {
      hash = build::mix(hash, token.opens);
    }
    if Some(index) != last {
      hash = build::mix(hash, token.closes);
    }
  }
  hash
}

/// Whether no parser of a grammar reads `character` where it stands between two tokens: whitespace (`White_Space` of
/// Unicode, of which a parser skips its own part), a byte order mark (every parser at the start of a file, oxc
/// anywhere), a zero width space (oxc), a left-to-right and a right-to-left mark (Rust) and Ctrl-Z (C#). What
/// stands between two tokens of a text with tokens is blank (`build::Builder`). A blank is not always whitespace of
/// the language (the zero width space and U+0085 of JavaScript and TypeScript are errors in the tokens): the same text
/// without its blanks is what texts equal by tokens have, never what makes them equal
pub const fn is_blank(character: char) -> bool {
  character.is_whitespace() || matches!(character, '\u{1a}' | '\u{200b}' | '\u{200e}' | '\u{200f}' | '\u{feff}')
}

/// Whether two texts, each given in pieces, are the same without their blanks (`is_blank`). Texts equal by tokens
/// (`Equality::Same`) always are, so a reader asks this before it reads tokens; texts that are may still differ (a
/// space inside a string that another line lost)
pub fn same_without_blanks<'a>(old: impl IntoIterator<Item = &'a str>, new: impl IntoIterator<Item = &'a str>) -> bool {
  let solid = |character: &char| !is_blank(*character);
  old.into_iter().flat_map(str::chars).filter(solid).eq(new.into_iter().flat_map(str::chars).filter(solid))
}

/// Whether two whole texts of `grammar` are equal by tokens; `Different` when either has no tokens
pub fn texts_equal_by_tokens(grammar: Grammar, old: &str, new: &str) -> Equality {
  match (tokens(grammar, old), tokens(grammar, new)) {
    (Some(old), Some(new)) => compare(old.all(), new.all()),
    _ => Equality::Different,
  }
}

/// The tokens of a text; `None` when it is over `MAX_BYTES`, its grammar is not in this build, or its tree does not
/// hold every token as the parser read it (the module's "What has no tokens")
pub fn tokens(grammar: Grammar, text: &str) -> Option<Tokens> {
  if text.len() > MAX_BYTES {
    return None;
  }
  match grammar {
    #[cfg(feature = "kotlin")]
    Grammar::Kotlin | Grammar::KotlinScript => crate::kotlin::tokens::tokens(crate::kotlin::file_kind(grammar), text),
    #[cfg(feature = "java")]
    Grammar::Java => crate::java::tokens::tokens(text),
    #[cfg(feature = "csharp")]
    Grammar::CSharp => crate::csharp::tokens::tokens(text),
    #[cfg(feature = "starlark")]
    Grammar::Starlark | Grammar::StarlarkBuild => crate::starlark::tokens::tokens(text, crate::starlark::file_kind(grammar)),
    #[cfg(feature = "rust")]
    Grammar::Rust => crate::rust::tokens::tokens(text),
    #[cfg(feature = "typescript")]
    Grammar::TypeScript | Grammar::Tsx | Grammar::JavaScript => crate::js::tokens::tokens(crate::js::dialect_of_grammar(grammar), text),
    #[cfg(not(all(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark", feature = "rust", feature = "typescript")))]
    _ => None,
  }
}

/// A stable hash of the rules and the parsers of this build: stored tokens, and what was derived from them, are valid
/// only for the same value
pub fn tokens_version() -> u32 {
  crate::extract::extractor_version() ^ RULES_VERSION.wrapping_mul(0x85eb_ca6b)
}

/// The extensions of the files whose indentation has a meaning: Python, Starlark (`MODULE.bazel` too), YAML, Makefiles
const INDENTED_EXTENSIONS: [&str; 8] = ["py", "pyi", "bzl", "star", "bazel", "yml", "yaml", "mk"];
/// The names of the files whose indentation has a meaning, beside `INDENTED_EXTENSIONS`
const INDENTED_NAMES: [&str; 5] = ["Makefile", "makefile", "GNUmakefile", "BUILD", "WORKSPACE"];

/// Whether the whitespace at the start of a line of `path` has a meaning, by its name: a change of it is no change of
/// formatting. A file of a grammar among them has its indents in its tokens, and every file that grammar reads is one
/// (Starlark: `WORKSPACE.bzlmod`, `third_party/zlib.BUILD` too); one of none is compared by its reader, which asks this
pub fn indentation_matters(path: &str) -> bool {
  let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
  let extension = name.rsplit_once('.').map(|(_, extension)| extension);
  INDENTED_NAMES.contains(&name)
    || extension.is_some_and(|extension| INDENTED_EXTENSIONS.iter().any(|known| extension.eq_ignore_ascii_case(known)))
    || matches!(grammar_of(path), Some(Grammar::Starlark | Grammar::StarlarkBuild))
}
