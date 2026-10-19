//! The longest common subsequence of two integer sequences, the core of every comparison of the crate.

pub mod diff;
pub mod enumerator;
pub mod myers;
pub mod patience;
pub mod reindexer;
pub mod unique_lcs;
pub mod window;

#[cfg(test)]
mod diff_test;
#[cfg(test)]
mod myers_test;
#[cfg(test)]
mod patience_test;
#[cfg(test)]
mod reindexer_test;
#[cfg(test)]
mod unique_lcs_test;
