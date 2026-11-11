use crate::highlight::Grammar;
use crate::navigate::{Access, ReferenceKind, usages_at, usages_of};
use crate::test_support::{at, kind_and_local, place};
use pretty_assertions::assert_eq;

/// The line, column and access of each usage of the name at the `occurrence`-th `needle`, and whether it is a local
fn usages(grammar: Grammar, text: &str, needle: &str, occurrence: usize) -> (bool, Vec<(u32, u32, Access)>) {
  let found = usages_at(grammar, text, place(text, needle, occurrence)).unwrap();
  (found.local, found.usages.iter().map(|it| (it.start.line, it.start.column, it.access)).collect())
}

#[test]
fn a_call_with_type_arguments_and_a_new_are_calls() {
  let text = "const a = read<Config>(path)\nconst b = new Box<T>()\nconst c = this.load<Data>()\n";
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "read", 0), (ReferenceKind::Call, None));
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "Box", 0), (ReferenceKind::Call, None));
  let load = at(Grammar::TypeScript, text, "load", 0);
  assert_eq!((load.kind, load.qualifier.as_deref()), (ReferenceKind::MemberCall, Some("this")));
}

/// A bare name in a method is no member of the class around it: only `this.` reaches a member
#[test]
fn a_bare_name_is_no_member_of_the_class_around_it() {
  let text = "let count = 0\nclass Counter {\n  count = 1\n  add() { return count + this.count }\n}\n";
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "count", 2), (ReferenceKind::Value, Some((0, 4))));
  let member = at(Grammar::TypeScript, text, "count", 3);
  assert_eq!((member.kind, member.qualifier.as_deref(), member.local), (ReferenceKind::Member, Some("this"), None));
  assert_eq!(at(Grammar::TypeScript, text, "count", 1).kind, ReferenceKind::Declaration);
}

/// A `var` is seen in its whole function, a `let` in its block
#[test]
fn a_var_is_seen_in_its_function_and_a_let_in_its_block() {
  let text = "function run(flag: boolean) {\n  use(total, kept)\n  if (flag) {\n    var total = 1\n    let kept = 2\n  }\n}\n";
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "total", 0), (ReferenceKind::Value, Some((3, 8))));
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "kept", 0), (ReferenceKind::Value, None));
  assert_eq!(
    usages(Grammar::TypeScript, text, "total", 0),
    (true, vec![(1, 6, Access::Read), (3, 8, Access::Declaration)])
  );
}

/// The targets of an assignment write, a destructuring one too, and so does `++`; the object of a member is read
#[test]
fn an_assignment_a_destructuring_and_an_increment_write() {
  let text = "let a = 0, b = 0\n;[a, b] = pair()\n;({ a } = other)\na++\nbox.size = a\n";
  assert_eq!(
    usages(Grammar::TypeScript, text, "a", 0),
    (
      false,
      vec![
        (0, 4, Access::Declaration),
        (1, 2, Access::Write),
        (2, 4, Access::Write),
        (3, 0, Access::Write),
        (4, 11, Access::Read)
      ]
    )
  );
  assert_eq!(usages(Grammar::TypeScript, text, "size", 0).1, [(4, 4, Access::Write)]);
}

/// The name of a member of an enum, a class, an interface and a private name are declarations; `this.#count` is a member
#[test]
fn the_names_of_members_are_declarations() {
  let text = "enum Kind { Round }\nclass Box {\n  #count = 0\n  grow() { this.#count++ }\n}\ninterface Shape { area(): number }\n";
  for (needle, occurrence) in [("Round", 0), ("#count", 0), ("grow", 0), ("area", 0)] {
    assert_eq!(at(Grammar::TypeScript, text, needle, occurrence).kind, ReferenceKind::Declaration, "{needle}");
  }
  let private = at(Grammar::TypeScript, text, "#count", 1);
  assert_eq!((private.kind, private.qualifier.as_deref()), (ReferenceKind::Member, Some("this")));
  assert_eq!(usages(Grammar::TypeScript, text, "#count", 0).1, [(2, 2, Access::Declaration), (3, 16, Access::Write)]);
}

/// A type parameter, what `infer` declares and the parameter of a type predicate go to their declarations
#[test]
fn type_parameters_infer_and_a_predicate_go_to_their_declarations() {
  let text = "type First<T> = T extends [infer H, ...unknown[]] ? H : never\nconst isText = (value: unknown): value is string => true\n";
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "T", 1), (ReferenceKind::Type, Some((0, 11))));
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "H", 1), (ReferenceKind::Type, Some((0, 33))));
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "value", 1), (ReferenceKind::Value, Some((1, 16))));
}

/// A name of an import is a value whose declaration is in the module it names: no local; its usages are the import and
/// the uses of the name
#[test]
fn a_name_of_an_import_goes_to_its_module() {
  let text = "import { helper, other as renamed } from \"./util\"\nimport * as path from \"node:path\"\nhelper(renamed, path.sep)\n";
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "helper", 0), (ReferenceKind::Value, None));
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "helper", 1), (ReferenceKind::Call, None));
  assert_eq!(usages(Grammar::TypeScript, text, "helper", 1), (false, vec![(0, 9, Access::Import), (2, 0, Access::Call)]));
  assert_eq!(usages(Grammar::TypeScript, text, "renamed", 1).1, [(0, 26, Access::Import), (2, 7, Access::Read)]);
  let separator = at(Grammar::TypeScript, text, "sep", 0);
  assert_eq!((separator.kind, separator.qualifier.as_deref()), (ReferenceKind::Member, Some("path")));
}

/// `a.B` in a type position is the type `B` qualified by `a`
#[test]
fn a_qualified_type_is_a_type_with_its_qualifier() {
  let text = "let view: React.FC<Props>\n";
  let fc = at(Grammar::TypeScript, text, "FC", 0);
  assert_eq!((fc.kind, fc.qualifier.as_deref()), (ReferenceKind::Type, Some("React")));
  assert_eq!(at(Grammar::TypeScript, text, "Props", 0).kind, ReferenceKind::Type);
}

/// A JSX tag of a component goes to its declaration in the file, a tag of a member is a member; JavaScript reads JSX
#[test]
fn a_jsx_tag_goes_to_its_component() {
  let text = "function Panel() { return null }\nexport const view = <Panel><Card.Footer /></Panel>\n";
  for grammar in [Grammar::Tsx, Grammar::JavaScript] {
    assert_eq!(kind_and_local(grammar, text, "Panel", 1), (ReferenceKind::Value, Some((0, 9))), "{grammar:?}");
    let footer = at(grammar, text, "Footer", 0);
    assert_eq!((footer.kind, footer.qualifier.as_deref()), (ReferenceKind::Member, Some("Card")), "{grammar:?}");
  }
}

/// A `.ts` file reads `<T>x` as an assertion, whose `x` is a name of the code
#[test]
fn a_typescript_file_reads_a_type_assertion() {
  let text = "const size = 1\nconst other = <number>size\n";
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "size", 1), (ReferenceKind::Value, Some((0, 6))));
  assert_eq!(usages_of(Grammar::TypeScript, text, "size").len(), 2);
}

/// The navigation reads any nesting the parse reads on a 2 MiB thread
#[test]
fn deep_nesting_is_navigated_on_a_small_stack() {
  let thread = std::thread::Builder::new().stack_size(2 << 20).spawn(|| {
    for (open, close) in crate::js::summary_test::NESTINGS {
      let text = crate::js::summary_test::nested_source(20_000, open, close);
      let deep = at(Grammar::TypeScript, &text, "inner", 0);
      assert_eq!(deep.kind, ReferenceKind::Value, "{open}");
      assert_eq!(usages_of(Grammar::TypeScript, &text, "deep").len(), 1, "{open}");
    }
  });
  thread.unwrap().join().unwrap();
}

/// The names after an import type are members of the module it names, the alias of `export * as` a name of its own
#[test]
fn the_names_after_an_import_type_are_members_of_its_module() {
  let text = "let bar: import(\"./title-bar\").TitleBar\nexport * as Dto from \"./dto\"\n";
  let bar = at(Grammar::TypeScript, text, "TitleBar", 0);
  assert_eq!((bar.kind, bar.qualifier.as_deref()), (ReferenceKind::Member, Some("import(\"./title-bar\")")));
  assert_eq!(kind_and_local(Grammar::TypeScript, text, "Dto", 0), (ReferenceKind::Value, None));
}

/// A function has one variable of a name: a `var` of the name of a parameter or of an earlier `var` is a use of that
/// variable (a write when it comes with a value), wherever its block stands
#[test]
fn a_function_has_one_variable_of_a_name() {
  let text = "function f(total, flag) {\n  use(later)\n  if (flag) { var total = 2 }\n  var later = 1\n  var later\n  for (var later of flag) {}\n  return total + later\n}\n";
  for grammar in [Grammar::JavaScript, Grammar::TypeScript] {
    assert_eq!(kind_and_local(grammar, text, "total = 2", 0), (ReferenceKind::Value, Some((0, 11))));
    assert_eq!(
      usages(grammar, text, "total,", 0),
      (true, vec![(0, 11, Access::Declaration), (2, 18, Access::Write), (6, 9, Access::Read)])
    );
    // the first `var` declares, also for the name before it; a later one declares it again or writes it
    assert_eq!(kind_and_local(grammar, text, "later)", 0), (ReferenceKind::Value, Some((3, 6))));
    assert_eq!(kind_and_local(grammar, text, "later\n  for", 0), (ReferenceKind::Value, Some((3, 6))));
    assert_eq!(
      usages(grammar, text, "later = 1", 0),
      (
        true,
        vec![
          (1, 6, Access::Read),
          (3, 6, Access::Declaration),
          (4, 6, Access::Declaration),
          (5, 11, Access::Write),
          (6, 17, Access::Read)
        ]
      )
    );
  }
  // a `var` in a `catch` of the name of its parameter writes the parameter
  let caught = "function f() {\n  try { g() } catch (e) { var e = 1; use(e) }\n}\n";
  assert_eq!(kind_and_local(Grammar::JavaScript, caught, "e = 1", 0), (ReferenceKind::Value, Some((1, 21))));
  assert_eq!(
    usages(Grammar::JavaScript, caught, "e)", 0),
    (true, vec![(1, 21, Access::Declaration), (1, 30, Access::Write), (1, 41, Access::Read)])
  );
  // the name of a function expression is another variable than its parameter of that name
  let named = "const g = function total(total) { return total }\n";
  assert_eq!(kind_and_local(Grammar::JavaScript, named, "total }", 0), (ReferenceKind::Value, Some((0, 25))));
}

/// The signature of a function is read before its body: the default of a parameter sees the parameters and the scope
/// around the function, never a declaration of the body
#[test]
fn the_default_of_a_parameter_is_read_outside_the_body() {
  let text = "function outer(total, sum) {\n  function f(a = total, b = a) { let total = a + b; return total }\n  const g = (c = sum) => { var sum = c; return sum }\n  return f() + g()\n}\n";
  for grammar in [Grammar::JavaScript, Grammar::TypeScript] {
    let local = |needle: &str, occurrence: usize| kind_and_local(grammar, text, needle, occurrence).1;
    // the defaults read the parameters of the function around, and a parameter before them
    assert_eq!((local("total, b", 0), local("a) {", 0), local("sum) =>", 0)), (Some((0, 15)), Some((1, 13)), Some((0, 22))));
    // the bodies read their own declarations
    assert_eq!((local("total }", 0), local("sum }", 0)), (Some((1, 37)), Some((2, 31))));
    assert_eq!(
      usages(grammar, text, "total,", 0),
      (true, vec![(0, 15, Access::Declaration), (1, 17, Access::Read)]),
      "the parameter of the function around, read by the default"
    );
  }
}

/// Types and values are apart: a name read as a type, and a namespace before the dot of one, is of no variable; the
/// members of an enum are seen by their bare names in its body
#[test]
fn a_type_is_of_no_variable_and_an_enum_sees_its_members() {
  let text = "type total = number\nfunction f() {\n  const total = 1\n  const typed: total = 2\n  const other: total.T = 3\n  enum E { sum = 1, twice = sum * 2 }\n  const sum = typeof total\n  return [total, typed, other, sum]\n}\n";
  let found = |needle: &str, occurrence: usize| kind_and_local(Grammar::TypeScript, text, needle, occurrence);
  // the type of the file, not the constant beside it; the namespace of a type is of no variable either
  assert_eq!(found("total = 2", 0), (ReferenceKind::Type, Some((0, 5))));
  assert_eq!(found("total.T", 0), (ReferenceKind::Value, Some((0, 5))));
  // `typeof` reads the value
  assert_eq!(found("total\n  return", 0), (ReferenceKind::Value, Some((2, 8))));
  // the member of the enum, not the constant declared after it
  assert_eq!(found("sum * 2", 0), (ReferenceKind::Value, Some((5, 11))));
  assert_eq!(found("sum]", 0), (ReferenceKind::Value, Some((6, 8))));
  assert_eq!(
    usages(Grammar::TypeScript, text, "total = 1", 0),
    (true, vec![(2, 8, Access::Declaration), (6, 21, Access::Read), (7, 10, Access::Read)])
  );
}

/// A name goes to the declaration the compiler binds it to: a `const` in its block, a `var` in its function, a
/// parameter in its defaults and its body, the parameter of a `catch` in its block alone, never a member of a class.
/// The readings of `testdata/navigate/scopes.ts` are tsc's (7.0)
#[test]
fn a_name_is_bound_where_the_compiler_binds_it() {
  let (wrong, reads) = crate::test_support::readings_unlike_the_compiler(Grammar::TypeScript, "scopes.ts", (2, 14));
  assert!(wrong.is_empty(), "{} of {reads} reads go wrong:\n{}", wrong.len(), wrong.join("\n"));
  assert_eq!(reads, 69);
}
