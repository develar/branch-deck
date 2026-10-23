use crate::module_roots::ModuleRoots;
use git_ops::object_texts::TreeEntry;
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

fn file(name: &str, id: &str) -> TreeEntry {
  TreeEntry {
    mode: "100644".to_string(),
    name: name.to_string(),
    id: id.to_string(),
  }
}

#[test]
fn the_nearest_directory_with_a_build_file_is_the_root() {
  let air = [file("intellij.air.iml", "a")];
  let support = [file("intellij.air.tests.support.iml", "b")];
  let crate_dir = [file("Cargo.toml", "c")];
  let web = [file("package.json", "d"), file("tsconfig.json", "e"), file("tsconfig.app.json", "f")];
  let roots = ModuleRoots::from_entries(&[
    ("plugins/air", &air),
    ("plugins/air/tests/integration/support", &support),
    ("tools/vm/crates/avl-guest", &crate_dir),
    ("", &web),
  ]);
  assert_eq!(roots.module_of("plugins/air/src/com/intellij/air/A.kt"), Some("plugins/air"));
  // a module inside a module: the nearest one
  assert_eq!(
    roots.module_of("plugins/air/tests/integration/support/src/Dsl.kt"),
    Some("plugins/air/tests/integration/support")
  );
  assert_eq!(roots.module_of("tools/other/B.kt"), None);
  assert_eq!(roots.crate_of("tools/vm/crates/avl-guest/src/lane/identity.rs"), Some("tools/vm/crates/avl-guest"));
  assert_eq!(roots.package_of("src/lib/a.ts"), Some(""));
  assert_eq!(
    roots.tsconfigs_of("src/lib/a.ts"),
    [("tsconfig.json".to_string(), "e"), ("tsconfig.app.json".to_string(), "f")]
  );
}

/// One `cat-file --batch` reads the directories above the files at HEAD; a directory HEAD lacks has no facts
#[test]
fn reads_the_directories_above_the_files_at_head() {
  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "crate",
    &[("crates/one/Cargo.toml", "[package]\nname = \"one\"\n"), ("crates/one/src/lib.rs", "pub fn f() {}\n")],
  );
  let roots = ModuleRoots::read(repo.git_executor(), repo.path_str(), ["crates/one/src/lib.rs", "gone/x.rs"]).unwrap();
  assert_eq!(roots.crate_of("crates/one/src/lib.rs"), Some("crates/one"));
  assert_eq!(roots.crate_of("gone/x.rs"), None);
  let manifest = repo.git_executor().execute_command(&["rev-parse", "HEAD:crates/one/Cargo.toml"], repo.path_str()).unwrap();
  assert_eq!(roots.manifests().collect::<Vec<_>>(), [("crates/one", manifest.as_str())]);
}
