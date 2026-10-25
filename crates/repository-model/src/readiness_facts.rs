//! The review readiness of a session (docs/design/rust-domain-authority.md, "Факты"; spec/branch/patronus.spec.md,
//! "Code-owner review"): the part `facts.readiness` of the view by subject (a branch by name, the whole stack of a root
//! as `stack:<root>`), which the scheduler of the facts asks Patronus for (`remote_facts::scheduler`) by what the
//! session tells it (`ReadinessDigest`, after each patch).
//!
//! - The ask of a subject is what a window shows of it: the tip (the copy of the newest commit, which the row shows
//!   once the sync built it; the stack branch of a whole stack while it is built), the pull request its robot would start from
//!   (`domain_policy::pull_request::start_pull_request`, with the GitHub repository Patronus names: of a branch also one
//!   into a virtual branch, which its start retargets first, D11) and the reviews of it.
//!   Its inputs are the hash of the ask and of the rechecks (Check Approvals Again): a new tip, start or review, and a
//!   recheck, make the readiness `Stale` in the patch that changes them, until the answer of the new inputs.
//! - Asked (`domain_policy::refresh::readiness_priority`): what a surface needs now and what a recheck asks, at once;
//!   what a window shows with a step that waits for it; then the hidden subjects whose last answer waits for approvals
//!   in the pull request of the same tip and start (only a review changed: its approval is news). A hidden subject of
//!   another tip or start stays `Stale` until it shows. Nothing while Patronus doesn't answer for the repository: the
//!   subjects a window asks for are `Unavailable` with why, so nothing waits for an answer that can't come.
//! - An answer applies by its number, so the answer of an ask that started before a recheck never replaces the
//!   recheck's; an answer of the inputs the subject has now that made a subject ready that waited for approvals is news
//!   (`NewsEvent::ApprovalsArrived`, each approval once), against the last answer Patronus gave (a failure between two
//!   answers hides no approval).

use crate::facts::{Applied, FactRecord, Observation};
use crate::table::TableState;
use domain_policy::news::approval_news;
use domain_policy::pull_request::start_pull_request;
use domain_policy::readiness::{Answered, awaits_approval};
use domain_policy::refresh::{ReadinessPriority, readiness_priority};
use domain_types::branch_names::baseline_on_remote;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::{BranchReviewReadiness, PatronusAvailability, ReviewReadiness, ReviewReadinessBranch, ReviewStart};
use domain_types::session::{BranchView, Demand, Fact, FactState, NewsEvent, PullRequestsView, RobotsHead, SubjectReadiness, stack_readiness_key, stack_root_of_readiness_key};
use domain_types::stack::StackBranchState;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::mem::discriminant;

/// A subject whose readiness the scheduler asks now
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessSubject {
  /// The branch, or `stack:<root>`
  pub key: String,
  pub ask: ReviewReadinessBranch,
  /// The hash of the ask and of the rechecks: an answer of other inputs is `Stale`
  pub inputs: u64,
  pub priority: ReadinessPriority,
  /// How often Check Approvals Again asked; `recheck`: the last of them has no answer yet, so Patronus is asked again
  /// instead of the kept answer
  pub rechecks: u32,
  pub recheck: bool,
}

/// What the scheduler asks the readiness of: the subjects with a reason to ask, in the order of the table
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadinessDigest {
  /// The baseline the commits of each subject are read against (`origin/master`)
  pub baseline: String,
  pub subjects: Vec<ReadinessSubject>,
  /// The last answer of one of them waits for approvals in the pull request it starts from: each poll of the review
  /// requests lists the pull requests too, whose new reviews ask its readiness again (the approval is news)
  pub awaits_approval: bool,
}

/// The answer of the readiness of a subject, with the ask it answers
#[derive(Clone, Debug)]
pub struct ReadinessAnswer {
  pub subject: ReadinessSubject,
  pub result: Result<BranchReviewReadiness, ApiError>,
}

#[derive(Debug)]
struct SubjectRecord {
  fact: FactRecord<SubjectReadiness>,
  /// The last answer Patronus gave (not `unavailable`), which the news of approvals compares with
  last_answered: Option<SubjectReadiness>,
  rechecks: u32,
  /// The rechecks the answer kept was asked with
  answered_rechecks: u32,
  /// Patronus doesn't answer for the repository: the fact says so until it does
  blocked: bool,
}

impl SubjectRecord {
  const fn new() -> Self {
    Self {
      fact: FactRecord::loading(),
      last_answered: None,
      rechecks: 0,
      answered_rechecks: 0,
      blocked: false,
    }
  }
}

/// The readiness of the subjects of a session (the module's header)
#[derive(Debug, Default)]
pub struct ReadinessFact {
  subjects: BTreeMap<String, SubjectRecord>,
  digest: ReadinessDigest,
  /// The approvals told: the subject, the pull request and whose approval
  told: BTreeSet<(String, u32, Vec<String>)>,
}

impl ReadinessFact {
  /// The readiness of each subject the session has one of
  pub fn facts(&self) -> impl Iterator<Item = (&String, &Fact<SubjectReadiness>)> {
    self.subjects.iter().map(|(key, record)| (key, record.fact.fact()))
  }

  #[must_use]
  pub fn fact(&self, key: &str) -> Option<&Fact<SubjectReadiness>> {
    self.subjects.get(key).map(|record| record.fact.fact())
  }

  #[must_use]
  pub fn len(&self) -> usize {
    self.subjects.len()
  }

  #[must_use]
  pub fn is_empty(&self) -> bool {
    self.subjects.is_empty()
  }

  /// What the scheduler asks now (`update`)
  #[must_use]
  pub const fn digest(&self) -> &ReadinessDigest {
    &self.digest
  }

  /// Check Approvals Again for `key`: its readiness is asked again, instead of the kept answer, and is `Stale` until
  /// that answer (applied by the next `update`)
  pub fn recheck(&mut self, key: &str) {
    let record = self.subjects.entry(key.to_owned()).or_insert_with(SubjectRecord::new);
    record.rechecks = record.rechecks.saturating_add(1);
  }

  /// The asks of the subjects for the table, the pull requests and the robots as the session has them now, with the
  /// demand of its windows: what is gone from the table is dropped, a subject a window asks for has a record, each
  /// record expects the inputs of its ask, and the digest names what the scheduler asks. `prefix`: the branch prefix of
  /// the session, whose virtual branches a pull request a start retargets may go into
  pub fn update(&mut self, state: &TableState, pull_requests: &PullRequestsView, robots: Option<&RobotsHead>, demand: &Demand, prefix: &str) {
    self.subjects.retain(|key, _| exists(state, key));
    let baseline = state.header().baseline_branch.clone();
    let target = baseline_on_remote(&baseline).to_owned();
    let github = robots.and_then(|head| head.github_repository.as_deref());
    let availability = robots.map(|head| &head.availability);
    let mut subjects = Vec::new();
    let mut awaiting = false;
    for name in state.order() {
      let stack = state.stack(name).map(|_| stack_readiness_key(name));
      for key in stack.into_iter().chain(std::iter::once(name.clone())) {
        let record = self.subjects.get(&key);
        let rechecking = record.is_some_and(|record| record.rechecks > record.answered_rechecks);
        let demanded = rechecking || demand.needed.contains(&key);
        let shown = match stack_root_of_readiness_key(&key) {
          Some(root) => demand.visible_stacks.iter().any(|shown| shown == root),
          None => demand.visible_branches.contains(&key),
        };
        // a subject nothing asks for and the session has no answer of costs nothing
        if !demanded && !shown && record.is_none() {
          continue;
        }
        let ask = subject_ask(state, pull_requests, github, &target, prefix, &key);
        let awaits = record
          .and_then(|record| record.last_answered.as_ref())
          .zip(ask.as_ref())
          .is_some_and(|(last, ask)| awaits_approval(answered_of(last), ask));
        let priority = readiness_priority(demanded, shown, awaits);
        let record = self.subjects.entry(key.clone()).or_insert_with(SubjectRecord::new);
        let Some(ask) = ask else {
          // no tip to ask about: the record waits for the sync
          continue;
        };
        let inputs = inputs_of(&ask, record.rechecks);
        record.fact.expect(inputs);
        let Some(priority) = priority else {
          continue;
        };
        match availability {
          // Patronus has not answered the list of the robots yet
          None => {}
          Some(PatronusAvailability::Available) => {
            if std::mem::take(&mut record.blocked) {
              record.fact.asked_again();
            }
            awaiting |= awaits;
            subjects.push(ReadinessSubject {
              key,
              ask,
              inputs,
              priority,
              rechecks: record.rechecks,
              recheck: rechecking,
            });
          }
          Some(availability) => {
            record.blocked = true;
            record.fact.fail(not_available(availability));
          }
        }
      }
    }
    self.digest = ReadinessDigest {
      baseline,
      subjects,
      awaits_approval: awaiting,
    };
  }

  /// Applies `answer` of the read of `observation`; whether it changed the readiness, and the news it is
  pub fn answer(&mut self, observation: Observation, answer: ReadinessAnswer) -> (bool, Option<NewsEvent>) {
    let ReadinessAnswer { subject, result } = answer;
    let Some(record) = self.subjects.get_mut(&subject.key) else {
      return (false, None);
    };
    let observation = Observation {
      inputs: subject.inputs,
      ..observation
    };
    let value = result.map(|answered| SubjectReadiness {
      ask: subject.ask,
      requirement: answered.requirement,
      readiness: answered.readiness,
    });
    let answered = value.as_ref().ok().cloned();
    if record.fact.answer(observation, value) == Applied::Older {
      return (false, None);
    }
    record.answered_rechecks = record.answered_rechecks.max(subject.rechecks);
    let Some(answered) = answered else {
      return (true, None);
    };
    let fresh = record.fact.fact().state == FactState::Fresh;
    let news = record
      .last_answered
      .as_ref()
      .filter(|_| fresh)
      .and_then(|previous| approval_news(answered_of(previous), answered_of(&answered)));
    if !matches!(answered.readiness, ReviewReadiness::Unavailable { .. }) {
      record.last_answered = Some(answered);
    }
    let news = news.and_then(|news| {
      let mut approvers: Vec<String> = news
        .approvers
        .iter()
        .map(|person| person.login.as_deref().or(person.full_name.as_deref()).unwrap_or_default().to_lowercase())
        .collect();
      approvers.sort_unstable();
      self.told.insert((subject.key.clone(), news.pull_request, approvers)).then(|| NewsEvent::ApprovalsArrived {
        subject: subject.key.clone(),
        pull_request: news.pull_request,
        approvers: news.approvers,
        groups: news.groups,
      })
    });
    (true, news)
  }
}

const fn answered_of(readiness: &SubjectReadiness) -> Answered<'_> {
  Answered {
    ask: &readiness.ask,
    readiness: &readiness.readiness,
  }
}

/// Whether the subject of `key` is in the table: a branch, a stack branch of a root
fn exists(state: &TableState, key: &str) -> bool {
  match stack_root_of_readiness_key(key) {
    Some(root) => state.stack(root).is_some(),
    None => state.branch(key).is_some(),
  }
}

/// The tip of a branch as a window shows it: the copy of its newest commit, else that commit
#[must_use]
pub fn local_tip(branch: &BranchView) -> Option<&str> {
  branch
    .copies
    .first()
    .and_then(|copy| copy.hash.as_deref())
    .or_else(|| branch.commits.first().map(|commit| commit.id.as_str()))
}

/// The ask of the subject `key` as the session has it now; `None` without a tip (a branch or a whole stack not built)
fn subject_ask(state: &TableState, pull_requests: &PullRequestsView, github: Option<&str>, target: &str, prefix: &str, key: &str) -> Option<ReviewReadinessBranch> {
  if let Some(root) = stack_root_of_readiness_key(key) {
    let stack = state.stack(root)?;
    let tip = stack.tip.as_ref().filter(|_| stack.state == StackBranchState::Built && !stack.members.is_empty())?;
    let start = start_pull_request(pull_requests.stack_pull_requests.get(root), target, prefix, github, true);
    return Some(ReviewReadinessBranch::starting_from(key.to_owned(), Some(stack.branch_name.clone()), tip.clone(), start));
  }
  // the copy the row shows: none before the sync built the branch
  let tip = state.branch(key)?.copies.first()?.hash.as_deref().filter(|tip| !tip.is_empty())?;
  let start = start_pull_request(pull_requests.pull_requests.get(key), target, prefix, github, false);
  Some(ReviewReadinessBranch::starting_from(key.to_owned(), None, tip.to_owned(), start))
}

/// The inputs of an ask with its rechecks
#[must_use]
pub fn inputs_of(ask: &ReviewReadinessBranch, rechecks: u32) -> u64 {
  let mut hasher = DefaultHasher::new();
  ask.branch_name.hash(&mut hasher);
  ask.source.hash(&mut hasher);
  ask.tip.hash(&mut hasher);
  match ask.start {
    ReviewStart::Rest => 0u32.hash(&mut hasher),
    ReviewStart::PullRequest { number } => (1u32, number).hash(&mut hasher),
  }
  for reviewer in &ask.reviewers {
    (&reviewer.login, discriminant(&reviewer.state), reviewer.requested).hash(&mut hasher);
  }
  ask.author.hash(&mut hasher);
  rechecks.hash(&mut hasher);
  hasher.finish()
}

/// Why nothing is asked while Patronus doesn't answer for the repository, in the words of a failed pre-check
fn not_available(availability: &PatronusAvailability) -> ApiError {
  match availability {
    PatronusAvailability::NotLoggedIn => ApiError::new(ErrorCode::NotLoggedIn, "Run ./safePush.cmd once to log in to Patronus"),
    PatronusAvailability::Error { message } => ApiError::new(ErrorCode::PatronusError, message.clone()),
    PatronusAvailability::Available | PatronusAvailability::NotPatronus => ApiError::new(ErrorCode::PatronusError, "Patronus is not available for the repository"),
  }
}

#[cfg(test)]
#[path = "readiness_facts_test.rs"]
mod readiness_facts_test;
