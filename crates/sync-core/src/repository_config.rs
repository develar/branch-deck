//! The `branchdeck.*` git config of a repository that Settings → Repository shows and writes: stacked branches, the
//! symbol mode, the GitHub repository and the branch prefix. One `git config --get-regexp` reads all four with their
//! scopes; a write goes to the local config (`.git/config`, shared by the linked worktrees) only, and `bd` reads the same
//! keys.

use crate::baseline::detect_baseline_branch;
use crate::branch_prefix::BRANCH_PREFIX_CONFIG_KEY;
use crate::commit_grouper::{CommitGrouper, GroupingInputs};
use crate::issue_groups::ISSUE_GROUPS;
use crate::pull_requests::{GITHUB_REPOSITORY_CONFIG, GitHubRepositorySource, detect_github_repository_source};
use crate::remote_url::split_owner_and_name;
use crate::stack::STACKS_CONFIG_KEY;
use crate::symbol_scan::{SYMBOLS_CONFIG_KEY, SymbolMode};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{is_valid_branch_prefix, normalize_branch_prefix};
use git_ops::commit_list::get_commit_list_with_handler;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use tracing::{instrument, warn};

/// The four keys, lowercase as `--get-regexp` prints them; the regexp matches the canonical (lowercase) names
const CONFIG_KEYS_REGEXP: &str = r"^branchdeck\.(stacks|symbols|githubrepository|branchprefix)$";

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
  /// The repository of `origin` or of the Space mirror, which applies when `github_repository` is not set
  pub detected_github_repository: Option<DetectedGitHubRepository>,
  /// With stacked branches on, the flat branches that turning them off would name beside a branch git cannot keep them
  /// with (`with_stacks_off_clashes`); empty with stacked branches off
  pub stacks_off_clashes: Vec<FlatNameClash>,
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
}

impl RepositoryConfigKey {
  /// The git config key, in the case Branch Deck writes it
  pub const fn config_key(self) -> &'static str {
    match self {
      Self::Stacks => STACKS_CONFIG_KEY,
      Self::Symbols => SYMBOLS_CONFIG_KEY,
      Self::GithubRepository => GITHUB_REPOSITORY_CONFIG,
      Self::BranchPrefix => BRANCH_PREFIX_CONFIG_KEY,
    }
  }

  fn of_entry_key(key: &str) -> Option<Self> {
    [Self::Stacks, Self::Symbols, Self::GithubRepository, Self::BranchPrefix]
      .into_iter()
      .find(|candidate| candidate.config_key().eq_ignore_ascii_case(key))
  }
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
}

impl RepositoryConfigChange {
  pub const fn key(&self) -> RepositoryConfigKey {
    match self {
      Self::Stacks(_) => RepositoryConfigKey::Stacks,
      Self::Symbols(_) => RepositoryConfigKey::Symbols,
      Self::GithubRepository(_) => RepositoryConfigKey::GithubRepository,
      Self::BranchPrefix(_) => RepositoryConfigKey::BranchPrefix,
    }
  }

  /// The value to write, validated as the reader takes it; None to unset. An empty text unsets too
  fn config_value(&self) -> Result<Option<String>> {
    Ok(match self {
      Self::Stacks(value) => value.map(|value| value.to_string()),
      Self::Symbols(value) => value.map(|mode| symbol_mode_name(mode).to_string()),
      Self::GithubRepository(value) => match value.as_deref().map(str::trim).filter(|value| !value.is_empty()) {
        None => None,
        Some(value) => Some(validate_github_repository(value).map_err(|reason| anyhow!(reason))?.to_string()),
      },
      Self::BranchPrefix(value) => match value.as_deref().map(normalize_branch_prefix).filter(|value| !value.is_empty()) {
        None => None,
        Some(value) => Some(validate_branch_prefix(value).map_err(|reason| anyhow!(reason))?.to_string()),
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

fn validate_branch_prefix(value: &str) -> Result<&str, String> {
  if !is_valid_branch_prefix(value) {
    // the words of the fields (`INVALID_PREFIX_TEXT` in src/lib/branch-prefix.ts), which show the value
    return Err(r#"A prefix cannot contain spaces, "..", "@{" or any of ~ ^ : ? * [ \"#.to_string());
  }
  Ok(value)
}

/// The four keys with their scopes, in git's order, from one `git config -z --show-scope --get-regexp`; the global
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

/// A git boolean as `git config --type=bool` reads it: `true`/`yes`/`on`, `false`/`no`/`off` (any case, no white space
/// around), the empty text is false, a key without a value is true, else an integer of git's (0 is false); `None` for
/// what git refuses
pub fn parse_git_bool(value: Option<&str>) -> Option<bool> {
  let Some(value) = value else {
    return Some(true);
  };
  if ["true", "yes", "on"].iter().any(|word| value.eq_ignore_ascii_case(word)) {
    return Some(true);
  }
  if value.is_empty() || ["false", "no", "off"].iter().any(|word| value.eq_ignore_ascii_case(word)) {
    return Some(false);
  }
  parse_git_int(value).map(|number| number != 0)
}

/// An integer as git's config reads it (`git_parse_int`): `strtoimax` with base 0 (white space before, a sign, `0x`
/// for hex, `0` for octal), then an optional unit `k`, `m` or `g` (any case, 1024-based), within the range of a C `int`.
/// C17's base 0, not C23's: `0b` is not a binary prefix, as in the git of macOS and Windows; the git of glibc 2.38+
/// reads `0b1` as 1, but which libc the user's git has is unknown, so `0b1` is refused everywhere.
fn parse_git_int(value: &str) -> Option<i64> {
  // C's `isspace`, which has the vertical tab too
  let rest = value.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
  let (negative, rest) = match rest.strip_prefix('-') {
    Some(rest) => (true, rest),
    None => (false, rest.strip_prefix('+').unwrap_or(rest)),
  };
  let hex = rest
    .strip_prefix("0x")
    .or_else(|| rest.strip_prefix("0X"))
    .filter(|hex| hex.starts_with(|c: char| c.is_ascii_hexdigit()));
  let (radix, digits) = match hex {
    Some(hex) => (16, hex),
    None if rest.starts_with('0') => (8, rest),
    None => (10, rest),
  };
  let end = digits.find(|c: char| !c.is_digit(radix)).unwrap_or(digits.len());
  if end == 0 {
    return None;
  }
  let number = i128::from_str_radix(&digits[..end], radix).ok()?;
  let factor = match &digits[end..] {
    "" => 1,
    unit if unit.eq_ignore_ascii_case("k") => 1 << 10,
    unit if unit.eq_ignore_ascii_case("m") => 1 << 20,
    unit if unit.eq_ignore_ascii_case("g") => 1 << 30,
    _ => return None,
  };
  let number = (if negative { -number } else { number }).checked_mul(factor)?;
  i32::try_from(number).ok().map(i64::from)
}

/// The settings the entries resolve to (the last entry of a key wins), with the detected GitHub repository
pub fn repository_config_from_entries(entries: &[ConfigEntry], detected_github_repository: Option<DetectedGitHubRepository>) -> RepositoryConfig {
  RepositoryConfig {
    stacks: resolve_setting(entries, RepositoryConfigKey::Stacks, false, |value| match parse_git_bool(value) {
      Some(enabled) => (enabled, None),
      None => (false, Some(format!("{} is not a boolean (true or false)", value.unwrap_or_default()))),
    }),
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
    detected_github_repository,
    stacks_off_clashes: Vec::new(),
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
/// config alone for an empty `repository_path`
#[instrument(skip(git))]
pub fn read_repository_config(git: &GitCommandExecutor, repository_path: &str) -> Result<RepositoryConfig> {
  let entries = read_config_entries(git, repository_path)?;
  if repository_path.is_empty() {
    return Ok(repository_config_from_entries(&entries, None));
  }
  let config = repository_config_from_entries(&entries, detect_github_repository(git, repository_path)?);
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

/// `detect_github_repository_source` as Settings → Repository shows it
pub fn detect_github_repository(git: &GitCommandExecutor, repository_path: &str) -> Result<Option<DetectedGitHubRepository>> {
  Ok(detect_github_repository_source(git, repository_path)?.map(|(repository, source)| DetectedGitHubRepository {
    full_name: repository.full_name(),
    source,
  }))
}

/// `write_repository_config`, then the settings read again
#[instrument(skip(git))]
pub fn set_repository_config(git: &GitCommandExecutor, repository_path: &str, change: &RepositoryConfigChange) -> Result<RepositoryConfig> {
  write_repository_config(git, repository_path, change)?;
  read_repository_config(git, repository_path)
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

  let (output, exit_code) = match &value {
    Some(value) => git.execute_command_with_status(&["config", "--local", "--replace-all", config_key, value], repository_path)?,
    None => git.execute_command_with_status(&["config", "--local", "--unset-all", config_key], repository_path)?,
  };
  // 5: the key to unset is not set
  if !(exit_code == 0 || (value.is_none() && exit_code == 5)) {
    bail!("git config {config_key} failed: {output}");
  }
  Ok(())
}
