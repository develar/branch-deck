//! The tokens of a psi-syntax tree (Kotlin, Java, C#, Starlark): one pass over its preorder ids, the leaves the
//! parser left as the tokens, the composites around them as their shapes, the errors the parser reported on its nodes
//! at the place of each. What a node is to the tokens is the language's answer (`Piece`).

use crate::build::{Builder, Kinds, hash_bytes};
use crate::{TokenClass, Tokens};
use psi_builder::language::TreeLanguage;
use psi_builder::{NodeId, Tree};

/// What a node of a tree is to its tokens
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Piece {
  /// Whitespace: nothing, its subtree neither
  Skip,
  /// A comment, with its subtree (a doc comment the parser read into nodes)
  Comment,
  /// A composite of the code: its children are walked, it is in the shapes of the tokens around it
  Enter,
  /// A leaf of the code; one without text is in the shapes only
  Token(TokenClass),
  /// Whitespace that ends a logical line where the indentation is the syntax (a line break of Starlark outside
  /// brackets): nothing itself, and the indent of the line of the next token of the code is part of that token
  LineEnd,
}

/// The errors of the error elements of `tree`, each by the hash of its message, in the order of the nodes: what
/// `stream` takes of a tree of PSI, whose errors are nodes of one kind with a message beside them
pub(crate) fn error_elements<L: TreeLanguage>(tree: &Tree<L>) -> Vec<(NodeId, u64)> {
  tree.errors().map(|(node, message)| (node, hash_bytes(message.as_bytes()))).collect()
}

/// The tokens of `tree`, the tree of `text` at its offsets, each node asked once in the order of the text; `indented`:
/// the first line is a logical line whose indent counts (`Piece::LineEnd`); `errors`: what the parser reported, the
/// node of each and the hash of its message, in the order of the nodes: an error is an event of the tree at its node,
/// one on a comment part of what the comment compares by. `None` for a tree the parser gave up nesting in (under its
/// error "Code is too deeply nested" the leaves are the lexer's, not the parser's) and for tokens that are not those
/// of the text (`Builder::finish`)
pub(crate) fn stream<L: TreeLanguage>(tree: &Tree<L>, text: &str, indented: bool, errors: &[(NodeId, u64)], mut piece: impl FnMut(&Tree<L>, NodeId) -> Piece) -> Option<Tokens> {
  if tree.errors().any(|(_, message)| message == psi_builder::builder::TOO_DEEPLY_NESTED) {
    return None;
  }
  let mut errors = errors.iter().peekable();
  let mut out = Builder::new(text);
  if indented {
    out.logical_line();
  }
  let mut kinds = Kinds::default();
  // the open composites: the end of the subtree of each and its kind
  let mut open: Vec<(u32, u64)> = Vec::new();
  let len = u32::try_from(tree.len()).unwrap_or(u32::MAX);
  let mut id = 0;
  while id < len {
    while let Some((_, kind)) = open.pop_if(|it| it.0 <= id) {
      out.close(kind);
    }
    let node = NodeId(id);
    let end = tree.subtree_end(node).0.max(id + 1);
    let element = tree.kind(node);
    let mut kind = || kinds.of(element.index(), || element.debug_name());
    let piece = piece(tree, node);
    if piece == Piece::Enter {
      let kind = kind();
      out.open(kind);
      open.push((end, kind));
      while let Some((_, message)) = errors.next_if(|(on, _)| on.0 <= id) {
        out.error(*message);
      }
      id += 1;
      continue;
    }
    // the subtree is not walked: the errors on its nodes are those of the piece
    while let Some((_, message)) = errors.next_if(|(on, _)| on.0 < end) {
      if piece == Piece::Comment {
        out.comment_error(*message);
      } else {
        out.error(*message);
      }
    }
    match piece {
      Piece::Skip | Piece::Enter => {}
      Piece::Comment => out.comment(tree.range(node)),
      Piece::Token(class) => {
        let range = tree.range(node);
        if range.is_empty() {
          out.empty(kind());
        } else {
          out.code(class, kind(), range);
        }
      }
      Piece::LineEnd => out.logical_line(),
    }
    id = end;
  }
  while let Some((_, kind)) = open.pop() {
    out.close(kind);
  }
  out.finish()
}

/// What a node is in a tree whose trivia are whitespace (`whitespace`) and comments, a doc comment a composite of its
/// own: Kotlin's and Java's
pub(crate) fn piece_of<L: TreeLanguage>(tree: &Tree<L>, id: NodeId, whitespace: impl Fn(L::Token) -> bool, class: impl Fn(L::Token) -> TokenClass) -> Piece {
  if tree.is_trivia(id) {
    return if tree.kind(id).token().is_some_and(whitespace) { Piece::Skip } else { Piece::Comment };
  }
  tree.token_kind(id).map_or(Piece::Enter, |token| Piece::Token(class(token)))
}
