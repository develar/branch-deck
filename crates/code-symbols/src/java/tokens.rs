//! The tokens of a Java file from the leaves of its PSI (`tokens`): `>>`, `>>>` and their assignments joined where the
//! parser read a shift, a string, a text block and the pieces of a string template whole, a doc comment whole as one
//! comment.

use crate::tokens::psi::{error_elements, piece_of, stream};
use crate::tokens::{TokenClass, Tokens};
use java_syntax::TokenKind;

/// The tokens of a Java text; `None` when it nests deeper than the parser reads
pub(crate) fn tokens(text: &str) -> Option<Tokens> {
  let tree = super::parse(text.as_bytes());
  stream(&tree, text, false, &error_elements(&tree), |tree, id| {
    piece_of(tree, id, TokenKind::is_psi_white_space, class)
  })
}

const fn class(token: TokenKind) -> TokenClass {
  match token {
    TokenKind::IDENTIFIER => TokenClass::Name,
    TokenKind::CHARACTER_LITERAL
    | TokenKind::STRING_LITERAL
    | TokenKind::TEXT_BLOCK_LITERAL
    | TokenKind::STRING_TEMPLATE_BEGIN
    | TokenKind::STRING_TEMPLATE_MID
    | TokenKind::STRING_TEMPLATE_END
    | TokenKind::TEXT_BLOCK_TEMPLATE_BEGIN
    | TokenKind::TEXT_BLOCK_TEMPLATE_MID
    | TokenKind::TEXT_BLOCK_TEMPLATE_END => TokenClass::Literal,
    token if token.is_keyword() => TokenClass::Keyword,
    _ => TokenClass::Other,
  }
}
