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
