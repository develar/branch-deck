//! The rules GitHub applies to pull requests into a branch, as far as the Safe Push form tells them before a push: a
//! repository whose rules dismiss stale reviews on push takes the approvals of an open pull request away with the push
//! of new commits. One `GET rules/branches/<branch>`, readable with read access to the repository (classic branch
//! protection needs admin rights and is not read).

use crate::pull_requests::GitHubRepository;
use github::service::{GitHubFailure, GitHubService, RequestKind};
use serde::{Deserialize, Serialize};
use tracing::instrument;

/// GitHub lists at most this many rules per page; a branch has a handful
const RULES_PER_PAGE: &str = "100";

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetBranchRulesParams {
  pub repository_path: String,
  /// The branch pull requests merge into, as GitHub names it (`master`)
  pub branch: String,
}

/// What the rules of a branch do to the pull requests into it
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchRules {
  /// A push of new commits dismisses the approvals of a pull request into the branch
  pub dismiss_stale_reviews_on_push: bool,
}

/// One rule of a branch (`GET repos/<owner>/<name>/rules/branches/<branch>`); only the pull request rule is read
#[derive(Debug, Deserialize)]
struct GhBranchRule {
  #[serde(rename = "type")]
  kind: String,
  #[serde(default)]
  parameters: Option<GhPullRequestRuleParameters>,
}

#[derive(Debug, Default, Deserialize)]
struct GhPullRequestRuleParameters {
  #[serde(default)]
  dismiss_stale_reviews_on_push: bool,
}

/// The rules of `branch` in `repository`: every ruleset that applies to it, active ones only (GitHub leaves out the
/// rulesets in evaluate mode); a conditional GET, so asking again costs no rate limit while they are the same
#[instrument(skip(github))]
pub async fn read_branch_rules(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, branch: &str) -> Result<BranchRules, GitHubFailure> {
  let path = ["repos", repository.owner.as_str(), repository.name.as_str(), "rules", "branches", branch];
  let path = &path;
  let rules: Vec<GhBranchRule> = github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      client.get(&token, path, &[("per_page", RULES_PER_PAGE)]).await
    })
    .await?;
  Ok(BranchRules {
    dismiss_stale_reviews_on_push: rules
      .iter()
      .filter(|rule| rule.kind == "pull_request")
      .any(|rule| rule.parameters.as_ref().is_some_and(|parameters| parameters.dismiss_stale_reviews_on_push)),
  })
}
