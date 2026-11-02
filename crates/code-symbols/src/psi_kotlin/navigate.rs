//! Go to Declaration and Find Usages in a Kotlin file from its PSI (spec/commit/symbol-navigation.spec.md): the name
//! at a place (`Tree::leaf_index`), its qualifier, the kind of the reference, its declaration in the file by the
//! scopes, and the usages of a name, answered by `navigate::{reference_answer, usage_answer}`. Until it lands,
//! tree-sitter's.

use crate::navigate::{Position, ReferenceAt, Usage, UsagesAt};
use kotlin_syntax::FileKind;

/// The name at `position` of a Kotlin text of `kind` and what its syntax says (`navigate::reference_at`)
pub(crate) fn reference_at(kind: FileKind, text: &str, position: Position) -> Option<ReferenceAt> {
  crate::navigate::reference_at_tree_sitter(super::grammar_of_kind(kind), text, position)
}

/// The usages of the name at `position` (`navigate::usages_at`)
pub(crate) fn usages_at(kind: FileKind, text: &str, position: Position) -> Option<UsagesAt> {
  crate::navigate::usages_at_tree_sitter(super::grammar_of_kind(kind), text, position)
}

/// The usages of `name` in a text that is not the one of the name (`navigate::usages_of`)
pub(crate) fn usages_of(kind: FileKind, text: &str, name: &str) -> Vec<Usage> {
  crate::navigate::usages_of_tree_sitter(super::grammar_of_kind(kind), text, name)
}

#[cfg(test)]
#[path = "navigate_test.rs"]
mod tests;
