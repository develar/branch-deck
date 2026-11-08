//! The declarations, imports and references of one source file (Kotlin, Java, C#, Rust, TypeScript and JavaScript), for
//! relating the commits of a branch: which declarations a commit adds or removes, which commit's lines use a name
//! another commit declares, and whether a word on a line is a real reference at all; and the highlighting
//! (`highlight`) and the navigation (`navigate`: Go to Declaration, Find Usages) of a file of a diff.
//!
//! A pure function of the file's bytes (`extract::summarize`): no git, no cache. A reference is a name the syntax tree
//! uses outside comments and strings (but inside interpolations), that is no declaration name, label or key, and is not
//! bound in a local scope; resolving it to a declaration is the caller's job, by the imports, the package and the
//! qualifier. Each language is a cargo feature, all on by default.
//!
//! Two parsers: Kotlin, Java and C# are read by psi-syntax's ports of the Kotlin compiler's, IntelliJ's and Roslyn's own
//! parsers, whose trees are their PSI and Roslyn's syntax tree (`kotlin`, `java`, `csharp`: a module per language with a
//! walk per consumer, `summary`, `highlight` and `navigate`), the summary of TypeScript, TSX and JavaScript by
//! psi-syntax's `js-syntax`, oxc's parser, the parse accio reads them with (`js`), Rust and the highlighting and the
//! navigation of TypeScript by tree-sitter (`extract.rs` with `rust_lang.rs`, `highlight/`, `navigate.rs`). Both build their answers by the same parser-neutral code: `summary_builder`,
//! `highlight::lines` and `highlight::paint`, the answers of `navigate`. `examples/facts.rs` prints every fact of the
//! three consumers of a list of files, the oracle of a change of parser or rules.

#![cfg_attr(
  not(all(feature = "kotlin", feature = "java", feature = "csharp", feature = "rust", feature = "typescript")),
  allow(dead_code, unused_imports, reason = "a build without every grammar leaves the rules only the others use")
)]

pub mod extract;
pub mod highlight;
pub mod language;
pub mod navigate;
pub mod summary;
pub mod summary_text;

mod summary_builder;

#[cfg(feature = "csharp")]
mod csharp;
#[cfg(feature = "java")]
mod java;
#[cfg(feature = "typescript")]
mod js;
#[cfg(any(feature = "kotlin", feature = "java", feature = "csharp"))]
mod jvm;
#[cfg(feature = "kotlin")]
mod kotlin;
#[cfg(any(feature = "java", feature = "csharp", feature = "starlark"))]
mod psi_navigate;
#[cfg(any(feature = "kotlin", feature = "java", feature = "csharp", feature = "starlark"))]
mod psi_tree;
#[cfg(any(feature = "java", feature = "csharp"))]
mod psi_walk;
#[cfg(feature = "rust")]
mod rust_lang;
#[cfg(feature = "starlark")]
mod starlark;

#[cfg(all(test, feature = "kotlin", feature = "java", feature = "rust", feature = "typescript"))]
mod highlight_test;
#[cfg(all(test, feature = "java"))]
mod java_test;
#[cfg(all(test, feature = "kotlin"))]
mod kotlin_test;
#[cfg(all(test, feature = "kotlin", feature = "java", feature = "rust", feature = "typescript"))]
mod navigate_test;
#[cfg(all(test, feature = "rust"))]
mod rust_test;
#[cfg(test)]
mod summary_builder_test;
#[cfg(test)]
mod summary_test;
#[cfg(test)]
mod test_support;
