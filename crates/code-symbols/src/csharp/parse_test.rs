//! The one parse of a C# file: the bytes as they are, a tree that tiles them

use crate::test_support::assert_leaves_tile;
use code_tokens::csharp::parse;
use csharp_syntax::NodeKind;

#[test]
fn a_file_with_a_byte_order_mark_and_crlf_parses_as_roslyn_reads_it() {
  let tree = parse(b"\xEF\xBB\xBFnamespace A;\r\nclass B {\r\n  int x;\r\n}\r\n");
  assert_eq!(tree.diagnostics().len(), 0);
  assert_leaves_tile(&tree);
  assert!(tree.descendants(tree.root()).any(|id| tree.node_kind(id) == Some(NodeKind::FieldDeclaration)));
}

#[test]
fn a_documentation_comment_is_parsed_as_its_xml() {
  let tree = parse(b"/// <summary>A <see cref=\"B\"/></summary>\nclass A {}\nclass B {}\n");
  assert!(tree.descendants(tree.root()).any(|id| tree.node_kind(id) == Some(NodeKind::XmlCrefAttribute)));
}

#[test]
fn a_branch_of_if_without_its_symbol_is_disabled_text() {
  let tree = parse(b"class A {\n#if DEBUG\n  int x;\n#endif\n}\n");
  assert!(!tree.descendants(tree.root()).any(|id| tree.node_kind(id) == Some(NodeKind::FieldDeclaration)));
  assert!(tree.tokens().any(|id| tree.token_kind(id) == Some(csharp_syntax::TokenKind::DisabledTextTrivia)));
}

#[test]
fn bytes_that_are_no_utf8_parse_and_tile() {
  let tree = parse(b"class A { string s = \"\xff\xfe\"; }\n");
  assert_leaves_tile(&tree);
}
