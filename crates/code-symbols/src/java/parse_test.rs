//! The one parse of a Java file: the document's text and a tree that tiles the text

use crate::java::parse;
use crate::test_support::assert_leaves_tile;
use java_syntax::NodeKind;

#[test]
fn a_file_with_a_byte_order_mark_and_crlf_parses_as_intellij_reads_it() {
  let tree = parse(b"\xEF\xBB\xBFpackage a;\r\nclass A {\r\n  int x;\r\n}\r\n");
  assert_eq!(tree.errors().len(), 0);
  assert_leaves_tile(&tree);
  assert!(tree.descendants(tree.root()).any(|id| tree.node_kind(id) == Some(NodeKind::FIELD)));
}
