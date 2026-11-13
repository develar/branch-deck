//! The tokens of a Java file from the leaves of its PSI (`tokens`): `>>`, `>>>` and their assignments joined where the
//! parser read a shift, a string, a text block and the pieces of a string template whole, a doc comment whole as one
//! comment.

use crate::psi::{error_elements, piece_of, stream};
use crate::{TokenClass, Tokens};
use java_syntax::{FileKind, Parser, TokenKind, Tree};
use std::cell::RefCell;

thread_local! {
  /// The thread's parser: psi-syntax keeps every buffer of a parse in it and reuses them from one file to the next
  static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// The tree of a Java file from its bytes, every body expanded: the one Java parse of the crate and of `code-symbols`,
/// by one parser per thread
pub fn parse(source: &[u8]) -> Tree {
  let text = java_syntax::document_text(source);
  PARSER.with_borrow_mut(|parser| parser.parse(&*text, FileKind::default()))
}

/// The tokens of a Java text; `None` when it nests deeper than the parser reads
pub(crate) fn tokens(text: &str) -> Option<Tokens> {
  let tree = parse(text.as_bytes());
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
