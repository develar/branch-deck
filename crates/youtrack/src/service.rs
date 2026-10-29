//! The YouTrack of the app: where the token comes from (`YOUTRACK_TOKEN`, else the token file), its renewal, what
//! YouTrack rejected, and the issues of the popovers, kept a minute.

use crate::client::{YouTrackClient, YouTrackError};
use crate::issue::{apply_command, fetch_issue, set_field_command};
use crate::sign_in::{RefreshError, refresh};
use crate::token_store::{StoredToken, read_token};
use crate::url::{is_issue_key, normalize_base_url};
use domain_types::youtrack::{YouTrackAuthProblem, YouTrackAuthStatus, YouTrackIssue, YouTrackIssueAnswer, YouTrackStateChange, YouTrackTokenSource, YouTrackUser};
use moka::sync::Cache;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use sync_utils::secret_store::{remove_secret, write_secret_json};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tokio::sync::watch;
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
  /// One more with every `forget`: a renewal started before a sign-in or a sign-out keeps nothing of its answer
  generation: u64,
  /// The renewal in flight in this generation (a single flight): a lookup that needs it waits for its answer, which
  /// it shares, a token or a failure; none once it answered, so a lookup after a failed renewal renews again
  renewal: Option<watch::Receiver<Option<Renewal>>>,
}

/// The answer of a renewal to every lookup that waited for it
#[derive(Clone)]
enum Renewal {
  /// The YouTrack and the renewed token
  Renewed(String, String),
  /// The Hub refused the refresh token (an expired sign-in) or did not answer
  Denied(Denied),
  /// A sign-in or a sign-out came while the Hub answered: the answer is of a sign-in that is gone, read the state again
  ReadAgain,
}

/// What `access` does with the token it read
enum Access {
  /// Send it: the YouTrack and the token
  Send(String, String),
  /// Renew it first, the stored token as read in `generation`; the lookups that need it meanwhile wait for this one
  Renew(StoredToken, u64, watch::Sender<Option<Renewal>>),
  /// Wait for the renewal in flight
  Wait(watch::Receiver<Option<Renewal>>),
}

/// The state of the service, shared with the writes of the token file that run off the async workers
struct State {
  /// Held only to read or change the state, never across a request or a write: `status` and a sign-out never wait for
  /// YouTrack, the Hub or the token file
  auth: Mutex<Auth>,
  /// The writes of the token file in order: a sign-in, a sign-out (`replace_stored`) and the write of a renewal. Held
  /// on the thread of the write, across the write alone (a local `security` process or a file), never across an await
  store: Mutex<()>,
  issues: Cache<String, YouTrackIssue>,
}

impl State {
  fn auth(&self) -> MutexGuard<'_, Auth> {
    self.auth.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn store(&self) -> MutexGuard<'_, ()> {
    self.store.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// `state` in a new generation: a renewal in flight keeps nothing of its answer, and the lookups that come now start
  /// their own
  fn start_generation(&self, state: Auth) {
    let mut auth = self.auth();
    *auth = Auth {
      generation: auth.generation + 1,
      ..state
    };
    drop(auth);
    self.issues.invalidate_all();
  }
}

/// The renewal a lookup runs, in the state until it answers. Dropped without an answer (its lookup went away while the
/// Hub answered), it leaves the state, and the lookups that waited for it read the state again
struct Renewing<'a> {
  state: &'a State,
  answer: watch::Sender<Option<Renewal>>,
}

impl Renewing<'_> {
  fn finish(self, renewal: Renewal) {
    self.answer.send_replace(Some(renewal));
  }
}

impl Drop for Renewing<'_> {
  fn drop(&mut self) {
    let mut auth = self.state.auth();
    if auth.renewal.as_ref().is_some_and(|renewal| renewal.same_channel(&self.answer.subscribe())) {
      auth.renewal = None;
    }
  }
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
  state: Arc<State>,
  /// A token that expires is renewed (the app); `false` for `bd`, which only reads
  renews: bool,
}

impl YouTrackService {
  pub fn new(client: YouTrackClient, environment: Option<EnvironmentToken>, token_file: Option<PathBuf>) -> Self {
    Self {
      client,
      environment,
      token_file,
      state: Arc::new(State {
        auth: Mutex::new(Auth::default()),
        store: Mutex::new(()),
        issues: Cache::builder().max_capacity(ISSUE_CACHE_CAPACITY).time_to_live(ISSUE_TTL).build(),
      }),
      renews: true,
    }
  }

  /// The service of a process beside the app (`bd`): it sends the stored token as it is and never renews it. A renewal
  /// writes the rotated refresh token, and the app, which keeps the old one in memory, would be signed out by its next
  /// renewal; a token that expires is left to the app.
  pub fn without_renewal(client: YouTrackClient, environment: Option<EnvironmentToken>, token_file: Option<PathBuf>) -> Self {
    Self {
      renews: false,
      ..Self::new(client, environment, token_file)
    }
  }

  pub const fn token_file(&self) -> Option<&PathBuf> {
    self.token_file.as_ref()
  }

  fn auth(&self) -> MutexGuard<'_, Auth> {
    self.state.auth()
  }

  /// After a sign-in or a sign-out: the token file is read again, a rejection is forgotten, and a renewal in flight
  /// keeps nothing of its answer
  pub fn forget(&self) {
    self.state.start_generation(Auth::default());
  }

  /// The token of a sign-in, or none for a sign-out, into the token file. The state takes it first, in a new
  /// generation: a lookup meanwhile sends it (or nothing) and never reads the token it replaces from the file, which
  /// the write has not reached yet, and a renewal in flight keeps nothing of its answer. A renewal writes under the
  /// same `store` lock, so it never writes over this write or after it. A failed write leaves the token file the truth.
  pub async fn replace_stored(&self, token: Option<StoredToken>) -> std::io::Result<()> {
    self
      .replace_stored_with(token, |file, token| match token {
        Some(token) => write_secret_json(file, token),
        None => remove_secret(file),
      })
      .await
  }

  /// `replace_stored` with the write of the token file as given (a test holds it): the new generation and the write on
  /// the thread of the write, under the `store` lock
  pub(crate) async fn replace_stored_with(
    &self,
    token: Option<StoredToken>,
    write: impl FnOnce(&Path, Option<&StoredToken>) -> std::io::Result<()> + Send + 'static,
  ) -> std::io::Result<()> {
    let (state, file) = (Arc::clone(&self.state), self.token_file.clone());
    tokio::task::spawn_blocking(in_current_span(move || {
      let _store = state.store();
      state.start_generation(Auth {
        loaded: true,
        stored: token.clone(),
        ..Auth::default()
      });
      let Some(file) = file else {
        return Ok(());
      };
      let result = write(&file, token.as_ref());
      if result.is_err() {
        state.start_generation(Auth::default());
      }
      result
    }))
    .await
    .unwrap_or_else(|e| Err(std::io::Error::other(e)))
  }

  /// Never waits for YouTrack or the Hub: a renewal in flight holds no lock this reads
  pub fn status(&self) -> YouTrackAuthStatus {
    let mut auth = self.auth();
    let problem = auth.problem;
    let credential = self.credential(&mut auth);
    YouTrackAuthStatus {
      url: credential.as_ref().map(|credential| credential.url.clone()),
      signed_in_as: credential.as_ref().and_then(|credential| credential.user.clone()),
      source: credential.as_ref().map(|credential| credential.source),
      signed_in_at: credential.as_ref().and_then(|credential| credential.stored.as_ref()).map(|stored| stored.created_at),
      manage_url: credential.as_ref().map(manage_url_of),
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

  /// The YouTrack and a token for it, renewed when it expires within a minute or when YouTrack rejected it (`rejected`,
  /// the token sent: a token another lookup renewed meanwhile is sent as it is). One renewal at a time, which the
  /// lookups that need it meanwhile wait for; the state is locked only to be read and changed, never across the
  /// request to the Hub
  async fn access(&self, rejected: Option<&str>) -> Result<(String, String), Denied> {
    loop {
      let renewal = match self.read_access(rejected)? {
        Access::Send(url, token) => return Ok((url, token)),
        // None: the lookup that renewed went away before the Hub answered
        Access::Wait(mut renewal) => renewal.wait_for(Option::is_some).await.ok().and_then(|renewal| renewal.clone()),
        Access::Renew(stored, generation, answer) => {
          let renewing = Renewing { state: &self.state, answer };
          let renewal = self.renew(stored, generation).await;
          renewing.finish(renewal.clone());
          Some(renewal)
        }
      };
      match renewal {
        Some(Renewal::Renewed(url, token)) => return Ok((url, token)),
        Some(Renewal::Denied(denied)) => return Err(denied),
        Some(Renewal::ReadAgain) | None => {}
      }
    }
  }

  /// The token to send, the stored token to renew first, or the renewal in flight to wait for, from the state as it is
  /// now: a lookup that needs a renewal while one runs in this generation waits for it, else it renews
  fn read_access(&self, rejected: Option<&str>) -> Result<Access, Denied> {
    let mut auth = self.auth();
    if let Some(problem) = auth.problem {
      return Err(Denied::Problem(problem));
    }
    let credential = self.credential(&mut auth).ok_or(Denied::NotSignedIn)?;
    let is_rejected = rejected == Some(credential.access_token.as_str());
    let Some(stored) = credential.stored.filter(|stored| stored.refresh.is_some()) else {
      if is_rejected {
        auth.problem = Some(YouTrackAuthProblem::Rejected);
        return Err(Denied::Problem(YouTrackAuthProblem::Rejected));
      }
      return Ok(Access::Send(credential.url, credential.access_token));
    };
    let expires_soon = stored.access_expires_at.is_some_and(|at| at <= unix_now() + RENEW_BEFORE);
    if !is_rejected && !expires_soon {
      return Ok(Access::Send(credential.url, credential.access_token));
    }
    if !self.renews {
      return Err(Denied::Failed("The YouTrack token needs a renewal: the app renews it".to_string()));
    }
    if let Some(renewal) = &auth.renewal {
      return Ok(Access::Wait(renewal.clone()));
    }
    let (answer, renewal) = watch::channel(None);
    auth.renewal = Some(renewal);
    Ok(Access::Renew(stored, auth.generation, answer))
  }

  /// The renewal of `stored`, read in `generation`, at the Hub; its answer kept in the state and the token file unless a
  /// sign-in or a sign-out came meanwhile
  async fn renew(&self, stored: StoredToken, generation: u64) -> Renewal {
    let Some(grant) = stored.refresh.clone() else {
      return Renewal::Renewed(stored.url, stored.access_token);
    };
    let answer = match refresh(&self.client, &grant.hub_url, &grant.service_id, &grant.refresh_token).await {
      Ok(answer) => answer,
      Err(error) => {
        let mut auth = self.auth();
        if auth.generation != generation {
          // signed in or out while the Hub answered: the answer is of a sign-in that is gone
          return Renewal::ReadAgain;
        }
        return match error {
          RefreshError::Expired(message) => {
            warn!(%message, "the YouTrack sign-in expired");
            auth.problem = Some(YouTrackAuthProblem::Expired);
            Renewal::Denied(Denied::Problem(YouTrackAuthProblem::Expired))
          }
          RefreshError::Failed(error) => Renewal::Denied(Denied::Failed(error.message())),
        };
      }
    };
    let mut renewed = stored;
    renewed.access_token = answer.access_token;
    renewed.access_expires_at = answer.expires_in.map(expires_at);
    // Hub may hand out a new refresh token; the old one is then no longer valid
    if let (Some(refresh_token), Some(grant)) = (answer.refresh_token, renewed.refresh.as_mut()) {
      grant.refresh_token = refresh_token;
    }
    let (state, file) = (Arc::clone(&self.state), self.token_file.clone());
    tokio::task::spawn_blocking(in_current_span(move || keep_renewed(&state, file.as_deref(), renewed, generation)))
      .await
      .unwrap_or_else(|e| Renewal::Denied(Denied::Failed(format!("Cannot keep the renewed YouTrack token: {e}"))))
  }

  /// A request with the token; after a 401 the token is renewed once and the request asked again, and a token that
  /// can't be renewed is not sent any more
  async fn with_token<T, F: Future<Output = Result<T, YouTrackError>>>(&self, request: impl Fn(String, String) -> F) -> Result<T, AskError> {
    let (url, token) = self.access(None).await.map_err(AskError::Denied)?;
    match request(url, token.clone()).await {
      Err(YouTrackError::Unauthorized) => {
        let (url, token) = self.access(Some(&token)).await.map_err(AskError::Denied)?;
        match request(url, token).await {
          Err(YouTrackError::Unauthorized) => {
            self.auth().problem = Some(YouTrackAuthProblem::Rejected);
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
    if let Some(issue) = self.state.issues.get(key) {
      return YouTrackIssueAnswer::Found { issue };
    }
    match self.with_token(|url, token| async move { fetch_issue(&self.client, &url, &token, key).await }).await {
      Ok(issue) => {
        self.state.issues.insert(key.to_string(), issue.clone());
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
    self.state.issues.invalidate(key);
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
    self.state.issues.invalidate(key);
    change
  }
}

/// The renewed token into the token file, then into the state, on the thread of the write: under the `store` lock, so a
/// sign-in or a sign-out (`replace_stored`) that writes now is waited for, its generation then the current one, and one
/// that comes later writes after it. Nothing of a generation that is gone
fn keep_renewed(state: &State, file: Option<&Path>, renewed: StoredToken, generation: u64) -> Renewal {
  let _store = state.store();
  if state.auth().generation != generation {
    return Renewal::ReadAgain;
  }
  if let Some(file) = file
    && let Err(e) = write_secret_json(file, &renewed)
  {
    warn!(error = %e, "cannot keep the renewed YouTrack token");
  }
  info!("renewed the YouTrack token");
  let access = Renewal::Renewed(renewed.url.clone(), renewed.access_token.clone());
  let mut auth = state.auth();
  // a `forget` meanwhile reads the token file again, which holds the renewed token
  if auth.generation == generation {
    auth.stored = Some(renewed);
  }
  access
}

/// The profile page that lists the token: of the Hub of a browser sign-in when that Hub is not built into the YouTrack
/// (`hub.example.com` beside `youtrack.example.com`), else Account Security of the YouTrack
fn manage_url_of(credential: &Credential) -> String {
  let hub = credential.stored.as_ref().and_then(|stored| stored.refresh.as_ref()).map(|grant| grant.hub_url.as_str());
  match hub {
    // the tab of a Hub's own profile is named differently across its versions: the profile, which shows it
    Some(hub) if !hub.starts_with(&format!("{}/", credential.url)) => format!("{hub}/users/me"),
    _ => format!("{}/users/me?tab=account-security", credential.url),
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
