//! The pull requests the user adds to Reviews by their links (`remote_facts::reviews`): kept in the settings of the app
//! for the GitHub repository, read again by the session with Reviews

use crate::context::AppContext;
use crate::transport::Call;
use domain_types::error::ApiError;
use domain_types::review_request::{LinkPullRequestParams, LinkedPullRequestAnswer, UnlinkPullRequestParams};
use remote_facts::reviews::{link_pull_request as core_link_pull_request, unlink_pull_request as core_unlink_pull_request};
use repository_model::session::SessionKey;
use sync_utils::timestamp::unix_now;

/// `remote_facts::reviews::link_pull_request` over the services of the context
pub async fn link_pull_request(ctx: &AppContext, _call: &Call, key: SessionKey, params: LinkPullRequestParams) -> Result<LinkedPullRequestAnswer, ApiError> {
  let (entry, kept) = core_link_pull_request(&*ctx.services, &ctx.sessions, &key, &params.input, unix_now()).await?;
  Ok(LinkedPullRequestAnswer { entry, kept })
}

/// `remote_facts::reviews::unlink_pull_request` over the services of the context
pub async fn unlink_pull_request(ctx: &AppContext, _call: &Call, key: SessionKey, params: UnlinkPullRequestParams) -> Result<(), ApiError> {
  core_unlink_pull_request(&*ctx.services, &ctx.sessions, &key, params.number).await
}
