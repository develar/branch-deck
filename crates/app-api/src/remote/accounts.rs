//! The accounts of the app, which no repository owns (`AppHost`): the sign-ins as a window without a session reads them
//! (spec/repository/github-sign-in.spec.md), and the issue of a popover from the YouTrack of the app
//! (spec/repository/youtrack.spec.md)

use crate::context::AppContext;
use crate::operation::operation;
use domain_types::error::ApiError;
use domain_types::sign_in::AuthStatus;
use domain_types::youtrack::YouTrackIssueAnswer;
use tracing::instrument;

/// The status of both sign-ins (`SignIns::status`); a window of a repository reads them as the fact `auth` of its session
#[instrument(skip_all)]
pub async fn get_auth_status(ctx: &AppContext) -> Result<AuthStatus, ApiError> {
  operation(async move {
    ctx.host.before_auth_status().await?;
    Ok(ctx.host.sign_ins()?.status().await)
  })
  .await
}

/// The issue of `key` from the YouTrack the token of the app is for; a missing sign-in is an answer
#[instrument(skip(ctx))]
pub async fn get_youtrack_issue(ctx: &AppContext, key: String) -> Result<YouTrackIssueAnswer, ApiError> {
  operation(async move { Ok(ctx.host.youtrack()?.issue(&key).await) }).await
}
