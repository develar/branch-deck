//! The tokens of a Kotlin file from the leaves of its PSI (`tokens`): the soft keywords the parser remapped, `?.`,
//! `?:` and `!!` joined, a string as its quotes, its text and its escapes, the code of a template as code, a `KDoc`
//! whole as one comment.

use crate::tokens::psi::{error_elements, piece_of, stream};
use crate::tokens::{TokenClass, Tokens};
use kotlin_syntax::{FileKind, TokenKind};

/// The tokens of a Kotlin text of `kind`; `None` when it nests deeper than the parser reads
pub(crate) fn tokens(kind: FileKind, text: &str) -> Option<Tokens> {
  let tree = super::parse(text.as_bytes(), kind);
  stream(&tree, text, false, &error_elements(&tree), |tree, id| {
    piece_of(tree, id, |token| token == TokenKind::WHITE_SPACE, class)
  })
}

const fn class(token: TokenKind) -> TokenClass {
  match token {
    TokenKind::IDENTIFIER | TokenKind::FIELD_IDENTIFIER => TokenClass::Name,
    TokenKind::CHARACTER_LITERAL
    | TokenKind::OPEN_QUOTE
    | TokenKind::CLOSING_QUOTE
    | TokenKind::REGULAR_STRING_PART
    | TokenKind::ESCAPE_SEQUENCE
    | TokenKind::DANGLING_NEWLINE
    | TokenKind::INTERPOLATION_PREFIX => TokenClass::Literal,
    token if token.is_keyword() => TokenClass::Keyword,
    _ => TokenClass::Other,
  }
}
