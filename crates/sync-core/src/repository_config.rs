//! The `branchdeck.*` git config of a repository that Settings shows and writes: stacked branches, the symbol mode, the
//! GitHub repository and the branch prefix (Settings → Repository), the resolve by words (Diffs & Merge), the push size
//! limit (Patronus), the route into master (`branchdeck.landing`, spec/branch/landing-routes.spec.md), how coding
//! agents work (Settings → Agents, read by `bd workspace`), and the experiments of the review windows
//! (`branchdeck.reviewExperimental`, spec/branch/review-plan.spec.md). One `git config --get-regexp` reads all ten with
//! their scopes; a write goes to the local config (`.git/config`, shared by the linked worktrees), or to the global one where
//! the request names it (`WriteScope`; never a `push` route), and `bd` reads the same keys.

use crate::branch_prefix::BRANCH_PREFIX_CONFIG_KEY;
use crate::config_snapshot::{SYMBOLS_CONFIG_KEY, SymbolMode};
use crate::github_repository::{GITHUB_REPOSITORY_CONFIG, GitHubRepositorySource, detect_github_repository_of};
use crate::patronus::parse_space_repository;
use crate::push_size::{PUSH_SIZE_LIMIT_CONFIG_KEY, parse_push_size_limit};
use crate::remote_url::origin_push_url;
use crate::remote_url::split_owner_and_name;
use anyhow::{Result, anyhow, bail};
use domain_types::branch_names::{is_valid_branch_prefix, normalize_branch_prefix};
use domain_types::landing_route::{LANDING_CONFIG_KEY, LandingSetting};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::get_commit_list_with_handler;
use history_rewrite::baseline::detect_baseline_branch;
use history_rewrite::commit_grouper::{CommitGrouper, GroupingInputs};
use history_rewrite::issue_groups::ISSUE_GROUPS;
use history_rewrite::stack::STACKS_CONFIG_KEY;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, LazyLock, Mutex, PoisonError};
use std::time::{Duration, Instant};
use sync_utils::git_bool::parse_git_bool;
use tracing::{instrument, warn};

/// The ten keys, lowercase as `--get-regexp` prints them; the regexp matches the canonical (lowercase) names
const CONFIG_KEYS_REGEXP: &str =
  r"^branchdeck\.(stacks|symbols|githubrepository|branchprefix|pushsizelimit|resolvebywords|landing|agentlease|agentsubjectbranch|reviewexperimental)$";

/// Why a `push` route of the global or the system config is taken as unset: a push into master of every repository of
/// the user that names no route of its own would be a trap
pub const GLOBAL_PUSH_ROUTE_REASON: &str = "a push is taken only from .git/config of the repository, never from the global config";

/// Whether a sync resolves the conflicts of git by words (`git_ops::word_resolve`), a git boolean, on when unset
pub const RESOLVE_BY_WORDS_CONFIG_KEY: &str = "branchdeck.resolveByWords";

/// Whether a coding agent works in a worktree of its own, leased from the pool (`bd lease`), and lands each commit with
/// `bd pick`; a git boolean, on when unset (`bd workspace`)
pub const AGENT_LEASE_CONFIG_KEY: &str = "branchdeck.agentLease";

/// Whether a coding agent starts each commit subject with the `(branch)` it belongs to; a git boolean, on when unset:
/// the prefix groups the commit, and the copies of the virtual branches drop it; a repository whose commits land as
/// they are turns it off (`bd workspace`)
pub const AGENT_SUBJECT_BRANCH_CONFIG_KEY: &str = "branchdeck.agentSubjectBranch";

/// Whether the review windows show the review plan (spec/branch/review-plan.spec.md, Status: Experimental): Mark
/// Reviewed and the local Since Your Review, the Review Summary card, the badges and the Mechanical group of the tree.
/// A git boolean, on when unset, and a value git refuses is off: an experiment never turns on from a broken value.
/// Settings → Diffs & Merge → Review writes it; `bd review-plan` and `bd mark-reviewed` work without it
pub const REVIEW_EXPERIMENTAL_CONFIG_KEY: &str = "branchdeck.reviewExperimental";

/// The config file an entry comes from, in git's order: a later scope wins over an earlier one
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ConfigScope {
  System,
  Global,
  /// `.git/config` of the repository, shared by its linked worktrees: the only one Branch Deck writes
  Local,
  /// `config.worktree` of one worktree (`extensions.worktreeConfig`)
  Worktree,
  /// `git -c` or `GIT_CONFIG_*` of the process; also a scope git names `unknown`
  Command,
}

impl ConfigScope {
  /// The scope `--show-scope` prints
  pub fn parse(scope: &str) -> Self {
    match scope {
      "system" => Self::System,
      "global" => Self::Global,
      "local" => Self::Local,
      "worktree" => Self::Worktree,
      _ => Self::Command,
    }
  }

  /// A value of this scope wins over the local config, so a local write would not change the effective value
  pub const fn overrides_local(self) -> bool {
    matches!(self, Self::Worktree | Self::Command)
  }

  /// A value of the repository itself: `.git/config`, the worktree config, or `git -c` and `GIT_CONFIG_*` of the process
  /// that runs git, which win over every config file; such a branch prefix wins over the app's
  pub const fn is_repository_scoped(self) -> bool {
    matches!(self, Self::Local | Self::Worktree | Self::Command)
  }
}

/// One line of `git config -z --show-scope --get-regexp`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigEntry {
  pub scope: ConfigScope,
  /// Lowercase, as git canonicalizes it
  pub key: String,
  /// None for a key without `=` (an implicit `true`)
  pub value: Option<String>,
}

/// A key as git resolves it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ConfigSetting<T> {
  /// The effective value; the default when no config sets the key or its value is invalid (the set text for a string)
  pub value: T,
  /// The scope of the effective entry; None when the default applies
  pub scope: Option<ConfigScope>,
  /// The local config sets the key, so unsetting it there changes something
  pub local_set: bool,
  /// Why the effective value is not taken, as git or Branch Deck would refuse it
  pub invalid: Option<String>,
  /// The value once the local config no longer sets the key: the last system or global entry, else the default
  pub fallback_value: T,
  /// The scope of `fallback_value`; None for the default
  pub fallback_scope: Option<ConfigScope>,
}

/// The GitHub repository found without `branchdeck.githubRepository`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DetectedGitHubRepository {
  /// `owner/name`
  pub full_name: String,
  pub source: GitHubRepositorySource,
}

/// The settings of Settings → Repository
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RepositoryConfig {
  /// `branchdeck.stacks`, off by default
  pub stacks: ConfigSetting<bool>,
  /// `branchdeck.symbols`, on by default
  pub symbols: ConfigSetting<SymbolMode>,
  /// `branchdeck.githubRepository` (`owner/name`)
  pub github_repository: ConfigSetting<Option<String>>,
  /// `branchdeck.branchPrefix`, normalized as the branches are named
  pub branch_prefix: ConfigSetting<Option<String>>,
  /// `branchdeck.pushSizeLimit` as written (`50M`, `0`, `off`); unset is `DEFAULT_PUSH_SIZE_LIMIT` (Settings → Landing)
  pub push_size_limit: ConfigSetting<Option<String>>,
  /// `branchdeck.resolveByWords`, on by default
  pub resolve_by_words: ConfigSetting<bool>,
  /// `branchdeck.landing`: unset by default, which is Patronus on Space and the pull request elsewhere; a `push` of the
  /// global or the system config is unset, with the reason (`GLOBAL_PUSH_ROUTE_REASON`)
  pub landing: ConfigSetting<Option<LandingSetting>>,
  /// `branchdeck.agentLease`, on by default
  pub agent_lease: ConfigSetting<bool>,
  /// `branchdeck.agentSubjectBranch`, off by default
  pub agent_subject_branch: ConfigSetting<bool>,
  /// `branchdeck.reviewExperimental`, on by default, off for a value git refuses
  pub review_experimental: ConfigSetting<bool>,
  /// The repository of `origin` or of the Space mirror, which applies when `github_repository` is not set
  pub detected_github_repository: Option<DetectedGitHubRepository>,
  /// The route an unset `landing` means, by the host of `origin` (`LandingSetting::default_for`)
  pub landing_default: LandingDefault,
  /// With stacked branches on, the flat branches that turning them off would name beside a branch git cannot keep them
  /// with (`with_stacks_off_clashes`); empty with stacked branches off
  pub stacks_off_clashes: Vec<FlatNameClash>,
  /// The config writes this process had made when the read began (`config_writes`): an answer with a larger one holds
  /// more writes, so the page keeps it over a smaller one that arrives later. 0 outside `read_repository_config`; `bd`
  /// ignores it
  pub seq: u32,
}

/// The route of a repository that names none, and why: `origin` pushes to Space or not
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingDefault {
  /// Patronus on Space, the pull request elsewhere; never a push
  pub route: LandingSetting,
  /// The push URL of `origin` is a repository on Space
  pub on_space: bool,
}

impl LandingDefault {
  #[must_use]
  pub const fn of(on_space: bool) -> Self {
    Self {
      route: LandingSetting::default_for(on_space),
      on_space,
    }
  }
}

/// What Settings reads of `origin`, from its push URL (one process, and the Space settings for the mirror of a Space
/// repository): the GitHub repository it names, and whether it pushes to Space
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OriginFacts {
  pub detected_github_repository: Option<DetectedGitHubRepository>,
  pub on_space: bool,
}

/// A branch whose ref git cannot hold beside another one: `<prefix>/virtual/auth/login` needs a directory where
/// `<prefix>/virtual/auth` is a file, so a sync fails on it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FlatNameClash {
  /// The flat name, `auth/login`
  pub branch: String,
  /// The branch whose name is a directory of it, `auth`
  pub beside: String,
}

/// A key of `RepositoryConfig`, as the change event names it
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RepositoryConfigKey {
  Stacks,
  Symbols,
  GithubRepository,
  BranchPrefix,
  PushSizeLimit,
  ResolveByWords,
  Landing,
  AgentLease,
  AgentSubjectBranch,
  ReviewExperimental,
}

impl RepositoryConfigKey {
  /// The git config key, in the case Branch Deck writes it
  pub const fn config_key(self) -> &'static str {
    match self {
      Self::Stacks => STACKS_CONFIG_KEY,
      Self::Symbols => SYMBOLS_CONFIG_KEY,
      Self::GithubRepository => GITHUB_REPOSITORY_CONFIG,
      Self::BranchPrefix => BRANCH_PREFIX_CONFIG_KEY,
      Self::PushSizeLimit => PUSH_SIZE_LIMIT_CONFIG_KEY,
      Self::ResolveByWords => RESOLVE_BY_WORDS_CONFIG_KEY,
      Self::Landing => LANDING_CONFIG_KEY,
      Self::AgentLease => AGENT_LEASE_CONFIG_KEY,
      Self::AgentSubjectBranch => AGENT_SUBJECT_BRANCH_CONFIG_KEY,
      Self::ReviewExperimental => REVIEW_EXPERIMENTAL_CONFIG_KEY,
    }
  }

  fn of_entry_key(key: &str) -> Option<Self> {
    [
      Self::Stacks,
      Self::Symbols,
      Self::GithubRepository,
      Self::BranchPrefix,
      Self::PushSizeLimit,
      Self::ResolveByWords,
      Self::Landing,
      Self::AgentLease,
      Self::AgentSubjectBranch,
      Self::ReviewExperimental,
    ]
    .into_iter()
    .find(|candidate| candidate.config_key().eq_ignore_ascii_case(key))
  }
}

/// The config file a write of Settings goes to
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum WriteScope {
  /// `.git/config` of the repository
  #[default]
  Local,
  /// The global config of the user (`git config --global`): every repository without a local value takes it
  Global,
}

/// A write of one key to the local config; `None` unsets it there (the global value or the default applies again)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "key", content = "value", rename_all = "camelCase")]
pub enum RepositoryConfigChange {
  Stacks(Option<bool>),
  Symbols(Option<SymbolMode>),
  GithubRepository(Option<String>),
  BranchPrefix(Option<String>),
  PushSizeLimit(Option<String>),
  ResolveByWords(Option<bool>),
  Landing(Option<LandingSetting>),
  AgentLease(Option<bool>),
  AgentSubjectBranch(Option<bool>),
  ReviewExperimental(Option<bool>),
}

impl RepositoryConfigChange {
  pub const fn key(&self) -> RepositoryConfigKey {
    match self {
      Self::Stacks(_) => RepositoryConfigKey::Stacks,
      Self::Symbols(_) => RepositoryConfigKey::Symbols,
      Self::GithubRepository(_) => RepositoryConfigKey::GithubRepository,
      Self::BranchPrefix(_) => RepositoryConfigKey::BranchPrefix,
      Self::PushSizeLimit(_) => RepositoryConfigKey::PushSizeLimit,
      Self::ResolveByWords(_) => RepositoryConfigKey::ResolveByWords,
      Self::Landing(_) => RepositoryConfigKey::Landing,
      Self::AgentLease(_) => RepositoryConfigKey::AgentLease,
      Self::AgentSubjectBranch(_) => RepositoryConfigKey::AgentSubjectBranch,
      Self::ReviewExperimental(_) => RepositoryConfigKey::ReviewExperimental,
    }
  }

  /// The value to write, validated as the reader takes it; None to unset. An empty text unsets too
  fn config_value(&self) -> Result<Option<String>> {
    Ok(match self {
      Self::Stacks(value) | Self::ResolveByWords(value) | Self::AgentLease(value) | Self::AgentSubjectBranch(value) | Self::ReviewExperimental(value) => {
        value.map(|value| value.to_string())
      }
      Self::Symbols(value) => value.map(|mode| symbol_mode_name(mode).to_string()),
      Self::Landing(value) => value.map(|setting| setting.config_value().to_string()),
      Self::GithubRepository(value) => match value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        None => None,
        Some(value) => Some(validate_github_repository(value).map_err(|reason| anyhow!(reason))?.to_string()),
      },
      Self::BranchPrefix(value) => match value.as_deref().map(normalize_branch_prefix).filter(|value| !value.is_empty()) {
        None => None,
        Some(value) => Some(validate_branch_prefix(value).map_err(|reason| anyhow!(reason))?.to_string()),
      },
      Self::PushSizeLimit(value) => match value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        None => None,
        Some(value) => Some(validate_push_size_limit(value).map_err(|reason| anyhow!(reason))?.to_string()),
      },
    })
  }
}

const fn symbol_mode_name(mode: SymbolMode) -> &'static str {
  match mode {
    SymbolMode::Off => "off",
    SymbolMode::Report => "report",
    SymbolMode::On => "on",
  }
}

/// `owner/name`, without whitespace
fn validate_github_repository(value: &str) -> Result<&str, String> {
  if value.chars().any(char::is_whitespace) || split_owner_and_name(value).is_none() {
    return Err(format!("{value} is not a GitHub repository: use owner/name"));
  }
  Ok(value)
}

/// A size (`50M`, `1G`), or `0` or `off` for no limit
fn validate_push_size_limit(value: &str) -> Result<&str, String> {
  // the words of the field (`invalidPushSizeLimitReason` in src/lib/repository-config.ts)
  if parse_push_size_limit(value).is_ok() {
    Ok(value)
  } else {
    Err(format!("{value} is not a size such as 50M or 1G, nor 0 or off"))
  }
}

fn validate_branch_prefix(value: &str) -> Result<&str, String> {
  if !is_valid_branch_prefix(value) {
    // the words of the fields (`INVALID_PREFIX_TEXT` in src/lib/branch-prefix.ts), which show the value
    return Err(r#"A prefix cannot contain spaces, "..", "@{" or any of ~ ^ : ? * [ \"#.to_string());
  }
  Ok(value)
}

/// The ten keys with their scopes, in git's order, from one `git config -z --show-scope --get-regexp`; the global
/// config alone for an empty `repository_path`
#[instrument(skip(git))]
pub fn read_config_entries(git: &GitCommandExecutor, repository_path: &str) -> Result<Vec<ConfigEntry>> {
  let (output, exit_code) = if repository_path.is_empty() {
    git.execute_command_with_status(&["config", "--global", "-z", "--show-scope", "--get-regexp", CONFIG_KEYS_REGEXP], ".")?
  } else {
    git.execute_command_with_status(&["config", "-z", "--show-scope", "--get-regexp", CONFIG_KEYS_REGEXP], repository_path)?
  };
  match exit_code {
    0 => Ok(parse_config_entries(&output)),
    // none of the keys is set
    1 => Ok(Vec::new()),
    _ => Err(anyhow!("Cannot read the git config of {repository_path}: {output}")),
  }
}

/// `<scope>\0<key>\n<value>\0` per entry, `<scope>\0<key>\0` for a key without a value
pub fn parse_config_entries(output: &str) -> Vec<ConfigEntry> {
  let mut fields = output.split('\0');
  let mut entries = Vec::new();
  while let (Some(scope), Some(key_value)) = (fields.next(), fields.next()) {
    let (key, value) = match key_value.split_once('\n') {
      Some((key, value)) => (key, Some(value.to_string())),
      None => (key_value, None),
    };
    entries.push(ConfigEntry {
      scope: ConfigScope::parse(scope),
      key: key.to_ascii_lowercase(),
      value,
    });
  }
  entries
}

/// The settings the entries resolve to (the last entry of a key wins), with what `origin` tells: the detected GitHub
/// repository and the default route
pub fn repository_config_from_entries(entries: &[ConfigEntry], origin: OriginFacts) -> RepositoryConfig {
  RepositoryConfig {
    stacks: resolve_setting(entries, RepositoryConfigKey::Stacks, false, |value| bool_setting(value, false)),
    symbols: resolve_setting(entries, RepositoryConfigKey::Symbols, SymbolMode::default(), |value| {
      match SymbolMode::parse(value.unwrap_or_default()) {
        Some(mode) => (mode, None),
        None => (SymbolMode::default(), Some(format!("{} is not off, report or on", value.unwrap_or_default()))),
      }
    }),
    github_repository: resolve_setting(entries, RepositoryConfigKey::GithubRepository, None, |value| {
      text_setting(value.map(str::trim), validate_github_repository)
    }),
    branch_prefix: resolve_setting(entries, RepositoryConfigKey::BranchPrefix, None, |value| {
      text_setting(value.map(normalize_branch_prefix), validate_branch_prefix)
    }),
    push_size_limit: resolve_setting(entries, RepositoryConfigKey::PushSizeLimit, None, |value| {
      text_setting(value.map(str::trim), validate_push_size_limit)
    }),
    resolve_by_words: resolve_setting(entries, RepositoryConfigKey::ResolveByWords, true, |value| bool_setting(value, true)),
    landing: landing_setting(entries),
    agent_lease: resolve_setting(entries, RepositoryConfigKey::AgentLease, true, |value| bool_setting(value, true)),
    agent_subject_branch: resolve_setting(entries, RepositoryConfigKey::AgentSubjectBranch, true, |value| bool_setting(value, true)),
    review_experimental: resolve_setting(entries, RepositoryConfigKey::ReviewExperimental, true, |value| bool_setting(value, false)),
    detected_github_repository: origin.detected_github_repository,
    landing_default: LandingDefault::of(origin.on_space),
    stacks_off_clashes: Vec::new(),
    seq: 0,
  }
}

/// `branchdeck.landing`: unset or empty is none, a value other than the three is invalid; a `push` that no config of
/// the repository itself sets (the global or the system one) is none, with the reason, and so is a `push` it falls back
/// to
fn landing_setting(entries: &[ConfigEntry]) -> ConfigSetting<Option<LandingSetting>> {
  let mut setting = resolve_setting(entries, RepositoryConfigKey::Landing, None, |value| {
    match value.map(str::trim).filter(|value| !value.is_empty()) {
      None => (None, None),
      Some(value) => match LandingSetting::parse(value) {
        Some(setting) => (Some(setting), None),
        None => (None, Some(format!("{value} is not patronus, push or pull-request"))),
      },
    }
  });
  if setting.value == Some(LandingSetting::Push) && setting.scope.is_some_and(|scope| !scope.is_repository_scoped()) {
    setting.value = None;
    setting.invalid = Some(GLOBAL_PUSH_ROUTE_REASON.to_string());
  }
  if setting.fallback_value == Some(LandingSetting::Push) {
    setting.fallback_value = None;
  }
  setting
}

/// A git boolean, `default` for what git refuses, with the reason
fn bool_setting(value: Option<&str>, default: bool) -> (bool, Option<String>) {
  match parse_git_bool(value) {
    Some(enabled) => (enabled, None),
    None => (default, Some(format!("{} is not a boolean (true or false)", value.unwrap_or_default()))),
  }
}

/// `parse` turns an entry's value into the setting's value and the reason it is invalid
fn resolve_setting<T: Clone>(entries: &[ConfigEntry], key: RepositoryConfigKey, default: T, parse: impl Fn(Option<&str>) -> (T, Option<String>)) -> ConfigSetting<T> {
  let of_key = || entries.iter().filter(|entry| RepositoryConfigKey::of_entry_key(&entry.key) == Some(key));
  let local_set = of_key().any(|entry| entry.scope == ConfigScope::Local);
  let (fallback_value, fallback_scope) = match of_key().rfind(|entry| matches!(entry.scope, ConfigScope::System | ConfigScope::Global)) {
    Some(entry) => (parse(entry.value.as_deref()).0, Some(entry.scope)),
    None => (default.clone(), None),
  };
  let Some(effective) = of_key().next_back() else {
    return ConfigSetting {
      value: default,
      scope: None,
      local_set,
      invalid: None,
      fallback_value,
      fallback_scope,
    };
  };
  let (value, invalid) = parse(effective.value.as_deref());
  ConfigSetting {
    value,
    scope: Some(effective.scope),
    local_set,
    invalid,
    fallback_value,
    fallback_scope,
  }
}

/// An empty or missing text is None; an invalid one keeps its text, so the field shows what is set
fn text_setting(value: Option<&str>, validate: fn(&str) -> Result<&str, String>) -> (Option<String>, Option<String>) {
  match value.filter(|value| !value.is_empty()) {
    None => (None, None),
    Some(value) => (Some(value.to_string()), validate(value).err()),
  }
}

/// The settings of the repository: one `git config`, and for the detected GitHub repository one or two more (the
/// push URL of `origin`, the Space settings), with stacked branches on the clashes of turning them off; the global
/// config alone for an empty `repository_path`. `seq` is taken before the config is read, so the answer holds every
/// write it counts
#[instrument(skip(git))]
pub fn read_repository_config(git: &GitCommandExecutor, repository_path: &str) -> Result<RepositoryConfig> {
  let seq = config_writes();
  let entries = read_config_entries(git, repository_path)?;
  if repository_path.is_empty() {
    return Ok(RepositoryConfig {
      seq,
      ..repository_config_from_entries(&entries, OriginFacts::default())
    });
  }
  let config = RepositoryConfig {
    seq,
    ..repository_config_from_entries(&entries, read_origin_facts(git, repository_path)?)
  };
  Ok(with_stacks_off_clashes(git, repository_path, config))
}

/// `config` with `stacks_off_clashes`, read only while stacked branches are on (`flat_name_clashes`). A failed read (no
/// baseline yet) is no clash: the sync says what fails
pub fn with_stacks_off_clashes(git: &GitCommandExecutor, repository_path: &str, mut config: RepositoryConfig) -> RepositoryConfig {
  if config.stacks.value {
    config.stacks_off_clashes = flat_name_clashes(git, repository_path).unwrap_or_else(|error| {
      warn!(repository_path, error = %error, "Cannot group the commits as with stacked branches off");
      Vec::new()
    });
  }
  config
}

/// The branches whose refs git cannot hold beside another one when the commits ahead of the baseline are grouped as
/// with stacked branches off: `(auth/login)` is then the flat `auth/login`, beside the `auth` of `(auth)`, and a sync
/// fails (`turning_stacks_off_fails_the_sync_on_the_flat_child` of `stack_sync_test.rs`). The grouping of a sync with
/// the stack setting off: baseline detection, one `cat-file --batch` for the issue groups, one `log`
#[instrument(skip(git))]
pub fn flat_name_clashes(git: &GitCommandExecutor, repository_path: &str) -> Result<Vec<FlatNameClash>> {
  let baseline_branch = detect_baseline_branch(git, repository_path)?;
  let inputs = GroupingInputs {
    stacks: false,
    issue_groups: ISSUE_GROUPS.read(git, repository_path)?,
    ..GroupingInputs::default()
  };
  let mut grouper = CommitGrouper::new_with(&inputs);
  get_commit_list_with_handler(git, repository_path, &baseline_branch, |commit| {
    grouper.add_commit(commit);
    Ok(())
  })?;
  let names: BTreeSet<String> = grouper.finish().branches.into_keys().collect();
  Ok(
    names
      .iter()
      .filter_map(|name| {
        // the shortest branch whose name is a directory of this one
        let beside = name.match_indices('/').map(|(end, _)| &name[..end]).find(|directory| names.contains(*directory))?;
        Some(FlatNameClash {
          branch: name.clone(),
          beside: beside.to_string(),
        })
      })
      .collect(),
  )
}

/// What Settings shows of `origin` (`OriginFacts`): one read of its push URL for the detected GitHub repository
/// (`detect_github_repository_source`) and for the host of the default route
pub fn read_origin_facts(git: &GitCommandExecutor, repository_path: &str) -> Result<OriginFacts> {
  let Some(url) = origin_push_url(git, repository_path)? else {
    return Ok(OriginFacts::default());
  };
  let detected = detect_github_repository_of(git, repository_path, &url)?.map(|(repository, source)| DetectedGitHubRepository {
    full_name: repository.full_name(),
    source,
  });
  Ok(OriginFacts {
    detected_github_repository: detected,
    on_space: parse_space_repository(&url).is_some(),
  })
}

/// `write_repository_config`, then the settings read again
#[instrument(skip(git))]
pub fn set_repository_config(git: &GitCommandExecutor, repository_path: &str, change: &RepositoryConfigChange) -> Result<RepositoryConfig> {
  write_repository_config(git, repository_path, change)?;
  read_repository_config(git, repository_path)
}

/// The write of `scope`: `set_repository_config` for the local config; for the global one `write_global_config`, then
/// the settings of the repository read again (of the global config alone for an empty `repository_path`)
#[instrument(skip(git))]
pub fn set_config(git: &GitCommandExecutor, repository_path: &str, change: &RepositoryConfigChange, scope: WriteScope) -> Result<RepositoryConfig> {
  match scope {
    WriteScope::Local => set_repository_config(git, repository_path, change),
    WriteScope::Global => {
      write_global_config(git, change)?;
      read_repository_config(git, repository_path)
    }
  }
}

/// Write one key to the global config of the user (`git config --global --replace-all`, or `--unset-all`). A local
/// value of a repository still wins there; the read of its settings shows that scope. A `push` route is refused: only
/// a repository's own config names it
pub fn write_global_config(git: &GitCommandExecutor, change: &RepositoryConfigChange) -> Result<()> {
  if *change == RepositoryConfigChange::Landing(Some(LandingSetting::Push)) {
    bail!("{LANDING_CONFIG_KEY}: {GLOBAL_PUSH_ROUTE_REASON}");
  }
  run_config_write(git, ".", "--global", change.key().config_key(), change.config_value()?.as_deref())
}

/// The exit code of `git config` when another writer holds the lock of the file (`config.lock`, `CONFIG_NO_LOCK`)
const CONFIG_LOCKED_EXIT_CODE: i32 = 255;

/// How long a write tries again while the config file is locked, as long as a write of another process
/// (`bd describe`, a `git config` of the user, a write of another worktree) takes at worst. Git 2.56 waits about as
/// long itself; 2.54 and 2.55 fail at once
const CONFIG_LOCK_WAIT: Duration = Duration::from_secs(1);

/// One mutex per config file this process writes (the local config by the repository's path, the global one by an
/// empty key): two writes of the app (a prefix saved on blur, then a switch) run one after the other instead of racing
/// for `config.lock`. A writer of another process is waited for up to `CONFIG_LOCK_WAIT`
fn config_file_writes(key: &str) -> Arc<Mutex<()>> {
  static WRITES: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = LazyLock::new(Mutex::default);
  let mut writes = WRITES.lock().unwrap_or_else(PoisonError::into_inner);
  Arc::clone(writes.entry(key.to_string()).or_default())
}

/// The writes of a config file (`run_config_write`) this process has made: one counter for the local and the global
/// files, as an answer of the settings of a repository holds both
static CONFIG_WRITES: AtomicU32 = AtomicU32::new(0);

/// How many config writes this process has made (`RepositoryConfig.seq`): a read that takes it before it reads the
/// config holds at least those writes
fn config_writes() -> u32 {
  CONFIG_WRITES.load(Ordering::Acquire)
}

/// `git config <file option> --replace-all <key> <value>`, or `--unset-all` for `None`: a key that is not set is no
/// error to unset. One write of the file at a time in this process, and a file that another process holds locked is
/// tried again for about a second; a write done counts in `CONFIG_WRITES` before the next write of the file starts
fn run_config_write(git: &GitCommandExecutor, repository_path: &str, file: &str, config_key: &str, value: Option<&str>) -> Result<()> {
  let args: Vec<&str> = match value {
    Some(value) => vec!["config", file, "--replace-all", config_key, value],
    None => vec!["config", file, "--unset-all", config_key],
  };
  let file_writes = config_file_writes(if file == "--global" { "" } else { repository_path });
  let _write = file_writes.lock().unwrap_or_else(PoisonError::into_inner);
  let deadline = Instant::now() + CONFIG_LOCK_WAIT;
  let mut wait = Duration::from_millis(10);
  loop {
    let (output, exit_code) = git.execute_command_with_status(&args, repository_path)?;
    // 5: the key to unset is not set
    if exit_code == 0 || (value.is_none() && exit_code == 5) {
      CONFIG_WRITES.fetch_add(1, Ordering::Release);
      return Ok(());
    }
    if exit_code != CONFIG_LOCKED_EXIT_CODE || Instant::now() + wait > deadline {
      bail!("git config {config_key} failed: {output}");
    }
    std::thread::sleep(wait);
    wait = (wait * 2).min(Duration::from_millis(200));
  }
}

/// Write one key to the local config of the repository (`git config --local --replace-all`, or `--unset-all`), after
/// one read of the scopes: refused when the worktree config or the command line sets the key, as the write would not
/// change the effective value
pub fn write_repository_config(git: &GitCommandExecutor, repository_path: &str, change: &RepositoryConfigChange) -> Result<()> {
  if repository_path.is_empty() {
    bail!("No repository is selected");
  }
  let key = change.key();
  let value = change.config_value()?;
  let config_key = key.config_key();

  let entries = read_config_entries(git, repository_path)?;
  if let Some(effective) = entries.iter().rfind(|entry| RepositoryConfigKey::of_entry_key(&entry.key) == Some(key))
    && effective.scope.overrides_local()
  {
    let place = match effective.scope {
      ConfigScope::Worktree => "in config.worktree, which overrides .git/config: change it there",
      _ => "by git -c or GIT_CONFIG_*, which overrides .git/config",
    };
    bail!("{config_key} is set {place}");
  }

  run_config_write(git, repository_path, "--local", config_key, value.as_deref())
}
