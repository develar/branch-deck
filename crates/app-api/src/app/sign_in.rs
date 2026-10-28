//! The sign-ins of the app's accounts (spec/repository/github-sign-in.spec.md, spec/repository/youtrack.spec.md):
//! commands of the app, not intents of a session, since an account has no repository (docs/design/rust-domain-authority.md,
//! "Решение 2026-10-28"). They run on the sign-ins of the host (`AppHost::sign_ins`, `remote_facts::sign_in`): one
//! sign-in of each provider at a time, a second call joins it, its events on the channel of each call. A sign-in or a
//! sign-out that changed an account is told by the host of the sign-ins (`SignInHost::changed`), which hands it to the
//! one rule of both hosts, [`account_changed`]: every session reads its fact `auth` again, and every window hears
//! `auth-changed`

use crate::context::AppContext;
use crate::events::auth_changed;
use crate::operation::operation;
use crate::transport::{AppEvents, Sink};
use domain_types::error::ApiError;
use domain_types::session::{FactFamily, FactRefreshReason};
use domain_types::sign_in::{Account, SignIn, SignInDone, SignInEvent, SignInProvider};
use tracing::instrument;

/// What a change of `account` has the sessions read again, and why: the sign-ins (`AfterWrite`, which follows up on a
/// read in flight), and after GitHub its pull requests and review requests too (`Manual`), which a token of another
/// user answers otherwise
const fn refreshes_after(account: Account) -> &'static [(FactFamily, FactRefreshReason)] {
  match account {
    Account::GitHub => &[
      (FactFamily::Auth, FactRefreshReason::AfterWrite),
      (FactFamily::PullRequests, FactRefreshReason::Manual),
      (FactFamily::ReviewRequests, FactRefreshReason::Manual),
    ],
    Account::YouTrack => &[(FactFamily::Auth, FactRefreshReason::AfterWrite)],
  }
}

/// A sign-in or a sign-out changed `account`: the rule of what that refreshes, one for the app and the e2e server
/// (`SignInHost::changed` of each). `refresh` asks a family of the sessions the host tells (every session of the app,
/// those of a test repository's worktree on the server), each of [`refreshes_after`], then every window hears
/// `auth-changed`
pub fn account_changed(account: Account, refresh: impl Fn(FactFamily, FactRefreshReason), events: &dyn AppEvents) {
  for &(family, reason) in refreshes_after(account) {
    refresh(family, reason);
  }
  auth_changed(events);
}

/// Signs in by `request`, or joins the sign-in of its provider that runs: its events on `events`, `done` last; a cancel
/// ends it with the code `canceled`
#[instrument(skip_all, fields(provider = ?request.provider()))]
pub async fn sign_in(ctx: &AppContext, request: SignIn, events: Sink<SignInEvent>) -> Result<SignInDone, ApiError> {
  operation(async move {
    let tell = move |event: SignInEvent| {
      // a window that went away hears nothing; the sign-in goes on for the others
      let _ = events.send(event);
    };
    ctx.host.sign_ins()?.sign_in(request, &tell).await
  })
  .await
}

/// Ends the sign-in of `provider` that runs, if any: its calls answer `canceled`
#[instrument(skip(ctx))]
pub async fn cancel_sign_in(ctx: &AppContext, provider: SignInProvider) -> Result<(), ApiError> {
  operation(async move {
    ctx.host.sign_ins()?.cancel(provider);
    Ok(())
  })
  .await
}

/// Removes the token of the app's own sign-in to `account`, after a sign-in of it that runs ended as canceled
#[instrument(skip(ctx))]
pub async fn sign_out(ctx: &AppContext, account: Account) -> Result<(), ApiError> {
  operation(async move { ctx.host.sign_ins()?.sign_out(account).await }).await
}

#[cfg(test)]
#[path = "sign_in_test.rs"]
mod sign_in_test;
