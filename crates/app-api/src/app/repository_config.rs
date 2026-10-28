//! A write of Settings → Repository (spec/repository/settings.spec.md): one key of the `branchdeck.*` git config of a
//! repository, local or global (`sync_core::repository_config::set_config`), read again for the answer; the main window
//! hears it (`repository_config_changed`) and acts for the selected repository

use crate::context::AppContext;
use crate::events::repository_config_changed;
use crate::operation::{operation, run_blocking};
use domain_types::error::ApiError;
use domain_types::settings::GitBranchPrefix;
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
}

/// Writes one key of the config and answers the config read again. A write of the branch prefix keeps the git prefix
/// the read found for the repository's later commands (`BranchPrefixes`); a global one forgets the prefix of every
/// other repository, which may take it
#[instrument(skip_all, fields(repository_path = %params.repository_path, key = ?params.change.key(), scope = ?params.scope))]
pub async fn set_repository_config(ctx: &AppContext, params: SetRepositoryConfigParams) -> Result<RepositoryConfig, ApiError> {
  operation(async move {
    let SetRepositoryConfigParams { repository_path, change, scope } = params;
    let key = change.key();
    let (git, path) = (ctx.git.clone(), repository_path.clone());
    let config = run_blocking(move || set_config(&git, &path, &change, scope)).await?;
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
    Ok(config)
  })
  .await
}
