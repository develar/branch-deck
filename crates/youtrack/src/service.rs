//! The YouTrack of the app: where the token comes from (`YOUTRACK_TOKEN`, else the token file), its renewal, what
//! YouTrack rejected, and the issues of the popovers, kept a minute.

use crate::client::{YouTrackClient, YouTrackError};
use crate::issue::{apply_command, fetch_issue, set_field_command};
use crate::sign_in::{RefreshError, refresh};
use crate::token_store::{StoredToken, read_token};
use crate::url::{is_issue_key, normalize_base_url};
use moka::sync::Cache;
use std::path::PathBuf;
use std::time::Duration;
use sync_types::youtrack::{YouTrackAuthProblem, YouTrackAuthStatus, YouTrackIssue, YouTrackIssueAnswer, YouTrackStateChange, YouTrackTokenSource, YouTrackUser};
use sync_utils::secret_store::write_secret_json;
use sync_utils::timestamp::unix_now;
use tokio::sync::Mutex;
use tracing::{info, warn};

/// An access token is renewed this long before it expires
const RENEW_BEFORE: u32 = 60;
/// A popover opened again within this time shows the same issue without asking
const ISSUE_TTL: Duration = Duration::from_secs(60);
const ISSUE_CACHE_CAPACITY: u64 = 500;

/// The token of `YOUTRACK_TOKEN` and `YOUTRACK_BASE_URL`, the variables of the `youtrack` CLI
#[derive(Clone)]
pub struct EnvironmentToken {
  pub url: String,
  pub token: String,
}

impl EnvironmentToken {
  pub fn from_process() -> Option<Self> {
    Self::of(std::env::var("YOUTRACK_BASE_URL").ok()?.as_str(), std::env::var("YOUTRACK_TOKEN").ok()?)
  }

  pub fn of(url: &str, token: String) -> Option<Self> {
    if token.trim().is_empty() {
      return None;
    }
    match normalize_base_url(url) {
      Ok(url) => Some(Self { url, token }),
      Err(e) => {
        warn!(error = %e, "YOUTRACK_BASE_URL is not used");
        None
      }
    }
  }
}

/// A token the service may send, and to which YouTrack
struct Credential {
  source: YouTrackTokenSource,
  url: String,
  access_token: String,
  user: Option<YouTrackUser>,
  /// The stored token, for a renewal
  stored: Option<StoredToken>,
}

#[derive(Default)]
struct Auth {
  /// The token file was read since the last `forget`
  loaded: bool,
  stored: Option<StoredToken>,
  problem: Option<YouTrackAuthProblem>,
}

/// Why no token can be sent
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Denied {
  NotSignedIn,
  Problem(YouTrackAuthProblem),
  Failed(String),
}

pub struct YouTrackService {
  pub client: YouTrackClient,
  environment: Option<EnvironmentToken>,
  token_file: Option<PathBuf>,
  /// Held across a renewal, so that parallel lookups renew once
  auth: Mutex<Auth>,
  issues: Cache<String, YouTrackIssue>,
}

impl YouTrackService {
  pub fn new(client: YouTrackClient, environment: Option<EnvironmentToken>, token_file: Option<PathBuf>) -> Self {
    Self {
      client,
      environment,
      token_file,
      auth: Mutex::new(Auth::default()),
      issues: Cache::builder().max_capacity(ISSUE_CACHE_CAPACITY).time_to_live(ISSUE_TTL).build(),
    }
  }

  pub const fn token_file(&self) -> Option<&PathBuf> {
    self.token_file.as_ref()
  }

  /// After a sign-in or a sign-out: the token file is read again, a rejection is forgotten
  pub async fn forget(&self) {
    *self.auth.lock().await = Auth::default();
    self.issues.invalidate_all();
  }

  pub async fn status(&self) -> YouTrackAuthStatus {
    let mut auth = self.auth.lock().await;
    let problem = auth.problem;
    let credential = self.credential(&mut auth);
    YouTrackAuthStatus {
      url: credential.as_ref().map(|credential| credential.url.clone()),
      signed_in_as: credential.as_ref().and_then(|credential| credential.user.clone()),
      source: credential.as_ref().map(|credential| credential.source),
      problem: credential.and(problem),
    }
  }

  fn credential(&self, auth: &mut Auth) -> Option<Credential> {
    if let Some(environment) = &self.environment {
      return Some(Credential {
        source: YouTrackTokenSource::Environment,
        url: environment.url.clone(),
        access_token: environment.token.clone(),
        user: None,
        stored: None,
      });
    }
    if !auth.loaded {
      auth.stored = self.token_file.as_deref().and_then(read_token);
      auth.loaded = true;
    }
    auth.stored.clone().map(|stored| Credential {
      source: if stored.refresh.is_some() {
        YouTrackTokenSource::SignIn
      } else {
        YouTrackTokenSource::PastedToken
      },
      url: stored.url.clone(),
      access_token: stored.access_token.clone(),
      user: Some(YouTrackUser {
        login: stored.login.clone(),
        full_name: stored.full_name.clone(),
      }),
      stored: Some(stored),
    })
  }

  /// The YouTrack and a token for it, renewed when it expires within a minute (or `force_renewal`, after a 401)
  async fn access(&self, force_renewal: bool) -> Result<(String, String), Denied> {
    let mut auth = self.auth.lock().await;
    if let Some(problem) = auth.problem {
      return Err(Denied::Problem(problem));
    }
    let credential = self.credential(&mut auth).ok_or(Denied::NotSignedIn)?;
    let Some(stored) = credential.stored.filter(|stored| stored.refresh.is_some()) else {
      if force_renewal {
        auth.problem = Some(YouTrackAuthProblem::Rejected);
        return Err(Denied::Problem(YouTrackAuthProblem::Rejected));
      }
      return Ok((credential.url, credential.access_token));
    };
    let expires_soon = stored.access_expires_at.is_some_and(|at| at <= unix_now() + RENEW_BEFORE);
    if !force_renewal && !expires_soon {
      return Ok((credential.url, credential.access_token));
    }
    let Some(grant) = stored.refresh.clone() else {
      return Ok((credential.url, credential.access_token));
    };
    match refresh(&self.client, &grant.hub_url, &grant.service_id, &grant.refresh_token).await {
      Ok(answer) => {
        let mut renewed = stored;
        renewed.access_token = answer.access_token;
        renewed.access_expires_at = answer.expires_in.map(expires_at);
        // Hub may hand out a new refresh token; the old one is then no longer valid
        if let (Some(refresh_token), Some(grant)) = (answer.refresh_token, renewed.refresh.as_mut()) {
          grant.refresh_token = refresh_token;
        }
        if let Some(file) = &self.token_file
          && let Err(e) = write_secret_json(file, &renewed)
        {
          warn!(error = %e, "cannot keep the renewed YouTrack token");
        }
        info!("renewed the YouTrack token");
        let access = (renewed.url.clone(), renewed.access_token.clone());
        auth.stored = Some(renewed);
        Ok(access)
      }
      Err(RefreshError::Expired(message)) => {
        warn!(%message, "the YouTrack sign-in expired");
        auth.problem = Some(YouTrackAuthProblem::Expired);
        Err(Denied::Problem(YouTrackAuthProblem::Expired))
      }
      Err(RefreshError::Failed(error)) => Err(Denied::Failed(error.message())),
    }
  }

  /// A request with the token; after a 401 the token is renewed once and the request asked again, and a token that
  /// can't be renewed is not sent any more
  async fn with_token<T, F: Future<Output = Result<T, YouTrackError>>>(&self, request: impl Fn(String, String) -> F) -> Result<T, AskError> {
    let (url, token) = self.access(false).await.map_err(AskError::Denied)?;
    match request(url, token).await {
      Err(YouTrackError::Unauthorized) => {
        let (url, token) = self.access(true).await.map_err(AskError::Denied)?;
        match request(url, token).await {
          Err(YouTrackError::Unauthorized) => {
            self.auth.lock().await.problem = Some(YouTrackAuthProblem::Rejected);
            Err(AskError::Denied(Denied::Problem(YouTrackAuthProblem::Rejected)))
          }
          result => result.map_err(AskError::YouTrack),
        }
      }
      result => result.map_err(AskError::YouTrack),
    }
  }

  /// The issue of `key` in the YouTrack the token is for
  pub async fn issue(&self, key: &str) -> YouTrackIssueAnswer {
    if !is_issue_key(key) {
      return YouTrackIssueAnswer::Error {
        message: format!("Not an issue key: {key}"),
      };
    }
    if let Some(issue) = self.issues.get(key) {
      return YouTrackIssueAnswer::Found { issue };
    }
    match self.with_token(|url, token| async move { fetch_issue(&self.client, &url, &token, key).await }).await {
      Ok(issue) => {
        self.issues.insert(key.to_string(), issue.clone());
        YouTrackIssueAnswer::Found { issue }
      }
      Err(AskError::Denied(Denied::NotSignedIn)) => YouTrackIssueAnswer::NotSignedIn,
      Err(AskError::Denied(Denied::Problem(problem))) => YouTrackIssueAnswer::SignInProblem { problem },
      Err(AskError::Denied(Denied::Failed(message))) => YouTrackIssueAnswer::Error { message },
      Err(AskError::YouTrack(YouTrackError::NotFound)) => YouTrackIssueAnswer::NotFound,
      Err(AskError::YouTrack(error)) => YouTrackIssueAnswer::Error { message: error.message() },
    }
  }

  /// Sets the state field of the issue to `value`, as the issue is now: with `expected`, only while the field still
  /// has that value (an undo does not override a change made since), without it only while the issue is open. The
  /// previous value, for the undo.
  pub async fn set_state(&self, key: &str, value: &str, expected: Option<&str>) -> Result<YouTrackStateChange, String> {
    if !is_issue_key(key) {
      return Err(format!("Not an issue key: {key}"));
    }
    self.issues.invalidate(key);
    let change = self
      .with_token(|url, token| async move {
        let issue = fetch_issue(&self.client, &url, &token, key).await?;
        let Some(state) = issue.state else {
          return Ok(Err(format!("{key} has no state field")));
        };
        let current = state.value.as_deref().unwrap_or("without a state");
        if let Some(expected) = expected
          && state.value.as_deref() != Some(expected)
        {
          return Ok(Err(format!("{key} is now {current}")));
        }
        // a change that expects nothing changes an open issue only: never a Duplicate into Fixed
        if expected.is_none() && state.resolved {
          return Ok(Err(format!("{key} is already {current}")));
        }
        apply_command(&self.client, &url, &token, key, &set_field_command(&state.field, value)).await?;
        Ok(Ok(YouTrackStateChange {
          field: state.field,
          previous: state.value,
        }))
      })
      .await
      .map_err(|e| e.message())?;
    self.issues.invalidate(key);
    change
  }
}

/// Unix seconds `expires_in` seconds from now
pub fn expires_at(expires_in: u64) -> u32 {
  unix_now().saturating_add(u32::try_from(expires_in).unwrap_or(u32::MAX))
}

enum AskError {
  Denied(Denied),
  YouTrack(YouTrackError),
}

impl AskError {
  fn message(&self) -> String {
    match self {
      Self::Denied(Denied::NotSignedIn) => "Not signed in to YouTrack".to_string(),
      Self::Denied(Denied::Problem(YouTrackAuthProblem::Rejected)) => "YouTrack rejected the token: sign in again".to_string(),
      Self::Denied(Denied::Problem(YouTrackAuthProblem::Expired)) => "The YouTrack sign-in expired: sign in again".to_string(),
      Self::Denied(Denied::Failed(message)) => message.clone(),
      Self::YouTrack(error) => error.message(),
    }
  }
}
