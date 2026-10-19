//! Line, word and char comparison of two texts, ported and modified from the diff of IntelliJ Platform (Apache License
//! 2.0, see `NOTICE`). The modules keep the structure of their Kotlin sources, one module per source file, so that a
//! later change of the source can be compared with the port.
//!
//! The texts are UTF-16 code units (`text::Utf16Text`), as in the source: the offsets of a result are UTF-16 offsets.
//! The entry point of a diff view is `provider::compare_lines_inner`; the model of the app is `model::diff_texts`.

pub mod apply;
pub mod by_char;
pub mod by_line;
pub mod by_word;
pub mod cancellation;
pub mod change_corrector;
pub mod character_utils;
pub mod chunk_optimizer;
pub mod comparison_util;
pub mod fragments;
pub mod inline;
pub mod iterables;
pub mod lcs;
pub mod line_fragment_splitter;
pub mod line_offsets;
pub mod manager;
pub mod model;
pub mod policy;
pub mod provider;
pub mod range;
pub mod side;
pub mod text;
pub mod trim_util;

#[cfg(test)]
mod apply_test;
#[cfg(test)]
mod blocks_comparison_util_test;
#[cfg(test)]
mod char_comparison_util_test;
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
mod line_comparison_util_test;
#[cfg(test)]
mod line_uplifter_test;
#[cfg(test)]
mod model_test;
#[cfg(test)]
mod split_comparison_util_test;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod trim_util_test;
#[cfg(test)]
mod word_comparison_util_test;
