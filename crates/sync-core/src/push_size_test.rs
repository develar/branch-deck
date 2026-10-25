use crate::config_snapshot::ConfigSnapshot;
use crate::push_size::{DEFAULT_PUSH_SIZE_LIMIT, check_push_size, parse_push_size_limit};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The limit is a size; `0` and the words of off turn the check off; anything else says what it takes
#[test]
fn the_limit_is_a_size_or_off() {
  for (text, limit) in [("50M", Some(50 << 20)), (" 1G ", Some(1 << 30)), ("0", None), ("off", None), ("OFF", None), ("false", None)] {
    assert_eq!(parse_push_size_limit(text), Ok(limit), "{text}");
  }
  let refused = parse_push_size_limit("lots").unwrap_err();
  assert!(refused.ends_with("; or 0 or off for no limit"), "{refused}");
}

/// The limit of an operation comes with its one `git config`: the default unset, none when off, an error for a value
/// that is neither, so the push is refused rather than sent unchecked
#[test]
fn the_limit_of_an_operation_is_the_config_or_the_default() {
  let repo = TestRepo::new();
  let limit = || ConfigSnapshot::read(repo.git_executor(), repo.path_str()).unwrap().push_size_limit();
  assert_eq!(limit().unwrap(), Some(DEFAULT_PUSH_SIZE_LIMIT));
  repo.git(&["config", "branchdeck.pushSizeLimit", "off"]);
  assert_eq!(limit().unwrap(), None);
  repo.git(&["config", "branchdeck.pushSizeLimit", "big"]);
  let error = limit().unwrap_err().to_string();
  assert!(error.starts_with("branchdeck.pushSizeLimit is not a size: "), "{error}");
}

/// Files spread over the tree name no directory, only the largest files; a push within the limit is no refusal
#[test]
fn files_spread_over_the_tree_name_no_directory() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Base", "README.md", "base");
  repo.git(&["update-ref", "refs/remotes/origin/main", &base]);
  let tip = repo.create_commit_with_files(
    "Spread",
    &[("a/one.txt", &"1".repeat(400)), ("b/two.txt", &"2".repeat(300)), ("three.txt", &"3".repeat(200))],
  );

  let too_large = check_push_size(repo.git_executor(), repo.path_str(), &[&tip], "origin", 800).unwrap().unwrap();
  assert_eq!(too_large.directory, None);
  assert_eq!(too_large.paths(), ["a/one.txt", "b/two.txt", "three.txt"]);
  assert_eq!(check_push_size(repo.git_executor(), repo.path_str(), &[&tip], "origin", 900).unwrap(), None);
}
