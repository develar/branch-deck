//! The summaries of C# files from Roslyn's tree, beside the golden files of `testdata/csharp/` (`summary_test.rs`): each
//! test here is a construct of C# the model of the summary reads in its own way

use crate::extract::summarize;
use crate::language::Language;
use crate::summary::{DeclKind, FileSummary, ImportTarget, MemberKind, Reach};
use crate::test_support::{NONE, constructors_of, declaration, fixtures, refs};
use pretty_assertions::assert_eq;

fn csharp(source: &str) -> FileSummary {
  summarize(Language::CSharp, source.as_bytes())
}

/// The references of a line as `text#container`, sorted
fn line(summary: &FileSummary, line: u32) -> Vec<String> {
  crate::test_support::line_references(summary, line)
}

#[test]
fn every_csharp_fixture_is_read() {
  for (name, source) in fixtures("csharp", "cs") {
    assert!(!summarize(Language::CSharp, &source).truncated, "{name}");
  }
}

#[test]
fn the_package_is_the_first_namespace_with_the_namespaces_around_it() {
  assert_eq!(csharp("namespace A.B { namespace C { class D {} } }\n").package.as_deref(), Some("A.B.C"));
  assert_eq!(csharp("namespace A.B;\nclass D {}\n").package.as_deref(), Some("A.B"));
  assert_eq!(csharp("class D {}\n").package, None);
  // a namespace's name is no reference
  assert_eq!(csharp("namespace A.B;\nclass D {}\n").references, []);
}

#[test]
fn usings_import_everything_of_their_path_and_an_alias_its_target() {
  let summary = csharp("using A.B;\nusing static A.C;\nglobal using X = A.D.E<int>;\nusing P = (int, int);\nclass F {}\n");
  let imports: Vec<(String, Option<String>, Option<String>)> = summary
    .imports
    .iter()
    .map(|it| {
      let path = it.path.iter().map(|id| summary.name(*id)).collect::<Vec<_>>().join(".");
      let target = match it.target {
        ImportTarget::Name(name) => Some(summary.name(name).to_string()),
        _ => None,
      };
      (path, target, it.alias.map(|alias| summary.name(alias).to_string()))
    })
    .collect();
  assert_eq!(
    imports,
    [
      ("A.B".to_string(), None, None),
      ("A.C".to_string(), None, None),
      ("A.D".to_string(), Some("E".to_string()), Some("X".to_string())),
    ]
  );
  assert_eq!(refs(&summary, "E"), ["3 ^A.D.E"]);
  assert_eq!(summary.import_lines.len(), 4);
}

#[test]
fn a_nested_type_is_static_and_a_member_is_private_without_a_modifier() {
  let summary = csharp("class Outer {\n  class Inner { int x; }\n  interface IFace { void F(); }\n  enum E { A }\n}\n");
  let traits = |name: &str| {
    let it = declaration(&summary, name);
    (it.reach, it.member)
  };
  assert_eq!(traits("Outer"), (Reach::Module, MemberKind::TopLevel));
  assert_eq!(traits("Inner"), (Reach::Container, MemberKind::Static));
  assert_eq!(traits("x"), (Reach::Container, MemberKind::Instance));
  // a member of an interface and an enum member are public
  assert_eq!(traits("F"), (Reach::Public, MemberKind::Instance));
  assert_eq!(traits("A"), (Reach::Public, MemberKind::Static));
}

#[test]
fn a_record_has_its_positional_parameters_as_properties_and_its_primary_constructor() {
  let summary = csharp("record Point(int X, int Y) { public Point(int x) : this(x, 0) {} }\nclass Box(int size) { int Twice() => size * 2; }\n");
  assert_eq!(declaration(&summary, "X").kind, DeclKind::Value);
  // a primary constructor is public, a declared one by its modifiers
  assert_eq!(constructors_of(&summary, "Point"), Some(vec!["new(X: int, Y: int)".to_string(), "new(x: int)".to_string()]));
  // a class's primary constructor: a constructor, its parameter a bound name, no property
  assert_eq!(constructors_of(&summary, "Box"), Some(vec!["new(size: int)".to_string()]));
  assert!(!summary.declares("size"));
  assert_eq!(refs(&summary, "size"), NONE);
}

#[test]
fn a_static_class_an_interface_and_an_enum_have_no_constructors() {
  let summary = csharp("static class S {}\ninterface I {}\nenum E { A }\nstruct P {}\nclass C { static C() {} }\n");
  assert_eq!(constructors_of(&summary, "S"), None);
  assert_eq!(constructors_of(&summary, "I"), None);
  assert_eq!(constructors_of(&summary, "E"), None);
  assert_eq!(constructors_of(&summary, "P"), Some(vec!["new()".to_string()]));
  // a static constructor is none: the class has the implicit one
  assert_eq!(constructors_of(&summary, "C"), Some(vec!["new()".to_string()]));
}

#[test]
fn an_extension_method_and_a_member_of_an_extension_block_have_their_receiver() {
  let summary = csharp(
    "static class X {\n  public static int Twice(this int value) => value * 2;\n  public static int Plain(int value) => value;\n  extension(Store store) { public void Reset() {} }\n}\n",
  );
  let receiver = |name: &str| declaration(&summary, name).receiver.map(|it| summary.name(it).to_string());
  assert_eq!(receiver("Twice").as_deref(), Some("int"));
  assert_eq!(receiver("Plain"), None);
  assert_eq!(receiver("Reset").as_deref(), Some("Store"));
  assert_eq!(declaration(&summary, "Reset").member, MemberKind::Static);
}

#[test]
fn an_attribute_names_its_class_too() {
  // the attributes of a type are of its container, as its modifiers
  let summary = csharp("[Obsolete, Serializable] [System.Diagnostics.DebuggerDisplay(\"x\")] [MyCheckAttribute]\nclass A {}\n");
  assert_eq!(
    line(&summary, 1),
    [
      ":MyCheckAttribute#A",
      ":Obsolete#A",
      ":ObsoleteAttribute#A",
      ":Serializable#A",
      ":SerializableAttribute#A",
      ":System#A",
      ":System.Diagnostics#A",
      ":System.Diagnostics.DebuggerDisplay#A",
      ":System.Diagnostics.DebuggerDisplayAttribute#A",
    ]
  );
}

#[test]
fn locals_parameters_patterns_and_query_variables_are_bound() {
  let summary = csharp(
    "class A {\n  int count;\n  void F(int count, object o) {\n    if (o is string text) text.Trim();\n    int Local() => count;\n    var q = from item in items let n = item.Size select n;\n    Local();\n    foreach (var each in items) each.Go();\n  }\n}\n",
  );
  // the parameter hides the field; the pattern's variable, the local function and the range variables are bound
  assert_eq!(refs(&summary, "count"), NONE);
  assert_eq!(refs(&summary, "text"), NONE);
  assert_eq!(refs(&summary, "Local"), NONE);
  assert_eq!(refs(&summary, "item"), NONE);
  assert_eq!(refs(&summary, "n"), NONE);
  assert_eq!(refs(&summary, "each"), NONE);
  assert_eq!(refs(&summary, "items"), ["6 items", "8 items"]);
}

#[test]
fn a_verbatim_identifier_is_its_value() {
  let summary = csharp("class @class { int @event; void F() { @event = 1; } }\n");
  assert!(summary.declares("class"));
  assert!(summary.declares("event"));
  assert_eq!(refs(&summary, "event"), ["1 event"]);
}

#[test]
fn a_branch_of_if_the_configuration_does_not_take_is_not_read() {
  let summary = csharp("class A {\n#if DEBUG\n  void Debug() { Trace(); }\n#else\n  void Release() {}\n#endif\n}\n");
  assert!(!summary.declares("Debug"));
  assert!(summary.declares("Release"));
  assert_eq!(refs(&summary, "Trace"), NONE);
}

#[test]
fn a_documentation_comment_names_nothing_and_starts_its_declaration() {
  let summary = csharp("class A {\n  /// <see cref=\"Other\"/>\n  [Obsolete]\n  public void F() {}\n}\n");
  assert_eq!(refs(&summary, "Other"), NONE);
  assert_eq!(declaration(&summary, "F").start_line, 2);
}

#[test]
fn a_missing_token_counts_as_an_error_and_marks_its_declaration() {
  let summary = csharp("class A {\n  int F( { }\n}\n");
  assert!(summary.error_bytes > 0);
  let clean = csharp("class A { void F() {} }\n");
  assert_eq!(clean.error_bytes, 0);
  assert!(!declaration(&clean, "F").in_error);
  // a type with an error in it has unknown constructors
  assert_eq!(constructors_of(&summary, "A"), None);
}

#[test]
fn top_level_statements_bind_their_locals() {
  let summary = csharp("var app = Builder.Create(args);\napp.Run();\nint Helper() => 1;\nHelper();\n");
  assert_eq!(refs(&summary, "app"), NONE);
  assert_eq!(refs(&summary, "Helper"), NONE);
  assert_eq!(refs(&summary, "Create"), ["1 Builder.Create(_)"]);
}

#[test]
fn a_deep_chain_and_deep_nesting_take_no_stack_per_level() {
  // a line each, within the limits of lines and of the average line
  let chain = format!("class A {{ object F() => a{}; }}\n", "\n.b()".repeat(10_000));
  assert!(!csharp(&chain).truncated);
  // past Roslyn's nesting the file is one skipped token, as Roslyn answers an exhausted stack: read, and an error
  let nested = format!("class A {{ int F() => {}1{}; }}\n", "(\n".repeat(5_000), ")\n".repeat(5_000));
  let summary = csharp(&nested);
  assert!(!summary.truncated);
  assert!(summary.error_bytes > 0 || summary.declares("F"));
}

/// C# has no separator inside a declaration for the summary to drop: Roslyn puts the `;` in the `FieldDeclaration`
/// around the declarator the summary declares and the `,` in the enum's list, so a declaration ends at its own last
/// token (`Tree::declaration_range`) and no `,` or `;` is ever cut from it.
#[test]
fn no_separator_is_cut_from_a_declaration() {
  let summary = csharp(
    "class Use {
  int field =
    Compute()
    ;
  enum Kind {
    First,
    Second
  }
  static int Compute() { return 1; }
}
",
  );
  let field = declaration(&summary, "field");
  assert_eq!(
    (field.start_line, field.end_line),
    (2, 3),
    "a declarator ends at its value, the `;` being the declaration's around it"
  );
  let first = declaration(&summary, "First");
  assert_eq!((first.start_line, first.end_line), (6, 6), "the `,` after an enum member is the list's");
}
