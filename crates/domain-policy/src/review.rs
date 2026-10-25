//! The code-owner review of a subject as both sides read it (spec/branch/pull-requests.spec.md "Request Review",
//! spec/branch/patronus.spec.md "Code-owner review"): whom Request Review… offers and `bd request-review` asks
//! (`candidates`, D5), what the answer of the readiness means for a start and where the approvals count, a repository
//! only on Space read once (`review_state`, D14), and when a Safe Push starts without them (`skips_approvals`, D2).

use domain_types::landing::ApprovalsSkip;
use domain_types::patronus::{MissingReviewGroup, PatronusMergeRequest, PatronusProfile, PatronusPushMode, PatronusReviewRequirement, Person, ReviewReadiness};
use domain_types::pull_request::{PullRequest, ReviewCandidate};

/// The code owners of the missing groups, as Request Review… offers them
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Candidates {
  /// Each once, in the order of the readiness; `preselected` the ones asked without a choice
  pub candidates: Vec<ReviewCandidate>,
  /// The missing groups nobody is asked for whose suggested code owners have no GitHub login to ask (or only the
  /// author, a member asked or one whose approval counts)
  pub without_reviewer: Vec<String>,
}

/// The suggested code owners with a GitHub login of the groups whose approval is missing (`readiness` `missing`), once
/// each; never the author (GitHub refuses a review request of the author), a member asked already, or one whose approval
/// counts. Preselected: one suggested code owner of each missing group nobody is asked for yet, each once (a person
/// chosen for another group covers this one too). None for any other readiness: the search finds whom else to ask
#[must_use]
pub fn candidates(readiness: Option<&ReviewReadiness>, pull_request: &PullRequest) -> Candidates {
  let Some(ReviewReadiness::Missing { groups, .. }) = readiness else {
    return Candidates::default();
  };
  let asked = |login: &str| pull_request.reviewers.iter().any(|reviewer| reviewer.requested && is_login(&reviewer.login, login));
  let among = |people: &[Person], login: &str| people.iter().any(|person| person.login.as_deref().is_some_and(|it| is_login(it, login)));
  let eligible = |group: &MissingReviewGroup, login: &str| {
    !pull_request.author.as_deref().is_some_and(|author| is_login(author, login)) && !asked(login) && !among(&group.requested, login) && !among(&group.approved_by, login)
  };
  let mut result = Candidates::default();
  for group in groups {
    let waiting = !group.requested.is_empty();
    let mut covered = waiting;
    for person in &group.suggested {
      let Some(login) = person.login.as_deref().filter(|login| eligible(group, login)) else {
        continue;
      };
      let preselect = !covered;
      covered = true;
      match result.candidates.iter_mut().find(|candidate| is_login(&candidate.login, login)) {
        Some(known) => {
          known.preselected |= preselect;
          if !known.groups.contains(&group.name) {
            known.groups.push(group.name.clone());
          }
        }
        None => result.candidates.push(ReviewCandidate {
          login: login.to_string(),
          full_name: person.full_name.clone(),
          groups: vec![group.name.clone()],
          preselected: preselect,
        }),
      }
    }
    if !covered {
      result.without_reviewer.push(group.name.clone());
    }
  }
  result
}

/// What the answer of the readiness of a subject means for its start
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewState<'a> {
  /// No answer yet: never "approved", never "missing"
  Pending,
  /// The changes need no approval
  NotRequired,
  /// Every group the changes need approved
  Approved,
  /// A Safe Push fails without the approvals of `groups`, which count in `counts_in`
  Missing { groups: &'a [MissingReviewGroup], counts_in: CountsIn<'a> },
  /// Patronus doesn't tell whether the approvals count: the robot decides
  Unknown,
  /// The pre-check failed (`message`): nobody can tell
  Unavailable { message: &'a str },
}

/// Where the approvals a start misses count
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CountsIn<'a> {
  /// The pull request the robot starts from
  PullRequest { number: u32, url: &'a str },
  /// The GitHub pull request of the branch, which it doesn't have yet (a start through the API counts none)
  NewPullRequest,
  /// A repository only on Space (no GitHub repository of the branches): the merge request Patronus found open
  MergeRequest(&'a PatronusMergeRequest),
  /// A repository only on Space without a merge request: the GitHub repository that would count them is unknown
  UnknownRepository,
}

/// The review of a subject by the answer of its readiness (`None` before the first), the requirement it came with, whether
/// Patronus names the GitHub repository of the branches (`github`), and the pull request the robot starts from
/// (`start_pull_request`). A repository only on Space is read here once: its approvals count in the merge request of
/// the requirement, whatever the answer calls the reason
#[must_use]
pub fn review_state<'a>(
  readiness: Option<&'a ReviewReadiness>,
  requirement: Option<&'a PatronusReviewRequirement>,
  github: bool,
  start: Option<&'a PullRequest>,
) -> ReviewState<'a> {
  match readiness {
    None => ReviewState::Pending,
    Some(ReviewReadiness::NotRequired) => ReviewState::NotRequired,
    Some(ReviewReadiness::Approved { .. }) => ReviewState::Approved,
    Some(ReviewReadiness::Unknown { .. }) => ReviewState::Unknown,
    Some(ReviewReadiness::Unavailable { message, .. }) => ReviewState::Unavailable { message },
    Some(ReviewReadiness::Missing { groups, .. }) => {
      let counts_in = match (github, start, requirement.and_then(|requirement| requirement.merge_request.as_ref())) {
        (true, Some(pull_request), _) => CountsIn::PullRequest {
          number: pull_request.number,
          url: &pull_request.url,
        },
        (true, None, _) => CountsIn::NewPullRequest,
        (false, _, Some(merge_request)) => CountsIn::MergeRequest(merge_request),
        (false, _, None) => CountsIn::UnknownRepository,
      };
      ReviewState::Missing { groups, counts_in }
    }
  }
}

/// What a start skips the approvals for, if anything: never a Dry Run (no review) or a pull request it opens instead of a
/// start (`create_pull_request`); `from_pull_request`: the start comments on a pull request, which takes no profile
#[must_use]
pub fn skips_approvals(
  mode: PatronusPushMode,
  create_pull_request: bool,
  without_approvals: bool,
  from_pull_request: bool,
  profile: Option<&PatronusProfile>,
) -> Option<ApprovalsSkip> {
  if mode == PatronusPushMode::DryRun || create_pull_request {
    None
  } else if without_approvals {
    Some(ApprovalsSkip::WithoutApprovals)
  } else if !from_pull_request && profile.is_some_and(|profile| profile.skips_review == Some(true)) {
    Some(ApprovalsSkip::Profile)
  } else {
    None
  }
}

/// GitHub logins are case-insensitive
const fn is_login(login: &str, other: &str) -> bool {
  login.eq_ignore_ascii_case(other)
}
