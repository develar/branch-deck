//! Asks GitHub with the token chain: a request rejected for its token is asked again with the next token, so a stale
//! `GH_TOKEN` or a token the organization does not allow does not hide the one that works. A listing refused for the
//! rate limit is not asked again with that token before GitHub's `retry_at`.

use crate::client::{GitHubClient, GitHubError};
use crate::token::{Token, TokenChain, token_key};
use domain_types::pull_request::TokenSource;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use sync_utils::timestamp::unix_now;

/// Enough for every source to be tried once
const MAX_TOKENS: usize = 4;

/// Why GitHub could not be asked
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubFailure {
  /// No source had a token
  SignedOut,
  /// The request failed with the token of `source` (the last one tried)
  Failed { error: GitHubError, source: TokenSource },
}

/// What a request asks. A listing of the repository (a search with the repository's visibility) that GitHub answers
/// "not found" or "forbidden" proves the token can't see the repository, and the next token is tried. A request about
/// one pull request or comment may be refused for another reason (it is gone), and is asked with the token the
/// listings found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKind {
  Listing,
  Single,
}

pub struct GitHubService {
  pub client: GitHubClient,
  pub tokens: TokenChain,
  /// By repository: why the last listing failed with every token. Repeated while the chain skips them all, instead of
  /// "signed out".
  denied: Mutex<HashMap<String, GitHubFailure>>,
  /// By token (`Token::key`): until when GitHub's rate limit refused a listing with it (Unix seconds). A listing with
  /// the token is not asked before then: a refresh after every sync, push and focus would extend the penalty. A
  /// single request (a write the user asked for) still goes, its limits are GitHub's to tell.
  rate_limited: Mutex<HashMap<u64, u32>>,
  /// The tokens GitHub refused a team's slug (`INSUFFICIENT_SCOPES`, no `read:org`): a token of `GH_TOKEN` or of
  /// git. Remembered until the token changes, so a listing asks the query without teams at once
  without_read_org: Mutex<HashSet<u64>>,
  /// GitHub's schema refused the fields of a stack of GitHub (`undefinedField`): a listing asks without them
  without_stacks: AtomicBool,
}

impl GitHubService {
  pub fn new(client: GitHubClient, tokens: TokenChain) -> Self {
    Self {
      client,
      tokens,
      denied: Mutex::new(HashMap::new()),
      rate_limited: Mutex::new(HashMap::new()),
      without_read_org: Mutex::new(HashSet::new()),
      without_stacks: AtomicBool::new(false),
    }
  }

  /// Runs `request` about `repository` (`owner/name`) with the first token GitHub accepts. A 401 rejects the token
  /// for good and the next one is tried; so is the next after a listing without access (403, 404, SAML SSO).
  /// `request` gets a handle of the client and the token (owned: a borrowing async closure would not be `Send` for a
  /// Tauri command).
  pub async fn ask<T, F, R>(&self, repository_path: &str, repository: &str, kind: RequestKind, request: F) -> Result<T, GitHubFailure>
  where
    F: Fn(GitHubClient, String) -> R,
    R: Future<Output = Result<T, GitHubError>>,
  {
    let mut failure: Option<GitHubFailure> = None;
    let mut tried: Vec<Token> = Vec::new();
    for _ in 0..MAX_TOKENS {
      let Some(token) = self.tokens.token(repository_path, repository).await else {
        break;
      };
      if tried.contains(&token) {
        break;
      }
      if kind == RequestKind::Listing
        && let Some(retry_at) = self.rate_limited_until(&token, unix_now())
      {
        return Err(GitHubFailure::Failed {
          error: GitHubError::RateLimited { retry_at },
          source: token.source,
        });
      }
      match request(self.client.clone(), token.value.clone()).await {
        Ok(value) => {
          self.tokens.accept(repository, &token);
          self.denied.lock().unwrap().remove(repository);
          return Ok(value);
        }
        Err(GitHubError::Unauthorized) => self.tokens.reject(&token),
        Err(error @ (GitHubError::NoAccess { .. } | GitHubError::SsoRequired { .. })) if kind == RequestKind::Listing => {
          self.tokens.deny(repository, &token);
          let denied = GitHubFailure::Failed { error, source: token.source };
          self.denied.lock().unwrap().insert(repository.to_string(), denied.clone());
          failure = Some(denied);
          tried.push(token);
          continue;
        }
        Err(error) => {
          if let (RequestKind::Listing, GitHubError::RateLimited { retry_at }) = (kind, &error) {
            self.rate_limited.lock().unwrap().insert(token.key(), *retry_at);
          }
          return Err(GitHubFailure::Failed { error, source: token.source });
        }
      }
      failure = Some(GitHubFailure::Failed {
        error: GitHubError::Unauthorized,
        source: token.source,
      });
      tried.push(token);
    }
    Err(failure.or_else(|| self.denied.lock().unwrap().get(repository).cloned()).unwrap_or(GitHubFailure::SignedOut))
  }

  /// Whether GitHub refused `token` a query that needs `read:org`
  pub fn lacks_read_org(&self, token: &str) -> bool {
    self.without_read_org.lock().unwrap().contains(&token_key(token))
  }

  /// GitHub refused `token` a query that needs `read:org` (`INSUFFICIENT_SCOPES`)
  pub fn note_lacks_read_org(&self, token: &str) {
    self.without_read_org.lock().unwrap().insert(token_key(token));
  }

  /// Whether GitHub's schema refused the fields of a stack of GitHub once: a listing asks without them from then on
  pub fn lacks_stacks(&self) -> bool {
    self.without_stacks.load(Ordering::Relaxed)
  }

  /// GitHub's schema refused the fields of a stack (`undefinedField`): github.com has one schema for every token
  pub fn note_lacks_stacks(&self) {
    self.without_stacks.store(true, Ordering::Relaxed);
  }

  /// Until when the rate limit holds listings with `token` back, if it still does at `now`
  fn rate_limited_until(&self, token: &Token, now: u32) -> Option<u32> {
    let mut limited = self.rate_limited.lock().unwrap();
    match limited.get(&token.key()) {
      Some(&retry_at) if retry_at > now => Some(retry_at),
      Some(_) => {
        limited.remove(&token.key());
        None
      }
      None => None,
    }
  }
}
