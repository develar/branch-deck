//! The `/patronus` command of a GitHub pull request. Patronus links a robot to a pull request only when a comment of
//! the pull request started it: then the code-owner approvals of the pull request count, Patronus reports in it and
//! closes it once merged. So a Safe Push of a branch with a pull request is this comment, and the bot answers with a
//! reaction: 👍 started, 👎 not (with a `⚠️` comment saying why). The one write of the app on GitHub.

use crate::pull_requests::{GitHubRepository, run_gh};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use sync_types::patronus::{PatronusCommandComment, PatronusCommandReply, PatronusPushMode};
use sync_types::pull_request::PullRequestAvailability;
use tracing::instrument;

/// The reason of a rejection is a hint in a toast, not the whole comment
const MAX_REASON_CHARS: usize = 500;

/// The comment text: a merge keeps the source branch (the sync archives a merged virtual branch, as after a Safe Push
/// started by the app)
pub fn patronus_command_body(mode: PatronusPushMode) -> Result<&'static str, String> {
  match mode {
    PatronusPushMode::RebaseAutosquash => Ok("/patronus merge --keep-source-branch"),
    PatronusPushMode::DryRun => Ok("/patronus dry-run"),
    other => Err(format!("A pull request comment starts a Safe Push or a Dry Run, not {other:?}")),
  }
}

/// Posts the command as a comment of pull request `number` (a regular comment: Patronus ignores review comments)
#[instrument(skip(gh))]
pub fn post_patronus_command(
  gh: &Path,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  mode: PatronusPushMode,
  timeout: Duration,
) -> Result<PatronusCommandComment, String> {
  let body = format!("body={}", patronus_command_body(mode)?);
  let endpoint = format!("repos/{}/{}/issues/{number}/comments", repository.owner, repository.name);
  let output = run_gh(gh, repository_path, &["api", "--method", "POST", &endpoint, "-f", &body], timeout).map_err(gh_failure)?;
  let comment: GhComment = serde_json::from_slice(&output).map_err(|e| format!("Unexpected gh output: {e}"))?;
  Ok(PatronusCommandComment {
    id: comment.id.to_string(),
    url: comment.html_url,
  })
}

/// How the bot answered the command comment `comment_id`: one `gh api` for its reactions, one more for the reason of a
/// rejection
#[instrument(skip(gh))]
pub fn read_patronus_command_reply(
  gh: &Path,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  comment_id: &str,
  timeout: Duration,
) -> Result<PatronusCommandReply, String> {
  let endpoint = format!("repos/{}/{}/issues/comments/{comment_id}", repository.owner, repository.name);
  let output = run_gh(gh, repository_path, &["api", &endpoint], timeout).map_err(gh_failure)?;
  let comment: GhComment = serde_json::from_slice(&output).map_err(|e| format!("Unexpected gh output: {e}"))?;
  let reactions = comment.reactions.unwrap_or_default();
  if reactions.minus_one > 0 {
    // `since` compares the update time: the bot's answer is newer than the command
    let endpoint = format!(
      "repos/{}/{}/issues/{number}/comments?since={}&per_page=100",
      repository.owner, repository.name, comment.created_at
    );
    let output = run_gh(gh, repository_path, &["api", &endpoint], timeout).map_err(gh_failure)?;
    let comments: Vec<GhComment> = serde_json::from_slice(&output).map_err(|e| format!("Unexpected gh output: {e}"))?;
    return Ok(PatronusCommandReply::Rejected {
      reason: rejection_reason(&comments, comment.id),
    });
  }
  if reactions.plus_one > 0 {
    return Ok(PatronusCommandReply::Accepted);
  }
  Ok(PatronusCommandReply::Pending)
}

/// The first `⚠️` comment of a bot after the command, without the sign
pub fn rejection_reason(comments: &[GhComment], command_id: u64) -> Option<String> {
  let reply = comments
    .iter()
    .filter(|comment| comment.id > command_id && comment.user.as_ref().is_some_and(|user| user.kind.as_deref() == Some("Bot")))
    .find_map(|comment| comment.body.as_deref()?.trim_start().strip_prefix("⚠️"))?;
  let reason = reply.trim();
  if reason.is_empty() {
    return None;
  }
  Some(match reason.char_indices().nth(MAX_REASON_CHARS) {
    Some((end, _)) => format!("{}…", &reason[..end]),
    None => reason.to_string(),
  })
}

fn gh_failure(availability: PullRequestAvailability) -> String {
  match availability {
    PullRequestAvailability::GhMissing => "The gh CLI is not installed".to_string(),
    PullRequestAvailability::NotAuthenticated => "gh is not logged in: run gh auth login".to_string(),
    PullRequestAvailability::Error { message } => message,
    PullRequestAvailability::NotGitHub | PullRequestAvailability::Available { .. } => "The repository is not on GitHub".to_string(),
  }
}

/// An issue comment of the GitHub REST API
#[derive(Debug, Deserialize)]
pub struct GhComment {
  pub id: u64,
  #[serde(default)]
  pub html_url: String,
  #[serde(default)]
  pub created_at: String,
  pub body: Option<String>,
  pub user: Option<GhUser>,
  pub reactions: Option<GhReactions>,
}

#[derive(Debug, Deserialize)]
pub struct GhUser {
  /// `User` or `Bot`
  #[serde(rename = "type")]
  pub kind: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct GhReactions {
  #[serde(rename = "+1", default)]
  pub plus_one: u32,
  #[serde(rename = "-1", default)]
  pub minus_one: u32,
}
