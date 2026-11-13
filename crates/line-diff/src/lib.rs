//! Line, word and char comparison of two texts, ported and modified from the diff of IntelliJ Platform (Apache License
//! 2.0, see `NOTICE`). The modules keep the structure of their Kotlin sources, one module per source file, so that a
//! later change of the source can be compared with the port.
//!
//! The texts are UTF-16 code units (`text::Utf16Text`), as in the source: the offsets of a result are UTF-16 offsets.
//! The entry point of a diff view is `provider::compare_lines_inner`; the model of a view is `model::diff_texts`, and
//! the lines a change moved are `moves` (within a file) and `moves_across` (between the files of a change).
//!
//! Pure functions of the texts: no git, no cache, no IO. The model is serde's; a consumer that describes it for the
//! bindings of its app declares its types again by the macros that declare them here (`diff_model!`,
//! `whitespace_policy!`, `line_end_kind!`), so both are one text (Branch Deck's `text-diff`). The merge of three
//! texts for a view is that consumer's (`text_diff::merge_model`), over `manager::merge_content_lines`.

pub mod apply;
pub mod by_char;
pub mod by_line;
pub mod by_word;
pub mod cancellation;
pub mod change_corrector;
pub mod character_utils;
pub mod chunk_optimizer;
pub mod comparison_merge_util;
pub mod comparison_util;
pub mod fragments;
pub mod inline;
pub mod iterables;
pub mod last_line;
pub mod lcs;
pub mod line_fragment_splitter;
pub mod line_offsets;
pub mod manager;
pub mod merge_conflict_type;
pub mod merge_range;
pub mod merge_range_util;
pub mod merge_resolve_util;
pub mod model;
pub mod moves;
pub mod moves_across;
pub mod policy;
pub mod provider;
pub mod range;
pub mod side;
pub mod text;
pub mod text_lines;
pub mod trim_util;

#[cfg(test)]
mod apply_test;
#[cfg(test)]
mod blocks_comparison_util_test;
#[cfg(test)]
mod char_comparison_merge_util_test;
#[cfg(test)]
mod char_comparison_util_test;
#[cfg(test)]
mod comparison_merge_util_test_base;
#[cfg(test)]
mod comparison_util_auto_test;
#[cfg(test)]
mod comparison_util_test;
#[cfg(test)]
mod comparison_util_test_base;
#[cfg(test)]
mod ignore_comparison_util_test;
#[cfg(test)]
mod inline_test;
#[cfg(test)]
mod line_comparison_merge_util_test;
#[cfg(test)]
mod line_comparison_util_test;
#[cfg(test)]
mod line_uplifter_test;
#[cfg(test)]
mod merge_resolve_util_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod moves_across_test;
#[cfg(test)]
mod moves_test;
#[cfg(test)]
mod split_comparison_util_test;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod text_lines_test;
#[cfg(test)]
mod trim_util_test;
#[cfg(test)]
mod word_comparison_util_test;
#[cfg(test)]
mod word_merge_comparison_util_test;
