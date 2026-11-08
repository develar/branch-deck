//! What the walks of the trees paint with: `Paint`, `paint_leaf` (a leaf of a tree whose leaves tile the text, as
//! psi-syntax's, rust-analyzer's and oxc's tokens do), `comment_paint` and the lexers; the open nodes of a walk over a
//! psi-syntax tree (`Open`, `close_before`).

use crate::highlight::lex::{Lex, lex};
use crate::highlight::lines::Builder;
use crate::highlight::scopes::Scopes;
use crate::highlight::{COMMENT, Role};

/// What the rules of a grammar make of a node
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Paint {
  /// The node has the style of the node around it, and its children are walked
  Inherit,
  /// The node has the style of the node around it, and its children are not walked: a name, whose tokens may be words
  /// a grammar also has as keywords
  Opaque,
  /// The node has this style, and its children may paint over it
  Style(u32),
  /// The node has this style, and its children are not walked: a comment, a literal
  Leaf(u32),
  /// A leaf in this style whose pieces a lexer finds: the escapes of a string's content, the tags of a doc comment
  Lexed(u32, Lex),
}

/// The paint of a comment of this text, of any parser (`comment`)
pub(crate) fn comment_paint(text: &str) -> Paint {
  let doc = (text.starts_with("/**") && text != "/**/") || (text.starts_with("///") && !text.starts_with("////")) || text.starts_with("//!");
  if doc { Paint::Lexed(Role::DocComment.style(), Lex::Doc) } else { Paint::Leaf(COMMENT) }
}

/// Paints one leaf of a tree whose leaves tile the text (psi-syntax's `Tree::leaves`), the text `text` at byte `start`,
/// as `paint` paints a leaf node: the gap before it and an `Inherit` or `Opaque` leaf in `around` (the style of the node
/// around it), a `Style` or `Leaf` in its style, a `Lexed` one in the pieces its lexer finds
pub(crate) fn paint_leaf(out: &mut Builder<'_>, start: usize, text: &str, paint: Paint, around: u32) {
  out.push(start, around);
  let end = start + text.len();
  match paint {
    Paint::Inherit | Paint::Opaque => out.push(end, around),
    Paint::Style(style) | Paint::Leaf(style) => out.push(end, style),
    Paint::Lexed(base, kind) => {
      lex(kind, text, start, base, &mut |end, style| out.push(end, style));
      out.push(end, base);
    }
  }
}

/// Paints a string literal of a psi-syntax tree: the literal in the colour of a string, its escapes in theirs, an
/// escape `escape_at` long at each byte of `scan` where one starts (0 where none does), an escaped character that
/// is a multi-byte one whole
#[cfg(any(feature = "java", feature = "csharp"))]
pub(crate) fn paint_escapes(out: &mut Builder<'_>, start: usize, text: &str, scan: std::ops::Range<usize>, around: u32, escape_at: impl Fn(&[u8]) -> usize) {
  let base = Role::String.style();
  out.push(start, around);
  let bytes = text.as_bytes();
  let mut index = scan.start;
  while index < scan.end {
    let length = escape_at(&bytes[index..scan.end]);
    if length == 0 {
      index += 1;
      continue;
    }
    let end = ((index + length).min(bytes.len())..=bytes.len())
      .find(|&it| text.is_char_boundary(it))
      .unwrap_or(bytes.len());
    out.push(start + index, base);
    out.push(start + end, Role::Escape.style());
    index = end;
  }
  out.push(start + bytes.len(), base);
}

/// The number of nodes of a psi-syntax tree, the end of a walk over its ids
#[cfg(any(feature = "java", feature = "csharp"))]
pub(crate) fn node_count<L: psi_builder::language::TreeLanguage>(tree: &psi_builder::Tree<L>) -> u32 {
  u32::try_from(tree.len()).unwrap_or(u32::MAX)
}

/// A node a walk over a psi-syntax tree is inside that colours its subtree or opened a scope, with what the language's
/// walk keeps of it
#[cfg(any(feature = "java", feature = "csharp"))]
pub(crate) struct Open<X> {
  /// The end of its subtree: the id after its last descendant
  pub(crate) end: u32,
  pub(crate) node: psi_builder::NodeId,
  pub(crate) style: Option<u32>,
  pub(crate) extra: X,
}

/// Closes the open nodes whose subtrees end before the node `id`, and the scopes they opened; the style around `id`
#[cfg(any(feature = "java", feature = "csharp"))]
pub(crate) fn close_before<X>(open: &mut Vec<Open<X>>, id: u32, scopes: &mut Scopes) -> u32 {
  while let Some(closed) = open.pop_if(|it| it.end <= id) {
    scopes.pop(closed.node.0 as usize);
  }
  open.iter().rev().find_map(|it| it.style).unwrap_or(0)
}
