//! The tokens of a Starlark file from the leaves of its PSI (`tokens`). The lexer ends a statement and opens or closes
//! a block at a line break outside brackets, and writes nothing of it into the tree but the nodes; so the walk counts
//! the brackets as the lexer does (`StarlarkIndentingLexer`: a `(`, `[` or `{` more, a `)`, `]` or `}` less), and the
//! indent of each line after such a line break is part of the line's first token. A line break inside brackets, or
//! after a `\`, continues the line: its indent is formatting. The `\` that continues a line is a token with its line
//! break, though the lexer calls it whitespace: it is no blank (`tokens::is_blank`), and a line joined without it is
//! another text.

use crate::tokens::psi::{Piece, error_elements, stream};
use crate::tokens::{TokenClass, Tokens};
use starlark_syntax::token_sets::{CLOSE_BRACKETS, OPEN_BRACKETS};
use starlark_syntax::{FileKind, TokenKind};

/// The tokens of a Starlark text of `kind`; `None` when it nests deeper than the parser reads
pub(crate) fn tokens(text: &str, kind: FileKind) -> Option<Tokens> {
  let tree = super::parse(text.as_bytes(), kind);
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
