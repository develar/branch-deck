//! The REST API of the `CodeOwners` service (`/app/rest/v1`), as far as Branch Deck uses it: the lookup of a group of
//! code owners by its name (`GET groups/lookup/<name>`), one HTTP request per group, asked again once after a transient
//! failure. A group the service doesn't know is a group without members, never an error.

use domain_types::code_owners::{GroupMember, OwnerGroup};
use serde::Deserialize;
use std::time::Duration;
use tracing::{debug, instrument};

pub const CODEOWNERS_URL: &str = "https://codeowners.labs.jb.gg";

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// A lookup is a read of the service's database
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(15);
/// A transient failure is asked again once, after this pause
const RETRY_DELAY: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeOwnersError {
  /// 401: the Space token is missing, expired or revoked
  Unauthorized,
  Http {
    status: u16,
  },
  Timeout,
  /// No connection: offline, DNS, TLS
  Network(String),
  /// The answer is not what this version expects
  InvalidResponse(String),
}

impl CodeOwnersError {
  const fn is_transient(&self) -> bool {
    match self {
      Self::Http { status } => *status >= 500,
      Self::Network(_) => true,
      Self::Unauthorized | Self::Timeout | Self::InvalidResponse(_) => false,
    }
  }

  /// For the user
  #[must_use]
  pub fn message(&self) -> String {
    match self {
      Self::Unauthorized => "The CodeOwners service rejected the Space token".to_owned(),
      Self::Http { status } => format!("The CodeOwners service answered HTTP {status}"),
      Self::Timeout => "The CodeOwners service did not answer in time".to_owned(),
      Self::Network(message) => format!("The CodeOwners service is not reachable: {message}"),
      Self::InvalidResponse(message) => format!("Unexpected answer of the CodeOwners service: {message}"),
    }
  }
}

/// `GroupDto` of the lookup: the fields Branch Deck reads
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct GroupDto {
  name: Option<String>,
  members: Vec<GroupMemberDto>,
  reviewers: Vec<GroupMemberDto>,
}

/// `GroupMemberDto`: `isReviewer` is null of a member, so the reviewers come from their own list
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct GroupMemberDto {
  #[serde(alias = "spaceUsername", alias = "login")]
  username: Option<String>,
  github_username: Option<String>,
  rotated: Option<bool>,
  availability: Option<AvailabilityDto>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct AvailabilityDto {
  state: Option<String>,
}

impl GroupMemberDto {
  fn member(self) -> GroupMember {
    GroupMember {
      username: self.username.unwrap_or_default(),
      github_login: self.github_username.filter(|login| !login.trim().is_empty()),
      // an answer without the state says nothing of an absence
      available: self
        .availability
        .and_then(|availability| availability.state)
        .is_none_or(|state| state.eq_ignore_ascii_case("AVAILABLE")),
      rotated: self.rotated.unwrap_or(false),
    }
  }
}

pub struct CodeOwnersClient {
  http: reqwest::Client,
  base_url: String,
  retry_delay: Duration,
}

impl CodeOwnersClient {
  /// `base_url` without the trailing slash, e.g. `CODEOWNERS_URL`
  #[must_use]
  pub fn new(base_url: impl Into<String>, user_agent: &str) -> Self {
    let base_url = base_url.into().trim_end_matches('/').to_owned();
    let builder = reqwest::ClientBuilder::new().user_agent(user_agent).connect_timeout(CONNECT_TIMEOUT);
    Self {
      http: builder.build().expect("the HTTP client of the CodeOwners service"),
      base_url,
      retry_delay: RETRY_DELAY,
    }
  }

  /// The client with another wait before a lookup is asked again after a transient failure (a test waits none)
  #[must_use]
  pub const fn with_retry_delay(mut self, retry_delay: Duration) -> Self {
    self.retry_delay = retry_delay;
    self
  }

  /// The group of code owners `name`, as an `OWNERSHIP` file names it; one the service doesn't know (404) has no members
  ///
  /// # Errors
  /// A rejected token, a failure of the service or of the network, an answer this version doesn't read
  #[instrument(skip(self, token))]
  pub async fn lookup_group(&self, token: &str, name: &str) -> Result<OwnerGroup, CodeOwnersError> {
    let mut url = reqwest::Url::parse(&format!("{}/app/rest/v1/groups/lookup/", self.base_url)).map_err(|error| CodeOwnersError::InvalidResponse(error.to_string()))?;
    // the name is one segment, its spaces and slashes escaped
    url
      .path_segments_mut()
      .map_err(|()| CodeOwnersError::InvalidResponse("not a base URL".to_owned()))?
      .pop_if_empty()
      .push(name);
    let answer = match self.get(&url, token).await {
      Err(error) if error.is_transient() => {
        debug!(%error, "the lookup of a group failed, asked once more");
        tokio::time::sleep(self.retry_delay).await;
        self.get(&url, token).await
      }
      answer => answer,
    }?;
    let Some(group) = answer else {
      return Ok(OwnerGroup {
        name: name.to_owned(),
        ..OwnerGroup::default()
      });
    };
    Ok(OwnerGroup {
      name: group.name.filter(|it| !it.is_empty()).unwrap_or_else(|| name.to_owned()),
      reviewers: group.reviewers.into_iter().map(GroupMemberDto::member).collect(),
      members: group.members.into_iter().map(GroupMemberDto::member).collect(),
    })
  }

  /// One GET of the group at `url`; `None` for a group the service doesn't know
  async fn get(&self, url: &reqwest::Url, token: &str) -> Result<Option<GroupDto>, CodeOwnersError> {
    let response = self
      .http
      .get(url.clone())
      .bearer_auth(token)
      .header(reqwest::header::ACCEPT, "application/json")
      .timeout(LOOKUP_TIMEOUT)
      .send()
      .await
      .map_err(|error| {
        if error.is_timeout() {
          CodeOwnersError::Timeout
        } else {
          CodeOwnersError::Network(error.to_string())
        }
      })?;
    match response.status().as_u16() {
      200..=299 => response
        .json::<GroupDto>()
        .await
        .map(Some)
        .map_err(|error| CodeOwnersError::InvalidResponse(error.to_string())),
      401 => Err(CodeOwnersError::Unauthorized),
      404 => Ok(None),
      status => Err(CodeOwnersError::Http { status }),
    }
  }
}

impl std::fmt::Display for CodeOwnersError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&self.message())
  }
}
