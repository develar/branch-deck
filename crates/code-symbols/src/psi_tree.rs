//! What every summary of a psi-syntax tree asks of it, whatever its language.

use psi_builder::language::TreeLanguage;
use psi_builder::{NodeId, Tree};

/// The bytes under the outermost error elements of a tree, one for an empty one (an expected token): the measure of
/// how much of a file the parser could not read (`FileSummary::error_bytes`, `is_degraded`)
pub(crate) fn error_bytes<L: TreeLanguage>(tree: &Tree<L>) -> usize {
  let mut bytes = 0;
  let mut end = 0;
  for (id, _) in tree.errors() {
    if id.0 >= end {
      bytes += tree.range(id).len().max(1);
      end = tree.subtree_end(id).0;
    }
  }
  bytes
}

/// The type under `node` as its source writes it (`Parameter::shown_type`): its tokens without the subtrees `annotation`
/// takes and without the comments, each run of whitespace and comments between two tokens one space, but none after
/// `(`, `<`, `[` or `.` and none before `)`, `>`, `]`, `,`, `.` or `?`, so `Map< K,\n V >` reads `Map<K, V>`; `None`
/// when no token is left
pub(crate) fn written_type<L: TreeLanguage>(tree: &Tree<L>, node: NodeId, annotation: impl Fn(NodeId) -> bool) -> Option<String> {
  let mut text = String::new();
  let mut gap = false;
  for id in tree.descendants_pruned(node, |id| annotation(id) || (!tree.is_leaf(id) && tree.is_trivia(id))) {
    if !tree.is_leaf(id) {
      continue;
    }
    if tree.is_trivia(id) {
      gap = !text.is_empty();
      continue;
    }
    let Some(token) = tree.utf8_text(id).filter(|token| !token.is_empty()) else { continue };
    if gap && !text.ends_with(['(', '<', '[', '.']) && !token.starts_with([')', '>', ']', ',', '.', '?']) {
      text.push(' ');
    }
    gap = false;
    text.push_str(token);
  }
  (!text.is_empty()).then_some(text)
}
