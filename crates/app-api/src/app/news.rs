//! The claim of a news of a repository (openspec/specs/repository/backend-protocol/spec.md, "Events: A news is told once
//! by the backend"): a window claims a news before it tells it (the OS notification of a failed robot), and only the
//! first claim of any client of the backend, the app or the bridge of the IDE (`x-branch-deck/claimNews`), tells it.

use crate::context::AppContext;
use crate::operation::operation;
use domain_types::error::ApiError;
use domain_types::session::NewsClaim;
use serde::Deserialize;
use tracing::instrument;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClaimNewsParams {
  pub repository_path: String,
  pub news: NewsClaim,
}

/// Whether this is the first claim of the news (`Connections::claim_news`)
///
/// # Errors
/// Never: a claim answers whether it is the first
#[instrument(skip(ctx))]
pub async fn claim_news(ctx: &AppContext, params: ClaimNewsParams) -> Result<bool, ApiError> {
  operation(async move { Ok(ctx.connections.claim_news(&params.repository_path, params.news)) }).await
}
