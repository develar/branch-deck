//! The open pull request of a subject, as a Safe Push of it reads it (spec/branch/pull-requests.spec.md,
//! spec/branch/next-step.spec.md rows 5 and 5′): where it goes (`base_of`: the target, a virtual branch of the user that
//! every such pull request leaves for the target, or another base that may be meant), the pull request a robot starts
//! from (`start_pull_request`), and what a push of new commits does to its approvals by the rules of its base
//! (`dismisses_approvals`, a fact without an answer never read as "keeps them"). The app (the view of the session, the
//! readiness asks) and `bd` (`bd push`, `bd safe-push`, `bd request-review`) read these, never a copy.

use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use domain_types::error::ApiError;
use domain_types::pull_request::{BranchRules, PullRequest, PullRequestBase, PullRequestState, ReviewerState};
use domain_types::session::{Fact, FactState};

/// Where the pull request goes: `target` (or no base read), a virtual branch of `prefix` (`{prefix}/virtual/<name>`, any
/// name: a parent's, another branch of the table, one that landed and is gone), else another base
#[must_use]
pub fn base_of(pull_request: &PullRequest, target: &str, prefix: &str) -> PullRequestBase {
  let base = pull_request.base_ref_name.as_str();
  if base.is_empty() || base == target {
    return PullRequestBase::Target;
  }
  match strip_branch_namespace(base, prefix, BranchKind::Virtual).filter(|name| !name.is_empty()) {
    Some(from) => PullRequestBase::Virtual {
      from: from.to_string(),
      retarget: target.to_string(),
    },
    None => PullRequestBase::Other { base: base.to_string() },
  }
}

/// The pull request a robot of the subject would start from: its open pull request into `target` (one without a base
/// read counts), while Patronus names the GitHub repository the branches are mirrored to; else none, and the robot
/// starts through the API, which counts no approval
#[must_use]
pub fn start_pull_request<'a>(pull_request: Option<&'a PullRequest>, target: &str, github_repository: Option<&str>) -> Option<&'a PullRequest> {
  github_repository?;
  pull_request.filter(|pull_request| pull_request.state == PullRequestState::Open && (pull_request.base_ref_name.is_empty() || pull_request.base_ref_name == target))
}

/// What a push of new commits does to the approvals of a pull request, by the rules of its base
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RulesAnswer<'a> {
  /// Nobody approved it, or the rules keep the approvals
  Keep,
  /// The rules dismiss stale reviews on push: the approvals are gone with the push
  Dismiss,
  /// The rules can't be read (`reason`): whether the push keeps the approvals is not known, never "keeps them"
  Unknown { reason: &'a ApiError },
  /// The rules are being read
  Loading,
}

/// What a push of new commits into `pull_request` does to its approvals, by `rules`, the rules of its base as the session
/// (or `bd`, one GET) read them: a pull request nobody approved keeps what it has; a read that failed is `Unknown`, one
/// not answered yet (or nobody asks for) `Loading`. A stale answer is the last one GitHub gave
#[must_use]
pub fn dismisses_approvals<'a>(rules: Option<&'a Fact<BranchRules>>, pull_request: &PullRequest) -> RulesAnswer<'a> {
  if !pull_request.reviewers.iter().any(|reviewer| reviewer.state == ReviewerState::Approved) {
    return RulesAnswer::Keep;
  }
  let Some(rules) = rules else {
    return RulesAnswer::Loading;
  };
  match (&rules.state, &rules.value) {
    (FactState::Unavailable { reason }, _) => RulesAnswer::Unknown { reason },
    (FactState::Fresh | FactState::Stale { .. }, Some(rules)) if rules.dismiss_stale_reviews_on_push => RulesAnswer::Dismiss,
    (FactState::Fresh | FactState::Stale { .. }, Some(_)) => RulesAnswer::Keep,
    _ => RulesAnswer::Loading,
  }
}
