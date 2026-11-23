//! A write of Settings → Repository (openspec/specs/repository/settings/spec.md): one key of the `branchdeck.*` git config of a
//! repository, local or global (`sync_core::repository_config::set_config`), or a companion repository of Settings →
//! Landing (`set_companion`, openspec/specs/branch/declared-needs/spec.md), read again for the answer; the main window hears it
//! (`repository_config_changed`) and acts for the selected repository. Each goes through the one writer of the config
//! file (`config_writer`), a write of a page of Settings in the order of its number there

use crate::api_error::IntoApiError;
use crate::app::config_writer::{ConfigWriters, page_write};
use crate::context::AppContext;
use crate::events::repository_config_changed;
use crate::operation::{operation, run_blocking};
use crate::transport::Call;
use domain_types::error::ApiError;
use domain_types::settings::{GitBranchPrefix, WriteOrder};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use sync_core::companion_write::{COMPANION_TRIES, Written, companion_moved, companion_of_write, write_companion_under};
use sync_core::repository_config::{ConfigScope, RepositoryConfig, RepositoryConfigChange, RepositoryConfigKey, WriteScope, set_config};
use tracing::{debug, instrument};

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetRepositoryConfigParams {
  pub repository_path: String,
  pub change: RepositoryConfigChange,
  /// The local config when omitted
  #[serde(default)]
  #[specta(optional)]
  pub scope: WriteScope,
  /// The page of the write and its number among the writes of that page (`nextWriteOrder`): an older write of the key of
  /// the same page than one the writer decided writes nothing. A write without one is applied in the order it came
  #[serde(default)]
  #[specta(optional)]
  pub order: Option<WriteOrder>,
}

/// Writes one key of the config through the writer of its file (`ConfigWriters`) and answers the config read again;
/// `None` for a write older than one of the same key of its page the writer decided, which writes nothing. A write of
/// the branch prefix keeps the git prefix the read found for the repository's later commands (`BranchPrefixes`); a
/// global one forgets the prefix of every other repository, which may take it
#[instrument(skip_all, fields(window = %call.window, repository_path = %params.repository_path, key = ?params.change.key(), scope = ?params.scope))]
pub async fn set_repository_config(ctx: &AppContext, call: &Call, params: SetRepositoryConfigParams) -> Result<Option<RepositoryConfig>, ApiError> {
  operation(async move {
    let SetRepositoryConfigParams {
      repository_path,
      change,
      scope,
      order,
    } = params;
    let key = change.key();
    // a write of the page without a number is applied in the order it came
    let order = order.map(|order| (page_write(call, order), format!("{scope:?}.{}", key.config_key())));
    let (git, path) = (ctx.git.clone(), repository_path.clone());
    let Some(config) = ctx
      .config_writers
      .write(&repository_path, scope, order, move || set_config(&git, &path, &change, scope))
      .await?
    else {
      return Ok(None);
    };
    if key == RepositoryConfigKey::BranchPrefix {
      if scope == WriteScope::Global {
        ctx.prefixes.clear();
      }
      let git_prefix = GitBranchPrefix {
        prefix: config.branch_prefix.value.clone().unwrap_or_default(),
        repository_scoped: config.branch_prefix.scope.is_some_and(ConfigScope::is_repository_scoped),
      };
      ctx.prefix_inputs().refreshed(&repository_path, git_prefix);
    }
    repository_config_changed(&*ctx.events, repository_path, key);
    Ok(Some(config))
  })
  .await
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SetCompanionParams {
  pub repository_path: String,
  pub name: String,
  /// The repository to add as the companion `name`, any worktree of it; `None` removes the companion
  pub path: Option<String>,
}

/// Adds or removes a companion repository (`sync_core::companion_write::write_companion`: both sides) and answers the
/// settings read again; the main window syncs again, as the states of the branches a need names change with it
#[instrument(skip_all, fields(repository_path = %params.repository_path, name = %params.name))]
pub async fn set_companion(ctx: &AppContext, params: SetCompanionParams) -> Result<RepositoryConfig, ApiError> {
  operation(async move {
    let SetCompanionParams { repository_path, name, path } = params;
    let config = write_companion(&ctx.config_writers, &ctx.git, &repository_path, name, path).await?;
    Ok(companions_changed(ctx, repository_path, config))
  })
  .await
}

/// The settings a write of the companions of the repository at `repository_path` answers, told to the main window
/// (`repository_config_changed`), which syncs again
fn companions_changed(ctx: &AppContext, repository_path: String, config: RepositoryConfig) -> RepositoryConfig {
  repository_config_changed(&*ctx.events, repository_path, RepositoryConfigKey::Companions);
  config
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UnlinkPrimaryParams {
  /// The companion repository whose side of a half link goes
  pub repository_path: String,
}

/// Unlinks the companion repository at `repository_path` from the primary its config names
/// (`sync_core::companion_write::unlink_primary`: the side of a half link on it, refused while the primary lists it) in
/// the writer of its config, and answers the settings read again; the main window syncs again, as the names a need of
/// the companion resolves change with it
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
pub async fn unlink_primary(ctx: &AppContext, params: UnlinkPrimaryParams) -> Result<RepositoryConfig, ApiError> {
  operation(async move {
    let repository_path = params.repository_path;
    let (git, path) = (ctx.git.clone(), repository_path.clone());
    let config = ctx
      .config_writers
      .write(&repository_path, WriteScope::Local, None, move || {
        sync_core::companion_write::unlink_primary(&git, &path)?;
        sync_core::repository_config::read_repository_config(&git, &path)
      })
      .await?
      .ok_or_else(|| ApiError::internal("A write without a number is never superseded"))?;
    Ok(companions_changed(ctx, repository_path, config))
  })
  .await
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AddCompanionParams {
  pub repository_path: String,
}

/// Settings → Landing → Add Companion…: the folder dialog picks the repository, named by its directory, then the write
/// of `set_companion`; `None` when the user canceled the dialog
#[instrument(skip_all, fields(repository_path = %params.repository_path, window = %call.window))]
pub async fn add_companion(ctx: &AppContext, call: &Call, params: AddCompanionParams) -> Result<Option<RepositoryConfig>, ApiError> {
  operation(async move {
    let dialog = ctx.desktop(&call.window).dialog;
    // the modal dialog blocks, off the async runtime and before the writer of the config, which it would hold
    let picked = run_blocking(move || Ok::<_, ApiError>(dialog.pick_folder("Select Companion Repository"))).await?;
    let Some(path) = picked else {
      return Ok(None);
    };
    let name = sync_core::companions::directory_name(path.trim_end_matches(['/', '\\']));
    let config = write_companion(&ctx.config_writers, &ctx.git, &params.repository_path, name, Some(path)).await?;
    Ok(Some(companions_changed(ctx, params.repository_path, config)))
  })
  .await
}

/// `sync_core::companion_write::write_companion` while it holds the writers of both configs it reads and writes
/// (`ConfigWriters::write_together`): the one of the repository at `repository_path` and the one of the companion the
/// write touches (`companion_of_write`, asked first, then once more under the companion locks), so no write of Settings to
/// either config runs inside it, then the settings read again there. A companion that changed meanwhile asks again
pub(crate) async fn write_companion(
  writers: &ConfigWriters,
  git: &GitCommandExecutor,
  repository_path: &str,
  name: String,
  path: Option<String>,
) -> Result<RepositoryConfig, ApiError> {
  for _ in 0..COMPANION_TRIES {
    let (asked_git, repository, asked_name, asked_path) = (git.clone(), repository_path.to_owned(), name.clone(), path.clone());
    let companion = run_blocking(move || companion_of_write(&asked_git, &repository, &asked_name, asked_path.as_deref())).await?;
    let of_companion = companion.clone();
    let held: Vec<&str> = std::iter::once(repository_path).chain(of_companion.as_deref()).collect();
    let (git, repository, name, path) = (git.clone(), repository_path.to_owned(), name.clone(), path.clone());
    let written = writers
      .write_together(&held, move || {
        let on_wait = || debug!("waiting for the lock of a companion another process holds");
        match write_companion_under(&git, &repository, &name, path.as_deref(), companion.as_deref(), &on_wait)? {
          Written::Done => sync_core::repository_config::read_repository_config(&git, &repository).map(Some),
          Written::Moved => Ok(None),
        }
      })
      .await?;
    if let Some(config) = written {
      return Ok(config);
    }
  }
  Err(companion_moved(repository_path, &name).into_api_error())
}
