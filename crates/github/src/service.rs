//! Asks GitHub with the token chain: a request rejected for its token is asked again with the next token, so a stale
//! `GH_TOKEN` or a token the organization does not allow does not hide the one that works.

use crate::client::{GitHubClient, GitHubError};
use crate::token::{Token, TokenChain};
use domain_types::pull_request::TokenSource;
use std::collections::HashMap;
use std::sync::Mutex;

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
}

impl GitHubService {
  pub fn new(client: GitHubClient, tokens: TokenChain) -> Self {
    Self {
      client,
      tokens,
      denied: Mutex::new(HashMap::new()),
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
        Err(error) => return Err(GitHubFailure::Failed { error, source: token.source }),
      }
      failure = Some(GitHubFailure::Failed {
        error: GitHubError::Unauthorized,
        source: token.source,
      });
      tried.push(token);
    }
    Err(failure.or_else(|| self.denied.lock().unwrap().get(repository).cloned()).unwrap_or(GitHubFailure::SignedOut))
  }
}
