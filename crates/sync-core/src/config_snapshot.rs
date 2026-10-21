//! The git config an operation reads, with one `git config` process: the `branchdeck.*` settings of
//! `repository_config` and the branch descriptions of `bd describe`. An operation reads it once and passes it on,
//! instead of one process per key (docs/design/git-operations.md, "Repository settings").

use crate::branch_prefix::GitBranchPrefix;
use crate::pull_requests::GitHubRepository;
use crate::remote_url::split_owner_and_name;
use crate::repository_config::{ConfigEntry, RESOLVE_BY_WORDS_CONFIG_KEY, RepositoryConfig, parse_config_entries, repository_config_from_entries};
use crate::symbol_scan::{SYMBOLS_CONFIG_KEY, SymbolMode};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_description::Description;
use git_ops::word_resolve::ResolveMode;
use history_rewrite::stack::STACKS_CONFIG_KEY;
use std::collections::HashMap;
use tracing::instrument;

/// The five `branchdeck.*` keys of `repository_config` that an operation reads (the agent keys only `bd workspace` and
/// Settings read) and every `branch.<name>.description`, as `--get-regexp` matches them: the section and the key
/// lowercase, the branch name as it is
const SNAPSHOT_REGEXP: &str = r"^(branchdeck\.(stacks|symbols|githubrepository|branchprefix|resolvebywords)|branch\..+\.description)$";

/// The keys of one `git config -z --show-scope --get-regexp`, in git's order (a later entry wins)
#[derive(Debug, Clone, Default)]
pub struct ConfigSnapshot {
  entries: Vec<ConfigEntry>,
  /// Full ref -> description of every branch that has one
  descriptions: HashMap<String, Description>,
}

impl ConfigSnapshot {
  /// One `git config -z --show-scope --get-regexp`
  #[instrument(skip(git))]
  pub fn read(git: &GitCommandExecutor, repository_path: &str) -> Result<Self> {
    let (output, exit_code) = git.execute_command_with_status(&["config", "-z", "--show-scope", "--get-regexp", SNAPSHOT_REGEXP], repository_path)?;
    match exit_code {
      0 => Ok(Self::parse(&output)),
      // none of the keys is set
      1 => Ok(Self::default()),
      _ => Err(anyhow!("Cannot read the git config of {repository_path}: {}", output.trim())),
    }
  }

  /// `<scope>\0<key>\n<value>\0` per entry, `<scope>\0<key>\0` for a key without a value
  fn parse(output: &str) -> Self {
    let mut descriptions = HashMap::new();
    let mut fields = output.split('\0');
    while let (Some(_scope), Some(key_value)) = (fields.next(), fields.next()) {
      // `branch.` and `.description` are lowercase, the branch name keeps its case
      if let Some((key, value)) = key_value.split_once('\n')
        && let Some(name) = key.strip_prefix("branch.").and_then(|key| key.strip_suffix(".description"))
      {
        descriptions.insert(format!("refs/heads/{name}"), Description::parse(value));
      }
    }
    let entries = parse_config_entries(output).into_iter().filter(|entry| entry.key.starts_with("branchdeck.")).collect();
    Self { entries, descriptions }
  }

  /// The settings as Settings → Repository resolves them, without the detected GitHub repository
  pub fn settings(&self) -> RepositoryConfig {
    repository_config_from_entries(&self.entries, None)
  }

  /// `branchdeck.stacks`, a git boolean, off when unset; an error for a value git would refuse
  pub fn stacks(&self) -> Result<bool> {
    let stacks = self.settings().stacks;
    match stacks.invalid {
      None => Ok(stacks.value),
      Some(_) => Err(anyhow!("{STACKS_CONFIG_KEY} is not a boolean (true or false)")),
    }
  }

  /// `branchdeck.resolveByWords`, a git boolean, on when unset; an error for a value git would refuse
  pub fn resolve_mode(&self) -> Result<ResolveMode> {
    let setting = self.settings().resolve_by_words;
    match setting.invalid {
      None if setting.value => Ok(ResolveMode::On),
      None => Ok(ResolveMode::Off),
      Some(_) => Err(anyhow!("{RESOLVE_BY_WORDS_CONFIG_KEY} is not a boolean (true or false)")),
    }
  }

  /// `branchdeck.symbols`, `SymbolMode::default()` when unset; an error for another value than off, report or on
  pub fn symbol_mode(&self) -> Result<SymbolMode> {
    let symbols = self.settings().symbols;
    match symbols.invalid {
      None => Ok(symbols.value),
      Some(_) => Err(anyhow!(
        "{SYMBOLS_CONFIG_KEY} is not off, report or on: {}",
        self.last_value("branchdeck.symbols").unwrap_or_default().trim()
      )),
    }
  }

  /// `branchdeck.branchPrefix` as git resolves it, trimmed, and whether the repository's own config sets it
  pub fn branch_prefix(&self) -> GitBranchPrefix {
    match self.entries.iter().rfind(|entry| entry.key == "branchdeck.branchprefix") {
      None => GitBranchPrefix::default(),
      Some(entry) => GitBranchPrefix {
        prefix: entry.value.as_deref().unwrap_or_default().trim().to_string(),
        repository_scoped: entry.scope.is_repository_scoped(),
      },
    }
  }

  /// `branchdeck.githubRepository` as `owner/name`; `None` when unset or not of that form
  pub fn github_repository(&self) -> Option<GitHubRepository> {
    let (owner, name) = split_owner_and_name(self.last_value("branchdeck.githubrepository")?.trim())?;
    Some(GitHubRepository {
      owner: owner.to_string(),
      name: name.to_string(),
    })
  }

  /// Full ref -> description of every branch that has one
  pub const fn descriptions(&self) -> &HashMap<String, Description> {
    &self.descriptions
  }

  /// The value of the last entry of `key` (lowercase)
  fn last_value(&self, key: &str) -> Option<&str> {
    self.entries.iter().rfind(|entry| entry.key == key)?.value.as_deref()
  }
}

#[cfg(test)]
#[path = "config_snapshot_test.rs"]
mod config_snapshot_test;
