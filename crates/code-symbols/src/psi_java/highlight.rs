//! The highlighting of a Java file from its PSI, as IntelliJ colours it without resolving names
//! (`JavaHighlightingColors`): the tokens from `Tree::leaves`, the roles of names from the nodes around them, painted
//! by `highlight::paint::paint_leaf`. Until it lands, tree-sitter's.

use crate::highlight::Grammar;
use crate::highlight::lines::LineRuns;

/// The highlighting of a Java text (its size within `highlight::MAX_BYTES`); `None` when it has none
pub(crate) fn highlight(text: &str) -> Option<LineRuns> {
  crate::highlight::highlight_tree_sitter(Grammar::Java, text)
}

#[cfg(test)]
#[path = "highlight_test.rs"]
mod tests;
