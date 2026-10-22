//! The `/patronus` command of a GitHub pull request. Patronus links a robot to a pull request only when a comment of
//! the pull request started it: then the code-owner approvals of the pull request count, Patronus reports in it and
//! closes it once merged. So a Safe Push of a branch with a pull request is this comment, and the bot answers with a
//! reaction: 👍 started, 👎 not (with a `⚠️` comment saying why). The other writes of the app on GitHub keep the pull
//! requests of a stack in line (`pull_request_writes.rs`).

use crate::pull_requests::{GitHubRepository, GitHubWriteError};
use github::service::{GitHubService, RequestKind};
use serde::Deserialize;
use sync_types::patronus::{PatronusCommandComment, PatronusCommandReply, PatronusCommandSubject, PatronusPushMode};
use tracing::instrument;

/// The repository of the pull request a `/patronus` comment goes to is the one the review requests name, else the
/// mirror of the Space repository: neither is known
pub const UNKNOWN_GITHUB_REPOSITORY_MESSAGE: &str =
  "The GitHub repository of the branches is unknown: the Space settings name no GitHub mirror; run git config branchdeck.githubRepository <owner>/<name>";

/// The reason of a rejection is a hint in a toast, not the whole comment
const MAX_REASON_CHARS: usize = 500;

/// The comment text: a merge of an own branch keeps it (the sync archives a merged virtual branch, as after a Safe Push
/// started by the app); a merge of a pull request the user reviewed deletes its branch, as a merge on GitHub does
pub fn patronus_command_body(mode: PatronusPushMode, subject: PatronusCommandSubject) -> Result<&'static str, String> {
  match (mode, subject) {
    (PatronusPushMode::RebaseAutosquash, PatronusCommandSubject::OwnBranch) => Ok("/patronus merge --keep-source-branch"),
    (PatronusPushMode::RebaseAutosquash, PatronusCommandSubject::ReviewedPullRequest) => Ok("/patronus merge"),
    (PatronusPushMode::DryRun, _) => Ok("/patronus dry-run"),
    (other, _) => Err(format!("A pull request comment starts a Safe Push or a Dry Run, not {other:?}")),
  }
}

/// Posts the command as a comment of pull request `number` (a regular comment: Patronus ignores review comments)
#[instrument(skip(github))]
pub async fn post_patronus_command(
  github: &GitHubService,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  mode: PatronusPushMode,
  subject: PatronusCommandSubject,
) -> Result<PatronusCommandComment, GitHubWriteError> {
  let body = patronus_command_body(mode, subject).map_err(GitHubWriteError::Refused)?;
  let number = number.to_string();
  let path = ["repos", repository.owner.as_str(), repository.name.as_str(), "issues", number.as_str(), "comments"];
  let (path, body) = (&path, &serde_json::json!({"body": body}));
  let comment: GhComment = github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      client.post(&token, path, body).await
    })
    .await?;
  Ok(PatronusCommandComment {
    id: comment.id.to_string(),
    url: comment.html_url,
  })
}

/// How the bot answered the command comment `comment_id`: one request for its reactions (conditional, so a poll that
/// sees no change costs no rate limit), one more for the reason of a rejection
#[instrument(skip(github))]
pub async fn read_patronus_command_reply(
  github: &GitHubService,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  comment_id: &str,
) -> Result<PatronusCommandReply, GitHubWriteError> {
  // the id comes from the page and goes into the request path
  if comment_id.is_empty() || !comment_id.bytes().all(|byte| byte.is_ascii_digit()) {
    return Err(GitHubWriteError::Refused(format!("{comment_id:?} is not the id of a GitHub comment")));
  }
  let number = number.to_string();
  let comment_path = ["repos", repository.owner.as_str(), repository.name.as_str(), "issues", "comments", comment_id];
  let comments_path = ["repos", repository.owner.as_str(), repository.name.as_str(), "issues", number.as_str(), "comments"];
  let (comment_path, comments_path) = (&comment_path, &comments_path);
  github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      let comment: GhComment = client.get(&token, comment_path, &[]).await?;
      let reactions = comment.reactions.unwrap_or_default();
      if reactions.minus_one > 0 {
        // `since` compares the update time: the bot's answer is newer than the command
        let comments: Vec<GhComment> = client.get(&token, comments_path, &[("since", &comment.created_at), ("per_page", "100")]).await?;
        return Ok(PatronusCommandReply::Rejected {
          reason: rejection_reason(&comments, comment.id),
        });
      }
      if reactions.plus_one > 0 {
        return Ok(PatronusCommandReply::Accepted);
      }
      Ok(PatronusCommandReply::Pending)
    })
    .await
    .map_err(GitHubWriteError::from)
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
