//! The sign-ins of the app's accounts (spec/repository/github-sign-in.spec.md, spec/repository/youtrack.spec.md):
//! commands of the app, not intents of a session, since an account has no repository (docs/design/rust-domain-authority.md,
//! "Решение 2026-10-28"). They run on the sign-ins of the host (`AppHost::sign_ins`, `remote_facts::sign_in`): one
//! sign-in of each provider at a time, a second call joins it, its events on the channel of each call. A sign-in or a
//! sign-out that changed an account is told by the host of the sign-ins (`SignInHost::changed`): every session reads its
//! fact `auth` again, and every window hears `auth-changed`

use crate::context::AppContext;
use crate::operation::operation;
use crate::transport::Sink;
use domain_types::error::ApiError;
use domain_types::sign_in::{Account, SignIn, SignInDone, SignInEvent, SignInProvider};
use tracing::instrument;

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
