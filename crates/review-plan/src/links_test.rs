use crate::ReviewPlan;
use crate::links::{FileLink, LinkState, LinkedSymbol};
use crate::test_support::{plan_of, range};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The plan of one commit that writes `files` on top of a commit of `base`
fn plan_of_change(base: &[(&str, &str)], files: &[(&str, &str)]) -> ReviewPlan {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files("Add files", base);
  let head = repo.create_commit_with_files("Change files", files);
  plan_of(&repo, None, &range(&repo, &base, &head)).unwrap()
}

/// The link counts of each file: (path, uses, used by, new links)
fn counts(plan: &ReviewPlan) -> Vec<(&str, u32, u32, u32)> {
  let mut counts: Vec<(&str, u32, u32, u32)> = plan.files.iter().map(|facts| (facts.path.as_str(), facts.uses, facts.used_by, facts.new_links)).collect();
  counts.sort_unstable();
  counts
}

fn link(from: &str, to: &str, symbols: &[(&str, u32)], state: LinkState, cross_module: bool) -> FileLink {
  FileLink {
    from: from.to_string(),
    to: to.to_string(),
    symbols: symbols.iter().map(|&(name, line)| LinkedSymbol { name: name.to_string(), line }).collect(),
    state,
    cross_module,
  }
}

/// A new `Store` that `User` now imports is an added link, with the name and the line of its first use; `User` uses one
/// file, newly, and `Store` is used by one
#[test]
fn a_new_file_that_another_imports_is_an_added_link() {
  let plan = plan_of_change(
    &[("src/a/User.kt", "package a\n\nclass User\n")],
    &[
      ("src/a/User.kt", "package a\n\nimport b.Store\n\nclass User(val store: Store)\n"),
      ("src/b/Store.kt", "package b\n\nclass Store\n"),
    ],
  );
  assert_eq!(plan.links, [link("src/a/User.kt", "src/b/Store.kt", &[("Store", 3)], LinkState::Added, false)]);
  assert_eq!(counts(&plan), [("src/a/User.kt", 1, 0, 1), ("src/b/Store.kt", 0, 1, 0)]);
}

/// A use before and after the commits is kept, with its symbols after them; a use the commits remove is removed, with
/// its symbols before them, and counts in neither file
#[test]
fn a_link_before_and_after_is_kept_and_one_only_before_is_removed() {
  let plan = plan_of_change(
    &[
      ("src/a/Reader.kt", "package a\n\nimport b.Store\n\nclass Reader(val store: Store)\n"),
      ("src/a/Writer.kt", "package a\n\nimport b.Store\n\nclass Writer(val store: Store)\n"),
      ("src/b/Store.kt", "package b\n\nclass Store\n"),
    ],
    &[
      ("src/a/Reader.kt", "package a\n\n// reads the store\nimport b.Store\n\nclass Reader(val store: Store?)\n"),
      ("src/a/Writer.kt", "package a\n\nclass Writer\n"),
      ("src/b/Store.kt", "package b\n\nclass Store(val size: Int)\n"),
    ],
  );
  assert_eq!(
    plan.links,
    [
      link("src/a/Reader.kt", "src/b/Store.kt", &[("Store", 4)], LinkState::Kept, false),
      link("src/a/Writer.kt", "src/b/Store.kt", &[("Store", 3)], LinkState::Removed, false),
    ]
  );
  assert_eq!(counts(&plan), [("src/a/Reader.kt", 1, 0, 0), ("src/a/Writer.kt", 0, 0, 0), ("src/b/Store.kt", 0, 1, 0)]);
}

/// A link between files of two modules (each with its build file) crosses them; one inside a module does not
#[test]
fn a_link_between_two_modules_crosses_them() {
  let plan = plan_of_change(
    &[("app/BUILD.bazel", "# app\n"), ("store/BUILD.bazel", "# store\n")],
    &[
      ("app/src/a/User.kt", "package a\n\nimport b.Store\n\nclass User(val store: Store, val name: Name)\n"),
      ("app/src/a/Name.kt", "package a\n\nclass Name\n"),
      ("store/src/b/Store.kt", "package b\n\nclass Store\n"),
    ],
  );
  let crossing: Vec<(&str, &str, bool)> = plan.links.iter().map(|link| (link.from.as_str(), link.to.as_str(), link.cross_module)).collect();
  assert_eq!(
    crossing,
    [("app/src/a/User.kt", "app/src/a/Name.kt", false), ("app/src/a/User.kt", "store/src/b/Store.kt", true)]
  );
}

/// A file renamed by the commits links before them at its old path, named by its new one: the use is kept
#[test]
fn a_renamed_file_links_at_its_new_path() {
  let repo = TestRepo::new();
  let base = repo.create_commit_with_files(
    "Add files",
    &[
      ("src/a/User.kt", "package a\n\nimport b.Store\n\nclass User(val store: Store)\n"),
      ("src/b/Store.kt", "package b\n\nclass Store\n\nfun open(): Int = 1\nfun close(): Int = 2\n"),
    ],
  );
  repo.git(&["mv", "src/b/Store.kt", "src/b/Stores.kt"]);
  std::fs::write(repo.path().join("src/a/User.kt"), "package a\n\nimport b.Store\n\nclass User(val store: Store?)\n").unwrap();
  repo.git(&["add", "-A"]);
  let head = repo.commit_with(&["-m", "Move the store"], &[]);
  let plan = plan_of(&repo, None, &range(&repo, &base, &head)).unwrap();
  let states: Vec<(&str, &str, LinkState)> = plan.links.iter().map(|link| (link.from.as_str(), link.to.as_str(), link.state)).collect();
  assert_eq!(states, [("src/a/User.kt", "src/b/Stores.kt", LinkState::Kept)]);
}
