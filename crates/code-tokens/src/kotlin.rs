//! The tokens of a Kotlin file from the leaves of its PSI (`tokens`): the soft keywords the parser remapped, `?.`,
//! `?:` and `!!` joined, a string as its quotes, its text and its escapes, the code of a template as code, a `KDoc`
//! whole as one comment.

use crate::Grammar;
use crate::psi::{error_elements, piece_of, stream};
use crate::{TokenClass, Tokens};
use kotlin_syntax::{FileKind, Parser, TokenKind, Tree};
use std::cell::RefCell;

thread_local! {
  /// The thread's parser: psi-syntax keeps every buffer of a parse in it and reuses them from one file to the next
  static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// The tree of a Kotlin file of `kind` from its bytes, every body expanded: the one Kotlin parse of the crate and of
/// `code-symbols`, by one parser per thread
pub fn parse(source: &[u8], kind: FileKind) -> Tree {
  let text = kotlin_syntax::document_text(source);
  PARSER.with_borrow_mut(|parser| parser.parse(&*text, kind))
}

/// How a file of this grammar is parsed: `Grammar::KotlinScript` (`.kts`) as a script, any other as a file
pub fn file_kind(grammar: Grammar) -> FileKind {
  if grammar == Grammar::KotlinScript { FileKind::Kts } else { FileKind::Kt }
}

/// The tokens of a Kotlin text of `kind`; `None` when it nests deeper than the parser reads
pub(crate) fn tokens(kind: FileKind, text: &str) -> Option<Tokens> {
  let tree = parse(text.as_bytes(), kind);
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
