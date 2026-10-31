use crate::files::changed_files;
use crate::interdiff::commits_of;
use crate::order::{ReviewOrder, order_files, review_order};
use crate::test_support::{plan_of, range};
use file_diffs::service::{FileDiffs, compare_tree_paths};
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use trial_check::diff_scan::read_commit_diffs_cached;
use trial_check::symbol_scan::read_symbol_input;

/// The order of the plan of one commit that adds `files` on top of a first commit
fn order_of_added(files: &[(&str, &str)]) -> Vec<String> {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add readme", "README.md", "readme\n");
  let head = repo.create_commit_with_files("Add files", files);
  plan_of(&repo, None, &range(&repo, &base, &head)).unwrap().order
}

/// A chain of imports across packages: each declaration before the file that imports it, against the tree order; a
/// file without an edge keeps its place in the tree
#[test]
fn a_declaration_comes_before_the_file_that_uses_it() {
  let order = order_of_added(&[
    ("src/a/A.kt", "package a\n\nimport b.B\n\nclass A(val b: B)\n"),
    ("src/b/B.kt", "package b\n\nimport c.C\n\nclass B(val c: C)\n"),
    ("src/c/C.kt", "package c\n\nclass C\n"),
    ("notes.txt", "notes\n"),
  ]);
  assert_eq!(order, ["src/c/C.kt", "src/b/B.kt", "src/a/A.kt", "notes.txt"]);

  // Rust, by the `use` of a module of its crate
  let order = order_of_added(&[
    ("crates/x/Cargo.toml", "[package]\nname = \"x\"\n"),
    ("crates/x/src/lib.rs", "pub mod user;\npub mod z_store;\n"),
    ("crates/x/src/user.rs", "use crate::z_store::Store;\n\npub struct User(pub Store);\n"),
    ("crates/x/src/z_store.rs", "pub struct Store;\n"),
  ]);
  assert_eq!(order, ["crates/x/src/lib.rs", "crates/x/src/z_store.rs", "crates/x/src/user.rs", "crates/x/Cargo.toml"]);
}

/// Two packages declare `Store`; the file that imports one of them comes after that one only, so the other keeps its
/// place in the tree
#[test]
fn a_name_of_two_modules_links_only_the_one_imported() {
  let order = order_of_added(&[
    ("src/a/User.kt", "package a\n\nimport m.Store\n\nclass User(val store: Store)\n"),
    ("src/m/Store.kt", "package m\n\nclass Store\n"),
    ("src/z/Store.kt", "package z\n\nclass Store\n"),
  ]);
  assert_eq!(order, ["src/m/Store.kt", "src/a/User.kt", "src/z/Store.kt"]);
}

/// Two files that use each other keep their tree order, after what they use and before what uses them; the same in
/// TypeScript, by its imports
#[test]
fn a_cycle_keeps_its_tree_order() {
  let order = order_of_added(&[
    ("src/a/A.kt", "package a\n\nimport b.B\nimport c.C\n\nclass A(val b: B, val c: C)\n"),
    ("src/app/Main.kt", "package app\n\nimport a.A\n\nclass Main(val a: A)\n"),
    ("src/b/B.kt", "package b\n\nimport a.A\n\nclass B(val a: A?)\n"),
    ("src/c/C.kt", "package c\n\nclass C\n"),
  ]);
  assert_eq!(order, ["src/c/C.kt", "src/a/A.kt", "src/b/B.kt", "src/app/Main.kt"]);

  let order = order_of_added(&[
    ("web/a.ts", "import { b } from \"./b\"\n\nexport function a(): number {\n  return b() + 1\n}\n"),
    ("web/b.ts", "import { c } from \"./c\"\n\nexport function b(): number {\n  return c()\n}\n"),
    ("web/c.ts", "export function c(): number {\n  return 1\n}\n"),
  ]);
  assert_eq!(order, ["web/c.ts", "web/b.ts", "web/a.ts"]);
}

/// A test comes right after its subject, also inside a cycle; a test whose subject the range doesn't change is a file
/// like any other
#[test]
fn a_test_comes_right_after_its_subject() {
  let order = order_of_added(&[
    ("src/a/Session.kt", "package a\n\nimport b.Login\n\nclass Session(val login: Login?)\n"),
    ("src/b/Login.kt", "package b\n\nimport a.Session\n\nclass Login(val session: Session?)\n"),
    ("src/test/a/GoneTest.kt", "package a\n\nclass GoneTest\n"),
    ("src/test/a/SessionTest.kt", "package a\n\nclass SessionTest(val session: Session)\n"),
    ("src/test/b/LoginTest.kt", "package b\n\nimport a.Session\n\nclass LoginTest(val session: Session)\n"),
  ]);
  assert_eq!(
    order,
    [
      "src/a/Session.kt",
      "src/test/a/SessionTest.kt",
      "src/b/Login.kt",
      "src/test/b/LoginTest.kt",
      "src/test/a/GoneTest.kt"
    ]
  );
}

/// Files of no language the summaries read are in tree order: directories first, names without case and numbers by
/// value; a renamed file at its new path, a deleted one at its path, one added and deleted again not at all
#[test]
fn files_of_no_known_language_keep_the_tree_order() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add files", &[("notes/old.txt", "old notes\n"), ("gone.txt", "gone\n")]);
  repo.create_commit_with_files(
    "Add docs",
    &[("docs/a10.txt", "ten\n"), ("docs/a2.txt", "two\n"), ("README.md", "readme\n"), ("tmp.txt", "tmp\n")],
  );
  repo.git(&["mv", "notes/old.txt", "notes/new.txt"]);
  repo.git(&["rm", "--quiet", "gone.txt", "tmp.txt"]);
  let head = repo.commit_with(&["-m", "Move notes"], &[]);
  let order = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap().order;
  assert_eq!(order, ["docs/a2.txt", "docs/a10.txt", "notes/new.txt", "gone.txt", "README.md"]);
}

/// The order returns the graph it read: a link per name and pair of files, from the file of the use to the file of the
/// declaration, with the line of the first use
#[test]
fn the_order_returns_its_graph_with_the_names_and_lines() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add readme", "README.md", "readme\n");
  let head = repo.create_commit_with_files(
    "Add files",
    &[
      (
        "src/a/User.kt",
        "package a\n\nimport b.Store\nimport b.Cache\n\nclass User(val store: Store, val cache: Cache)\n",
      ),
      ("src/b/Store.kt", "package b\n\nclass Store\n\nclass Cache\n"),
    ],
  );
  let (git, path) = (repo.git_executor(), repo.path_str());
  let (cache, file_diffs) = (ObjectCache::disabled(), FileDiffs::for_scans());
  let ids: Vec<String> = commits_of(git, path, &base, &head).unwrap().into_iter().map(|(commit, _)| commit.id).collect();
  let diffs = read_commit_diffs_cached(git, path, &cache, &file_diffs, &ids).unwrap();
  let input = read_symbol_input(git, path, &cache, &diffs).unwrap();
  let ReviewOrder { order, links } = review_order(&input, &changed_files(&diffs));
  assert_eq!(order, ["src/b/Store.kt", "src/a/User.kt"]);
  let links: Vec<(&str, &str, &str, u32)> = links
    .iter()
    .map(|link| (link.used_in.as_str(), link.declared_in.as_str(), link.name.as_str(), link.line))
    .collect();
  assert_eq!(links, [("src/a/User.kt", "src/b/Store.kt", "Cache", 4), ("src/a/User.kt", "src/b/Store.kt", "Store", 3)]);
}

/// The graph alone: a cycle in tree order before what uses it; a test after the subject of its stem it uses rather
/// than the one nearer in the tree, else after the nearest
#[test]
fn the_order_of_a_graph() {
  let paths = [
    "a/Main.kt",
    "b/Store.kt",
    "c/Store.kt",
    "c/StoreTest.kt",
    "d/Cache.kt",
    "d/CacheTest.kt",
    "e/Cache.kt",
    "f/Loop.kt",
    "g/Loop.kt",
  ];
  let uses: BTreeSet<(usize, usize)> = [
    // Main uses both Loops, which use each other
    (7, 0),
    (8, 0),
    (7, 8),
    (8, 7),
    // c/StoreTest uses b/Store
    (1, 3),
  ]
  .into_iter()
  .collect();
  assert_eq!(
    order_files(&paths, &uses),
    [
      "b/Store.kt",
      "c/StoreTest.kt",
      "c/Store.kt",
      "d/Cache.kt",
      "d/CacheTest.kt",
      "e/Cache.kt",
      "f/Loop.kt",
      "g/Loop.kt",
      "a/Main.kt"
    ]
  );
}

/// The order breaks its ties as the tree sorts: directories before files at each level, then the names without case,
/// numbers by value
#[test]
fn the_tree_order_of_paths() {
  let mut paths = vec!["b.txt", "a/z.txt", "A.txt", "file10.txt", "file2.txt", "a/b/c.txt", "_x.txt"];
  paths.sort_by(|a, b| compare_tree_paths(a, b));
  assert_eq!(paths, ["a/b/c.txt", "a/z.txt", "_x.txt", "A.txt", "b.txt", "file2.txt", "file10.txt"]);
}
