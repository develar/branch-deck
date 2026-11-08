//! What every summary of a psi-syntax tree asks of it, whatever its language.

use psi_builder::Tree;
use psi_builder::language::TreeLanguage;

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
