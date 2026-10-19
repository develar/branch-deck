use crate::pull_requests::GitHubRepositorySource;
use crate::repository_config::{
  ConfigEntry, ConfigScope, ConfigSetting, DetectedGitHubRepository, FlatNameClash, RepositoryConfigChange, WriteScope, parse_config_entries, parse_git_bool, read_config_entries,
  read_repository_config, repository_config_from_entries, set_config, set_repository_config,
};
use crate::symbol_scan::SymbolMode;
use pretty_assertions::assert_eq;
use test_utils::git_test_utils::TestRepo;

fn entry(scope: ConfigScope, key: &str, value: Option<&str>) -> ConfigEntry {
  ConfigEntry {
    scope,
    key: key.to_string(),
    value: value.map(str::to_string),
  }
}

fn unset<T: Clone>(value: T) -> ConfigSetting<T> {
  ConfigSetting {
    value: value.clone(),
    scope: None,
    local_set: false,
    invalid: None,
    fallback_value: value,
    fallback_scope: None,
  }
}

#[test]
fn parses_the_scoped_entries_of_get_regexp() {
  let output = "global\0branchdeck.branchprefix\njane\0local\0branchdeck.stacks\0local\0branchdeck.githubrepository\n\0worktree\0branchdeck.symbols\nreport\nmore\0";
  assert_eq!(
    parse_config_entries(output),
    [
      entry(ConfigScope::Global, "branchdeck.branchprefix", Some("jane")),
      entry(ConfigScope::Local, "branchdeck.stacks", None),
      entry(ConfigScope::Local, "branchdeck.githubrepository", Some("")),
      entry(ConfigScope::Worktree, "branchdeck.symbols", Some("report\nmore")),
    ]
  );
  assert_eq!(parse_config_entries(""), []);
}

#[test]
fn nothing_set_is_the_defaults() {
  let config = repository_config_from_entries(&[], None);
  assert_eq!(config.stacks, unset(false));
  assert_eq!(config.symbols, unset(SymbolMode::On));
  assert_eq!(config.github_repository, unset(None));
  assert_eq!(config.branch_prefix, unset(None));
  assert_eq!(config.detected_github_repository, None);
}

#[test]
fn the_last_scope_wins_and_keys_are_case_insensitive() {
  let detected = DetectedGitHubRepository {
    full_name: "acme/app".to_string(),
    source: GitHubRepositorySource::Origin,
  };
  let config = repository_config_from_entries(
    &[
      entry(ConfigScope::Global, "branchdeck.stacks", Some("true")),
      entry(ConfigScope::Local, "BranchDeck.Stacks", Some("off")),
      entry(ConfigScope::Local, "branchdeck.branchprefix", Some("local/")),
      entry(ConfigScope::Command, "branchdeck.branchprefix", Some(" cli ")),
      entry(ConfigScope::System, "branchdeck.githubrepository", Some("acme/fork")),
    ],
    Some(detected.clone()),
  );
  assert_eq!(
    config.stacks,
    ConfigSetting {
      value: false,
      scope: Some(ConfigScope::Local),
      local_set: true,
      invalid: None,
      // the global value applies once the local one is unset
      fallback_value: true,
      fallback_scope: Some(ConfigScope::Global),
    }
  );
  assert_eq!(
    config.branch_prefix,
    ConfigSetting {
      value: Some("cli".to_string()),
      scope: Some(ConfigScope::Command),
      local_set: true,
      invalid: None,
      fallback_value: None,
      fallback_scope: None,
    }
  );
  assert_eq!(
    config.github_repository,
    ConfigSetting {
      value: Some("acme/fork".to_string()),
      scope: Some(ConfigScope::System),
      local_set: false,
      invalid: None,
      fallback_value: Some("acme/fork".to_string()),
      fallback_scope: Some(ConfigScope::System),
    }
  );
  assert_eq!(config.detected_github_repository, Some(detected));
}

#[test]
fn invalid_values_fall_back_to_the_default_and_say_why() {
  let config = repository_config_from_entries(
    &[
      entry(ConfigScope::Local, "branchdeck.stacks", Some("maybe")),
      entry(ConfigScope::Local, "branchdeck.symbols", Some("sometimes")),
      entry(ConfigScope::Local, "branchdeck.githubrepository", Some("acme")),
      entry(ConfigScope::Local, "branchdeck.branchprefix", Some("jane doe")),
    ],
    None,
  );
  assert_eq!(
    (config.stacks.value, config.stacks.invalid.as_deref()),
    (false, Some("maybe is not a boolean (true or false)"))
  );
  assert_eq!(
    (config.symbols.value, config.symbols.invalid.as_deref()),
    (SymbolMode::On, Some("sometimes is not off, report or on"))
  );
  assert_eq!(config.github_repository.value.as_deref(), Some("acme"));
  assert_eq!(config.github_repository.invalid.as_deref(), Some("acme is not a GitHub repository: use owner/name"));
  assert_eq!(config.branch_prefix.value.as_deref(), Some("jane doe"));
  assert!(config.branch_prefix.invalid.is_some());
  // a key without a value is a `true` for git, but not a symbol mode
  let config = repository_config_from_entries(
    &[entry(ConfigScope::Local, "branchdeck.stacks", None), entry(ConfigScope::Local, "branchdeck.symbols", None)],
    None,
  );
  assert_eq!((config.stacks.value, config.stacks.invalid), (true, None));
  assert!(config.symbols.invalid.is_some());
}

#[test]
fn an_empty_text_is_not_set() {
  let config = repository_config_from_entries(
    &[
      entry(ConfigScope::Local, "branchdeck.branchprefix", Some(" / ")),
      entry(ConfigScope::Local, "branchdeck.githubrepository", None),
    ],
    None,
  );
  assert_eq!(
    (config.branch_prefix.value, config.branch_prefix.scope, config.branch_prefix.invalid),
    (None, Some(ConfigScope::Local), None)
  );
  assert_eq!((config.github_repository.value, config.github_repository.invalid), (None, None));
}

#[test]
fn git_booleans_as_git_reads_them() {
  for (value, expected) in [
    (None, Some(true)),
    (Some("true"), Some(true)),
    (Some("YES"), Some(true)),
    (Some("On"), Some(true)),
    (Some("1"), Some(true)),
    (Some("-2"), Some(true)),
    (Some("false"), Some(false)),
    (Some("no"), Some(false)),
    (Some("OFF"), Some(false)),
    (Some("0"), Some(false)),
    (Some(""), Some(false)),
    (Some("maybe"), None),
  ] {
    assert_eq!(parse_git_bool(value), expected, "{value:?}");
  }
}

// ---- against a repository: only the local entries are asserted, the developer's global config may set the keys ----

fn local_entries(test_repo: &TestRepo) -> Vec<ConfigEntry> {
  read_config_entries(test_repo.git_executor(), test_repo.path_str())
    .unwrap()
    .into_iter()
    .filter(|entry| entry.scope == ConfigScope::Local)
    .collect()
}

#[test]
fn a_write_goes_to_the_local_config_and_reads_back() {
  let test_repo = TestRepo::new();
  let git = test_repo.git_executor();
  let path = test_repo.path_str();

  let config = set_repository_config(git, path, &RepositoryConfigChange::Stacks(Some(true))).unwrap();
  assert_eq!(
    config.stacks,
    ConfigSetting {
      value: true,
      scope: Some(ConfigScope::Local),
      local_set: true,
      invalid: None,
      fallback_value: false,
      fallback_scope: None,
    }
  );
  set_repository_config(git, path, &RepositoryConfigChange::Symbols(Some(SymbolMode::Report))).unwrap();
  set_repository_config(git, path, &RepositoryConfigChange::GithubRepository(Some(" acme/app ".to_string()))).unwrap();
  let config = set_repository_config(git, path, &RepositoryConfigChange::BranchPrefix(Some(" jane/ ".to_string()))).unwrap();
  assert_eq!(config.symbols.value, SymbolMode::Report);
  assert_eq!(config.github_repository.value.as_deref(), Some("acme/app"));
  assert_eq!(
    (config.branch_prefix.value.as_deref(), config.branch_prefix.scope),
    (Some("jane"), Some(ConfigScope::Local))
  );

  assert_eq!(
    local_entries(&test_repo),
    [
      entry(ConfigScope::Local, "branchdeck.stacks", Some("true")),
      entry(ConfigScope::Local, "branchdeck.symbols", Some("report")),
      entry(ConfigScope::Local, "branchdeck.githubrepository", Some("acme/app")),
      entry(ConfigScope::Local, "branchdeck.branchprefix", Some("jane")),
    ]
  );
  assert_eq!(read_repository_config(git, path).unwrap(), config);
}

#[test]
fn a_false_is_written_explicitly_and_wins_over_a_set_value() {
  let test_repo = TestRepo::new();
  test_repo.set_config("branchdeck.stacks", "yes").unwrap();
  // a second line: `--replace-all` replaces both
  test_repo.git(&["config", "--add", "BranchDeck.Stacks", "on"]);

  let config = set_repository_config(test_repo.git_executor(), test_repo.path_str(), &RepositoryConfigChange::Stacks(Some(false))).unwrap();

  assert_eq!((config.stacks.value, config.stacks.scope, config.stacks.local_set), (false, Some(ConfigScope::Local), true));
  assert_eq!(local_entries(&test_repo), [entry(ConfigScope::Local, "branchdeck.stacks", Some("false"))]);
}

#[test]
fn an_unset_removes_every_local_line_and_a_missing_key_is_fine() {
  let test_repo = TestRepo::new();
  let (git, path) = (test_repo.git_executor(), test_repo.path_str());
  set_repository_config(git, path, &RepositoryConfigChange::Symbols(None)).unwrap();

  test_repo.git(&["config", "--add", "branchdeck.symbols", "off"]);
  test_repo.git(&["config", "--add", "BranchDeck.Symbols", "report"]);
  let config = set_repository_config(git, path, &RepositoryConfigChange::Symbols(None)).unwrap();

  assert!(!config.symbols.local_set);
  assert_ne!(config.symbols.scope, Some(ConfigScope::Local));
  assert_eq!(local_entries(&test_repo), []);
  // an empty text unsets too
  set_repository_config(git, path, &RepositoryConfigChange::BranchPrefix(Some("jane".to_string()))).unwrap();
  let config = set_repository_config(git, path, &RepositoryConfigChange::BranchPrefix(Some("  ".to_string()))).unwrap();
  assert!(!config.branch_prefix.local_set);
}

#[test]
fn invalid_values_are_refused_before_git_runs() {
  let test_repo = TestRepo::new();
  let (git, path) = (test_repo.git_executor(), test_repo.path_str());
  for change in [
    RepositoryConfigChange::BranchPrefix(Some("jane doe".to_string())),
    RepositoryConfigChange::BranchPrefix(Some("a..b".to_string())),
    RepositoryConfigChange::GithubRepository(Some("acme".to_string())),
    RepositoryConfigChange::GithubRepository(Some("acme/app/x".to_string())),
    RepositoryConfigChange::GithubRepository(Some("acme/my app".to_string())),
  ] {
    set_repository_config(git, path, &change).expect_err(&format!("{change:?}"));
  }
  assert_eq!(local_entries(&test_repo), []);
  set_repository_config(git, "", &RepositoryConfigChange::Stacks(Some(true))).expect_err("no repository");
}

#[test]
fn a_linked_worktree_writes_the_common_config() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  let linked = tempfile::tempdir().unwrap();
  let linked_path = linked.path().join("linked");
  test_repo.git(&["worktree", "add", "-q", "-b", "other", linked_path.to_str().unwrap()]);

  let config = set_repository_config(test_repo.git_executor(), linked_path.to_str().unwrap(), &RepositoryConfigChange::Stacks(Some(true))).unwrap();

  assert_eq!(config.stacks.scope, Some(ConfigScope::Local));
  assert_eq!(local_entries(&test_repo), [entry(ConfigScope::Local, "branchdeck.stacks", Some("true"))]);
}

#[test]
fn a_value_of_the_worktree_config_refuses_the_write() {
  let test_repo = TestRepo::new();
  test_repo.git(&["config", "extensions.worktreeConfig", "true"]);
  test_repo.git(&["config", "--worktree", "branchdeck.stacks", "true"]);
  let (git, path) = (test_repo.git_executor(), test_repo.path_str());

  let config = read_repository_config(git, path).unwrap();
  assert_eq!(
    (config.stacks.value, config.stacks.scope, config.stacks.local_set),
    (true, Some(ConfigScope::Worktree), false)
  );

  let error = set_repository_config(git, path, &RepositoryConfigChange::Stacks(Some(false))).unwrap_err();
  assert_eq!(
    error.to_string(),
    "branchdeck.stacks is set in config.worktree, which overrides .git/config: change it there"
  );
  assert_eq!(local_entries(&test_repo).iter().filter(|entry| entry.key == "branchdeck.stacks").count(), 0);
}

#[test]
fn the_detected_github_repository_ignores_the_configured_one() {
  let test_repo = TestRepo::new();
  test_repo.add_remote("origin", "git@github.com:acme/app.git").unwrap();
  test_repo.set_config("branchdeck.githubRepository", "acme/mirror").unwrap();

  let config = read_repository_config(test_repo.git_executor(), test_repo.path_str()).unwrap();

  assert_eq!(config.github_repository.value.as_deref(), Some("acme/mirror"));
  assert_eq!(
    config.detected_github_repository,
    Some(DetectedGitHubRepository {
      full_name: "acme/app".to_string(),
      source: GitHubRepositorySource::Origin,
    })
  );
}

#[test]
fn the_fallback_is_the_last_global_or_system_value_parsed() {
  let config = repository_config_from_entries(
    &[
      entry(ConfigScope::System, "branchdeck.symbols", Some("off")),
      entry(ConfigScope::Global, "branchdeck.symbols", Some("report")),
      entry(ConfigScope::Local, "branchdeck.symbols", Some("on")),
      entry(ConfigScope::Global, "branchdeck.stacks", Some("maybe")),
    ],
    None,
  );
  assert_eq!(
    (config.symbols.value, config.symbols.fallback_value, config.symbols.fallback_scope),
    (SymbolMode::On, SymbolMode::Report, Some(ConfigScope::Global))
  );
  // an invalid global value falls back to the default, as the effective one would
  assert_eq!((config.stacks.fallback_value, config.stacks.fallback_scope), (false, Some(ConfigScope::Global)));
}

// ---- turning stacked branches off ----

/// A repository with stacked branches on and a parent with commits of its own below its child
fn stacked_parent_and_child() -> TestRepo {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  test_repo.create_commit("(auth/login/oauth) Add OAuth", "oauth.rs", "// oauth");
  test_repo.create_commit("(docs/api) Document", "api.md", "api");
  test_repo.set_config("branchdeck.stacks", "true").unwrap();
  test_repo
}

#[test]
fn with_stacks_on_the_read_names_the_flat_branches_that_would_clash() {
  let test_repo = stacked_parent_and_child();

  let config = read_repository_config(test_repo.git_executor(), test_repo.path_str()).unwrap();

  let clash = |branch: &str, beside: &str| FlatNameClash {
    branch: branch.to_string(),
    beside: beside.to_string(),
  };
  // `docs/api` alone is a flat branch git can keep: no `docs` beside it
  assert_eq!(config.stacks_off_clashes, [clash("auth/login", "auth"), clash("auth/login/oauth", "auth")]);
}

#[test]
fn with_stacks_off_the_read_groups_nothing() {
  let test_repo = stacked_parent_and_child();
  test_repo.set_config("branchdeck.stacks", "false").unwrap();

  let config = read_repository_config(test_repo.git_executor(), test_repo.path_str()).unwrap();

  assert_eq!(config.stacks_off_clashes, []);
}

#[test]
fn a_global_write_goes_to_the_global_config_and_a_local_value_still_wins() {
  let test_repo = TestRepo::new();
  let global = tempfile::NamedTempFile::new().unwrap();
  // the global config of this test alone, never the developer's
  let git = test_repo.git_executor().with_env("GIT_CONFIG_GLOBAL", global.path().to_str().unwrap());
  let path = test_repo.path_str();

  let config = set_config(&git, path, &RepositoryConfigChange::ResolveByWords(Some(false)), WriteScope::Global).unwrap();
  assert_eq!((config.resolve_by_words.value, config.resolve_by_words.scope), (false, Some(ConfigScope::Global)));
  assert_eq!(std::fs::read_to_string(global.path()).unwrap(), "[branchdeck]\n\tresolveByWords = false\n");
  // the global config alone, as Settings reads it without a repository
  assert!(
    !set_config(&git, "", &RepositoryConfigChange::ResolveByWords(Some(false)), WriteScope::Global)
      .unwrap()
      .resolve_by_words
      .value
  );

  let config = set_config(&git, path, &RepositoryConfigChange::ResolveByWords(Some(true)), WriteScope::Local).unwrap();
  assert_eq!((config.resolve_by_words.value, config.resolve_by_words.scope), (true, Some(ConfigScope::Local)));
  assert_eq!(
    (config.resolve_by_words.fallback_value, config.resolve_by_words.fallback_scope),
    (false, Some(ConfigScope::Global))
  );

  // an unset of the global value, twice: a key that is not set is fine
  set_config(&git, path, &RepositoryConfigChange::ResolveByWords(None), WriteScope::Global).unwrap();
  let config = set_config(&git, path, &RepositoryConfigChange::ResolveByWords(None), WriteScope::Global).unwrap();
  assert_eq!(config.resolve_by_words.fallback_scope, None);
  assert_eq!(std::fs::read_to_string(global.path()).unwrap(), "");
}
