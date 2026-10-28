//! The view of a session and what a window asks of it: its snapshot, the subscription to its patches, the focus and the
//! demand of the window, a sync, a fact read again, and the group a typed message puts a commit into (answered by Rust,
//! so the page keeps no grammar of its own; the rest of that grammar is `reads::grammar`)

use crate::context::AppContext;
use crate::transport::{Call, Sink};
use app_settings::journals::remember_journal;
use domain_types::commit::{CommitGroup, PreviewGroupParams};
use domain_types::error::ApiError;
use domain_types::session::{RefreshFactsParams, RepositoryView, SetDemandParams, SyncAnswer, SyncReason, Version, ViewPatch, ViewSubscription};
use repository_model::session::{SessionKey, Subscriber, SubscriberGone, SubscriptionId};
use serde::Deserialize;
use tracing::{instrument, warn};

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionParams {
  pub repository_path: String,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeViewParams {
  pub id: u32,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowFocusParams {
  pub repository_path: String,
  pub focused: bool,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncParams {
  pub repository_path: String,
  pub reason: SyncReason,
}

/// `Sessions::get_snapshot`
pub async fn get_snapshot(ctx: &AppContext, _call: &Call, key: SessionKey, _params: SessionParams) -> Result<Option<RepositoryView>, ApiError> {
  Ok(ctx.sessions.get_snapshot(&key).await)
}

/// The patches of a session to the channel of a window's page
struct PagePatches(Sink<ViewPatch>);

impl Subscriber for PagePatches {
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone> {
    self.0.send(patch.clone()).map_err(|_gone| SubscriberGone)
  }
}

/// Through the registry, which watches the operations registry of the repository while the window subscribes, kept
/// under the page of the call (`Windows::keep`)
pub async fn subscribe_view(ctx: &AppContext, call: &Call, key: SessionKey, _params: SessionParams, patches: Sink<ViewPatch>) -> Result<ViewSubscription, ApiError> {
  remember_journal_of(ctx, &key).await;
  let (id, view) = ctx
    .registry
    .subscribe(&key, Box::new(PagePatches(patches)))
    .await
    .ok_or_else(|| ApiError::internal("The session closed while the window subscribed"))?;
  if !ctx.windows.keep(call, id) {
    // the page went away: a Tauri channel never fails a send, so the session would keep it
    ctx.sessions.unsubscribe(id);
    return Err(ApiError::internal("The page of the window went away while it subscribed"));
  }
  Ok(ViewSubscription { id: id.0, view })
}

/// Remembers the journal of the repository of the session of `key` for its project, which Activity reads without a git
/// process (`app_settings::journals`); a change goes to the windows, a failure is only logged
async fn remember_journal_of(ctx: &AppContext, key: &SessionKey) {
  let Some(journal) = ctx.registry.journal_of(key).await else {
    return;
  };
  match remember_journal(&ctx.settings, &key.worktree, &journal.to_string_lossy()) {
    Ok(Some(change)) => ctx.events.settings_changed(&change),
    Ok(None) => {}
    Err(error) => warn!(%error, "cannot remember the journal of the repository"),
  }
}

/// The subscription leaves the page of the call and the session
#[instrument(skip_all, fields(id = params.id, window = %call.window))]
#[expect(clippy::needless_pass_by_value, reason = "the registry hands a body its arguments by value")]
pub fn unsubscribe_view(ctx: &AppContext, call: &Call, params: UnsubscribeViewParams) {
  let id = SubscriptionId(params.id);
  ctx.windows.drop_subscription(call, id);
  ctx.sessions.unsubscribe(id);
}

/// `Sessions::set_demand` of the subscription
#[instrument(skip_all, fields(subscription = params.subscription))]
#[expect(clippy::needless_pass_by_value, reason = "the registry hands a body its arguments by value")]
pub fn set_demand(ctx: &AppContext, _call: &Call, params: SetDemandParams) {
  ctx.sessions.set_demand(SubscriptionId(params.subscription), &params.demand);
}

/// A gained focus reads the operations registry again, the stand-in for a watch the system refused, then asks the
/// session (`Sessions::set_window_focus`)
pub async fn set_window_focus(ctx: &AppContext, _call: &Call, key: SessionKey, params: WindowFocusParams) -> Result<Option<SyncAnswer>, ApiError> {
  if params.focused {
    ctx.registry.reread(&key).await;
  }
  match ctx.sessions.set_window_focus(&key, params.focused) {
    Some(ticket) => ticket.wait().await.map(Some),
    None => Ok(None),
  }
}

/// The ticket of `Sessions::sync`, to the end of the run that serves it
pub async fn sync(ctx: &AppContext, _call: &Call, key: SessionKey, params: SyncParams) -> Result<SyncAnswer, ApiError> {
  ctx.sessions.sync(&key, params.reason).wait().await
}

/// `Sessions::refresh` of the family
pub async fn refresh_facts(ctx: &AppContext, _call: &Call, key: SessionKey, params: RefreshFactsParams) -> Result<Option<Version>, ApiError> {
  Ok(ctx.sessions.refresh(&key, params.family, params.reason, params.subject).await)
}

/// `Sessions::preview_group`: no git, no lock, no effect
pub async fn preview_group(ctx: &AppContext, _call: &Call, key: SessionKey, params: PreviewGroupParams) -> Result<Option<CommitGroup>, ApiError> {
  Ok(ctx.sessions.preview_group(&key, &params.message).await)
}
