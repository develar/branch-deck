//! The open pull request of a subject, as a Safe Push of it reads it (spec/branch/pull-requests.spec.md,
//! spec/branch/next-step.spec.md rows 5 and 5′): where it goes (`base_of`: the target, a virtual branch of the user that
//! every such pull request leaves for the target, or another base that may be meant), the pull request a robot starts
//! from (`start_pull_request`: one into the target or, of a branch, into a virtual branch it retargets first, D11), and what a push of new commits does to its approvals by the rules of its base
//! (`dismisses_approvals`, a fact without an answer never read as "keeps them"). The app (the view of the session, the
//! readiness asks) and `bd` (`bd push`, `bd safe-push`, `bd request-review`) read these, never a copy.

use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use domain_types::error::ApiError;
use domain_types::pull_request::{BranchRules, PullRequest, PullRequestBase, PullRequestState, ReviewerState};
use domain_types::session::{Fact, FactState};

/// Where the pull request goes: `target` (or no base read), a virtual branch of `prefix` (`{prefix}/virtual/<name>`, any
/// name: a parent's, another branch of the table, one that landed and is gone), else another base. A pull request into a
/// virtual branch that GitHub lists in a stack into `target` is a layer of it: GitHub's membership decides, never a
/// fact of the session that may change its answer (docs/design/github-stacks.md)
#[must_use]
pub fn base_of(pull_request: &PullRequest, target: &str, prefix: &str) -> PullRequestBase {
  let base = pull_request.base_ref_name.as_str();
  if base.is_empty() || base == target {
    return PullRequestBase::Target;
  }
  let into_target = pull_request.stack.as_ref().is_some_and(|stack| stack.base_ref_name == target);
  match strip_branch_namespace(base, prefix, BranchKind::Virtual).filter(|name| !name.is_empty()) {
    Some(parent) if into_target => PullRequestBase::Layer { parent: parent.to_string() },
    Some(from) => PullRequestBase::Virtual {
      from: from.to_string(),
      retarget: target.to_string(),
    },
    None => PullRequestBase::Other { base: base.to_string() },
  }
}

/// Where the pull request of a branch goes (`base_of`), `layer_parent` the branch below it in its stack of GitHub
/// (`RepositoryView::github_layers`): into that branch's virtual branch it is a layer too while GitHub doesn't list it in
/// the stack yet (the link of the stack follows its creation, or failed), so nothing retargets it into the target
#[must_use]
pub fn layer_base_of(pull_request: &PullRequest, target: &str, prefix: &str, layer_parent: Option<&str>) -> PullRequestBase {
  match base_of(pull_request, target, prefix) {
    PullRequestBase::Virtual { from, .. } if layer_parent == Some(from.as_str()) => PullRequestBase::Layer { parent: from },
    base => base,
  }
}

/// Where the pull request of a subject goes for its start: `base_of`, but of a whole stack a virtual branch is another
/// base too, as its stack branch is no virtual branch and no retarget touches it
#[must_use]
pub fn start_base(pull_request: &PullRequest, target: &str, prefix: &str, whole_stack: bool) -> PullRequestBase {
  match base_of(pull_request, target, prefix) {
    PullRequestBase::Virtual { .. } if whole_stack => PullRequestBase::Other {
      base: pull_request.base_ref_name.clone(),
    },
    base => base,
  }
}

/// A robot starts from a pull request that goes there: into the target, or into a virtual branch, which the start
/// retargets into the target first (D11); never one into another base, whose approvals count for no start into the target,
/// nor a layer of a stack of GitHub, which Patronus would merge into the branch below it
#[must_use]
pub const fn starts_from(base: &PullRequestBase) -> bool {
  !matches!(base, PullRequestBase::Other { .. } | PullRequestBase::Layer { .. })
}

/// The pull request a robot of the subject would start from (`whole_stack`: of the whole stack, else of a branch): its
/// open pull request that goes into the target or, of a branch, into a virtual branch of `prefix`, which the start
/// retargets first (`start_base`, `starts_from`), while Patronus names the GitHub repository the branches are mirrored
/// to; else none, and the robot starts through the API, which counts no approval
#[must_use]
pub fn start_pull_request<'a>(pull_request: Option<&'a PullRequest>, target: &str, prefix: &str, github_repository: Option<&str>, whole_stack: bool) -> Option<&'a PullRequest> {
  github_repository?;
  pull_request.filter(|pull_request| pull_request.state == PullRequestState::Open && starts_from(&start_base(pull_request, target, prefix, whole_stack)))
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
/// not answered yet (or nobody asks for) `Loading`, as is the value of the cached frame of an earlier run, which a start
/// waits on. A stale answer is the last one GitHub gave
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
