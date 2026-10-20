//! The code-owner review of a branch as the outputs print it: `bd safe-push`, `bd request-review`, and the error
//! `approvals_missing`, which carries it

use schemars::JsonSchema;
use serde::Serialize;

/// The code-owner review of the branch, as the pre-check of Patronus answers it
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewSummary {
  /// The code owners still have to approve: a Safe Push fails without these approvals
  pub required: bool,
  /// The branch has too many commits to check (500), as the IDE skips it
  pub skipped: bool,
  /// `space_merge_request`, or `pull_request` for branches reviewed on GitHub: where the approvals count
  pub counts_in: String,
  /// An open Space merge request of the branch whose approvals count; a start through the API links it
  pub merge_request: Option<MergeRequestSummary>,
  /// The groups whose approval is missing; when Patronus evaluated the pull request of the start, every group the
  /// changes need, each with `approved`
  pub groups: Vec<ReviewGroupSummary>,
  /// Who approved the pull request the Safe Push starts from with an approval that counts: the members Patronus counts
  /// when it evaluated the pull request, else every approver (an older Patronus doesn't say whose count)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub approved_on_github: Vec<String>,
  /// What the review means for the start: `not_required`, `approved`, `missing` (refused as `approvals_missing`), or
  /// `unknown` (Patronus doesn't say whether the approvals of the pull request count, or the branch is too long: the
  /// robot decides)
  pub readiness: String,
  /// Why: `approvals_missing`, `no_pull_request_context` (a start through the API counts no approval of a pull
  /// request), `no_eligible_reviewer` (a group has nobody who can approve) for `missing`; `approvals_not_evaluated` or
  /// `skipped` for `unknown`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub readiness_reason: Option<String>,
  /// The code owners of the missing groups asked to review the pull request who haven't approved: the start waits for
  /// them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub waiting_for: Vec<PersonSummary>,
  /// Approvers of the pull request Patronus doesn't count (no eligible code owner of the files, e.g. a bot)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub not_counted: Vec<PersonSummary>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewGroupSummary {
  pub name: String,
  pub reviewers: Vec<ReviewerSummary>,
  /// The files the group reviews: the ones Patronus names, else the ones its `OWNERSHIP` entries own; empty while
  /// neither says
  pub paths: Vec<String>,
  /// The group reviews every file of the branch
  pub branch_wide: bool,
  /// Whether the pull request has the approvals of the group; only when Patronus evaluated it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub approved: Option<bool>,
  /// How many more approvals count for the group; only when Patronus says
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub needed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewerSummary {
  /// The Space username; `null` for a member without one (a bot member)
  pub username: Option<String>,
  pub full_name: String,
  /// The GitHub login, for `--reviewer`; `null` when Patronus doesn't say
  pub github_login: Option<String>,
  pub available: bool,
  /// Patronus suggests asking this reviewer
  pub suggested: bool,
  /// What the review of the member in the pull request counts for: `approved`, `author_self_approved`, `reviewing`
  /// (asked, or the latest review comments or requests changes), `not_added` or `not_eligible`; only when Patronus
  /// evaluated the pull request
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub approval_state: Option<String>,
}

/// A person of the review: the full name Patronus knows, the GitHub login when known
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PersonSummary {
  pub login: Option<String>,
  pub full_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MergeRequestSummary {
  /// e.g. `IJ-CR-7`
  pub key: String,
  pub url: String,
}
