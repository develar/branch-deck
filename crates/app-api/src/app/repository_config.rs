//! A write of Settings → Repository (openspec/specs/repository/settings/spec.md): one key of the `branchdeck.*` git config of a
//! repository, local or global (`sync_core::repository_config::set_config`), or a companion repository of Settings →
//! Landing (`set_companion`, openspec/specs/branch/declared-needs/spec.md), read again for the answer; the main window hears it
//! (`repository_config_changed`) and acts for the selected repository. Each goes through the one writer of the config
//! file (`config_writer`), a write of a page of Settings in the order of its number there

use crate::app::config_writer::page_write;
use crate::context::AppContext;
use crate::events::repository_config_changed;
use crate::operation::{operation, run_blocking};
use crate::transport::Call;
use domain_types::error::ApiError;
use domain_types::settings::{GitBranchPrefix, WriteOrder};
use serde::Deserialize;
use sync_core::repository_config::{ConfigScope, RepositoryConfig, RepositoryConfigChange, RepositoryConfigKey, WriteScope, set_config};
use tracing::instrument;

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

/// Adds or removes a companion repository (`sync_core::repository_config::write_companion`: both sides) and answers the
/// settings read again; the main window syncs again, as the states of the branches a need names change with it
#[instrument(skip_all, fields(repository_path = %params.repository_path, name = %params.name))]
pub async fn set_companion(ctx: &AppContext, params: SetCompanionParams) -> Result<RepositoryConfig, ApiError> {
  operation(async move {
    let SetCompanionParams { repository_path, name, path } = params;
    let config = write_companion(ctx, &repository_path, name, path).await?;
    repository_config_changed(&*ctx.events, repository_path, RepositoryConfigKey::Companions);
    Ok(config)
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
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
pub async fn add_companion(ctx: &AppContext, params: AddCompanionParams) -> Result<Option<RepositoryConfig>, ApiError> {
  operation(async move {
    let dialog = std::sync::Arc::clone(&ctx.desktop.dialog);
    // the modal dialog blocks, off the async runtime and before the writer of the config, which it would hold
    let picked = run_blocking(move || Ok::<_, ApiError>(dialog.pick_folder("Select Companion Repository"))).await?;
    let Some(path) = picked else {
      return Ok(None);
    };
    let name = sync_core::companions::directory_name(path.trim_end_matches(['/', '\\']));
    let config = write_companion(ctx, &params.repository_path, name, Some(path)).await?;
    repository_config_changed(&*ctx.events, params.repository_path, RepositoryConfigKey::Companions);
    Ok(Some(config))
  })
  .await
}

/// `sync_core::repository_config::set_companion` through the writer of the config of the repository at
/// `repository_path`; the write of the `branchdeck.primary` of the companion runs in that task too, beside any writer of
/// the companion's config, whose lock it waits for as for a writer of another process
async fn write_companion(ctx: &AppContext, repository_path: &str, name: String, path: Option<String>) -> Result<RepositoryConfig, ApiError> {
  let (git, repository) = (ctx.git.clone(), repository_path.to_owned());
  let write = move || sync_core::repository_config::set_companion(&git, &repository, &name, path.as_deref());
  let config = ctx.config_writers.write(repository_path, WriteScope::Local, None, write).await?;
  config.ok_or_else(|| ApiError::internal("A write without a number is never superseded"))
}
