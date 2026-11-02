//! The highlighting of a Kotlin file from its PSI, as IntelliJ colours it without resolving names
//! (`KotlinHighlightingColors`): the tokens from `Tree::leaves` (soft keywords, `?.`, `?:` and `!!` are exact only
//! there), the roles of names from the nodes around them, painted by `highlight::paint::paint_leaf`. Until it lands,
//! tree-sitter's.

use crate::highlight::lines::LineRuns;
use kotlin_syntax::FileKind;

/// The highlighting of a Kotlin text of `kind` (its size within `highlight::MAX_BYTES`); `None` when it has none
pub(crate) fn highlight(kind: FileKind, text: &str) -> Option<LineRuns> {
  crate::highlight::highlight_tree_sitter(super::grammar_of_kind(kind), text)
}

#[cfg(test)]
#[path = "highlight_test.rs"]
mod tests;
