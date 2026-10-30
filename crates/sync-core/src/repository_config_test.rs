use crate::config_snapshot::SymbolMode;
use crate::github_repository::GitHubRepositorySource;
use crate::repository_config::{
  ConfigEntry, ConfigScope, ConfigSetting, DetectedGitHubRepository, FlatNameClash, GLOBAL_PUSH_ROUTE_REASON, LandingDefault, OriginFacts, RepositoryConfigChange, WriteScope,
  parse_config_entries, read_config_entries, read_repository_config, repository_config_from_entries, set_config, set_repository_config,
};
use domain_types::landing_route::LandingSetting;
use pretty_assertions::assert_eq;
use sync_utils::git_bool::parse_git_bool;
use test_utils::git_test_utils::{TestRepo, append_config};
use test_utils::process_budget::{counting, processes};

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
  let config = repository_config_from_entries(&[], OriginFacts::default());
  assert_eq!(config.stacks, unset(false));
  assert_eq!(config.symbols, unset(SymbolMode::On));
  assert_eq!(config.github_repository, unset(None));
  assert_eq!(config.branch_prefix, unset(None));
  // unset is the default limit of a push
  assert_eq!(config.push_size_limit, unset(None));
  // an agent works in a lease of its own, and names the branch in each subject
  assert_eq!(config.agent_lease, unset(true));
  assert_eq!(config.agent_subject_branch, unset(true));
  // the review windows show the review plan
  assert_eq!(config.review_experimental, unset(false));
  assert_eq!(config.detected_github_repository, None);
  assert_eq!(config.landing_default, LandingDefault::of(false));
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
    OriginFacts {
      detected_github_repository: Some(detected.clone()),
      on_space: false,
    },
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
    OriginFacts::default(),
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
    OriginFacts::default(),
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
    OriginFacts::default(),
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
    RepositoryConfigChange::PushSizeLimit(Some("lots".to_string())),
    RepositoryConfigChange::PushSizeLimit(Some("1.5G".to_string())),
  ] {
    set_repository_config(git, path, &change).expect_err(&format!("{change:?}"));
  }
  assert_eq!(local_entries(&test_repo), []);
  set_repository_config(git, "", &RepositoryConfigChange::Stacks(Some(true))).expect_err("no repository");
}

/// The push size limit is written as set, trimmed, a size or off; a value git holds that is neither keeps its text and
/// says why
#[test]
fn the_push_size_limit_is_a_size_or_off_as_written() {
  let test_repo = TestRepo::new();
  let (git, path) = (test_repo.git_executor(), test_repo.path_str());
  let config = set_repository_config(git, path, &RepositoryConfigChange::PushSizeLimit(Some(" 200M ".to_string()))).unwrap();
  assert_eq!((config.push_size_limit.value.as_deref(), config.push_size_limit.invalid), (Some("200M"), None));
  let config = set_repository_config(git, path, &RepositoryConfigChange::PushSizeLimit(Some("off".to_string()))).unwrap();
  assert_eq!(config.push_size_limit.value.as_deref(), Some("off"));
  assert_eq!(local_entries(&test_repo), [entry(ConfigScope::Local, "branchdeck.pushsizelimit", Some("off"))]);

  test_repo.git(&["config", "branchdeck.pushSizeLimit", "lots"]);
  let config = read_repository_config(git, path).unwrap();
  assert_eq!(config.push_size_limit.value.as_deref(), Some("lots"));
  assert_eq!(config.push_size_limit.invalid.as_deref(), Some("lots is not a size such as 50M or 1G, nor 0 or off"));
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

/// The route of an unset key follows the host of `origin`, read with the detected GitHub repository from one read of
/// its push URL: Patronus on Space, the pull request elsewhere and without `origin`, never a push
#[test]
fn the_default_route_follows_the_host_of_origin_in_the_one_read_of_its_push_url() {
  let test_repo = TestRepo::new();
  let read = || read_repository_config(test_repo.git_executor(), test_repo.path_str()).unwrap();
  assert_eq!(read().landing_default, LandingDefault::of(false));
  test_repo.add_remote("origin", "git@github.com:acme/app.git").unwrap();
  let git = counting();
  let config = read_repository_config(&git, test_repo.path_str()).unwrap();
  assert_eq!(
    config.landing_default,
    LandingDefault {
      route: LandingSetting::PullRequest,
      on_space: false,
    }
  );
  // the config and the push URL of origin, which names the GitHub repository too
  assert_eq!(processes(&git), [("config".to_string(), 1), ("remote".to_string(), 1)]);
  test_repo.git(&["remote", "set-url", "origin", "ssh://git@git.jetbrains.team/ij/app.git"]);
  assert_eq!(
    read().landing_default,
    LandingDefault {
      route: LandingSetting::Patronus,
      on_space: true,
    }
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
    OriginFacts::default(),
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

/// The git of `test_repo` with a global config of the test alone, never the developer's
fn with_own_global_config(test_repo: &TestRepo) -> (tempfile::NamedTempFile, git_executor::git_command_executor::GitCommandExecutor) {
  let global = tempfile::NamedTempFile::new().unwrap();
  let git = test_repo.git_executor().with_env("GIT_CONFIG_GLOBAL", global.path().to_str().unwrap());
  (global, git)
}

#[test]
fn a_global_write_goes_to_the_global_config_and_a_local_value_still_wins() {
  let test_repo = TestRepo::new();
  let (global, git) = with_own_global_config(&test_repo);
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

#[test]
fn the_agent_keys_are_written_globally_and_a_repository_keeps_its_own() {
  let test_repo = TestRepo::new();
  let (global, git) = with_own_global_config(&test_repo);
  let path = test_repo.path_str();

  set_config(&git, path, &RepositoryConfigChange::AgentLease(Some(false)), WriteScope::Global).unwrap();
  let config = set_config(&git, path, &RepositoryConfigChange::AgentSubjectBranch(Some(false)), WriteScope::Local).unwrap();
  assert_eq!(std::fs::read_to_string(global.path()).unwrap(), "[branchdeck]\n\tagentLease = false\n");
  assert_eq!((config.agent_lease.value, config.agent_lease.scope), (false, Some(ConfigScope::Global)));
  assert_eq!(
    (
      config.agent_subject_branch.value,
      config.agent_subject_branch.scope,
      config.agent_subject_branch.fallback_value
    ),
    (false, Some(ConfigScope::Local), true)
  );
  assert_eq!(local_entries(&test_repo), [entry(ConfigScope::Local, "branchdeck.agentsubjectbranch", Some("false"))]);

  // "Like Global": the local value goes, the default applies again
  let config = set_config(&git, path, &RepositoryConfigChange::AgentSubjectBranch(None), WriteScope::Local).unwrap();
  assert_eq!(config.agent_subject_branch, unset(true));
}

/// `branchdeck.reviewExperimental` is read with the other keys in the one `git config` of the settings, and written to the
/// local config like them; unset is off, and a value git refuses is off, with the reason
#[test]
fn the_review_experiments_are_off_until_the_config_turns_them_on() {
  let test_repo = TestRepo::new();
  let path = test_repo.path_str();
  let git = test_repo.git_executor();
  let config = set_config(git, path, &RepositoryConfigChange::ReviewExperimental(Some(true)), WriteScope::Local).unwrap();
  assert_eq!((config.review_experimental.value, config.review_experimental.scope), (true, Some(ConfigScope::Local)));
  assert_eq!(local_entries(&test_repo), [entry(ConfigScope::Local, "branchdeck.reviewexperimental", Some("true"))]);

  // "Use Default": the local value goes, the default applies again
  let config = set_config(git, path, &RepositoryConfigChange::ReviewExperimental(None), WriteScope::Local).unwrap();
  assert_eq!(config.review_experimental, unset(false));

  let config = repository_config_from_entries(&[entry(ConfigScope::Local, "branchdeck.reviewExperimental", Some("maybe"))], OriginFacts::default());
  assert_eq!(
    (config.review_experimental.value, config.review_experimental.invalid),
    (false, Some("maybe is not a boolean (true or false)".to_string()))
  );
}

/// Settings → Repository saves a typed prefix on blur and a switch at once: two writes of the app, each answering the
/// settings read again, never a "Cannot set …" for a `config.lock` the other write held. Each answer counts more writes
/// (`seq`) than the one before it of its thread, and a read after them counts them all, so the page keeps the newest
#[test]
fn two_writes_at_once_both_succeed() {
  let test_repo = TestRepo::new();
  let git = test_repo.git_executor();
  let path = test_repo.path_str();

  let (switches, prefixes) = std::thread::scope(|scope| {
    let switches = scope.spawn(|| {
      (0..50)
        .map(|round| set_repository_config(git, path, &RepositoryConfigChange::Stacks(Some(round % 2 == 0))).unwrap().seq)
        .collect::<Vec<_>>()
    });
    let prefixes = scope.spawn(|| {
      (0..50)
        .map(|round| {
          set_repository_config(git, path, &RepositoryConfigChange::BranchPrefix(Some(format!("jane{round}"))))
            .unwrap()
            .seq
        })
        .collect::<Vec<_>>()
    });
    (switches.join().unwrap(), prefixes.join().unwrap())
  });

  for answers in [&switches, &prefixes] {
    assert!(
      answers.windows(2).all(|pair| pair[0] < pair[1]),
      "each answer of a thread counts its own write more: {answers:?}"
    );
  }
  let config = read_repository_config(git, path).unwrap();
  assert_eq!((config.stacks.value, config.branch_prefix.value.as_deref()), (false, Some("jane49")));
  let newest = switches.iter().chain(&prefixes).max().copied();
  assert_eq!(
    (Some(config.seq), config.seq),
    (newest, 100),
    "the read after both threads counts the 100 writes, as the newest answer does"
  );
}

/// A `config.lock` of another process (`bd describe`, a `git config` of the user) released within the second is waited
/// for: the write succeeds once it is gone
#[test]
fn a_write_waits_for_a_config_lock_another_process_holds() {
  let test_repo = TestRepo::new();
  // a git that refuses a locked config at once, as 2.54 and 2.55 do: the wait is the write's own (2.56 waits a second
  // itself, the default of `core.configLockTimeout`)
  append_config(test_repo.path(), &[("core.configLockTimeout", "0")]);
  let git = counting();
  let path = test_repo.path_str();
  let lock = test_repo.path().join(".git").join("config.lock");
  std::fs::write(&lock, "").unwrap();

  std::thread::scope(|scope| {
    // the other process lets go once git refused the write: the read of the scopes and the first write have ended
    let release = scope.spawn(|| {
      let deadline = std::time::Instant::now() + std::time::Duration::from_mins(1);
      while processes(&git).iter().find(|(subcommand, _)| subcommand == "config").map_or(0, |(_, count)| *count) < 2 {
        assert!(std::time::Instant::now() < deadline, "the write never ran");
        std::thread::sleep(std::time::Duration::from_millis(1));
      }
      std::fs::remove_file(&lock).unwrap();
    });
    let config = set_repository_config(&git, path, &RepositoryConfigChange::Stacks(Some(true))).unwrap();
    assert!(config.stacks.value);
    release.join().unwrap();
  });
}

/// A lock that stays is an error after about a second, not a wait without end
#[test]
fn a_write_fails_on_a_config_lock_that_stays() {
  let test_repo = TestRepo::new();
  std::fs::write(test_repo.path().join(".git").join("config.lock"), "").unwrap();
  let error = set_repository_config(test_repo.git_executor(), test_repo.path_str(), &RepositoryConfigChange::Stacks(Some(true))).unwrap_err();
  assert!(error.to_string().starts_with("git config branchdeck.stacks failed"), "{error}");
}

// ---- the route into master ----

#[test]
fn the_route_is_one_of_three_and_a_push_of_the_global_config_is_unset() {
  let landing = |entries: &[ConfigEntry]| repository_config_from_entries(entries, OriginFacts::default()).landing;
  assert_eq!(landing(&[]), unset(None));
  let local = landing(&[entry(ConfigScope::Local, "branchdeck.landing", Some("pull-request"))]);
  assert_eq!(
    (local.value, local.scope, local.invalid),
    (Some(LandingSetting::PullRequest), Some(ConfigScope::Local), None)
  );
  let wrong = landing(&[entry(ConfigScope::Local, "branchdeck.landing", Some("merge"))]);
  assert_eq!((wrong.value, wrong.invalid.as_deref()), (None, Some("merge is not patronus, push or pull-request")));

  // a push into master of every repository of the user is a trap: only the repository's own config names it
  let global = landing(&[entry(ConfigScope::Global, "branchdeck.landing", Some("push"))]);
  assert_eq!(
    (global.value, global.scope, global.invalid.as_deref(), global.fallback_value),
    (None, Some(ConfigScope::Global), Some(GLOBAL_PUSH_ROUTE_REASON), None)
  );
  let local_over_global = landing(&[
    entry(ConfigScope::Global, "branchdeck.landing", Some("push")),
    entry(ConfigScope::Local, "branchdeck.landing", Some("push")),
  ]);
  assert_eq!(
    (local_over_global.value, local_over_global.invalid, local_over_global.fallback_value),
    (Some(LandingSetting::Push), None, None)
  );
  let worktree = landing(&[entry(ConfigScope::Worktree, "branchdeck.landing", Some("push"))]);
  assert_eq!(worktree.value, Some(LandingSetting::Push));
}

#[test]
fn a_push_route_is_written_only_to_the_local_config() {
  let test_repo = TestRepo::new();
  let (global, git) = with_own_global_config(&test_repo);
  let path = test_repo.path_str();

  let refused = set_config(&git, path, &RepositoryConfigChange::Landing(Some(LandingSetting::Push)), WriteScope::Global).unwrap_err();
  assert_eq!(refused.to_string(), format!("branchdeck.landing: {GLOBAL_PUSH_ROUTE_REASON}"));
  assert_eq!(std::fs::read_to_string(global.path()).unwrap(), "");

  let config = set_config(&git, path, &RepositoryConfigChange::Landing(Some(LandingSetting::PullRequest)), WriteScope::Global).unwrap();
  assert_eq!((config.landing.value, config.landing.scope), (Some(LandingSetting::PullRequest), Some(ConfigScope::Global)));
  let config = set_config(&git, path, &RepositoryConfigChange::Landing(Some(LandingSetting::Push)), WriteScope::Local).unwrap();
  assert_eq!((config.landing.value, config.landing.scope), (Some(LandingSetting::Push), Some(ConfigScope::Local)));
  assert_eq!(
    (config.landing.fallback_value, config.landing.fallback_scope),
    (Some(LandingSetting::PullRequest), Some(ConfigScope::Global))
  );
  let config = set_config(&git, path, &RepositoryConfigChange::Landing(None), WriteScope::Local).unwrap();
  assert_eq!(config.landing.value, Some(LandingSetting::PullRequest));
}
