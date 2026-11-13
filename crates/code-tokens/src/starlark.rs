//! The tokens of a Starlark file from the leaves of its PSI (`tokens`). The lexer ends a statement and opens or closes
//! a block at a line break outside brackets, and writes nothing of it into the tree but the nodes; so the walk counts
//! the brackets as the lexer does (`StarlarkIndentingLexer`: a `(`, `[` or `{` more, a `)`, `]` or `}` less), and the
//! indent of each line after such a line break is part of the line's first token. A line break inside brackets, or
//! after a `\`, continues the line: its indent is formatting. The `\` that continues a line is a token with its line
//! break, though the lexer calls it whitespace: it is no blank (`crate::is_blank`), and a line joined without it is
//! another text.

use crate::Grammar;
use crate::psi::{Piece, error_elements, stream};
use crate::{TokenClass, Tokens};
use starlark_syntax::token_sets::{CLOSE_BRACKETS, OPEN_BRACKETS};
use starlark_syntax::{FileKind, Parser, TokenKind, Tree};
use std::cell::RefCell;

thread_local! {
  /// The thread's parser: psi-syntax keeps every buffer of a parse in it and reuses them from one file to the next
  static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// The tree of a Starlark file from its bytes: the one Starlark parse of the crate and of `code-symbols`, by one parser
/// per thread. Every Bazel file kind shares one grammar; the tree's offsets are the file's (`document_text` writes a
/// byte order mark as spaces)
pub fn parse(source: &[u8], kind: FileKind) -> Tree {
  let text = starlark_syntax::document_text(source);
  PARSER.with_borrow_mut(|parser| parser.parse(&*text, kind))
}

/// How a file of this grammar is read: `Grammar::StarlarkBuild` as a file whose top-level calls declare (a BUILD or
/// MODULE.bazel file), any other as a `.bzl` file
pub fn file_kind(grammar: Grammar) -> FileKind {
  if grammar == Grammar::StarlarkBuild { FileKind::Build } else { FileKind::Bzl }
}

/// The tokens of a Starlark text of `kind`; `None` when it nests deeper than the parser reads
pub(crate) fn tokens(text: &str, kind: FileKind) -> Option<Tokens> {
  let tree = parse(text.as_bytes(), kind);
  // the open brackets, as the lexer counts them
  let mut brackets = 0i32;
  stream(&tree, text, true, &error_elements(&tree), |tree, id| {
    let Some(token) = tree.token_kind(id) else { return Piece::Enter };
    if OPEN_BRACKETS.contains(token) {
      brackets += 1;
    } else if CLOSE_BRACKETS.contains(token) {
      brackets -= 1;
    }
    match token {
      TokenKind::COMMENT => Piece::Comment,
      TokenKind::LINE_BREAK if brackets == 0 => Piece::LineEnd,
      TokenKind::LINE_CONTINUATION => Piece::Token(TokenClass::Other),
      token if token.is_whitespace() => Piece::Skip,
      TokenKind::IDENTIFIER => Piece::Token(TokenClass::Name),
      TokenKind::STRING | TokenKind::BYTES => Piece::Token(TokenClass::Literal),
      token if token.is_keyword() => Piece::Token(TokenClass::Keyword),
      _ => Piece::Token(TokenClass::Other),
    }
  })
}
