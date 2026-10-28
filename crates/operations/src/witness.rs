//! The witness of a plan (docs/design/rust-domain-authority.md, "Инварианты планов"): what a plan was prepared on, as
//! its `Fingerprint`, and the check that the facts still have it before the first effect. It is read twice: cheaply
//! before the repository lock, which turns a stale plan away without a wait, and in full under the lock, before the first
//! effect; both read the tips of the refs the plan moves and the remote branches it leases with one `for-each-ref`
//! (`read_refs`) and take the pull requests and the robots it reads from the session, so no HTTP runs under the lock. A
//! fingerprint that differs is `plan_changed`, and nothing was done; its words name what the user asked and say to ask
//! it again (`plan_changed`).

use anyhow::Result;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::operation::{Fingerprint, Intent};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestState};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::ref_tips;
use serde::Serialize;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use sync_core::patronus_start::push_mode_name;

/// What a plan was prepared on
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Witness {
  /// The tip of each ref the plan moves or leases, by full name (`refs/heads/…`, `refs/remotes/origin/…`); `None` for one
  /// that doesn't exist
  pub refs: BTreeMap<String, Option<String>>,
  /// The pull requests it reads, by number
  pub pull_requests: BTreeMap<u32, PullRequestWitness>,
  /// The robots it reads (the ones that land a part of what it lands, the one it replaces), by id
  pub robots: BTreeMap<String, PatronusRobotStatus>,
  /// The remote heads a push leases on as the plan saw them, by the full name of their remote-tracking ref
  /// (`PushLease::Planned`): in the fingerprint, never read again, so a remote-tracking ref another push or a fetch
  /// moved meanwhile refuses nothing before the push, and the remote refuses the lease instead
  pub leases: BTreeMap<String, Option<String>>,
}

/// What a plan reads of a pull request
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PullRequestWitness {
  pub head: String,
  pub base: String,
  pub draft: bool,
  pub state: PullRequestState,
}

impl Witness {
  /// The witness of `refs` (`read_refs`), `pull_requests` and `robots`
  #[must_use]
  pub fn of<'a>(refs: BTreeMap<String, Option<String>>, pull_requests: impl IntoIterator<Item = &'a PullRequest>, robots: impl IntoIterator<Item = &'a PatronusRobot>) -> Self {
    Self {
      refs,
      pull_requests: pull_requests
        .into_iter()
        .map(|pull_request| {
          let witness = PullRequestWitness {
            head: pull_request.head_ref_oid.clone(),
            base: pull_request.base_ref_name.clone(),
            draft: pull_request.is_draft,
            state: pull_request.state,
          };
          (pull_request.number, witness)
        })
        .collect(),
      robots: robots.into_iter().map(|robot| (robot.id.clone(), robot.status)).collect(),
      leases: BTreeMap::new(),
    }
  }

  /// This witness with `leases`, the remote heads its push leases on
  #[must_use]
  pub fn leasing(self, leases: BTreeMap<String, Option<String>>) -> Self {
    Self { leases, ..self }
  }

  /// The fingerprint of `planned` (an intent, or the plan of one) prepared on this witness: equal only for the same
  /// plan of the same intent on the same facts, within one process
  #[must_use]
  pub fn fingerprint(&self, planned: &impl Serialize) -> Fingerprint {
    let mut hasher = DefaultHasher::new();
    // both serialize: their types have no map with a key that is not a string, and no float
    serde_json::to_string(planned).unwrap_or_default().hash(&mut hasher);
    serde_json::to_string(self).unwrap_or_default().hash(&mut hasher);
    Fingerprint(format!("{:016x}", hasher.finish()))
  }
}

/// The tip of each of `refs` (full names): one `for-each-ref`, an absent ref `None`. A pattern of `for-each-ref` matches
/// the refs under it too, so only the exact names count
pub fn read_refs(git: &GitCommandExecutor, repo_path: &str, refs: &[String]) -> Result<BTreeMap<String, Option<String>>> {
  let patterns: Vec<&str> = refs.iter().map(String::as_str).collect();
  let mut tips = ref_tips(git, repo_path, &patterns)?;
  Ok(refs.iter().map(|name| (name.clone(), tips.remove(name))).collect())
}

/// Whether the facts still have the fingerprint `planned` a plan of `intent` was prepared on (`now`, the fingerprint of
/// the same intent on the witness read now)
///
/// # Errors
/// `plan_changed` when they differ: nothing of the plan may run
pub fn check(planned: &Fingerprint, now: &Fingerprint, intent: &Intent) -> Result<(), ApiError> {
  if planned == now {
    return Ok(());
  }
  Err(plan_changed(intent, None))
}

/// `plan_changed` of `intent`: what its plan was decided on changed before its first effect (`why`, when it is known), so
/// nothing was done. The page shows it after "Cannot push auth" and `bd` prints it, so it names what the user asked and
/// says to ask it again: "What the push of auth was decided on changed before it began (auth moved); nothing was
/// done: push again"
#[must_use]
pub fn plan_changed(intent: &Intent, why: Option<&str>) -> ApiError {
  let (asked, again) = asked(intent);
  let why = why.map_or_else(String::new, |why| format!(" ({why})"));
  ApiError::new(
    ErrorCode::PlanChanged,
    format!("What {asked} was decided on changed before it began{why}; nothing was done: {again}"),
  )
}

/// What the user asked of `intent`, and how to ask it again
fn asked(intent: &Intent) -> (String, &'static str) {
  match intent {
    Intent::Land(land) => (format!("the {} of {}", push_mode_name(land.mode), land.subject.words()), "start it again"),
    Intent::Push(push) => (format!("the push of {}", push.subject.words()), "push again"),
    Intent::RestartRobot { .. } => ("the restart of the robot".to_string(), "restart it again"),
    Intent::CancelRobot { .. } => ("the cancel of the robot".to_string(), "cancel it again"),
    Intent::Retarget { number } => (format!("the retarget of pull request #{number}"), "retarget it again"),
    Intent::CreatePullRequest(create) => (format!("the pull request of {}", create.subject.words()), "create it again"),
    Intent::RequestReview { subject, .. } => (format!("the review request of {}", subject.words()), "request the review again"),
    Intent::ClosePullRequest { number, .. } => (format!("the close of pull request #{number}"), "close it again"),
    Intent::CloseLandedPullRequests { .. } => ("the closes of the landed pull requests".to_string(), "close them again"),
    Intent::SubmitReview { number, .. } => (format!("the review of pull request #{number}"), "review it again"),
    Intent::SetIssueState { key, .. } => (format!("the state of {key}"), "set it again"),
    Intent::ForgetStart { op } => (format!("the forget of start {op}"), "forget it again"),
    Intent::Rewrite(rewrite) => (
      rewrite.subject().map_or_else(|| "the rewrite".to_string(), |subject| format!("the rewrite of {subject}")),
      "sync, then ask it again",
    ),
    Intent::FetchBaseline => ("the fetch of the baseline".to_string(), "fetch again"),
    Intent::UpdateProject { .. } => ("the update of the project".to_string(), "update again"),
    Intent::DismissLanded => ("the dismissal of the landed commits".to_string(), "dismiss them again"),
  }
}

#[cfg(test)]
#[path = "witness_test.rs"]
mod witness_test;
