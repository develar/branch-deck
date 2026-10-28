//! The sign-ins of the app's accounts (spec/repository/github-sign-in.spec.md, youtrack.spec.md): what a window asks
//! (`SignIn`), what the sign-in tells it on the way (`SignInEvent`) and how it ended (`SignInDone`), and the status of
//! both accounts (`AuthStatus`), which the sessions keep as their fact `auth` (`remote_facts::sign_in`). A sign-in has
//! no repository as its subject: it is a command of the app, never an intent of a session.

use crate::pull_request::GitHubAuthStatus;
use crate::youtrack::{YouTrackAuthStatus, YouTrackUser};
use serde::{Deserialize, Serialize};

/// What a window signs in to
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "provider", rename_all = "camelCase")]
pub enum SignIn {
  /// The device flow of the Branch Deck OAuth app: a code the user enters on GitHub
  GitHub,
  /// The browser sign-in through the Hub of the YouTrack at `url`; with `token`, the token the user created in YouTrack
  /// (Profile → Account Security) instead, kept once YouTrack accepted it
  #[serde(rename_all = "camelCase")]
  YouTrack {
    url: String,
    #[serde(default)]
    #[cfg_attr(feature = "specta", specta(optional))]
    token: Option<String>,
  },
  /// The one-shot browser sign-in that gets the `youtrack` CLI a permanent token of the YouTrack at `url`
  YouTrackCli { url: String },
}

impl SignIn {
  #[must_use]
  pub const fn provider(&self) -> SignInProvider {
    match self {
      Self::GitHub => SignInProvider::GitHub,
      Self::YouTrack { .. } => SignInProvider::YouTrack,
      Self::YouTrackCli { .. } => SignInProvider::YouTrackCli,
    }
  }
}

/// Who a sign-in runs with: one sign-in of each at a time, a second call joins it
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SignInProvider {
  GitHub,
  YouTrack,
  YouTrackCli,
}

impl SignInProvider {
  /// The account of the app a sign-in of the provider changes; the `youtrack` CLI keeps its token itself
  #[must_use]
  pub const fn account(self) -> Option<Account> {
    match self {
      Self::GitHub => Some(Account::GitHub),
      Self::YouTrack => Some(Account::YouTrack),
      Self::YouTrackCli => None,
    }
  }
}

/// An account of the app, which a sign-out removes the token of
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum Account {
  GitHub,
  YouTrack,
}

/// What a sign-in tells the windows that run or joined it, in order; the last one is `done`, unless it failed
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SignInEvent {
  /// It asks the service, and the user has nothing to do: the code of GitHub or the page of the Hub is asked, a pasted
  /// token is checked, the `youtrack` CLI gets its token
  Waiting,
  /// GitHub waits until the user entered `code` at `url`, for `expires_in` more seconds
  #[serde(rename_all = "camelCase")]
  DeviceCode { code: String, url: String, expires_in: u32 },
  /// The Hub waits until the user signed in on the page at `url` in the browser
  Browser { url: String },
  /// It ended, or the sign-in it joined did
  Done { done: SignInDone },
}

/// How a sign-in ended
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "provider", rename_all = "camelCase")]
pub enum SignInDone {
  /// The token of the GitHub user `login` is kept
  GitHub { login: String },
  /// The token of `user` of the YouTrack at `url` is kept
  YouTrack { url: String, user: YouTrackUser },
  /// The `youtrack` CLI took its token: "Signed youtrack in to yt.example as jane"
  YouTrackCli { title: String },
}

/// The sign-ins of the app's accounts and where their tokens come from
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
  pub github: GitHubAuthStatus,
  pub youtrack: YouTrackAuthStatus,
}
