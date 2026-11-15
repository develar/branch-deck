//! `brd companion`: the companion repositories of a primary (openspec/specs/branch/declared-needs/spec.md#companion-repositories),
//! whose branches a need names as `<companion>:<branch>`. The same write as Settings → Landing
//! (`sync_core::companion_write::write_companion`, under the companion locks of both repositories):
//! `branchdeck.companion.<name>.path` in the local config of the primary, `branchdeck.primary` in the local config of the
//! companion

use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use history_rewrite::repository_lock::RepositoryLocked;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::Path;
use sync_core::companion_write::{unlink_primary, write_companion};
use sync_core::companions::directory_name;
use sync_core::repository_config::{CompanionSetting, read_repository_config};

#[derive(Debug, clap::Args)]
pub struct CompanionArgs {
  #[command(subcommand)]
  pub command: CompanionCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum CompanionCommand {
  /// Add the repository at <path> as a companion of this one, its primary; --name defaults to the name of its directory
  Add(CompanionAddArgs),
  /// Remove the companion <name>: its key here, and the primary of the companion while it names this repository
  Remove(CompanionRemoveArgs),
  /// In a companion, unlink the primary it names (its `branchdeck.primary`): the side of a half link on it, its primary
  /// gone, not readable or not listing it; refused while the primary lists it (remove it there)
  Unlink,
  /// List the companions of this repository, or its primary when it is a companion. Read only
  List,
}

#[derive(Debug, clap::Args)]
pub struct CompanionAddArgs {
  /// The repository, any worktree of it: its main worktree is written
  pub path: String,
  /// The name a need gives it (`<name>:<branch>`): letters, digits, -, _ and .
  #[arg(long)]
  pub name: Option<String>,
}

#[derive(Debug, clap::Args)]
pub struct CompanionRemoveArgs {
  pub name: String,
}

/// `brd companion`
#[derive(Debug, Serialize, JsonSchema)]
pub struct CompanionOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// The companions of this repository, by name
  pub companions: Vec<CompanionSummary>,
  /// The primary of this repository when it is a companion
  #[serde(skip_serializing_if = "Option::is_none")]
  pub primary: Option<String>,
  /// Why the link to `primary` is broken: the primary is gone, or does not list this repository (an add there links it)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub primary_problem: Option<String>,
  /// Whether this call wrote the config
  pub changed: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CompanionSummary {
  pub name: String,
  /// Its main worktree
  pub path: String,
  /// Why it can not be read (its path is gone, it is no git repository): its needs refuse with this reason
  #[serde(skip_serializing_if = "Option::is_none")]
  pub problem: Option<String>,
  /// Why its link is half: it names another primary, or none (an add links both sides, a removal unlists it)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub link_problem: Option<String>,
}

pub fn companion(context: &RepoContext, args: &CompanionArgs) -> Result<CompanionOutput, CliError> {
  let changed = match &args.command {
    CompanionCommand::Add(add) => {
      let name = add.name.clone().unwrap_or_else(|| directory_name(add.path.trim_end_matches(['/', '\\'])));
      if !Path::new(&add.path).is_dir() {
        return Err(CliError::new(ErrorCode::InvalidArguments, format!("{} is no directory", add.path)));
      }
      write(context, &name, Some(&add.path))?;
      true
    }
    CompanionCommand::Remove(remove) => {
      write(context, &remove.name, None)?;
      true
    }
    CompanionCommand::Unlink => {
      let unlinked = unlink_primary(&context.git, &context.repository);
      context.forget_config();
      unlinked.map_err(|error| refusal(&error))?;
      true
    }
    CompanionCommand::List => false,
  };
  let config = read_repository_config(&context.git, &context.repository)?;
  Ok(CompanionOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    companions: config.companions.into_iter().map(summary).collect(),
    primary: config.primary,
    primary_problem: config.primary_problem,
    changed,
  })
}

/// The write, refused as `invalid_arguments` with the reason of `write_companion`, `locked` while another process held a
/// companion lock past its wait; the command's config is read again after it
fn write(context: &RepoContext, name: &str, path: Option<&str>) -> Result<(), CliError> {
  let written = write_companion(&context.git, &context.repository, name, path);
  context.forget_config();
  written.map_err(|error| refusal(&error))
}

/// The refusal of a write of a companion: `locked` for a companion lock another process held past the wait, else
/// `invalid_arguments` with the reason
fn refusal(error: &anyhow::Error) -> CliError {
  match error.downcast_ref::<RepositoryLocked>() {
    Some(locked) => CliError::new(ErrorCode::Locked, locked.to_string()),
    None => CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")),
  }
}

fn summary(setting: CompanionSetting) -> CompanionSummary {
  CompanionSummary {
    name: setting.name,
    path: setting.path,
    problem: setting.problem,
    link_problem: setting.link_problem,
  }
}
