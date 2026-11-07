//! `brd companion`: the companion repositories of a primary (spec/branch/declared-needs.spec.md#companion-repositories),
//! whose branches a need names as `<companion>:<branch>`. The same write as Settings → Landing
//! (`sync_core::repository_config::write_companion`): `branchdeck.companion.<name>.path` in the local config of the
//! primary, `branchdeck.primary` in the local config of the companion

use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::Path;
use sync_core::companions::directory_name;
use sync_core::repository_config::{CompanionSetting, read_repository_config, write_companion};

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
    CompanionCommand::List => false,
  };
  let config = read_repository_config(&context.git, &context.repository)?;
  Ok(CompanionOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    companions: config.companions.into_iter().map(summary).collect(),
    primary: config.primary,
    changed,
  })
}

/// The write, refused as `invalid_arguments` with the reason of `write_companion`; the command's config is read again
/// after it
fn write(context: &RepoContext, name: &str, path: Option<&str>) -> Result<(), CliError> {
  let written = write_companion(&context.git, &context.repository, name, path);
  context.forget_config();
  written.map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))
}

fn summary(setting: CompanionSetting) -> CompanionSummary {
  CompanionSummary {
    name: setting.name,
    path: setting.path,
    problem: setting.problem,
  }
}
