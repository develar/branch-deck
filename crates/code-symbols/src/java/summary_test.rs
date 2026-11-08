//! The summaries of Java files from their PSI, beside the golden files of `testdata/java/` (`summary_test.rs`) and the
//! rules of `java_test.rs`: each test here is a construct whose facts moved from tree-sitter's walk, a misread of it or
//! a regression of the port fixed, with the reading of PSI.

use crate::language::Language;
use crate::summary::{FileSummary, ImportTarget, MemberKind, Reach};
use crate::test_support::{NONE, constructors_of, declaration, fixtures, refs, signatures, summarize, supertypes_of};

fn java(source: &str) -> FileSummary {
  summarize(Language::Java, source.as_bytes())
}

/// The names of a path of name ids
fn names<'s>(summary: &'s FileSummary, path: Option<&[u32]>) -> Vec<&'s str> {
  path.unwrap_or_default().iter().map(|it| summary.name(*it)).collect()
}

/// The references of a line as `text#container`, sorted
fn line(summary: &FileSummary, line: u32) -> Vec<String> {
  crate::test_support::line_references(summary, line)
}

#[test]
fn every_java_fixture_is_read() {
  for (name, source) in fixtures("java", "java") {
    assert!(!summarize(Language::Java, &source).truncated, "{name}");
  }
}

/// A member of a call's result has an empty qualifier, as of any expression (`Reference::qualifier`): tree-sitter's
/// walk took the callee's path for one (`Hosts.getInstance.known()`), and a field of the callee's name for the type of
/// `this.items().add(…)`
#[test]
fn a_member_of_a_call_has_an_empty_qualifier() {
  let summary = java(
    "class Use {
  private final java.util.List<String> items = null;
  void f() {
    Hosts.getInstance().known(1);
    this.items().add(2);
    items.add(3);
    a.b().c.d();
  }
}
",
  );
  assert_eq!(line(&summary, 4), ["?.known(Number)#f", "Hosts#f", "Hosts.getInstance()#f"]);
  assert_eq!(line(&summary, 5), ["?.add(Number)#f", "this.items()#f"]);
  assert_eq!(line(&summary, 6), ["<java.util.List>items.add(Number)#f", "items#f"], "a field of the class keeps its type");
  assert_eq!(line(&summary, 7), ["?.c#f", "?.d()#f", "a#f", "a.b()#f"]);
}

/// The modifiers and the type of a field are of the class around it, which they are written in for every declarator of
/// the statement; what follows the name is of the field
#[test]
fn the_type_of_a_field_is_of_the_class_its_initializer_of_the_field() {
  let summary = java(
    "class Use {
  @Inject private Service service = Service.create(), other;
}
",
  );
  assert_eq!(line(&summary, 2), [":Inject#Use", ":Service#Use", "Service#service", "Service.create()#service"]);
  assert_eq!(declaration(&summary, "other").header.as_deref(), Some("Service other"));
}

/// A doc comment before a later declarator of a field statement starts it; the head's, before the modifiers, does not,
/// as in tree-sitter's walk
#[test]
fn a_doc_comment_before_a_later_declarator_starts_it() {
  let summary = java("class Foo {\n  /** x */\n  int x = 2, /**\n   * y\n   */\n  y = 1;\n}\n");
  assert_eq!((declaration(&summary, "x").start_line, declaration(&summary, "y").start_line), (3, 3));
  assert_eq!(declaration(&summary, "y").name_line, 6);
}

/// A doc comment documents the declaration right below it: one above another comment (a commented-out method) is not
/// the next declaration's, which tree-sitter's walk extended to every doc comment since the previous declaration
#[test]
fn only_the_doc_comment_right_above_starts_a_declaration() {
  let summary = java("class A {\n  /** gone */\n  /* void gone() {} */\n  /** kept */\n  void kept() {}\n}\n");
  assert_eq!(declaration(&summary, "kept").start_line, 4);
  let summary = java("/** Doc */\n/// Markdown\nclass B {}\n");
  assert_eq!(declaration(&summary, "B").start_line, 2);
}

/// An annotated type is the type: `extends @A List<…>` has the supertype `List` (tree-sitter's `annotated_type` had no
/// path), and its reference no qualifier of the annotation
#[test]
fn an_annotated_supertype_is_a_supertype() {
  let summary = java("class Test extends @Foo(2) List<@Foo(3) String> implements java.io.@A Serializable {}\n");
  assert_eq!(supertypes_of(&summary, "Test"), ["List", "java.io.Serializable"]);
  assert_eq!(refs(&summary, "List"), ["1 :List"]);
  assert_eq!(refs(&summary, "Serializable"), ["1 :java.io.Serializable"]);
}

/// The vararg of `String @NotNull ... params` (JLS 8.4.1), which tree-sitter-java 0.23.5 could not read: the method lost
/// the parameter, its class its constructors
#[test]
fn an_annotated_vararg_is_a_parameter() {
  let summary = java("class Bundle {\n  static String message(String key, Object @NotNull ... params) { return key; }\n}\n");
  assert_eq!(signatures(&summary, "message"), ["Bundle (key: String, vararg params: Object[])"]);
  assert_eq!(constructors_of(&summary, "Bundle").unwrap(), ["Module new()"]);
  assert_eq!(summary.error_bytes, 0);
}

/// `case null, default ->` (JEP 441): `default` is the label, no name
#[test]
fn the_default_of_a_case_label_is_no_name() {
  let summary = java("class A {\n  int f(Object o) {\n    return switch (o) { case null, default -> 0; };\n  }\n}\n");
  assert_eq!(refs(&summary, "default"), NONE);
}

/// `case R() when …` is a record pattern, no call; `case BOLD -> x -> …` a label and a lambda
#[test]
fn a_case_label_is_a_pattern_or_a_constant() {
  let summary = java(
    "class A {
  void f(Object o, Style s) {
    switch (o) { case R() when true: break; default: break; }
    Function<String, String> g = switch (s) { case BOLD -> x -> x; default -> null; };
  }
}
",
  );
  assert_eq!(refs(&summary, "R"), ["3 :R"]);
  assert_eq!(refs(&summary, "BOLD"), ["4 BOLD"]);
  assert_eq!(refs(&summary, "x"), NONE, "the parameter of the lambda");
}

/// `import module java.base;` (JEP 476) names a module, no package and no type: no import, its lines still an import's
#[test]
fn a_module_import_imports_no_name() {
  let summary = java("import module java.base;\nimport java.util.List;\nclass A {}\n");
  assert_eq!(summary.imports.len(), 1);
  assert_eq!(summary.imports[0].target, ImportTarget::Name(summary.name_id("List").unwrap()));
  assert_eq!(summary.import_lines.len(), 2);
  assert_eq!(refs(&summary, "base"), NONE);
}

/// Comments inside an import are no part of its path, which tree-sitter lost
#[test]
fn an_import_with_comments_inside() {
  let summary = java("import  /* a */   lombok./* b */Getter   /* c */   ;\nclass A {}\n");
  assert_eq!(refs(&summary, "Getter"), ["1 ^lombok.Getter"]);
}

/// `Outer.super::method` names the method of the superclass of `Outer`, qualified by `Outer` as `Outer.super.method()`
/// is; tree-sitter read `Outer.super` as a type of that name
#[test]
fn a_method_reference_of_a_qualified_super() {
  let summary = java("class T extends Base {\n  void f() { run(T.super::setUp); }\n}\n");
  assert_eq!(refs(&summary, "setUp"), ["2 T.setUp"]);
  assert_eq!(refs(&summary, "super"), NONE);
}

/// The receiver parameter `@A Type this` (JLS 8.4) is no parameter: not of the signature, no names
#[test]
fn an_annotated_receiver_parameter_is_no_parameter() {
  let summary = java("class R {\n  void m(@Nullable R this, int i) {}\n}\n");
  assert_eq!(signatures(&summary, "m"), ["R (i: int)"]);
  assert_eq!(refs(&summary, "Nullable"), NONE);
}

/// The default value of an annotation's element is in its header as its tokens are: a string literal is one token,
/// which tree-sitter split into its quotes and its text
#[test]
fn the_header_of_an_annotation_element_has_its_default() {
  let summary = java("@interface A {\n  String value() default \"a b\";\n  String[] names() default {\"x\", \"\"};\n}\n");
  assert_eq!(declaration(&summary, "value").header.as_deref(), Some("String value ( ) \"a b\""));
  assert_eq!(declaration(&summary, "names").header.as_deref(), Some("String [ ] names ( ) { \"x\" , \"\" }"));
}

/// A compact source file (JEP 512): its fields and methods are those of the implicit class, the top level of the file;
/// a field has the header of a field, which tree-sitter read as a local variable statement
#[test]
fn a_compact_source_file_declares_at_the_top_level() {
  let summary = java("import java.util.List;\nint x = 1;\nvoid main() { foo(); }\n");
  assert_eq!(declaration(&summary, "x").header.as_deref(), Some("int x"));
  assert_eq!(declaration(&summary, "main").container, None);
  assert_eq!(refs(&summary, "foo"), ["3 foo()"]);
}

/// An anonymous class is a body: its methods are locals, the calls in them `enclosed`; a local of a `case` group is
/// bound until the next group, as in tree-sitter's walk
#[test]
fn bodies_and_case_groups_scope_their_names() {
  let summary = java(
    "class A {
  void f(int k) {
    switch (k) { case 1: int z = 1; z++; break; case 2: z = 2; }
    new Base() { void run() { go(); } };
  }
}
",
  );
  assert_eq!(refs(&summary, "z"), ["3 z"]);
  assert_eq!(refs(&summary, "go"), ["4 go(){}"]);
  assert_eq!(refs(&summary, "run"), NONE, "no `@Override`");
}

/// A vararg component of a record is a public field like any other component, whose header has its type; tree-sitter's
/// walk read its declarator as a field of the package without a type, which no use from another package reached
#[test]
fn a_vararg_record_component_is_a_public_field() {
  let summary = java("record V(String name, Foo... rest) {}\n");
  let rest = declaration(&summary, "rest");
  assert_eq!((rest.reach, rest.member), (Reach::Public, MemberKind::Instance));
  assert_eq!(rest.header.as_deref(), Some("Foo ... rest"));
  assert_eq!(constructors_of(&summary, "V").unwrap(), ["Module new(name: String, vararg rest: Foo[])"]);
}

/// `Foo a[]` declares an array (JLS 10.2): a field, a local or a parameter of it has no `value_type`, and a member of it
/// no receiver type, which tree-sitter's walk took from the type before the name
#[test]
fn an_array_after_the_name_has_no_type() {
  let summary = java(
    "class A {
  Foo field[], other;
  void f(Foo args[], Foo plain) {
    Foo local[] = null;
    local.length(); args.clone(); plain.bar(); field.baz(); other.qux();
  }
}
",
  );
  assert_eq!(declaration(&summary, "field").value_type, None);
  assert_eq!(names(&summary, declaration(&summary, "other").value_type.as_deref()), ["Foo"]);
  assert_eq!(
    line(&summary, 5),
    ["<Foo>.bar()#f", "<Foo>other.qux()#f", "?.clone()#f", "?.length()#f", "field#f", "field.baz()#f", "other#f"]
  );
}

/// A chain of names `a.b.c…` is a reference per segment, each qualified by the segments before it: past `MAX_PATH`
/// segments a qualifier is empty, as of an expression, so that the qualifiers of a chain of `n` segments don't take `n²/2`
/// names
#[test]
fn a_long_chain_of_names_takes_linear_space() {
  let segments = 5_000;
  let source = format!(
    "class A {{ Object v = a{}; }}\n",
    (0..segments).map(|i| if i % 50 == 0 { "\n.a" } else { ".a" }).collect::<String>()
  );
  let summary = java(&source);
  assert!(!summary.truncated);
  let names: usize = summary.references.iter().filter_map(|it| it.qualifier.as_ref()).map(Vec::len).sum();
  assert!(names <= summary.references.len() * 64, "{names} names in {} references", summary.references.len());
  // a short path is whole
  let summary = java("class A { Object v = a.b.c.d; }\n");
  assert_eq!(refs(&summary, "d"), ["1 a.b.c.d"]);
}

/// A construct nested as deep as the parser allows is walked without a frame of the stack per level
#[test]
fn a_deep_file_is_read() {
  let depth = 900;
  // a line per bracket: a long line would be over `MAX_AVERAGE_LINE`
  let source = format!("class A {{ int v = {}1{}; }}\n", "(\n".repeat(depth), ")\n".repeat(depth));
  let summary = std::thread::Builder::new().stack_size(8 << 20).spawn(move || java(&source)).unwrap().join().unwrap();
  assert!(!summary.truncated);
  assert_eq!(declaration(&summary, "v").header.as_deref(), Some("int v"));
}

/// A field's and an abstract method's `;` is their own text, as `PsiField.getTextRange()` holds it, while the `,`
/// between enum constants is the class's: the tree answers where a declaration ends (`Tree::declaration_range`), so
/// a `;` written on a line of its own is the declaration's last line.
#[test]
fn a_declaration_ends_at_the_semicolon_that_is_its_own() {
  let summary = java(
    "class Use {
  int field =
    compute()
    ;
  enum Kind {
    FIRST,
    SECOND
  }
  static int compute() { return 1; }
}
",
  );
  let field = declaration(&summary, "field");
  assert_eq!((field.start_line, field.end_line), (2, 4), "the `;` of a field is its own");
  let first = declaration(&summary, "FIRST");
  assert_eq!((first.start_line, first.end_line), (6, 6), "the `,` after an enum constant is the class's");
}
