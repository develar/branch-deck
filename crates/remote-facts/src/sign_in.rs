//! The sign-ins of the app's accounts as the app's commands run them (docs/design/rust-domain-authority.md, "Решение
//! 2026-10-28"; spec/repository/github-sign-in.spec.md): `sign_in`, `cancel`, `sign_out` and `status` over the cores of
//! each account (`github::sign_in_session`, `youtrack::sign_in_session`, `youtrack_cli`). One sign-in of each provider
//! runs at a time, in a task of its own: a second call of the same request joins it and hears its current event (the
//! code of GitHub, the page of the Hub), then the rest of its events and its end; a cancel, another request of the same
//! provider (a pasted token while the browser signs in, another YouTrack), or a sign-in of the other provider of the
//! same YouTrack session, ends it first, as canceled. The lock of the runs only registers them, never spans a
//! request or the browser. A sign-in or a sign-out that changed an account tells the host once (`SignInHost::changed`),
//! which has every session read the fact `auth` again and tells the windows without one. Hosted by the app
//! (`src-tauri/src/commands/sign_in.rs`) and by each test repository of the E2E server (`RemoteHost`).

use crate::youtrack_cli::hand_token_to_cli;
use cli_tools::environment::ToolEnvironment;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::sign_in::{Account, AuthStatus, SignIn, SignInDone, SignInEvent, SignInProvider};
use github::service::GitHubService;
use github::sign_in_session::GitHubSignInSession;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use tokio::sync::{oneshot, watch};
use tracing::{Instrument, instrument};
use youtrack::service::YouTrackService;
use youtrack::sign_in_session::YouTrackSignInSession;

/// What the sign-ins of a host run with
pub trait SignInHost: Send + Sync + 'static {
  /// The GitHub service whose token chain a sign-in or a sign-out makes forget what it knew
  fn github(&self) -> Arc<GitHubService>;

  /// The OAuth app of the build (`GITHUB_OAUTH_CLIENT_ID`); None for a build that signs in to GitHub nowhere
  fn github_client_id(&self) -> Option<String>;

  /// The YouTrack service whose token file a sign-in writes
  fn youtrack(&self) -> Arc<YouTrackService>;

  /// The environment the `youtrack` CLI is found and run in, and the file of the id of the token the app made for it
  fn youtrack_cli(&self) -> (ToolEnvironment, Option<PathBuf>);

  /// A failure of a sign-in as the host's commands answer it (`canceled` for an end by a cancel or a newer sign-in)
  fn api_error(&self, error: anyhow::Error) -> ApiError;

  /// A sign-in or a sign-out changed `account`: the sessions read the fact `auth` again, the windows hear it
  fn changed(&self, account: Account);
}

/// The sign-ins of one host; a clone is the same sign-ins
#[derive(Clone)]
pub struct SignIns {
  inner: Arc<Inner>,
}

struct Inner {
  host: Arc<dyn SignInHost>,
  github: GitHubSignInSession,
  youtrack: YouTrackSignInSession,
  runs: Mutex<HashMap<SignInProvider, Run>>,
  next_run: AtomicU64,
}

/// A sign-in that runs
struct Run {
  id: u64,
  /// What it signs in by: only a call of the same request joins it
  request: SignIn,
  progress: watch::Receiver<Progress>,
  /// Taken by a cancel: the run ends as canceled, and no call joins it any more
  cancel: Option<oneshot::Sender<()>>,
}

/// Where a run is: its last event and when it came, then its end
#[derive(Clone)]
enum Progress {
  Event { event: SignInEvent, at: Instant },
  Ended(Result<SignInDone, ApiError>),
}

impl Progress {
  const fn is_end(&self) -> bool {
    matches!(self, Self::Ended(_))
  }
}

/// The providers whose runs share a session with `provider`: a newer sign-in of one ends the wait of the other
const fn shares_session(provider: SignInProvider, other: SignInProvider) -> bool {
  matches!(
    (provider, other),
    (SignInProvider::YouTrack, SignInProvider::YouTrackCli) | (SignInProvider::YouTrackCli, SignInProvider::YouTrack)
  )
}

impl SignIns {
  /// The sign-ins of `host`, on the GitHub session of its token file; the YouTrack session is theirs alone
  pub fn new(host: Arc<dyn SignInHost>, github: GitHubSignInSession) -> Self {
    Self {
      inner: Arc::new(Inner {
        host,
        github,
        youtrack: YouTrackSignInSession::default(),
        runs: Mutex::new(HashMap::new()),
        next_run: AtomicU64::new(1),
      }),
    }
  }

  /// Signs in by `request`, or joins the sign-in of the same request that runs: `events` hears where it is, from its
  /// current event on, and `done` last; answers how it ended. A cancel, or a newer sign-in of another request of its
  /// provider or of its YouTrack session, ends it with the code `canceled`
  ///
  /// # Errors
  /// Why it failed, as the host words a failure (`SignInHost::api_error`)
  #[instrument(skip_all, fields(provider = ?request.provider()))]
  pub async fn sign_in(&self, request: SignIn, events: &(dyn Fn(SignInEvent) + Send + Sync)) -> Result<SignInDone, ApiError> {
    let provider = request.provider();
    let progress = loop {
      let before = match self.join_or_start(provider, &request) {
        Ok(progress) => break progress,
        Err(before) => before,
      };
      for mut ended in before {
        let _ = ended.wait_for(Progress::is_end).await;
      }
    };
    follow(progress, events).await
  }

  /// The progress of the run of `request` that is not canceled, else of a new one: none starts while another run of its
  /// provider (canceled, or of another request) or a run sharing its session is still there (those are canceled, and
  /// the caller waits for their ends)
  fn join_or_start(&self, provider: SignInProvider, request: &SignIn) -> Result<watch::Receiver<Progress>, Vec<watch::Receiver<Progress>>> {
    let mut runs = self.inner.runs();
    if let Some(run) = runs.get(&provider).filter(|run| run.cancel.is_some() && run.request == *request) {
      return Ok(run.progress.clone());
    }
    let before = cancel_runs(&mut runs, |other| other == provider || shares_session(provider, other));
    if !before.is_empty() {
      return Err(before);
    }
    Ok(self.start(&mut runs, request.clone()))
  }

  fn start(&self, runs: &mut HashMap<SignInProvider, Run>, request: SignIn) -> watch::Receiver<Progress> {
    let provider = request.provider();
    let id = self.inner.next_run.fetch_add(1, Ordering::Relaxed);
    let (progress, receiver) = watch::channel(Progress::Event {
      event: SignInEvent::Waiting,
      at: Instant::now(),
    });
    let (cancel, canceled) = oneshot::channel();
    runs.insert(
      provider,
      Run {
        id,
        request: request.clone(),
        progress: receiver.clone(),
        cancel: Some(cancel),
      },
    );
    let inner = Arc::clone(&self.inner);
    // the run outlives the call that started it, in a span of its own under the caller's
    let span = tracing::info_span!("sign_in_run", ?provider, id);
    let run = async move {
      let result = tokio::select! {
        // a sign-in that kept its token as the cancel came is a sign-in
        biased;
        result = inner.flow(request, &progress) => result,
        _ = canceled => {
          // the flow was dropped where it waited: what it kept of the sign-in goes too
          inner.end_session(provider);
          Err(ApiError::new(ErrorCode::Canceled, "The sign-in was canceled"))
        }
      };
      if result.is_ok()
        && let Some(account) = provider.account()
      {
        inner.host.changed(account);
      }
      // a call after this starts a new run, never joins the one that ended
      {
        let mut runs = inner.runs();
        if runs.get(&provider).is_some_and(|run| run.id == id) {
          runs.remove(&provider);
        }
      }
      let _ = progress.send(Progress::Ended(result));
    };
    span.in_scope(|| tokio::spawn(run.in_current_span()));
    receiver
  }

  /// Ends the sign-in of `provider` that runs, if any: it answers `canceled`
  pub fn cancel(&self, provider: SignInProvider) {
    let cancel = self.inner.runs().get_mut(&provider).and_then(|run| run.cancel.take());
    if let Some(cancel) = cancel {
      let _ = cancel.send(());
    }
  }

  /// Removes the token of the app's own sign-in to `account`, after the sign-ins of it that run ended as canceled
  ///
  /// # Errors
  /// Why the token stays, as the host words a failure
  #[instrument(skip(self))]
  pub async fn sign_out(&self, account: Account) -> Result<(), ApiError> {
    let providers: &[SignInProvider] = match account {
      Account::GitHub => &[SignInProvider::GitHub],
      Account::YouTrack => &[SignInProvider::YouTrack, SignInProvider::YouTrackCli],
    };
    let running = cancel_runs(&mut self.inner.runs(), |provider| providers.contains(&provider));
    for mut ended in running {
      let _ = ended.wait_for(Progress::is_end).await;
    }
    let host = &self.inner.host;
    let removed = match account {
      Account::GitHub => self.inner.github.sign_out(&host.github()).await,
      Account::YouTrack => self.inner.youtrack.sign_out(&host.youtrack()).await,
    };
    removed.map_err(|error| host.api_error(anyhow::anyhow!(error)))?;
    host.changed(account);
    Ok(())
  }

  /// The sign-ins of both accounts and where their tokens come from
  pub async fn status(&self) -> AuthStatus {
    let host = &self.inner.host;
    let client_id = host.github_client_id();
    AuthStatus {
      github: self.inner.github.status(&host.github(), client_id.as_deref()).await,
      youtrack: host.youtrack().status(),
    }
  }

  /// A sign-in of GitHub waits for its code (the E2E server's fixture of the sign-in)
  pub fn github_pending(&self) -> bool {
    self.inner.github.is_pending()
  }

  /// A sign-in of YouTrack or of the `youtrack` CLI waits for the browser (the E2E server's fixture of the YouTrack)
  pub fn youtrack_pending(&self) -> bool {
    self.inner.youtrack.is_pending()
  }
}

impl Inner {
  fn runs(&self) -> MutexGuard<'_, HashMap<SignInProvider, Run>> {
    self.runs.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// The session of `provider` forgets the sign-in it waits for
  fn end_session(&self, provider: SignInProvider) {
    match provider {
      SignInProvider::GitHub => self.github.cancel(),
      SignInProvider::YouTrack | SignInProvider::YouTrackCli => self.youtrack.cancel(),
    }
  }

  /// The steps of a sign-in, each event told on `progress`
  async fn flow(&self, request: SignIn, progress: &watch::Sender<Progress>) -> Result<SignInDone, ApiError> {
    let tell = |event: SignInEvent| {
      let _ = progress.send(Progress::Event { event, at: Instant::now() });
    };
    let failed = |error: anyhow::Error| self.host.api_error(error);
    let message = |error: String| failed(anyhow::anyhow!(error));
    match request {
      SignIn::GitHub => {
        let (service, client_id) = (self.host.github(), self.host.github_client_id());
        let code = self.github.start(&service, client_id.as_deref()).await.map_err(message)?;
        tell(SignInEvent::DeviceCode {
          code: code.user_code,
          url: code.verification_uri,
          expires_in: code.expires_in,
        });
        let login = self.github.wait(&service, client_id.as_deref()).await.map_err(|error| failed(error.into()))?;
        Ok(SignInDone::GitHub { login })
      }
      SignIn::YouTrack { url, token: Some(token) } => {
        let service = self.host.youtrack();
        let user = self.youtrack.save_token(&service, &url, &token).await.map_err(message)?;
        Ok(SignInDone::YouTrack {
          url: signed_in_url(&service, url),
          user,
        })
      }
      SignIn::YouTrack { url, token: None } => {
        let service = self.host.youtrack();
        let started = self.youtrack.start(&service, &url).await.map_err(message)?;
        tell(SignInEvent::Browser { url: started.authorize_url });
        let user = self.youtrack.wait(&service).await.map_err(|error| failed(error.into()))?;
        Ok(SignInDone::YouTrack {
          url: signed_in_url(&service, url),
          user,
        })
      }
      SignIn::YouTrackCli { url } => {
        let service = self.host.youtrack();
        let started = self.youtrack.start_for_cli(&service, &url).await.map_err(message)?;
        tell(SignInEvent::Browser { url: started.authorize_url });
        let grant = self.youtrack.wait_for_cli(&service).await.map_err(|error| failed(error.into()))?;
        tell(SignInEvent::Waiting);
        let (environment, kept) = self.host.youtrack_cli();
        let title = hand_token_to_cli(&service, grant, environment, kept.as_deref()).await.map_err(message)?;
        Ok(SignInDone::YouTrackCli { title })
      }
    }
  }
}

/// Cancels the runs of the providers `of` names (those canceled before stay so) and answers their progress, which the
/// caller waits for the ends of
fn cancel_runs(runs: &mut HashMap<SignInProvider, Run>, of: impl Fn(SignInProvider) -> bool) -> Vec<watch::Receiver<Progress>> {
  runs
    .iter_mut()
    .filter(|(provider, _)| of(**provider))
    .map(|(_, run)| {
      if let Some(cancel) = run.cancel.take() {
        let _ = cancel.send(());
      }
      run.progress.clone()
    })
    .collect()
}

/// The YouTrack the kept token is for, as its status names it
fn signed_in_url(service: &YouTrackService, asked: String) -> String {
  service.status().url.unwrap_or(asked)
}

/// Tells `events` each event of `progress` from the current one, the code of GitHub with the seconds it has left, and
/// `done` at the end; answers the end
async fn follow(mut progress: watch::Receiver<Progress>, events: &(dyn Fn(SignInEvent) + Send + Sync)) -> Result<SignInDone, ApiError> {
  loop {
    let current = progress.borrow_and_update().clone();
    match current {
      Progress::Event { event, at } => events(aged(event, at)),
      Progress::Ended(result) => {
        if let Ok(done) = &result {
          events(SignInEvent::Done { done: done.clone() });
        }
        return result;
      }
    }
    if progress.changed().await.is_err() {
      return Err(ApiError::internal("The sign-in ended without an answer"));
    }
  }
}

/// `event` as of now: the code of GitHub with the seconds it has left since it came at `at`
fn aged(event: SignInEvent, at: Instant) -> SignInEvent {
  match event {
    SignInEvent::DeviceCode { code, url, expires_in } => {
      let elapsed = u32::try_from(at.elapsed().as_secs()).unwrap_or(u32::MAX);
      SignInEvent::DeviceCode {
        code,
        url,
        expires_in: expires_in.saturating_sub(elapsed),
      }
    }
    event => event,
  }
}
