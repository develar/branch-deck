//! Kotlin through psi-syntax's `kotlin-syntax`: a port of the Kotlin compiler's own lexer and parser whose tree is
//! Kotlin's PSI (`FUN`, `CLASS`, `CALL_EXPRESSION`, `DOT_QUALIFIED_EXPRESSION`, …), read by walks of this module over its
//! preorder ids by `kotlin_syntax::NodeKind` and `TokenKind`, never as tree-sitter nodes; what a PSI class answers of
//! its node (`KtNamedDeclaration.getNameIdentifier`, `KtModifierListOwner.hasModifier`, `KtTypeReference.getTypeElement`,
//! …) by its port in `kotlin_syntax::psi`, not by a walk of its own. `parse` is the one call into
//! the parser; `summary` makes the `FileSummary`, `highlight` the `LineRuns`, `navigate` the answers of Go to
//! Declaration and Find Usages, each by the parser-neutral builders (`summary_builder` and `jvm`, `highlight::lines` and
//! `highlight::paint::paint_leaf`, `navigate::{reference_answer, Names, byte_offset}`).
//!
//! - **The text** is the file as IntelliJ's document holds it (`kotlin_syntax::document_text`): a byte order mark as
//!   spaces, the `\r` of a `\r\n` as a space and a lone `\r` as `\n`, at the file's offsets, so a range of the tree is a
//!   range of the file and of the view. The tree owns that text (`Tree::text`, `Tree::utf8_text`).
//! - **The file kind** by extension, as the compiler chooses (`FileKind::from_file_name`): a `.kts` script's top-level
//!   statements are a `SCRIPT`'s `BLOCK`, a `.kt` file's are errors. The highlighting and the navigation know it from
//!   the grammar (`Grammar::KotlinScript`, `file_kind`); a summary is asked by language alone (`extract::summarize`).
//! - **Bounded by the parser**: any bytes parse, losslessly and without a panic; a construct nested past 1000 levels is
//!   an error element "Code is too deeply nested" over its rest. A parse needs 1 MB of stack in a release build: the
//!   summaries run on the workers of `trial-check` (`std::thread::scope`, 2 MiB) and on tokio's blocking threads
//!   (2 MiB, `app-api::operation::run_blocking`), the highlighting and the navigation of `file-diffs` on the latter, `bd`
//!   on its main thread (8 MiB). A walk of the tree must not recurse per level of nesting beyond a small frame: past a
//!   few hundred bytes a level, a 2 MiB thread overflows at the parser's limit; an explicit stack is safe.
//! - **The size limits** stay the callers': `extract::{MAX_BYTES, MAX_LINES, MAX_AVERAGE_LINE}` before a summary,
//!   `highlight::MAX_BYTES` before the highlighting.

pub(crate) mod highlight;
pub(crate) mod navigate;
pub(crate) mod summary;

#[cfg(test)]
mod parse_test;
