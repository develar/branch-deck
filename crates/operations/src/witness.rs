//! The witness of a plan (docs/design/rust-domain-authority.md, "Инварианты планов"): what a plan was prepared on, as
//! its `Fingerprint`, and the check that the facts still have it before the first effect. It is read twice: cheaply
//! before the repository lock, which turns a stale plan away without a wait, and in full under the lock, before the first
//! effect; both read the tips of the refs the plan moves and the remote branches it leases with one `for-each-ref`
//! (`read_refs`) and take the pull requests and the robots it reads from the session, so no HTTP runs under the lock. A
//! fingerprint that differs is `plan_changed`, and nothing was done.

use anyhow::Result;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::operation::Fingerprint;
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestState};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::ref_tips;
use serde::Serialize;
use std::collections::BTreeMap;
use std::hash::{DefaultHasher, Hash, Hasher};

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
    }
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

/// Whether the facts still have the fingerprint `planned` a plan was prepared on (`now`, the fingerprint of the same
/// intent on the witness read now)
///
/// # Errors
/// `plan_changed` when they differ: nothing of the plan may run
pub fn check(planned: &Fingerprint, now: &Fingerprint) -> Result<(), ApiError> {
  if planned == now {
    return Ok(());
  }
  Err(ApiError::new(
    ErrorCode::PlanChanged,
    "What the operation was planned on changed before its first effect; nothing was done",
  ))
}

#[cfg(test)]
#[path = "witness_test.rs"]
mod witness_test;
