use crate::ownership::{FileOwner, OwnershipRules, read_file_owners};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn owner(rules: &OwnershipRules, path: &str) -> Option<String> {
  rules.owner_of(path).map(|owner| owner.owner)
}

fn single(pattern: &str) -> OwnershipRules {
  OwnershipRules::from_files([("OWNERSHIP", format!("{pattern}\nowner 'A'\n").leak() as &str)])
}

#[test]
fn patterns_match_as_jgit_fast_ignore_rule_with_path_match_false() {
  let cases: &[(&str, &str, bool)] = &[
    // a plain pattern matches the path and everything below it
    ("/a", "a", true),
    ("/a", "a/b/c.txt", true),
    ("/a", "ab", false),
    ("/a/b", "a/b/c", true),
    ("/a/b", "x/a/b", false),
    // a trailing slash: only a directory
    ("/a/", "a", false),
    ("/a/", "a/b", true),
    // a trailing `**`: everything inside, not the directory itself
    ("/a/**", "a", false),
    ("/a/**", "a/x", true),
    ("/a/**", "a/x/y", true),
    ("/**", "anything/at/all.kt", true),
    // `**` in the middle: zero or more directories
    ("/a/**/b", "a/b", true),
    ("/a/**/b", "a/x/y/b", true),
    ("/a/**/b", "a/x/y/c", false),
    ("/**/test", "x/y/test/A.kt", true),
    // globs within a segment
    ("/*.kt", "A.kt", true),
    ("/*.kt", "src/A.kt", false),
    ("/src/*.kt", "src/A.kt", true),
    ("/src/*", "src/a/b.kt", true),
    ("/a?c", "abc", true),
    ("/a?c", "ac", false),
    ("/[ab]x", "bx", true),
    ("/[ab]x", "cx", false),
    ("/[a-c]x", "cx", true),
    ("/[!a]x", "ax", false),
    ("/[!a]x", "bx", true),
    ("/\\*x", "*x", true),
    ("/\\*x", "ax", false),
    ("/a**b", "axyb", true),
  ];
  for &(pattern, path, expected) in cases {
    assert_eq!(owner(&single(pattern), path).is_some(), expected, "{pattern} against {path}");
  }
}

#[test]
fn the_last_match_wins_by_depth_then_file_then_order() {
  let rules = OwnershipRules::from_files([
    ("plugins/air/OWNERSHIP", "owner 'Air Plugin'\n\n/release/\nowner 'Air Plugin Release Team'\n"),
    ("OWNERSHIP", "# root\nowner 'Monorepo'\n/plugins/**\nowner 'Plugins'\n"),
    ("plugins/OWNERSHIP", "/*/build.gradle\nowner 'Build'\n"),
  ]);
  assert_eq!(owner(&rules, "README.md").as_deref(), Some("Monorepo"));
  assert_eq!(owner(&rules, "plugins/x/A.kt").as_deref(), Some("Plugins"));
  assert_eq!(owner(&rules, "plugins/x/build.gradle").as_deref(), Some("Build"));
  // the deeper file wins over the shallower one, whatever their rules
  assert_eq!(owner(&rules, "plugins/air/build.gradle").as_deref(), Some("Air Plugin"));
  assert_eq!(owner(&rules, "plugins/air/release/notes.md").as_deref(), Some("Air Plugin Release Team"));
  assert_eq!(
    rules.owner_of("plugins/air/release/notes.md"),
    Some(FileOwner {
      path: "plugins/air/release/notes.md".to_string(),
      owner: "Air Plugin Release Team".to_string(),
      rule: "plugins/air/release/".to_string(),
      source_file: "plugins/air/OWNERSHIP".to_string(),
    })
  );
}

#[test]
fn a_section_without_patterns_owns_the_directory_and_several_patterns_share_an_owner() {
  let rules = OwnershipRules::from_files([("a/OWNERSHIP", "/x\n/y/\nowner 'XY'\nowner 'Rest'\n")]);
  // the section without patterns comes after the others, so it wins everywhere in `a`
  assert_eq!(owner(&rules, "a/x").as_deref(), Some("Rest"));
  let rules = OwnershipRules::from_files([("a/OWNERSHIP", "owner 'Rest'\n/x\n/y/\nowner 'XY'\n")]);
  assert_eq!(owner(&rules, "a/x").as_deref(), Some("XY"));
  assert_eq!(owner(&rules, "a/y/z").as_deref(), Some("XY"));
  assert_eq!(owner(&rules, "a/z").as_deref(), Some("Rest"));
  assert_eq!(owner(&rules, "b/z"), None);
}

#[test]
fn a_file_that_does_not_parse_is_skipped() {
  for text in ["/x\n", "owner \"A\"\n", "x\nowner 'A'\n", "owner ''\n"] {
    let rules = OwnershipRules::from_files([("OWNERSHIP", "owner 'Root'\n"), ("a/OWNERSHIP", text)]);
    assert_eq!(owner(&rules, "a/x").as_deref(), Some("Root"), "{text:?}");
  }
  let rules = OwnershipRules::from_files([("OWNERSHIP", "owner 'Root'\r\n# windows\r\n")]);
  assert_eq!(owner(&rules, "x").as_deref(), Some("Root"));
}

#[test]
fn reads_the_ownership_files_of_the_ancestor_directories_of_a_tree() {
  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "Owners",
    &[
      ("OWNERSHIP", "owner 'Root'\n"),
      ("plugins/air/OWNERSHIP", "owner 'Air Plugin'\n"),
      ("plugins/air/src/A.kt", "a\n"),
      ("build/OWNERSHIP", "/scripts/\nowner 'Build Scripts'\n"),
      ("other/OWNERSHIP", "owner 'Never Read'\n"),
    ],
  );
  let paths = ["plugins/air/src/A.kt", "plugins/air/new file.kt", "build/scripts/x.kts", "README.md"].map(str::to_string);
  let owners = read_file_owners(&repo.git_executor(), repo.path().to_str().unwrap(), "HEAD", &paths).unwrap();
  let owners: Vec<(&str, &str)> = owners.iter().map(|owner| (owner.path.as_str(), owner.owner.as_str())).collect();
  assert_eq!(
    owners,
    vec![
      ("plugins/air/src/A.kt", "Air Plugin"),
      ("plugins/air/new file.kt", "Air Plugin"),
      ("build/scripts/x.kts", "Build Scripts"),
      ("README.md", "Root"),
    ]
  );
}

#[test]
fn a_tree_without_ownership_files_has_no_owners() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("a/b.txt", "b\n")]);
  let owners = read_file_owners(&repo.git_executor(), repo.path().to_str().unwrap(), "HEAD", &["a/b.txt".to_string()]).unwrap();
  assert_eq!(owners, Vec::new());
}
