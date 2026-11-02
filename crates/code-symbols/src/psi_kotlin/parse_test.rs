//! The one parse of a Kotlin file: the document's text, the file kind, a tree that tiles the text

use crate::highlight::Grammar;
use crate::psi_kotlin::{file_kind, parse};
use crate::test_support::assert_leaves_tile;
use kotlin_syntax::{FileKind, NodeKind};

#[test]
fn a_file_with_a_byte_order_mark_and_crlf_parses_as_the_compiler_reads_it() {
  let tree = parse(b"\xEF\xBB\xBFpackage a\r\nval x =\r\n  f()\r\n", FileKind::Kt);
  assert_eq!(tree.errors().len(), 0);
  assert_leaves_tile(&tree);
  assert!(tree.descendants(tree.root()).any(|id| tree.node_kind(id) == Some(NodeKind::PROPERTY)));
}

#[test]
fn a_script_has_statements_at_the_top_level_and_a_file_has_errors_there() {
  let source = b"plugins {\n  id(\"x\")\n}\n";
  assert_eq!(parse(source, file_kind(Grammar::KotlinScript)).errors().len(), 0);
  assert_ne!(parse(source, file_kind(Grammar::Kotlin)).errors().len(), 0);
}
