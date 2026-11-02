//! The summary of a Java file from its PSI (`FileSummary`), by the rules that the head of `extract.rs` states for
//! every language: locals dropped, overrides by the stack of supertypes, `Shape::Call` of a call, the constructors of a
//! class, `enclosed`. Until its walk lands, tree-sitter's.

use crate::summary::FileSummary;

// jscpd:ignore-start: the stub of each language delegates to tree-sitter alike until the lane's walk replaces it
/// Fills `summary` (`summary_builder::empty_summary` of the file, its size within the limits) with the facts of the
/// Java file `source`; whether it was read, else it stays `truncated`
pub(crate) fn summarize(source: &[u8], summary: &mut FileSummary) -> bool {
  crate::extract::summarize_tree_sitter(source, crate::extract::PARSE_BUDGET, summary)
}
// jscpd:ignore-end

#[cfg(test)]
#[path = "summary_test.rs"]
mod tests;
