//! The marks of the diffs of the service (`diff_marks::marks`; openspec/specs/commit/quiet-changes/spec.md): the tokens
//! of a side read once per blob and grammar and kept in memory (`TOKENS_CACHE_BYTES`), and the bindings of the names
//! of a text answered by `code-symbols` (`bindings::CodeSymbolBindings`).

use crate::bindings::CodeSymbolBindings;
use crate::service::FileDiffs;
use code_tokens::{Tokens, tokens};
use diff_marks::marks::{PairMarks, marks_of, pair_grammar, pair_indented};
use line_diff::model::TextDiff;
use line_diff::policy::WhitespacePolicy;
use std::sync::Arc;

/// The tokens that memory keeps, weighed by their size (64 bytes a token): the sides of some three hundred files of
/// 200 lines, for the next commit of a branch and another whitespace policy
pub const TOKENS_CACHE_BYTES: u64 = 32 << 20;

/// The memory the tokens of a blob take in the cache
pub fn tokens_weight(tokens: Option<&Tokens>) -> u32 {
  u32::try_from(tokens.map_or(0, Tokens::heap_bytes).saturating_add(64)).unwrap_or(u32::MAX)
}

impl FileDiffs {
  /// The marks of `diff`, the diff of a file whose sides are at `paths` (the old one first), have the blob `ids` and
  /// the `texts`. The tokens of a side are read once per blob and grammar and kept in memory: the next pair with the
  /// blob (the next commit of a branch, another whitespace policy) parses nothing
  pub fn marks(&self, diff: &TextDiff, policy: WhitespacePolicy, paths: [&str; 2], ids: [Option<&str>; 2], texts: [&str; 2]) -> PairMarks {
    self.marks_told(diff, policy, paths, ids, texts, &|_| {})
  }

  /// `marks`, telling `asked` of each side whose tokens the marks read (`true` the new one): a file none is read of
  /// answered the question before the tokens with no at every block
  pub fn marks_told(&self, diff: &TextDiff, policy: WhitespacePolicy, paths: [&str; 2], ids: [Option<&str>; 2], texts: [&str; 2], asked: &dyn Fn(bool)) -> PairMarks {
    let grammar = pair_grammar(paths[0], paths[1]);
    let tokens_of = |new_side| {
      asked(new_side);
      let (side, grammar) = (usize::from(new_side), grammar?);
      match ids[side] {
        Some(id) => self
          .marks_memory
          .tokens
          .get_with((grammar.id(), id.to_string()), || tokens(grammar, texts[side]).map(Arc::new)),
        None => tokens(grammar, texts[side]).map(Arc::new),
      }
    };
    marks_of(diff, texts[0], texts[1], grammar, policy, pair_indented(paths[0], paths[1]), tokens_of, &CodeSymbolBindings)
  }
}
