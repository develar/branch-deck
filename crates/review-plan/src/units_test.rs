use crate::ReviewPlan;
use crate::api::FormChange;
use crate::files::MechanicalKind;
use crate::test_support::{plan_of, range};
use crate::units::{OtherReason, ReviewUnit, UnitKind};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The plan of `base..head` of `repo`, without an index
fn plan(repo: &TestRepo, base: &str, head: &str) -> ReviewPlan {
  plan_of(repo, None, &range(repo, base, head)).unwrap()
}

/// A unit as a test reads it: its kind, its name, and per slice its path with the places of its blocks
type Unit<'a> = (UnitKind, Option<&'a str>, Vec<(&'a str, Vec<u32>)>);

/// Each unit of `plan` (`Unit`)
fn units(plan: &ReviewPlan) -> Vec<Unit<'_>> {
  plan
    .units
    .iter()
    .map(|unit| {
      let slices = unit.slices.iter().map(|slice| (slice.path.as_str(), slice.blocks.iter().map(|block| block.index).collect())).collect();
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
  return x
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
    &[("src/a/Store.kt", STORE_AFTER), ("README.md", "readme 2\n"), ("src/a/Fresh.kt", "package a\n\nclass Fresh\n")],
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
  let write = |body: &dyn Fn(&str) -> &'static str| -> Vec<(String, String)> {
    names.iter().map(|name| (format!("src/a/{name}.kt"), source(name, body(name)))).collect()
  };
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
  let other = repo.create_commit("Change open", "src/a/Open.kt", &after.replace("x + 1", "x + 2").replace("package a\n", "package a\n\n// moved\n"));
  let third = behaviour(&repo, &again, &other);
  assert_eq!(first.fingerprint, second.fingerprint);
  assert_ne!(first.fingerprint, third.fingerprint);
  assert_eq!(first.id, second.id);
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

/// A commit is read against its first parent, as the diffs of a commit show it: its plan is that of `<id>^..<id>`; a
/// root commit has no parent to read it against
#[test]
fn a_commit_is_read_against_its_first_parent() {
  let repo = TestRepo::new();
  let root = repo.create_commit("Add open", "src/a/Open.kt", "package a\n\nfun open(x: Int): Int {\n  return x\n}\n");
  let head = repo.create_commit("Change open", "src/a/Open.kt", "package a\n\nfun open(x: Int): Int {\n  return x + 1\n}\n");
  let commit = |id: &str| crate::ReviewPlanParams {
    subject: crate::ReviewSubject::Commit { id: id.to_string() },
    ..range(&repo, &root, &head)
  };
  let of_commit = plan_of(&repo, None, &commit(&head)).unwrap();
  assert_eq!(of_commit, plan(&repo, &root, &head));
  let error = plan_of(&repo, None, &commit(&root)).unwrap_err();
  assert!(error.downcast_ref::<crate::UnknownRevision>().is_some(), "{error:#}");
}
