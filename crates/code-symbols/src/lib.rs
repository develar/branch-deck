//! The declarations, imports and references of one source file (Kotlin, Java, Rust, TypeScript and JavaScript), from a
//! tree-sitter syntax tree, for relating the commits of a branch: which declarations a commit adds or removes, which
//! commit's lines use a name another commit declares, and whether a word on a line is a real reference at all.
//!
//! A pure function of the file's bytes (`extract::summarize`): no git, no cache. A reference is a name the syntax tree
//! uses outside comments and strings (but inside interpolations), that is no declaration name, label or key, and is not
//! bound in a local scope; resolving it to a declaration is the caller's job, by the imports, the package and the
//! qualifier. Each grammar is a cargo feature, all on by default.

#![cfg_attr(
  not(all(feature = "kotlin", feature = "java", feature = "rust", feature = "typescript")),
  allow(dead_code, unused_imports, reason = "a build without every grammar leaves the rules only the others use")
)]

pub mod extract;
pub mod language;
pub mod summary;

#[cfg(feature = "java")]
mod java;
#[cfg(feature = "kotlin")]
mod kotlin;
#[cfg(feature = "rust")]
mod rust_lang;
#[cfg(feature = "typescript")]
mod typescript;

#[cfg(all(test, feature = "java"))]
mod java_test;
#[cfg(all(test, feature = "kotlin"))]
mod kotlin_test;
#[cfg(all(test, feature = "rust"))]
mod rust_test;
#[cfg(test)]
mod summary_test;
#[cfg(test)]
mod test_support;
#[cfg(all(test, feature = "typescript"))]
mod typescript_test;
