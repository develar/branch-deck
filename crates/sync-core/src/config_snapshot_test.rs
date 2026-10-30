use crate::branch_prefix::get_branch_prefix_from_git_config_sync;
use crate::config_snapshot::{ConfigSnapshot, SymbolMode};
use crate::github_repository::GitHubRepository;
use domain_types::landing_route::LandingSetting;
use domain_types::settings::GitBranchPrefix;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_description::{Description, read_descriptions};
use git_ops::word_resolve::ResolveMode;
use history_rewrite::stack::stacks_enabled;
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

fn snapshot(repo: &TestRepo) -> ConfigSnapshot {
  ConfigSnapshot::read(repo.git_executor(), repo.path_str()).unwrap()
}

/// `branchdeck.stacks` as `git config --type=bool` reads it: `Some` of its answer, `None` when git refuses the value
fn git_bool(repo: &TestRepo) -> Option<bool> {
  let (output, exit_code) = repo
    .git_executor()
    .execute_command_with_status(&["config", "--type=bool", "--get", "branchdeck.stacks"], repo.path_str())
    .unwrap();
  (exit_code == 0).then(|| output == "true")
}

/// A boolean is what git makes of it, git itself the oracle: the words in any case, the empty text, git's integers
/// (base 0 of `strtoimax`, a unit, the range of an `int`) and what it refuses; `0b1` aside, which git answers by its
/// libc
#[test]
fn a_boolean_is_what_git_makes_of_it() {
  let repo = TestRepo::new();
  let values = [
    "true",
    "TRUE",
    "Yes",
    "oN",
    "false",
    "No",
    "OFF",
    "",
    "1",
    "0",
    "2",
    "-1",
    "+1",
    "00",
    "-0",
    "010",
    "08",
    "0x10",
    "0X1F",
    "-0x1",
    "0x0",
    "0x",
    "1k",
    "1K",
    "0k",
    "1m",
    "1G",
    "1t",
    "1kb",
    "k",
    "+",
    "-",
    " 1",
    "\t1",
    "1 ",
    " true ",
    "1.5",
    "maybe",
    "2147483647",
    "2147483648",
    "-2147483648",
    "-2147483649",
    "2097151k",
    "2097152k",
    "9999999999999999999",
  ];
  for value in values {
    repo.set_config("branchdeck.stacks", value).unwrap();
    let expected = git_bool(&repo);
    let snapshot = snapshot(&repo);
    assert_eq!(snapshot.stacks().ok(), expected, "{value:?}");
    assert_eq!(stacks_enabled(repo.git_executor(), repo.path_str()).ok(), expected, "{value:?}");
  }

  // `0b1` is refused on every platform, and git is no oracle for it: git defines `_GNU_SOURCE`, so with glibc 2.38+
  // its `strtoimax` follows C23 and reads `0b` as binary (true), while the git of macOS and Git for Windows refuse it.
  // Nobody writes a boolean in binary, and the glibc of the git we run is unknown, so there is one answer, C17's.
  repo.set_config("branchdeck.stacks", "0b1").unwrap();
  assert_eq!(snapshot(&repo).stacks().ok(), None);
  assert_eq!(stacks_enabled(repo.git_executor(), repo.path_str()).ok(), None);

  // a key without a value is true
  repo.git(&["config", "--unset-all", "branchdeck.stacks"]);
  let config = repo.path().join(".git/config");
  let text = std::fs::read_to_string(&config).unwrap();
  std::fs::write(&config, format!("{text}[branchdeck]\n\tstacks\n")).unwrap();
  assert_eq!(git_bool(&repo), Some(true));
  assert_eq!(snapshot(&repo).stacks().ok(), Some(true));
}

/// The snapshot says what the reader of each key says (which also see the global config of the machine)
#[track_caller]
fn assert_same_as_the_readers(repo: &TestRepo) {
  let (git, path) = (repo.git_executor(), repo.path_str());
  let snapshot = snapshot(repo);
  assert_eq!(snapshot.stacks().unwrap(), stacks_enabled(git, path).unwrap());
  assert_eq!(snapshot.branch_prefix(), get_branch_prefix_from_git_config_sync(git, path).unwrap());
  assert_eq!(snapshot.descriptions(), &read_descriptions(git, path).unwrap());
}

/// One process for the settings and the descriptions, each as its own reader resolves it: the last entry of a key
/// wins, a description by the full ref with the case of the branch name
#[test]
fn one_read_of_the_settings_and_the_descriptions() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  assert_same_as_the_readers(&repo);

  repo.set_config("branchdeck.stacks", "yes").unwrap();
  repo.set_config("branchdeck.symbols", "report").unwrap();
  repo.set_config("branchDeck.branchPrefix", " dev ").unwrap();
  repo.set_config("branchdeck.githubRepository", "acme/widgets").unwrap();
  repo.set_config("branch.test/virtual/Auth.description", "Login\n\nThe body").unwrap();
  repo.set_config("branch.test/virtual/ui.description", "Form").unwrap();
  assert_same_as_the_readers(&repo);

  let git = GitCommandExecutor::new().with_process_stats("test");
  let read = ConfigSnapshot::read(&git, repo.path_str()).unwrap();
  let processes = git.process_stats().unwrap().total();
  assert_eq!(processes, 1);
  assert!(read.stacks().unwrap());
  assert_eq!(read.symbol_mode().unwrap(), SymbolMode::Report);
  assert_eq!(
    read.branch_prefix(),
    GitBranchPrefix {
      prefix: "dev".to_string(),
      repository_scoped: true,
    }
  );
  assert_eq!(
    read.github_repository(),
    Some(GitHubRepository {
      owner: "acme".to_string(),
      name: "widgets".to_string(),
    })
  );
  assert_eq!(
    read.descriptions()["refs/heads/test/virtual/Auth"],
    Description {
      title: "Login".to_string(),
      body: "The body".to_string(),
    }
  );

  repo.set_config("branchdeck.symbols", "sometimes").unwrap();
  assert_eq!(
    snapshot(&repo).symbol_mode().unwrap_err().to_string(),
    "branchdeck.symbols is not off, report or on: sometimes"
  );
}

/// The resolve by words is on when unset, off for a false of any scope, and a value git refuses is an error
#[test]
fn the_resolve_by_words_is_on_unless_the_config_turns_it_off() {
  let repo = TestRepo::new();
  assert_eq!(snapshot(&repo).resolve_mode().unwrap(), ResolveMode::On);
  assert!(snapshot(&repo).settings().resolve_by_words.value);
  repo.set_config("branchdeck.resolveByWords", "false").unwrap();
  assert_eq!(snapshot(&repo).resolve_mode().unwrap(), ResolveMode::Off);
  let setting = snapshot(&repo).settings().resolve_by_words;
  assert_eq!((setting.value, setting.local_set, setting.fallback_value), (false, true, true));
  repo.set_config("branchdeck.resolveByWords", "maybe").unwrap();
  snapshot(&repo).resolve_mode().unwrap_err();
}

/// The flag of the review windows rides on the snapshot of a sync: on when unset, off for a false, and a value git
/// refuses is off without failing the sync
#[test]
fn the_review_experiments_are_on_unless_the_config_turns_them_off() {
  let repo = TestRepo::new();
  assert!(snapshot(&repo).review_experimental());
  repo.set_config("branchdeck.reviewExperimental", "no").unwrap();
  assert!(!snapshot(&repo).review_experimental());
  repo.set_config("branchdeck.reviewExperimental", "yes").unwrap();
  assert!(snapshot(&repo).review_experimental());
  repo.set_config("branchdeck.reviewExperimental", "maybe").unwrap();
  assert!(!snapshot(&repo).review_experimental());
}

#[test]
fn the_mode_comes_from_git_config_and_is_on_without_it() {
  let repo = TestRepo::new();
  let mode = || snapshot(&repo).symbol_mode();
  assert_eq!(mode().unwrap(), SymbolMode::On);
  repo.set_config("branchdeck.symbols", "report").unwrap();
  assert_eq!(mode().unwrap(), SymbolMode::Report);
  repo.set_config("branchdeck.symbols", "off").unwrap();
  assert_eq!(mode().unwrap(), SymbolMode::Off);
  repo.set_config("branchdeck.symbols", "maybe").unwrap();
  mode().unwrap_err();
}

/// The route an operation reads: unset is none (the rule decides by the host), a value other than the three refuses
#[test]
fn the_route_comes_from_git_config_and_is_none_without_it() {
  let repo = TestRepo::new();
  let landing = || snapshot(&repo).landing();
  assert_eq!(landing().unwrap(), None);
  repo.set_config("branchdeck.landing", "push").unwrap();
  assert_eq!(landing().unwrap(), Some(LandingSetting::Push));
  repo.set_config("branchdeck.landing", "Pull-Request").unwrap();
  assert_eq!(landing().unwrap(), Some(LandingSetting::PullRequest));
  repo.set_config("branchdeck.landing", "merge").unwrap();
  assert_eq!(landing().unwrap_err().to_string(), "branchdeck.landing: merge is not patronus, push or pull-request");
}

/// A push of the global config is unset for an operation too: it lands nothing into master of a repository that names
/// no route of its own
#[test]
fn a_push_route_of_the_global_config_is_none() {
  let repo = TestRepo::new();
  let global = tempfile::NamedTempFile::new().unwrap();
  std::fs::write(global.path(), "[branchdeck]\n\tlanding = push\n").unwrap();
  let git = repo.git_executor().with_env("GIT_CONFIG_GLOBAL", global.path().to_str().unwrap());
  assert_eq!(ConfigSnapshot::read(&git, repo.path_str()).unwrap().landing().unwrap(), None);
  repo.set_config("branchdeck.landing", "push").unwrap();
  assert_eq!(ConfigSnapshot::read(&git, repo.path_str()).unwrap().landing().unwrap(), Some(LandingSetting::Push));
}
