//! The parser-neutral half of a summary walk: lines, declarations, the `JUnit` 3 rule and the finish step

use crate::language::Language;
use crate::summary::{Constructor, DeclKind, MemberKind, Reach, Role, Signature};
use crate::summary_builder::{SummaryBuilder, blank_declaration, count_lines, empty_summary, end_line, is_junit3_test};

/// The 1-based line of an offset by its `\n`s
fn line_of(source: &[u8]) -> impl Fn(usize) -> u32 + '_ {
  |offset| 1 + u32::try_from(source[..offset].split(|it| *it == b'\n').count() - 1).unwrap()
}

#[test]
fn a_node_that_ends_with_its_line_break_ends_on_the_line_before() {
  let source = b"fun a() {\n}\nval b = 1";
  // `fun a() {\n}\n`: ends after the `\n` of line 2
  assert_eq!(end_line(line_of(source), source, 0, 12), 2);
  // `fun a() {\n}`: ends on line 2
  assert_eq!(end_line(line_of(source), source, 0, 11), 2);
  // a node of one line break alone ends on the line it breaks
  assert_eq!(end_line(line_of(source), source, 9, 10), 1);
  assert_eq!((count_lines(source), count_lines(b"a\n"), count_lines(b"")), (3, 1, 0));
}

#[test]
fn a_junit3_test_is_named_test_and_lies_in_a_test_class_or_extends_a_test_case() {
  let none = std::iter::empty::<&str>;
  assert!(is_junit3_test("testParse", true, false, Some("ParserTest"), none()));
  assert!(is_junit3_test("test_parse", true, false, Some("Parser"), ["LightPlatformTestCase"].into_iter()));
  assert!(!is_junit3_test("testParse", false, false, Some("ParserTest"), none()));
  assert!(!is_junit3_test("testParse", true, true, Some("ParserTest"), none()));
  assert!(!is_junit3_test("tested", true, false, Some("ParserTest"), none()));
  assert!(!is_junit3_test("testParse", true, false, Some("Parser"), none()));
}

#[test]
fn finish_sorts_the_names_adds_the_implicit_constructor_and_drops_duplicate_references() {
  let source = b"class B\nclass A\n";
  let mut summary = empty_summary(Language::Java, source);
  let mut builder = SummaryBuilder::new();
  let b = builder.intern("B");
  let a = builder.intern("A");
  let class_b = builder.add_declaration(blank_declaration(b, DeclKind::Type, Reach::Public, MemberKind::TopLevel, None));
  builder.declarations[class_b as usize].constructors = Some(Vec::new());
  let mut class_a = blank_declaration(a, DeclKind::Type, Reach::Module, MemberKind::TopLevel, None);
  class_a.constructors = Some(vec![Constructor {
    signature: Signature { parameters: Vec::new() },
    reach: Reach::Container,
  }]);
  builder.add_declaration(class_a);
  for _ in 0..2 {
    builder.add_reference(a, 1, Role::Type, None, Some(class_b), None, Some(false));
  }
  builder.finish(&mut summary, Some(|reach| reach));
  assert_eq!(summary.names, ["A", "B"]);
  assert_eq!(summary.name(summary.declarations[0].name), "B");
  let reaches = |index: usize| summary.declarations[index].constructors.iter().flatten().map(|it| it.reach).collect::<Vec<_>>();
  assert_eq!((reaches(0), reaches(1)), (vec![Reach::Public], vec![Reach::Container]));
  assert_eq!(summary.references.len(), 1);
  assert_eq!(summary.name(summary.references[0].name), "A");
  assert!(summary.truncated, "the caller clears it once the parse succeeded");
}
