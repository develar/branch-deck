//! Branch names for the selected commits (spec/branch/branch-name-suggestions.spec.md): the namer of the app
//! (`AppHost::suggest_branch_names`, the user's Claude Code), one request at a time

use crate::context::AppContext;
use crate::operation::run_blocking;
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::error::ApiError;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::instrument;

/// The latest request for branch names: an older one still running is `Superseded`, its `claude` killed
#[derive(Clone, Default)]
pub struct BranchNameRequests {
  latest: Arc<AtomicU64>,
}

impl BranchNameRequests {
  /// A new request, which supersedes the one before
  fn next(&self) -> u64 {
    self.latest.fetch_add(1, Ordering::SeqCst) + 1
  }

  /// A newer request came after `request`
  fn superseded(&self, request: u64) -> bool {
    self.latest.load(Ordering::SeqCst) != request
  }
}

/// Two names from the namer of the app, in a blocking step; a missing or logged-out `claude` and its failures are
/// outcomes
#[instrument(skip_all, fields(repository_path = %params.repository_path, commits = params.commits.len()))]
pub async fn suggest_branch_names(ctx: &AppContext, params: SuggestBranchNamesParams) -> Result<BranchNameSuggestionOutcome, ApiError> {
  let (host, git, requests) = (Arc::clone(&ctx.host), ctx.git.clone(), ctx.remote.branch_names.clone());
  let request = requests.next();
  run_blocking(move || Ok::<_, ApiError>(host.suggest_branch_names(&git, &params, &|| requests.superseded(request)))).await
}
