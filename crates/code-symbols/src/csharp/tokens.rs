//! The tokens of a C# file from the leaves of Roslyn's tree (`tokens`): `>>` and its kin joined where the parser read
//! a shift, a string of any form whole, the text of an interpolated string as its own tokens.
//!
//! Roslyn calls trivia more than whitespace and comments, and that is code here: a directive (`#if DEBUG`, `#region`,
//! `#pragma`) is its tokens, the text of a branch of `#if` the configuration does not take one token that compares by
//! its bytes (another configuration compiles it), so are the message of `#error` and `#region`, a conflict marker and
//! the tokens the parser skipped. A documentation comment is one comment, whole.
//!
//! Roslyn's diagnostics are a list beside the tree, each on a node, and some change no node (`#line` with a span and
//! no space before its file name is CS9028): each is an event of the tree at its node, by its code and its message,
//! one on a comment (the XML of a documentation comment that does not close) part of that comment. A text Roslyn gave
//! up nesting in (`ERR_InsufficientStack`) is one skipped token, its whole text: no tokens.

use crate::tokens::build::{hash_bytes, mix};
use crate::tokens::psi::{Piece, stream};
use crate::tokens::{TokenClass, Tokens};
use csharp_syntax::{ErrorCode, NodeId, NodeKind, TokenKind, facts};

/// The tokens of a C# text; `None` when the parser gave up nesting in it, when its tree is not of the text byte for
/// byte (Roslyn drops the name before a misplaced `explicit operator`: the offsets of the tree are those of its own
/// text then), or its tokens are not those of the text (`stream`: where Roslyn cuts a character of two UTF-16 units)
pub(crate) fn tokens(text: &str) -> Option<Tokens> {
  let tree = super::parse(text.as_bytes());
  if tree.source() != text.as_bytes() || tree.diagnostics().iter().any(|diagnostic| diagnostic.code == ErrorCode::ERR_InsufficientStack) {
    return None;
  }
  let mut errors: Vec<(NodeId, u64)> = tree
    .diagnostics()
    .iter()
    .map(|diagnostic| (diagnostic.node, mix(hash_bytes(diagnostic.message.as_bytes()), u64::from(diagnostic.code.number()))))
    .collect();
  errors.sort_by_key(|(node, _)| *node);
  stream(&tree, text, false, &errors, |tree, id| {
    let Some(token) = tree.token_kind(id) else {
      let doc = matches!(
        tree.node_kind(id),
        Some(NodeKind::SingleLineDocumentationCommentTrivia | NodeKind::MultiLineDocumentationCommentTrivia)
      );
      return if doc { Piece::Comment } else { Piece::Enter };
    };
    match token {
      TokenKind::WhitespaceTrivia | TokenKind::EndOfLineTrivia => Piece::Skip,
      TokenKind::SingleLineCommentTrivia | TokenKind::MultiLineCommentTrivia | TokenKind::DocumentationCommentExteriorTrivia => Piece::Comment,
      token => Piece::Token(class(token)),
    }
  })
}

fn class(token: TokenKind) -> TokenClass {
  match token {
    TokenKind::IdentifierToken => TokenClass::Name,
    TokenKind::StringLiteralToken
    | TokenKind::Utf8StringLiteralToken
    | TokenKind::CharacterLiteralToken
    | TokenKind::SingleLineRawStringLiteralToken
    | TokenKind::MultiLineRawStringLiteralToken
    | TokenKind::Utf8SingleLineRawStringLiteralToken
    | TokenKind::Utf8MultiLineRawStringLiteralToken
    | TokenKind::InterpolatedStringStartToken
    | TokenKind::InterpolatedVerbatimStringStartToken
    | TokenKind::InterpolatedSingleLineRawStringStartToken
    | TokenKind::InterpolatedMultiLineRawStringStartToken
    | TokenKind::InterpolatedStringEndToken
    | TokenKind::InterpolatedRawStringEndToken
    | TokenKind::InterpolatedStringTextToken
    // what Roslyn takes for Razor's content, to the end of its line
    | TokenKind::RazorContentToken => TokenClass::Literal,
    token if facts::is_keyword_kind(token.syntax_kind()) || facts::is_preprocessor_keyword(token.syntax_kind()) => TokenClass::Keyword,
    _ => TokenClass::Other,
  }
}
