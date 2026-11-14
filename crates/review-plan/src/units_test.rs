use crate::ReviewPlan;
use crate::api::{ApiChangeKind, FormChange};
use crate::fan_in_test::{Counts, reordered};
use crate::files::MechanicalKind;
use crate::test_support::{plan_by_accio, plan_of, range};
use crate::units::{OtherReason, ReviewUnit, UnitKind};
use diff_marks::index::Order;
use file_diffs::accio::SymbolIndex;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The plan of `base..head` of `repo`, without an index
fn plan(repo: &TestRepo, base: &str, head: &str) -> ReviewPlan {
  plan_of(repo, None, &range(repo, base, head)).unwrap()
}

/// The lines of the function `name` of `text`, from 1, both ends included: from the line it starts at to the first
/// line after it that is `}`
fn function_lines(text: &str, name: &str) -> [u32; 2] {
  let lines: Vec<&str> = text.lines().collect();
  let start = lines.iter().position(|line| line.starts_with(&format!("fun {name}("))).unwrap();
  let end = start + lines[start..].iter().position(|line| *line == "}").unwrap();
  [u32::try_from(start + 1).unwrap(), u32::try_from(end + 1).unwrap()]
}

/// A unit as a test reads it: its kind, its name, and per slice its path with the places of its blocks
type Unit<'a> = (UnitKind, Option<&'a str>, Vec<(&'a str, Vec<u32>)>);

/// Each unit of `plan` (`Unit`)
fn units(plan: &ReviewPlan) -> Vec<Unit<'_>> {
  plan
    .units
    .iter()
    .map(|unit| {
      let slices = unit
        .slices
        .iter()
        .map(|slice| (slice.path.as_str(), slice.blocks.iter().map(|block| block.index).collect()))
        .collect();
      (unit.kind.clone(), unit.name.as_deref(), slices)
    })
    .collect()
}

/// The places of the blocks per path over all the units: each once, none missing before the last
fn blocks_by_path(plan: &ReviewPlan) -> BTreeMap<&str, Vec<u32>> {
  let mut blocks: BTreeMap<&str, Vec<u32>> = BTreeMap::new();
  for slice in plan.units.iter().flat_map(|unit| &unit.slices) {
    blocks.entry(slice.path.as_str()).or_default().extend(slice.blocks.iter().map(|block| block.index));
  }
  for indices in blocks.values_mut() {
    indices.sort_unstable();
  }
  blocks
}

const STORE_BEFORE: &str = "package a

import b.Cache

fun open(x: Int): Int {
  return x
}

fun close(x: Int): Int {
  return x
}

fun gone(x: Int): Int {
  return x
}
";

const STORE_AFTER: &str = "package a

import b.Cache
import b.Index

fun open(x: Int, y: Int): Int {
  return x
}

fun close(x: Int): Int {
  return x + 1
}

fun fresh(x: Int): Int {
  return x * 2
}
";

/// A change of a form, of a body, a new and a removed declaration and a new import of one file are each a unit; a
/// file of no language, a new file and a deleted one each one unit of the whole file; every block in one unit
#[test]
fn each_change_of_a_file_is_one_unit_and_every_block_is_in_one() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add store", &[("src/a/Store.kt", STORE_BEFORE), ("README.md", "readme\n")]);
  let head = repo.create_commit_with_files(
    "Change store",
    &[
      ("src/a/Store.kt", STORE_AFTER),
      ("README.md", "readme 2\n"),
      ("src/a/Fresh.kt", "package a\n\nclass Fresh\n"),
    ],
  );
  let plan = plan(&repo, &base, &head);
  let kinds: Vec<(UnitKind, Option<&str>)> = units(&plan).into_iter().map(|(kind, name, _)| (kind, name)).collect();
  assert_eq!(
    kinds,
    [
      (UnitKind::NewFile, Some("Fresh.kt")),
      (
        UnitKind::Shape {
          changes: vec![FormChange::Parameters]
        },
        Some("open")
      ),
      (UnitKind::Behaviour, Some("close")),
      (UnitKind::Added, Some("fresh")),
      (UnitKind::Removed, Some("gone")),
      (UnitKind::Other { reason: OtherReason::NoLanguage }, Some("README.md")),
      (
        UnitKind::Mechanical {
          mechanical: MechanicalKind::ImportsOnly
        },
        None
      ),
    ]
  );
  let blocks = blocks_by_path(&plan);
  let store = &blocks["src/a/Store.kt"];
  assert_eq!(*store, (0..u32::try_from(store.len()).unwrap()).collect::<Vec<_>>());
}

/// Of two overloads of one name, only the one whose body changed is a unit, named by its signature
#[test]
fn an_overload_is_a_unit_of_its_own() {
  let repo = TestRepo::new();
  let before = "package a\n\nfun open(x: Int): Int {\n  return x\n}\n\nfun open(x: String): Int {\n  return 0\n}\n";
  let after = "package a\n\nfun open(x: Int): Int {\n  return x\n}\n\nfun open(x: String): Int {\n  return 1\n}\n";
  let base = repo.create_commit("Add open", "src/a/Open.kt", before);
  let head = repo.create_commit("Change open", "src/a/Open.kt", after);
  let plan = plan(&repo, &base, &head);
  let found: Vec<(&UnitKind, Option<&str>)> = plan.units.iter().map(|unit| (&unit.kind, unit.signature.as_deref())).collect();
  assert_eq!(found, [(&UnitKind::Behaviour, Some("x:String"))]);
}

/// A function added after another takes the block, though the block starts at the closing brace of the other: the
/// declaration that holds most of its changed lines owns it
#[test]
fn a_block_belongs_to_the_declaration_of_most_of_its_lines() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add open", "src/a/Open.kt", "package a\n\nfun open(x: Int): Int {\n  return x\n}\n");
  let head = repo.create_commit(
    "Add close",
    "src/a/Open.kt",
    "package a\n\nfun open(x: Int): Int {\n  return x\n}\n\nfun close(x: Int): Int {\n  return x\n}\n",
  );
  let plan = plan(&repo, &base, &head);
  assert_eq!(units(&plan), [(UnitKind::Added, Some("close"), vec![("src/a/Open.kt", vec![0])])]);
}

/// The declarations of one block that hold none of it follow the one that holds it in the order the source declares
/// them, not by their names: the hunk names them so
#[test]
fn the_declarations_of_one_block_follow_in_the_order_of_their_lines() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add open", "src/a/Open.kt", "package a\n\nfun open(x: Int): Int {\n  return x\n}\n");
  let head = repo.create_commit(
    "Add the constants",
    "src/a/Open.kt",
    "package a\n\nfun open(x: Int): Int {\n  return x\n}\n\nfun zeta(x: Int): Int {\n  return x\n}\nconst val MIDDLE = 2\nconst val ALPHA = 1\n",
  );
  let plan = plan(&repo, &base, &head);
  assert_eq!(
    units(&plan),
    [
      (UnitKind::Added, Some("zeta"), vec![("src/a/Open.kt", vec![0])]),
      (UnitKind::Added, Some("MIDDLE"), vec![("src/a/Open.kt", vec![])]),
      (UnitKind::Added, Some("ALPHA"), vec![("src/a/Open.kt", vec![])]),
    ]
  );
}

/// The files as `create_commit_with_files` takes them
fn as_refs(files: &[(String, String)]) -> Vec<(&str, &str)> {
  files.iter().map(|(path, text)| (path.as_str(), text.as_str())).collect()
}

/// The same edit of three files is one unit over the three, and the files that only reindent one line are one unit
/// of whitespace, after the rest
#[test]
fn a_repeated_edit_and_the_whitespace_are_one_unit_each_over_their_files() {
  let repo = TestRepo::new();
  let source = |name: &str, body: &str| format!("package a\n\nfun {name}(x: Int): Int {{\n{body}\n}}\n");
  let names = ["login", "logout", "session", "token"];
  let write = |body: &dyn Fn(&str) -> &'static str| -> Vec<(String, String)> { names.iter().map(|name| (format!("src/a/{name}.kt"), source(name, body(name)))).collect() };
  let before = write(&|_| "  return x");
  let after = write(&|name| if name == "token" { "return x" } else { "  return checkedValueOf(x)" });
  let base = repo.create_commit_with_files("Add", &as_refs(&before));
  let head = repo.create_commit_with_files("Edit", &as_refs(&after));
  let plan = plan(&repo, &base, &head);
  let kinds: Vec<(&UnitKind, Vec<&str>)> = plan
    .units
    .iter()
    .map(|unit| (&unit.kind, unit.slices.iter().map(|slice| slice.path.as_str()).collect()))
    .collect();
  assert_eq!(
    kinds,
    [
      (&UnitKind::RepeatedEdit { group: 0 }, vec!["src/a/login.kt", "src/a/logout.kt", "src/a/session.kt"]),
      (
        &UnitKind::Mechanical {
          mechanical: MechanicalKind::Whitespace
        },
        vec!["src/a/token.kt"]
      ),
    ]
  );
}

/// The generated files are one unit over all of them, the last of the mechanical units: a reviewer skips them whole; a
/// deleted file is no mechanical change
#[test]
fn the_generated_files_are_one_unit_after_the_other_mechanical_ones() {
  let repo = TestRepo::new();
  let agents = |tool: &str| format!("<!-- Generated by tools/render.mjs; edit src/guide.md -->\n\nSearch the source with {tool}.\n");
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("AGENTS.md", &agents("grep")),
      ("pnpm-lock.yaml", "lockfileVersion: '9.0'\n"),
      ("src/a/Gone.kt", "package a\n"),
      ("src/a/App.kt", "package a\n\nfun app() = 1\n"),
    ],
  );
  repo.git(&["rm", "-q", "src/a/Gone.kt"]);
  let head = repo.create_commit_with_files(
    "Edit",
    &[
      ("AGENTS.md", &agents("accio")),
      ("pnpm-lock.yaml", "lockfileVersion: '9.1'\n"),
      ("src/a/App.kt", "package a\n\nfun app() = 2\n"),
    ],
  );
  let plan = plan(&repo, &base, &head);
  let mechanical: Vec<(&str, Vec<&str>)> = plan
    .units
    .iter()
    .filter(|unit| matches!(unit.kind, UnitKind::Mechanical { .. }))
    .map(|unit| (unit.id.as_str(), unit.slices.iter().map(|slice| slice.path.as_str()).collect()))
    .collect();
  assert_eq!(mechanical, [("mechanical:generated", vec!["AGENTS.md", "pnpm-lock.yaml"])]);
}

/// A deleted file is a unit of its own in the story, its author's decision; three or more deleted from one directory
/// are one unit, the deletion of a part of the tree
#[test]
fn a_deleted_file_is_a_unit_and_the_deleted_files_of_a_directory_are_one() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("dotnet/.codex/config.toml", "[mcp_servers.ijproxy]\ntype = \"stdio\"\n"),
      ("old/a/A.kt", "package a\n"),
      ("old/a/B.kt", "package a\n"),
      ("old/a/C.kt", "package a\n"),
      ("src/a/App.kt", "package a\n\nfun app() = 1\n"),
    ],
  );
  repo.git(&["rm", "-q", "dotnet/.codex/config.toml", "old/a/A.kt", "old/a/B.kt", "old/a/C.kt"]);
  let head = repo.create_commit_with_files("Delete", &[("src/a/App.kt", "package a\n\nfun app() = 2\n")]);
  let plan = plan(&repo, &base, &head);
  let kinds: Vec<(&str, &UnitKind, Vec<&str>)> = plan
    .units
    .iter()
    .map(|unit| (unit.id.as_str(), &unit.kind, unit.slices.iter().map(|slice| slice.path.as_str()).collect()))
    .collect();
  let directory = UnitKind::DeletedFiles { directory: "old/a".to_string() };
  assert_eq!(
    kinds,
    [
      ("deleted:dotnet/.codex/config.toml", &UnitKind::DeletedFile, vec!["dotnet/.codex/config.toml"]),
      ("deleted:old/a/", &directory, vec!["old/a/A.kt", "old/a/B.kt", "old/a/C.kt"]),
      ("declaration:src/a/App.kt:a.app:", &UnitKind::Behaviour, vec!["src/a/App.kt"]),
    ]
  );
  let deleted = plan.files.iter().filter(|file| file.path != "src/a/App.kt");
  assert!(deleted.clone().all(|file| file.mechanical.is_none() && !file.untested), "{:#?}", plan.files);
}

/// A unit's fingerprint is that of its change, not of where it sits: the same change of a body on another base gives
/// the same fingerprint, a different change another one
#[test]
fn the_fingerprint_names_the_change_wherever_it_sits() {
  let behaviour = |repo: &TestRepo, base: &str, head: &str| -> ReviewUnit {
    let plan = plan(repo, base, head);
    plan.units.into_iter().find(|unit| unit.kind == UnitKind::Behaviour).unwrap()
  };
  let before = "package a\n\nfun open(x: Int): Int {\n  return x\n}\n";
  let after = "package a\n\nfun open(x: Int): Int {\n  return x + 1\n}\n";
  let repo = TestRepo::new();
  let base = repo.create_commit("Add open", "src/a/Open.kt", before);
  let head = repo.create_commit("Change open", "src/a/Open.kt", after);
  let first = behaviour(&repo, &base, &head);
  // the same change above a line added before the function
  let moved = repo.create_commit("Move open", "src/a/Open.kt", &before.replace("package a\n", "package a\n\n// moved\n"));
  let again = repo.create_commit("Change open", "src/a/Open.kt", &after.replace("package a\n", "package a\n\n// moved\n"));
  let second = behaviour(&repo, &moved, &again);
  let other = repo.create_commit(
    "Change open",
    "src/a/Open.kt",
    &after.replace("x + 1", "x + 2").replace("package a\n", "package a\n\n// moved\n"),
  );
  let third = behaviour(&repo, &again, &other);
  assert_eq!(first.fingerprint, second.fingerprint);
  assert_ne!(first.fingerprint, third.fingerprint);
  assert_eq!(first.id, second.id);
}

/// A block and a declaration are whitespace when their tokens are the same: a call wrapped inside its brackets goes to
/// the whitespace of the plan, a space inside a string is a change of the behaviour
#[test]
fn a_block_of_whitespace_is_told_by_its_tokens() {
  let source = |wrap: &str, text: &str| format!("package a\n\nfun wrap(x: Int): Int {{\n  return {wrap}\n}}\n\nfun text(): String {{\n  return {text}\n}}\n");
  let repo = TestRepo::new();
  let base = repo.create_commit("Add", "src/a/Open.kt", &source("compute(x, 1)", "\"a  b\""));
  let head = repo.create_commit("Format", "src/a/Open.kt", &source("compute(\n    x,\n    1\n  )", "\"a b\""));
  let plan = plan(&repo, &base, &head);
  assert_eq!(
    units(&plan),
    [
      (UnitKind::Behaviour, Some("text"), vec![("src/a/Open.kt", vec![1])]),
      (
        UnitKind::Mechanical {
          mechanical: MechanicalKind::Whitespace
        },
        None,
        vec![("src/a/Open.kt", vec![0])]
      ),
    ]
  );
}

/// A declaration whose lines differ by whitespace alone is whitespace with every line its blocks change: a block that
/// reaches past it, into the comment after it that changed too, is a change of the behaviour and not of whitespace
#[test]
fn a_declaration_is_whitespace_only_with_every_line_of_its_blocks() {
  let source = |one: &str, comment: &str| format!("package a\n\nfun one() ={one}compute(1)\n// {comment}\n\nfun two() = 2\n");
  let repo = TestRepo::new();
  let files = |one: &str, comment: &str| [("src/a/Reach.kt", source(one, comment)), ("src/a/Joined.kt", source(one, "a"))];
  let base = repo.create_commit_with_files("Add", &as_refs(&files("\n  ", "a").map(|(path, text)| (path.to_string(), text))));
  let head = repo.create_commit_with_files("Join", &as_refs(&files(" ", "b").map(|(path, text)| (path.to_string(), text))));
  let plan = plan(&repo, &base, &head);
  assert_eq!(
    units(&plan),
    [
      (UnitKind::Behaviour, Some("one"), vec![("src/a/Reach.kt", vec![0])]),
      (
        UnitKind::Mechanical {
          mechanical: MechanicalKind::Whitespace
        },
        None,
        vec![("src/a/Joined.kt", vec![0])]
      ),
    ]
  );
}

/// A declaration that stands at another place of its file with the same code is moved where the index says its order
/// is free, whatever stands around it there, wrapped again or not; read before the index answered, without one, and
/// where its order matters. One that moved and changed is a change of its behaviour, a statement that moved inside its
/// function too, whatever the index says, and so is one that moved with other code beside it in its blocks; a line
/// that only stays where it was with other whitespace is whitespace
#[test]
fn a_declaration_at_another_place_with_the_same_code_is_moved_where_the_index_says_its_order_is_free() {
  let function = |name: &str, body: &str| format!("fun {name}(x: Int): Int {{\n  val doubled = x * 2\n  return {body}\n}}\n");
  let file = |functions: &[String]| format!("package a\n\n{}", functions.join("\n"));
  let (one, two, three, four) = (
    function("one", "compute(doubled, 1)"),
    function("two", "compute(doubled, 2)"),
    function("three", "compute(doubled, 3)"),
    function("four", "compute(doubled, 4)"),
  );
  let before = file(&[one.clone(), two.clone(), three.clone(), four.clone()]);
  let moved = UnitKind::Mechanical {
    mechanical: MechanicalKind::Moved,
  };
  // the units of the file of `functions` after the four in their order, or after `first` (`Some`), the index saying
  // of `one` that its order is `order` (`None`: no index): the kind, the name and the blocks of each
  let units_in = |first: Option<&str>, functions: &[String], order: Option<Order>| -> Vec<(UnitKind, Option<String>, Vec<u32>)> {
    let repo = TestRepo::new();
    let before = first.map_or_else(|| before.clone(), str::to_string);
    let after = file(functions);
    let base = repo.create_commit("Add", "src/a/Store.kt", &before);
    let head = repo.create_commit("Move", "src/a/Store.kt", &after);
    let index = order.map(|order| {
      Counts::of(&[]).with_reordered(vec![reordered(
        "src/a/Store.kt",
        "one",
        function_lines(&before, "one"),
        function_lines(&after, "one"),
        order,
      )])
    });
    let plan = plan_of(&repo, index.as_ref().map(|index| index as &dyn SymbolIndex), &range(&repo, &base, &head)).unwrap();
    units(&plan)
      .into_iter()
      .map(|(kind, name, slices)| (kind, name.map(str::to_string), slices.into_iter().flat_map(|(_, blocks)| blocks).collect()))
      .collect()
  };
  let units_of = |functions: &[String], order: Option<Order>| units_in(None, functions, order);
  // to the end of the file, where other code stands around it; between two other functions; wrapped on the way
  let wrapped = function("one", "compute(\n    doubled,\n    1\n  )");
  let read = |_: &[String]| [(UnitKind::Behaviour, Some("one".to_string()), vec![0, 1])];
  for functions in [
    [two.clone(), three.clone(), four.clone(), one.clone()],
    [two.clone(), three.clone(), one.clone(), four.clone()],
    [two.clone(), three.clone(), wrapped, four.clone()],
  ] {
    assert_eq!(units_of(&functions, Some(Order::Free)), [(moved.clone(), None, vec![0, 1])], "{functions:?}");
    assert_eq!(units_of(&functions, Some(Order::Matters)), read(&functions), "{functions:?}");
    assert_eq!(units_of(&functions, None), read(&functions), "{functions:?}");
  }
  // moved and changed, and a statement at another place of its function: changes of its behaviour
  let changed = function("one", "compute(doubled, 100)");
  let statements = one.replace(
    "  val doubled = x * 2\n  return compute(doubled, 1)\n",
    "  return compute(doubled, 1)\n  val doubled = x * 2\n",
  );
  assert_eq!(
    units_of(&[two.clone(), three.clone(), changed, four.clone()], Some(Order::Free)),
    [(UnitKind::Behaviour, Some("one".to_string()), vec![0, 1])]
  );
  let behaviour: Vec<UnitKind> = units_of(&[statements, two.clone(), three.clone(), four.clone()], Some(Order::Free))
    .into_iter()
    .map(|(kind, _, _)| kind)
    .collect();
  assert_eq!(behaviour, [UnitKind::Behaviour]);
  // with a property beside it in its blocks, which the index did not say is free: other code moved with it
  let cached = "val cached: Int = compute(0, 0)\n".to_string();
  let with_cached = file(&[cached.clone(), one.clone(), two.clone(), three.clone(), four.clone()]);
  assert_eq!(
    units_in(Some(&with_cached), &[two.clone(), three.clone(), four.clone(), cached, one.clone()], Some(Order::Free)),
    [(UnitKind::Behaviour, Some("one".to_string()), vec![0, 1])]
  );
  // a blank line added where the function stays: the same tokens in a block of one side, and no line of it moved
  let spaced = one.replace("  return", "\n  return");
  assert_eq!(
    units_of(&[spaced, two, three, four], Some(Order::Free)),
    [(
      UnitKind::Mechanical {
        mechanical: MechanicalKind::Whitespace
      },
      None,
      vec![0]
    )]
  );
}

/// A declaration that moved is folded only where the accio of this machine says its place has no meaning: a function
/// or a type of Kotlin, Java, C# and Rust that crossed declarations alone, a hoisted function of TypeScript, a method
/// of it without a decorator. A value with its initializer, an enum entry, a function that crossed a macro, an
/// initializer block or the end of an `impl`, a function a `const` of TypeScript holds, a class of TypeScript, an
/// overload among its overloads, a method that moved with its decorator or with a decorator of a parameter, one named
/// by an expression, and a function of Starlark are changes of behaviour, each a unit that is read: where they stand
/// is what the program does. A machine without an accio that answers `moves` proves nothing here
#[test]
fn a_moved_declaration_is_folded_only_where_the_index_says_its_place_has_no_meaning() {
  // each file: its path, its head, the declaration that moves (its lines), the declarations that stay, whether the
  // moved one stands after them in the end (before them at first), and the unit it is, `None` for the fold
  type Case = (&'static str, &'static str, &'static str, &'static str, bool, Option<&'static str>);
  let cases: Vec<Case> = vec![
    // Kotlin: a property that another reads as it is made; a top-level one; an enum entry
    (
      "src/a/Store.kt",
      "package a\n\nclass Store(private val source: Source) {\n",
      "  val items: List<Item> =\n    source.loadItems()\n",
      "  val total: Int = items.sumOf { it.price }\n\n  fun describe(): String {\n    return \"store of $total\"\n  }\n",
      true,
      Some("items"),
    ),
    (
      "src/a/Values.kt",
      "package a\n\n",
      "val first: Int =\n  computeFirst()\n",
      "val second: Int = first + computeSecond()\n\nval third: Int = second + computeThird()\n",
      true,
      Some("first"),
    ),
    (
      "src/a/Level.kt",
      "package a\n\nenum class Level(val weight: Int) {\n",
      "  LOW(computeLow()),\n",
      "  HIGH(computeHigh()),\n  HIGHER(computeHigher()),\n",
      true,
      Some("LOW"),
    ),
    // a function across a property and a function: no order of them means anything
    (
      "src/a/Service.kt",
      "package a\n\nclass Service(private val source: Source) {\n",
      "  fun first(): Int {\n    return source.read(1)\n  }\n",
      "  val cached: Int = source.read(0)\n\n  fun second(): Int {\n    return source.read(2)\n  }\n",
      true,
      None,
    ),
    // a class, whole: the unit of the member that holds most of its lines is folded as the class is
    (
      "src/a/Types.kt",
      "package a\n\n",
      "class Base {\n  fun read(): Int {\n    return 1\n  }\n}\n",
      "class Derived : Base() {\n  fun write(): Int {\n    return 2\n  }\n}\n\nclass Last : Base() {\n  fun both(): Int {\n    return 3\n  }\n}\n",
      true,
      None,
    ),
    // a function with the property beside it, in one block: the property moved too
    (
      "src/a/Both.kt",
      "package a\n\nclass Both(private val source: Source) {\n",
      "  fun first(): Int {\n    return source.read(1)\n  }\n\n  val cached: Int = source.read(0)\n",
      "  fun second(): Int {\n    return source.read(2)\n  }\n\n  fun third(): Int {\n    return source.read(3)\n  }\n",
      true,
      Some("first"),
    ),
    // across an initializer block: code of no declaration
    (
      "src/a/Session.kt",
      "package a\n\nclass Session(private val source: Source) {\n",
      "  fun open(): Int {\n    return source.read(1)\n  }\n",
      "  init {\n    source.prepare()\n    source.verify()\n    source.warm()\n  }\n",
      true,
      Some("open"),
    ),
    // Java: a constant made of another; a method across a field
    (
      "src/a/Order.java",
      "package a;\n\nclass Order {\n",
      "  static final int DOUBLED =\n      BASE * 2 + offset(1);\n",
      "  static final int BASE = offset(21);\n\n  static final int LIMIT = offset(34);\n\n  static final int LAST = offset(55);\n",
      false,
      Some("DOUBLED"),
    ),
    (
      "src/a/Worker.java",
      "package a;\n\nclass Worker {\n",
      "  int first() {\n    return read(1);\n  }\n",
      "  private final int cached = read(0);\n\n  int second() {\n    return read(2);\n  }\n",
      true,
      None,
    ),
    // C#: a field made of another; a method across a field
    (
      "src/a/Order.cs",
      "namespace A;\n\nclass Order {\n",
      "  static readonly int Doubled =\n      Base * 2 + Offset(1);\n",
      "  static readonly int Base = Offset(21);\n\n  static readonly int Limit = Offset(34);\n\n  static readonly int Last = Offset(55);\n",
      false,
      Some("Doubled"),
    ),
    (
      "src/a/Worker.cs",
      "namespace A;\n\nclass Worker {\n",
      "  int First() {\n    return Read(1);\n  }\n",
      "  private readonly int cached = Read(0);\n\n  int Second() {\n    return Read(2);\n  }\n",
      true,
      None,
    ),
    // Rust: a function above the macros it calls; a variant of an ordered enum; a function across a type, a
    // constant and a function; a method into another `impl` of its type
    (
      "src/a/answer.rs",
      "",
      "pub fn answer() -> u32 {\n  double!(7) + triple!(9)\n}\n",
      "macro_rules! double {\n  ($value:expr) => {\n    $value * 2\n  };\n}\n\nmacro_rules! triple {\n  ($value:expr) => {\n    $value * 3\n  };\n}\n",
      false,
      Some("answer"),
    ),
    (
      "src/a/rank.rs",
      "#[derive(PartialEq, Eq, PartialOrd, Ord)]\npub enum Rank {\n",
      "  Low(u32),\n",
      "  High(u32),\n  Higher(u32),\n",
      true,
      Some("Low"),
    ),
    (
      "src/a/shapes.rs",
      "",
      "pub fn area(shape: &Shape) -> u32 {\n  shape.width * shape.height\n}\n",
      "pub struct Shape {\n  pub width: u32,\n  pub height: u32,\n}\n\npub const LIMIT: u32 = 64;\n\npub fn within(shape: &Shape) -> bool {\n  area(shape) < LIMIT\n}\n",
      true,
      None,
    ),
    (
      "src/a/blocks.rs",
      "pub struct Shape;\n\nimpl Shape {\n",
      "  pub fn area(&self) -> u32 {\n    self.width() * self.height()\n  }\n",
      "  pub fn width(&self) -> u32 {\n    4\n  }\n}\n\nimpl Shape {\n  pub fn height(&self) -> u32 {\n    2\n  }\n",
      true,
      Some("area"),
    ),
    // TypeScript: a constant another reads as the module loads; a function a constant holds; a hoisted function; a
    // class; an overload among its overloads
    (
      "src/setup.ts",
      "",
      "export const defaultTimeout =\n  readTimeoutFromEnvironment()\n",
      "export const retryTimeout = defaultTimeout * retryFactor()\n\nexport const longTimeout = retryTimeout * retryFactor()\n",
      true,
      Some("defaultTimeout"),
    ),
    (
      "src/handlers.ts",
      "",
      "export const first = (value: number): number => {\n  return read(value, 1)\n}\n",
      "export const second = first(2)\n\nexport const third = second + read(3, 3)\n\nexport const fourth = third + read(4, 4)\n",
      true,
      Some("first"),
    ),
    (
      "src/functions.ts",
      "",
      "export function first(value: number): number {\n  return read(value, 1)\n}\n",
      "export const cached = read(0, 0)\n\nexport function second(value: number): number {\n  return read(value, 2)\n}\n",
      true,
      None,
    ),
    (
      "src/classes.ts",
      "",
      "export class Base {\n  read(): number {\n    return 1\n  }\n}\n",
      "export class Derived extends Base {\n  write(): number {\n    return 2\n  }\n}\n\nexport class Last extends Base {\n  both(): number {\n    return 3\n  }\n}\n",
      true,
      // the unit of the member that holds most of the lines of the class
      Some("read"),
    ),
    (
      "src/overloads.ts",
      "",
      "export function parse(value: string, radix: number): string\n",
      "export function parse(value: number): number\nexport function parse(value: boolean): boolean\n",
      true,
      Some("parse"),
    ),
    // a method with a decorator, on a line of its own and on the line of the method, and with a decorator of a
    // parameter: the decorators of a class run in the order of its members, and the routes of a controller are
    // matched in the order of its methods
    (
      "src/cats.controller.ts",
      "@Controller(\"cats\")\nexport class CatsController {\n",
      "  @Get(\":id\")\n  findOne(id: string): string {\n    return this.cats.one(id)\n  }\n",
      "  @Get(\"search\")\n  search(): string {\n    return this.cats.search()\n  }\n\n  @Get(\"stats\")\n  stats(): string {\n    return this.cats.stats()\n  }\n",
      false,
      Some("findOne"),
    ),
    (
      "src/dogs.controller.ts",
      "@Controller(\"dogs\")\nexport class DogsController {\n",
      "  @Get(\":id\") findOne(id: string): string {\n    return this.dogs.one(id)\n  }\n",
      "  @Get(\"search\") search(): string {\n    return this.dogs.search()\n  }\n\n  @Get(\"stats\") stats(): string {\n    return this.dogs.stats()\n  }\n",
      true,
      Some("findOne"),
    ),
    (
      "src/birds.controller.js",
      "export class BirdsController {\n",
      "  @Get(\":id\")\n  findOne(id) {\n    return this.birds.one(id)\n  }\n",
      "  @Get(\"search\")\n  search() {\n    return this.birds.search()\n  }\n\n  @Get(\"stats\")\n  stats() {\n    return this.birds.stats()\n  }\n",
      false,
      Some("findOne"),
    ),
    (
      "src/fish.service.ts",
      "export class FishService {\n",
      "  findOne(@Inject(Token) source: Source, id: string): string {\n    return source.one(id)\n  }\n",
      "  search(source: Source): string {\n    return source.search()\n  }\n\n  stats(source: Source): string {\n    return source.stats()\n  }\n",
      true,
      Some("findOne"),
    ),
    // a method without a decorator across decorated ones: their order among themselves stays
    (
      "src/owls.controller.ts",
      "@Controller(\"owls\")\nexport class OwlsController {\n",
      "  private describe(id: string): string {\n    return this.owls.one(id)\n  }\n",
      "  @Get(\"search\")\n  search(): string {\n    return this.owls.search()\n  }\n\n  @Get(\"stats\")\n  stats(): string {\n    return this.owls.stats()\n  }\n",
      true,
      None,
    ),
    // a method named by an expression, which runs in its turn as the class is made: no declaration of the file
    (
      "src/registry.ts",
      "export class Registry {\n",
      "  [nextKeyOfTheRegistry()](id: string): string {\n    return this.entries.one(id)\n  }\n",
      "  search(): string {\n    return this.entries.search()\n  }\n\n  stats(): string {\n    return this.entries.stats()\n  }\n",
      true,
      Some("Registry"),
    ),
    // Starlark: a file runs from its first line to its last
    (
      "src/defs.bzl",
      "",
      "def first(name):\n    native.genrule(\n        name = name,\n    )\n",
      "def second(name):\n    first(name + \"_a\")\n\ndef third(name):\n    second(name + \"_b\")\n\ndef fourth(name):\n    third(name + \"_c\")\n",
      true,
      Some("first"),
    ),
  ];
  // a file: its head, the moved declaration before or after the ones that stay, a blank line between the two unless
  // they are lines of one list (enum entries, overloads), and the brace that closes a head that opened one
  let text = |(_, head, moved, stay, _, _): &Case, moved_last: bool| {
    let end = if head.trim_end().ends_with('{') { "}\n" } else { "" };
    let overloads = moved.starts_with("export function parse");
    let between = if overloads || head.contains("enum ") { "" } else { "\n" };
    let tail = if overloads {
      "export function parse(value: unknown): unknown {\n  return value\n}\n"
    } else {
      ""
    };
    if moved_last {
      format!("{head}{stay}{between}{moved}{tail}{end}")
    } else {
      format!("{head}{moved}{between}{stay}{tail}{end}")
    }
  };
  let files = |after: bool| -> Vec<(&str, String)> { cases.iter().map(|case| (case.0, text(case, case.4 == after))).collect() };
  let (before, after) = (files(false), files(true));
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add", &before.iter().map(|(path, text)| (*path, text.as_str())).collect::<Vec<_>>());
  let head = repo.create_commit_with_files("Move", &after.iter().map(|(path, text)| (*path, text.as_str())).collect::<Vec<_>>());
  let Some(plan) = plan_by_accio(&repo, &base, &head) else {
    eprintln!("no accio that answers moves on this machine: nothing asked");
    return;
  };

  let (mut folded, mut read): (Vec<&str>, Vec<(&str, &str)>) = (Vec::new(), Vec::new());
  for (kind, name, slices) in units(&plan) {
    let paths = slices.iter().map(|(path, _)| *path);
    match kind {
      UnitKind::Mechanical {
        mechanical: MechanicalKind::Moved,
      } => folded.extend(paths),
      UnitKind::Behaviour => read.extend(paths.map(|path| (path, name.unwrap_or_default()))),
      other => panic!("{other:?} of {name:?} in {slices:?}"),
    }
  }
  folded.sort_unstable();
  read.sort_unstable();
  let mut expected_folded: Vec<&str> = cases.iter().filter(|case| case.5.is_none()).map(|case| case.0).collect();
  let mut expected_read: Vec<(&str, &str)> = cases.iter().filter_map(|case| Some((case.0, case.5?))).collect();
  expected_folded.sort_unstable();
  expected_read.sort_unstable();
  assert_eq!((folded, read), (expected_folded, expected_read));
}

/// The blocks name the lines of the diffs, where a lone `\r` ends a line, and the declarations of a summary those of
/// git: a block below a lone `\r` goes to the declaration of its lines, a change of its code is a change of its
/// behaviour and one of its whitespace whitespace, not what the line after it is
#[test]
fn a_block_below_a_lone_carriage_return_goes_to_the_declaration_of_its_lines() {
  let source = |one: &str, three: &str| format!("/* a\rb */\npackage a\n\nfun one() = {one}\n\nfun two() = 2\n\nfun three() = {three}\n\nfun four() = 4\n");
  let repo = TestRepo::new();
  let base = repo.create_commit("Add", "src/a/Cr.kt", &source("1", "compute(3, 4)"));
  let head = repo.create_commit("Change", "src/a/Cr.kt", &source("100", "compute(\n  3,\n  4\n)"));
  let plan = plan(&repo, &base, &head);
  assert_eq!(
    units(&plan),
    [
      (UnitKind::Behaviour, Some("one"), vec![("src/a/Cr.kt", vec![0])]),
      (
        UnitKind::Mechanical {
          mechanical: MechanicalKind::Whitespace
        },
        None,
        vec![("src/a/Cr.kt", vec![1])]
      ),
    ]
  );
  let block = &plan.units[0].slices[0].blocks[0];
  assert_eq!((block.old_start, block.old_end, block.new_start, block.new_end), (4, 5, 4, 5));
}

/// The fingerprint of a change reads the tokens of a file of a grammar and the tree between them: the same change
/// written with other whitespace, or in a function that is the last of its file, has the same fingerprint; one that
/// differs by a space inside a string, or by a line break that ends a statement, another
#[test]
fn the_fingerprint_is_the_same_after_a_change_of_whitespace_alone() {
  let source = |body: &str, last: &str| format!("package a\n\nfun open(x: Int): Any {{\n{body}\n}}\n{last}");
  let repo = TestRepo::new();
  // each change is a pair of commits of the one file: the plan of the pair
  let fingerprint_in = |before: &str, after: &str, last: &str| -> String {
    let base = repo.create_commit("Add open", "src/a/Open.kt", &source(before, last));
    let head = repo.create_commit("Change open", "src/a/Open.kt", &source(after, last));
    let plan = plan(&repo, &base, &head);
    plan.units.into_iter().find(|unit| unit.kind == UnitKind::Behaviour).unwrap().fingerprint
  };
  let fingerprint = |before: &str, after: &str| fingerprint_in(before, after, "\nfun last() = 1\n");
  let change = fingerprint("  return x", "  return compute(x, \"a b\")");
  assert_eq!(change, fingerprint("  return x", "  return compute(\n    x,\n    \"a b\"\n  )"));
  assert_eq!(change, fingerprint("      return  x", "  return compute( x,\"a b\" )"));
  // the function is the last of its file: the file closes after its last token, the change is the same
  assert_eq!(change, fingerprint_in("  return x", "  return compute(x, \"a b\")", ""));
  assert_ne!(change, fingerprint("  return x", "  return compute(x, \"a  b\")"));
  assert_ne!(change, fingerprint("  return x", "  return compute(x, \"ab\")"));
  // the same words, another program: the `return` ends at its line break
  assert_ne!(change, fingerprint("  return x", "  return\n    compute(x, \"a b\")"));
}

/// A file that two commits change is cut by the blocks of its first blob against its last, and the files come in the
/// order of the plan: the one that declares before the one that uses
#[test]
fn the_units_follow_the_order_of_the_files_and_the_pair_of_a_file_changed_twice() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add",
    &[
      ("src/a/Use.kt", "package a\n\nfun use(): Int {\n  return open(1)\n}\n"),
      ("src/a/Api.kt", "package a\n\nfun open(x: Int): Int {\n  return x\n}\n"),
    ],
  );
  repo.create_commit_with_files(
    "Change",
    &[
      ("src/a/Use.kt", "package a\n\nfun use(): Int {\n  return open(1, 2)\n}\n"),
      ("src/a/Api.kt", "package a\n\nfun open(x: Int, y: Int): Int {\n  return x\n}\n"),
    ],
  );
  let head = repo.create_commit("Change again", "src/a/Use.kt", "package a\n\nfun use(): Int {\n  return open(2, 2)\n}\n");
  let plan = plan(&repo, &base, &head);
  let found: Vec<(&str, &UnitKind)> = plan.units.iter().map(|unit| (unit.slices[0].path.as_str(), &unit.kind)).collect();
  assert_eq!(
    found,
    [
      (
        "src/a/Api.kt",
        &UnitKind::Shape {
          changes: vec![FormChange::Parameters]
        }
      ),
      ("src/a/Use.kt", &UnitKind::Behaviour),
    ]
  );
}

/// A member of a new type is part of the unit of the type, not a unit of its own
#[test]
fn a_member_of_a_new_type_is_part_of_the_type() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add open", "src/a/Open.kt", "package a\n\nfun open(x: Int): Int {\n  return x\n}\n");
  let head = repo.create_commit(
    "Add span",
    "src/a/Open.kt",
    "package a\n\nfun open(x: Int): Int {\n  return x\n}\n\nclass Span(val start: Int, val end: Int) {\n  fun length(): Int = end - start\n}\n",
  );
  let plan = plan(&repo, &base, &head);
  assert_eq!(units(&plan), [(UnitKind::Added, Some("Span"), vec![("src/a/Open.kt", vec![0])])]);
}

/// A commit is read against its first parent, as the diffs of a commit show it: its plan is that of `<id>^..<id>`, named
/// by its own short id; a root commit has no parent to read it against
#[test]
fn a_commit_is_read_against_its_first_parent() {
  let repo = TestRepo::new();
  let root = repo.create_commit("Add open", "src/a/Open.kt", "package a\n\nfun open(x: Int): Int {\n  return x\n}\n");
  let head = repo.create_commit("Change open", "src/a/Open.kt", "package a\n\nfun open(x: Int): Int {\n  return x + 1\n}\n");
  let commit = |id: &str| crate::ReviewPlanParams {
    subject: crate::ReviewSubject::Commit {
      id: id.to_string(),
      reviewed: None,
    },
    ..range(&repo, &root, &head)
  };
  let of_commit = plan_of(&repo, None, &commit(&head)).unwrap();
  assert_eq!(of_commit.title, head[..8]);
  assert_eq!(
    of_commit,
    ReviewPlan {
      title: of_commit.title.clone(),
      ..plan(&repo, &root, &head)
    }
  );
  let error = plan_of(&repo, None, &commit(&root)).unwrap_err();
  assert!(error.downcast_ref::<crate::UnknownRevision>().is_some(), "{error:#}");
}

const SURFACE_BEFORE: &str = "package a

fun open(x: Int): Int {
  return x
}

private fun hidden(x: Int): Int {
  return x
}

@ApiStatus.Internal
fun promoted(x: Int): Int {
  return x
}

@ApiStatus.Internal
fun inner(x: Int): Int {
  return x
}
";

const SURFACE_AFTER: &str = "package a

fun open(x: Int): Int {
  return x + 1
}

private fun hidden(x: Int): Int {
  return x + 1
}

fun promoted(x: Int): Int {
  return x
}

@ApiStatus.Internal
fun inner(x: Int, y: Int): Int {
  return x
}

fun added(x: Int): Int {
  return x
}
";

const SURFACE_TEST: &str = "package a

class StoreTest {
  @BeforeEach
  fun setUp() {
    println(1)
  }

  @Test
  fun `opens a store`() {
    println(1)
  }
}
";

/// A unit of a declaration is `api` by the rule of the API of the plan: an addition when it is API after the commits,
/// any other change when it is API or unstable API on a side, so a change of a body of API and a promotion are too;
/// a test, a fixture of its class, an addition of unstable API and a private function are not, and a test and its
/// fixture are `test`. Every unit of an addition, a removal or a change of a form is `api` exactly when the plan lists
/// its key
#[test]
fn a_unit_is_api_by_the_rule_of_the_api_of_the_plan() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add store",
    &[
      ("src/a/Store.kt", SURFACE_BEFORE),
      ("src/a/Extra.kt", "package a\n\nfun extra(): Int = 1\n"),
      ("src/test/a/StoreTest.kt", SURFACE_TEST),
    ],
  );
  let head = repo.create_commit_with_files(
    "Change store",
    &[
      ("src/a/Store.kt", SURFACE_AFTER),
      (
        "src/a/Extra.kt",
        "package a\n\nfun extra(): Int = 1\n\n@ApiStatus.Internal\nfun addedInternal(x: Int): Int = x\n",
      ),
      ("src/test/a/StoreTest.kt", &SURFACE_TEST.replace("println(1)", "println(2)")),
    ],
  );
  let plan = plan(&repo, &base, &head);
  let flags: BTreeMap<&str, (bool, bool)> = plan
    .units
    .iter()
    .filter_map(|unit| Some((unit.name.as_deref()?, (unit.api, unit.test))))
    .filter(|(name, _)| !name.contains('.'))
    .collect();
  assert_eq!(
    flags,
    BTreeMap::from([
      ("`opens a store`", (false, true)),
      ("added", (true, false)),
      ("addedInternal", (false, false)),
      ("hidden", (false, false)),
      ("inner", (true, false)),
      ("open", (true, false)),
      ("promoted", (true, false)),
      ("setUp", (false, true)),
    ])
  );
  let listed: BTreeMap<&str, ApiChangeKind> = plan.api.iter().map(|change| (change.name.as_str(), change.kind)).collect();
  assert_eq!(
    listed,
    BTreeMap::from([("added", ApiChangeKind::Added), ("inner", ApiChangeKind::Changed), ("promoted", ApiChangeKind::Added)])
  );
  for unit in plan
    .units
    .iter()
    .filter(|unit| matches!(unit.kind, UnitKind::Added | UnitKind::Removed | UnitKind::Shape { .. }))
  {
    assert_eq!(unit.api, plan.api.iter().any(|change| Some(&change.key) == unit.key.as_ref()), "{unit:#?}");
  }
}

/// Each block of a unit counts its lines as the engine does (`UnitBlock::additions`), what the header of a part of a
/// file says: a block that reaches the end of the file and holds the empty line after its final line end, which the
/// commit adds, counts without it, so the blocks of a file sum to the engine's count of the file
#[test]
fn each_block_counts_its_lines_as_the_engine_does() {
  let repo = TestRepo::new();
  let old = "class A {\n  fun a() {\n    a()\n  }\n\n}";
  let new = "class A {\n  fun a() {\n    a()\n  }\n\n  fun b() {\n    b()\n  }\n}\n\nprivate fun c() {\n  c()\n}\n\nprivate fun d() = 1\n";
  let base = repo.create_commit_with_files("Add A", &[("src/A.kt", old)]);
  let head = repo.create_commit_with_files("Add b, c and d", &[("src/A.kt", new)]);
  let plan = plan(&repo, &base, &head);
  let mut blocks: Vec<(u32, u32, u32)> = plan
    .units
    .iter()
    .flat_map(|unit| &unit.slices)
    .flat_map(|slice| &slice.blocks)
    .map(|block| (block.index, block.additions, block.deletions))
    .collect();
  blocks.sort_unstable();
  blocks.dedup();
  // the engine's +9 −0 in two blocks: `b` before the closing brace, then the blank line, `c` and `d` with the empty
  // last line
  assert_eq!(blocks, [(0, 3, 0), (1, 6, 0)]);
}
