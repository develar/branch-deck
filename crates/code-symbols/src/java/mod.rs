//! Java through psi-syntax's `java-syntax`: a port of IntelliJ's own Java lexer and parser (`java-syntax`) whose tree is
//! IntelliJ's Java PSI (`CLASS`, `METHOD`, `METHOD_CALL_EXPRESSION`, `REFERENCE_EXPRESSION`, …), read by walks of this
//! module over its preorder ids by `java_syntax::NodeKind` and `TokenKind`, never as tree-sitter nodes; what a PSI class
//! answers of its node (`PsiFieldImpl.getTypeElement`, `PsiJavaCodeReferenceElementImpl.getQualifier`,
//! `PsiModifierListImpl.hasExplicitModifier`, …) by its port in `java_syntax::psi`, not by a walk of its own. `parse` is the
//! one call into the parser; `summary` makes the `FileSummary`, `highlight` the `LineRuns`, `navigate` the answers of Go
//! to Declaration and Find Usages, each by the parser-neutral builders (`summary_builder` and `jvm`, `highlight::lines` and
//! `highlight::paint::paint_leaf`, `navigate::{reference_answer, Names, byte_offset}`).
//!
//! - **The text** is the file as IntelliJ's document holds it (`java_syntax::document_text`): a byte order mark as
//!   spaces, at the file's offsets; the Java lexer reads `\r`, `\n` and `\r\n` as line ends itself. The tree owns that
//!   text (`Tree::text`, `Tree::utf8_text`).
//! - **The file kind**: a `.java` file at the highest language level (`FileKind::default()`), as
//!   `JavaSyntaxDefinition.parse(builder)` parses it.
//! - **Bounded by the parser**: any bytes parse, losslessly and without a panic; a construct nested past 1200 levels is
//!   an error element "Code is too deeply nested" over its rest. A parse needs 1 MB of stack in a release build (772 KB
//!   at the limit): the threads that run it have 2 MiB or more (`kotlin`, "Bounded by the parser"). A walk of the
//!   tree must not recurse per level of nesting beyond a small frame; an explicit stack is safe.
//! - **The size limits** stay the callers': `extract::{MAX_BYTES, MAX_LINES, MAX_AVERAGE_LINE}` before a summary,
//!   `highlight::MAX_BYTES` before the highlighting.

pub(crate) mod highlight;
pub(crate) mod navigate;
mod pattern_scope;
pub(crate) mod summary;
pub(crate) mod tokens;

#[cfg(test)]
mod parse_test;

use java_syntax::{FileKind, Parser, Tree};
use std::cell::RefCell;

thread_local! {
  /// The thread's parser: psi-syntax keeps every buffer of a parse in it and reuses them from one file to the next
  static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// The tree of a Java file from its bytes, every body expanded: the one Java parse of the crate
pub(crate) fn parse(source: &[u8]) -> Tree {
  let text = java_syntax::document_text(source);
  PARSER.with_borrow_mut(|parser| parser.parse(&*text, FileKind::default()))
}
