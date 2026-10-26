//! Every pull request of a virtual branch goes into the baseline, with the commits of the branches it is built on: the
//! approvals Patronus counts are the ones of a pull request into its target (`docs/design/landing.md`). An open pull
//! request whose base is a virtual branch of the user (`{prefix}/virtual/…`: a parent's branch, from before, or the one
//! of a parent that landed; `domain_policy::pull_request::base_of`) is retargeted to the baseline (`PATCH pulls/<n>`,
//! `operations::pull_request`), before the push: by `bd push` for the branches it pushes (D3a) and by `bd safe-push` for
//! the branch it starts and the branches its push takes along (D11), as the app's push does. A pull request into any
//! other base (a release branch) is left as it is: that base may be meant.

use crate::request_review_command::availability_error;
use bd_core::context::RepoContext;
use domain_policy::pull_request::base_of;
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestBase, PullRequestState, PullRequestsResult, ReviewerState};
use github::service::GitHubService;
use operations::pull_request::retarget_each;
use remote_facts::pull_requests::{GitHubWriteError, failure_message, list_pull_requests};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::BTreeMap;
use sync_core::github_repository::GitHubRepository;
use tokio::runtime::Runtime;

/// An open pull request whose base was a virtual branch, retargeted to the baseline
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RetargetedPullRequest {
  pub branch: String,
  pub number: u32,
  pub url: String,
  /// The virtual branch it went into, e.g. `jane/virtual/auth`
  pub old_base: String,
  /// The baseline as the remote names it, e.g. `master`
  pub new_base: String,
  /// Who had approved it: GitHub may dismiss the approvals of a pull request whose base changes; tell the user
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub approved_by: Vec<String>,
}

/// The retargets of the pull requests of `names` (`pull_requests` by virtual branch name): open, into a virtual branch of
/// `prefix`, to `base`, in the order of `names`
pub(crate) fn virtual_base_retargets<'a>(
  pull_requests: &BTreeMap<String, PullRequest>,
  names: impl IntoIterator<Item = &'a str>,
  prefix: &str,
  base: &str,
) -> Vec<RetargetedPullRequest> {
  names
    .into_iter()
    .filter_map(|name| {
      let pull_request = pull_requests.get(name)?;
      let PullRequestBase::Virtual { .. } = base_of(pull_request, base, prefix) else {
        return None;
      };
      (pull_request.state == PullRequestState::Open).then(|| RetargetedPullRequest::of(name, pull_request, base))
    })
    .collect()
}

impl RetargetedPullRequest {
  /// The retarget of `pull_request` of the virtual branch `branch` from its base to `new_base`, with who approved it
  pub(crate) fn of(branch: &str, pull_request: &PullRequest, new_base: &str) -> Self {
    Self {
      branch: branch.to_string(),
      number: pull_request.number,
      url: pull_request.url.clone(),
      old_base: pull_request.base_ref_name.clone(),
      new_base: new_base.to_string(),
      approved_by: pull_request
        .reviewers
        .iter()
        .filter(|reviewer| reviewer.state == ReviewerState::Approved)
        .map(|reviewer| reviewer.login.clone())
        .collect(),
    }
  }
}

/// What the retargets did: the pull requests GitHub retargeted, and a warning per one it refused or couldn't be asked about
#[derive(Debug, Default)]
pub(crate) struct Retargets {
  pub(crate) retargeted: Vec<RetargetedPullRequest>,
  pub(crate) warnings: Vec<String>,
}

/// Retargets `retargets` on GitHub, one `PATCH` each (`operations::pull_request::retarget_each`); one GitHub refuses
/// stays in `from` and is a warning with the `gh` command that retargets it (`refusal_warning`). Nothing fails: the push
/// goes after it either way
pub(crate) fn retarget_all(context: &RepoContext, github: &GitHubService, runtime: &Runtime, repository: &GitHubRepository, retargets: Vec<RetargetedPullRequest>) -> Retargets {
  let writes: Vec<(u32, &str)> = retargets.iter().map(|retarget| (retarget.number, retarget.new_base.as_str())).collect();
  let answers = runtime.block_on(retarget_each(github, &context.repository, repository, &writes));
  let mut result = Retargets::default();
  for (retarget, answer) in retargets.into_iter().zip(answers) {
    match answer {
      Ok(()) => result.retargeted.push(retarget),
      Err(error) => result.warnings.push(refusal_warning(&retarget, repository, error)),
    }
  }
  result
}

/// "cannot retarget pull request #8 of login to master: <what GitHub said>; `gh pr edit 8 --repo acme/monorepo --base
/// master` retargets it"
pub(crate) fn refusal_warning(retarget: &RetargetedPullRequest, repository: &GitHubRepository, error: GitHubWriteError) -> String {
  let reason = match error {
    GitHubWriteError::GitHub(failure) => failure_message(&failure),
    GitHubWriteError::Refused(message) => message,
  };
  format!(
    "cannot retarget pull request #{} of {} to {}: {reason}; `gh pr edit {} --repo {} --base {}` retargets it",
    retarget.number,
    retarget.branch,
    retarget.new_base,
    retarget.number,
    repository.full_name(),
    retarget.new_base
  )
}

/// Lists the pull requests of the branches (one search) and retargets the open ones of `names` into a virtual branch to
/// `base`, before their push; with `dry_run`, only says which it would retarget. Without a GitHub token nothing is asked or said; another
/// failure of the listing is a warning.
pub(crate) fn retarget_pull_requests_of(
  context: &RepoContext,
  github: &GitHubService,
  runtime: &Runtime,
  repository: &GitHubRepository,
  (names, base): (&[&str], &str),
  dry_run: bool,
) -> Retargets {
  let listed = runtime.block_on(list_pull_requests(github, &context.repository, repository, &context.prefix));
  retarget_listed(context, github, runtime, repository, &listed, (names, base), dry_run)
}

/// `retarget_pull_requests_of` on the pull requests a search of this command already listed (`bd push` lists them under
/// its lock for the stack branch of a whole stack, `--with-children`)
pub(crate) fn retarget_listed(
  context: &RepoContext,
  github: &GitHubService,
  runtime: &Runtime,
  repository: &GitHubRepository,
  listed: &PullRequestsResult,
  (names, base): (&[&str], &str),
  dry_run: bool,
) -> Retargets {
  match &listed.availability {
    PullRequestAvailability::Available { .. } => {}
    PullRequestAvailability::SignedOut | PullRequestAvailability::NotGitHub => return Retargets::default(),
    other => {
      return Retargets {
        retargeted: Vec::new(),
        warnings: vec![format!("the pull requests weren't checked for a base to retarget: {}", availability_error(other).message)],
      };
    }
  }
  let retargets = virtual_base_retargets(&listed.pull_requests, names.iter().copied(), &context.prefix, base);
  if dry_run {
    return Retargets {
      retargeted: retargets,
      warnings: Vec::new(),
    };
  }
  retarget_all(context, github, runtime, repository, retargets)
}
