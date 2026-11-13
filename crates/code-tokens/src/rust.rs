//! The tokens of a Rust file from rust-analyzer's tree (`tokens`): every token of the tree but the whitespace, a
//! comment (a doc comment too) as a comment, the nodes around them as their shapes. The parser glues `>>`, `=>`, `::`
//! where the grammar reads them and leaves the tokens of a macro's input single characters, which a macro tells apart
//! by what follows them: inside a token tree a punctuation mark with another right after it is another token than
//! one before a space. The errors of the lexer, the parser and the validation are a list beside the tree
//! (`rust_syntax::parse_with_errors`): each is an event before the token it stands at or before, by its message.

use crate::build::{Builder, Kinds, hash_bytes, joint};
use crate::{TokenClass, Tokens};
use rust_syntax::ra_ap_syntax::{NodeOrToken, SyntaxKind as K, SyntaxToken, TextRange, WalkEvent};

/// The tokens of a Rust text; `None` for a file nested deeper than any code (`rust_syntax::guarded`)
pub(crate) fn tokens(text: &str) -> Option<Tokens> {
  rust_syntax::guarded(text.as_bytes(), || {
    let (root, errors) = rust_syntax::parse_with_errors(text.as_bytes());
    let mut errors = errors.iter().peekable();
    let mut out = Builder::new(text);
    let mut kinds = Kinds::default();
    let mut kind_of = |kind: K| kinds.of(kind as usize, || format!("{kind:?}"));
    // how many token trees the walk is inside
    let mut token_trees = 0u32;
    for event in root.preorder_with_tokens() {
      match event {
        WalkEvent::Enter(NodeOrToken::Node(node)) => {
          token_trees += u32::from(node.kind() == K::TOKEN_TREE);
          out.open(kind_of(node.kind()));
        }
        WalkEvent::Leave(NodeOrToken::Node(node)) => {
          token_trees -= u32::from(node.kind() == K::TOKEN_TREE);
          if node.parent().is_none() {
            // the errors after the last token, at the end of the text
            for error in errors.by_ref() {
              out.error(hash_bytes(error.to_string().as_bytes()));
            }
          }
          out.close(kind_of(node.kind()));
        }
        WalkEvent::Enter(NodeOrToken::Token(token)) => {
          let range = span(token.text_range());
          if token.kind() != K::WHITESPACE {
            while let Some(error) = errors.next_if(|error| usize::from(error.range().start()) < range.end) {
              out.error(hash_bytes(error.to_string().as_bytes()));
            }
          }
          match token.kind() {
            K::WHITESPACE => {}
            K::COMMENT | K::OUTER_DOC_COMMENT | K::INNER_DOC_COMMENT | K::SHEBANG => out.comment(range),
            kind if range.is_empty() => out.empty(kind_of(kind)),
            kind => {
              let hash = kind_of(kind);
              let hash = if token_trees > 0 && is_joint(&token) { joint(hash) } else { hash };
              out.code(class(kind), hash, range);
            }
          }
        }
        WalkEvent::Leave(NodeOrToken::Token(_)) => {}
      }
    }
    out.finish()
  })
  .flatten()
}

/// The byte range of a node or a token
fn span(range: TextRange) -> std::ops::Range<usize> {
  usize::from(range.start())..usize::from(range.end())
}

/// A punctuation mark with another right after it, no space between them: `Spacing::Joint` of a macro's input, which
/// no bracket has
fn is_joint(token: &SyntaxToken) -> bool {
  let mark = |kind: K| kind.is_punct() && !matches!(kind, K::L_PAREN | K::R_PAREN | K::L_BRACK | K::R_BRACK | K::L_CURLY | K::R_CURLY);
  mark(token.kind()) && token.next_token().is_some_and(|next| mark(next.kind()))
}

fn class(kind: K) -> TokenClass {
  match kind {
    K::IDENT | K::LIFETIME_IDENT => TokenClass::Name,
    K::STRING | K::BYTE_STRING | K::C_STRING | K::CHAR | K::BYTE => TokenClass::Literal,
    kind if kind.is_keyword(rust_syntax::EDITION) => TokenClass::Keyword,
    _ => TokenClass::Other,
  }
}
