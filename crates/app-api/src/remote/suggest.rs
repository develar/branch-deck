//! Branch names for the selected commits (openspec/specs/branch/branch-name-suggestions/spec.md): the namer of the app
//! (`AppHost::suggest_branch_names`, the user's Claude Code), one request of a window at a time

use crate::context::AppContext;
use crate::operation::run_blocking;
use crate::transport::Call;
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::error::ApiError;
use std::sync::Arc;
use tracing::instrument;

/// Two names from the namer of the app, in a blocking step; a missing or logged-out `claude` and its failures are
/// outcomes. A newer request of the window, and its close, supersede it (`Windows::begin_branch_names`): its `claude`
/// is killed
#[instrument(skip_all, fields(repository_path = %params.repository_path, commits = params.commits.len(), window = %call.window))]
pub async fn suggest_branch_names(ctx: &AppContext, call: &Call, params: SuggestBranchNamesParams) -> Result<BranchNameSuggestionOutcome, ApiError> {
  let (host, git) = (Arc::clone(&ctx.host), ctx.git.clone());
  let request = ctx.windows.begin_branch_names(&call.window);
  run_blocking(move || Ok::<_, ApiError>(host.suggest_branch_names(&git, &params, &|| request.is_superseded()))).await
}
