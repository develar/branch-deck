//! The next step of a subject in the words of `bd` (`domain_policy::next_step`, the step the app's row shows;
//! spec/branch/next-step.spec.md, "The table"): the row of the table, the step, the fact it waits for, its pull request
//! and why a Safe Push or a Dry Run can't start now. `bd status` prints it of every branch and whole stack, `bd
//! safe-push --check` of the subject it checks.

use domain_types::next_step::{NextStep, StepBlocker, StepFact, StepFailure, StepKind};
use domain_types::patronus::{MissingReviewGroup, PatronusRobot, Person};
use repository_model::refusal_words::start_refusal_text;
use schemars::JsonSchema;
use serde::Serialize;

/// The next step of a branch or a whole stack, as the app's row shows it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct NextStepSummary {
  /// The row of the table of the next step (spec/branch/next-step.spec.md): `0` waits for a fact, `1` has nothing to
  /// start, `1′` needs a branch outside what the Safe Push lands, `2`/`3` a restart, `5`/`5′` a pull request into
  /// another base, `6` a draft, `7` to `10` the approvals that count are missing, `8′`/`10′` the wait GitHub tells while
  /// Patronus can't judge them, `11` a Safe Push; `no slot` off Space
  pub row: String,
  /// `pending`, `unavailable`, `settled`, `needs_branch`, `restart`, `robot_pull_request`, `retarget`, `other_base`,
  /// `draft`, `create_pull_request`, `changes_requested`, `request_review`, `awaiting`, `changes_requested_unjudged`,
  /// `awaiting_unjudged`, `safe_push` or `hidden`
  pub step: String,
  /// Row 0: the fact without an answer, or whose read failed: `patronus`, `pull_requests`, `landing_needs`,
  /// `robot_problems`, `readiness`. `bd status` reads no pull request, readiness or problem of a failed robot: a step
  /// that needs them waits for them; `bd safe-push --check` reads them
  #[serde(skip_serializing_if = "Option::is_none")]
  pub waits_for: Option<String>,
  /// The pull request the step is about: the open one, the one a robot failed in, of the whole stack the branch is in
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pull_request: Option<u32>,
  /// Why a Safe Push can't start now; `null` when it can
  pub safe_push_blocked: Option<String>,
  /// Why a Dry Run can't start now: never the approvals, a draft, another base or a robot that lands an ancestor
  pub dry_run_blocked: Option<String>,
}

impl NextStepSummary {
  /// `step` of the subject `what` ("auth", "the whole air stack"), the robots its reasons name among `robots`
  #[must_use]
  pub fn new(step: &NextStep, what: &str, robots: &[PatronusRobot]) -> Self {
    let blocked = |blocker: Option<&StepBlocker>| blocker.map(|blocker| blocker_words(blocker, step, what, robots));
    Self {
      row: step.row.name().to_owned(),
      step: kind_name(&step.kind).to_owned(),
      waits_for: waits_for(&step.kind).map(|fact| fact_name(fact).to_owned()),
      pull_request: pull_request_of(&step.kind),
      safe_push_blocked: blocked(step.safe_push.as_ref()),
      dry_run_blocked: blocked(step.dry_run.as_ref()),
    }
  }
}

const fn kind_name(kind: &StepKind) -> &'static str {
  match kind {
    StepKind::Hidden => "hidden",
    StepKind::Settled => "settled",
    StepKind::Pending { .. } => "pending",
    StepKind::Unavailable { .. } => "unavailable",
    StepKind::NeedsBranch { .. } => "needs_branch",
    StepKind::PickRemoteCommits { .. } => "pick_remote_commits",
    StepKind::Restart { .. } => "restart",
    StepKind::RobotPullRequest { .. } => "robot_pull_request",
    StepKind::Retarget { .. } => "retarget",
    StepKind::OtherBase { .. } => "other_base",
    StepKind::Draft { .. } => "draft",
    StepKind::CreatePullRequest { .. } => "create_pull_request",
    StepKind::ChangesRequested { .. } => "changes_requested",
    StepKind::RequestReview { .. } => "request_review",
    StepKind::Awaiting { .. } => "awaiting",
    StepKind::ChangesRequestedUnjudged { .. } => "changes_requested_unjudged",
    StepKind::AwaitingUnjudged { .. } => "awaiting_unjudged",
    StepKind::SafePush => "safe_push",
    StepKind::PushToBaseline { .. } => "push_to_baseline",
    StepKind::UpdateProject { .. } => "update_project",
    StepKind::OpenPullRequest => "open_pull_request",
    StepKind::MergePullRequest { auto: false, .. } => "merge_pull_request",
    StepKind::MergePullRequest { auto: true, .. } => "auto_merge",
    StepKind::AutoMerging { .. } => "auto_merging",
    StepKind::MergeHeld { .. } => "merge_held",
  }
}

const fn fact_name(fact: StepFact) -> &'static str {
  match fact {
    StepFact::Patronus => "patronus",
    StepFact::PullRequests => "pull_requests",
    StepFact::LandingNeeds => "landing_needs",
    StepFact::RobotProblems => "robot_problems",
    StepFact::Readiness => "readiness",
    StepFact::Permission => "permission",
    StepFact::TargetRules => "target_rules",
    StepFact::MergeState => "merge_state",
  }
}

const fn waits_for(kind: &StepKind) -> Option<StepFact> {
  match kind {
    StepKind::Pending { fact } => Some(*fact),
    StepKind::Unavailable { failure, .. } => Some(failure.fact()),
    _ => None,
  }
}

fn pull_request_of(kind: &StepKind) -> Option<u32> {
  match kind {
    StepKind::RobotPullRequest { number } => *number,
    StepKind::Retarget { number, .. }
    | StepKind::OtherBase { number, .. }
    | StepKind::Draft { number }
    | StepKind::ChangesRequested { number, .. }
    | StepKind::RequestReview { number, .. }
    | StepKind::Awaiting { number, .. }
    | StepKind::ChangesRequestedUnjudged { number, .. }
    | StepKind::AwaitingUnjudged { number, .. }
    | StepKind::MergePullRequest { number, .. }
    | StepKind::AutoMerging { number, .. }
    | StepKind::MergeHeld { number } => Some(*number),
    StepKind::CreatePullRequest { stack, .. } => stack.as_ref().map(|stack| stack.number),
    StepKind::Hidden
    | StepKind::Settled
    | StepKind::Pending { .. }
    | StepKind::Unavailable { .. }
    | StepKind::NeedsBranch { .. }
    | StepKind::PickRemoteCommits { .. }
    | StepKind::Restart { .. }
    | StepKind::SafePush
    | StepKind::PushToBaseline { .. }
    | StepKind::UpdateProject { .. }
    | StepKind::OpenPullRequest => None,
  }
}

/// Why a start of `what` can't now, in the words of `bd`
fn blocker_words(blocker: &StepBlocker, step: &NextStep, what: &str, robots: &[PatronusRobot]) -> String {
  match blocker {
    StepBlocker::Rule { reason } => start_refusal_text(reason, what, robots),
    StepBlocker::Syncing => "Wait for the sync to finish".to_owned(),
    StepBlocker::SyncFailed { problem } => problem.clone().unwrap_or_else(|| format!("{what} failed to sync")),
    StepBlocker::NoCommits => "No commits to push".to_owned(),
    StepBlocker::StaleReadiness => "Waits for Patronus to check the approvals again".to_owned(),
    StepBlocker::Step => step_words(&step.kind, what),
  }
}

/// The reason of the step itself (rows 0, 5′, 6, 7 to 10)
fn step_words(kind: &StepKind, what: &str) -> String {
  match kind {
    StepKind::Pending { fact } => match fact {
      StepFact::Patronus => "Waits for the answer of Patronus".to_owned(),
      StepFact::PullRequests => "Waits for GitHub to list the pull requests".to_owned(),
      StepFact::LandingNeeds => format!("Checking what {what} needs to land"),
      StepFact::RobotProblems => "Waits for Patronus to tell why the Safe Push failed".to_owned(),
      StepFact::Readiness => "Waits for Patronus to check the approvals".to_owned(),
      StepFact::Permission => "Waits for GitHub to tell what the token may do in the repository".to_owned(),
      StepFact::TargetRules => "Waits for GitHub to tell the rules of the target".to_owned(),
      StepFact::MergeState => "Waits for GitHub to tell whether it can merge the pull request".to_owned(),
    },
    StepKind::Unavailable { failure, .. } => match failure {
      StepFailure::NotLoggedIn => "Run ./safePush.cmd once to log in to Patronus".to_owned(),
      StepFailure::PatronusError { message } => format!("Patronus fails: {message}"),
      StepFailure::ViewFailed { reason } => format!("Patronus didn't tell what its robots hold and land ({})", reason.message),
      StepFailure::ProblemsFailed { message } => format!("Patronus didn't tell why the Safe Push failed ({message})"),
    },
    StepKind::OtherBase { number, base } => format!("Approvals in #{number} don't count: its base is {base}"),
    StepKind::Draft { number } => format!("#{number} is a draft: Patronus merges it once it is ready for review"),
    StepKind::CreatePullRequest { groups, .. } => format!("Code owners must approve in a pull request: {}", group_names(groups)),
    StepKind::ChangesRequested { number, groups } => {
      let people: Vec<&Person> = groups.iter().flat_map(|group| &group.changes_requested_by).collect();
      format!("{} requested changes on #{number}", person_names(&people))
    }
    StepKind::RequestReview { number, groups, no_eligible } => {
      if *no_eligible {
        format!("Nobody of {} can approve these changes", group_names(groups))
      } else {
        format!("Waits for the approval of {}: nobody of it is requested on #{number}", group_names(groups))
      }
    }
    StepKind::Awaiting { number, groups } => {
      let people: Vec<&Person> = groups.iter().flat_map(|group| &group.requested).collect();
      format!("Waits for the approval of {}: {} requested on #{number}", group_names(groups), person_names(&people))
    }
    StepKind::PickRemoteCommits { count } => {
      let commits = if *count == 1 { "1 commit".to_owned() } else { format!("{count} commits") };
      format!("The remote branch has {commits} of someone else that {what} lacks: `bd pick --remote {what}` takes them in, a push drops them")
    }
    StepKind::UpdateProject { upstream } => format!("{upstream} has new commits: `bd update` builds the copy of {what} anew on them before its push"),
    StepKind::AutoMerging { number, .. } => format!("GitHub merges #{number} once its checks pass"),
    StepKind::Hidden
    | StepKind::Settled
    | StepKind::NeedsBranch { .. }
    | StepKind::Restart { .. }
    | StepKind::RobotPullRequest { .. }
    | StepKind::Retarget { .. }
    | StepKind::ChangesRequestedUnjudged { .. }
    | StepKind::AwaitingUnjudged { .. }
    | StepKind::SafePush
    | StepKind::PushToBaseline { .. }
    | StepKind::OpenPullRequest
    | StepKind::MergePullRequest { .. }
    | StepKind::MergeHeld { .. } => String::new(),
  }
}

fn group_names(groups: &[MissingReviewGroup]) -> String {
  groups.iter().map(|group| group.name.as_str()).collect::<Vec<_>>().join(", ")
}

/// The people, each once, by full name (the login without one)
fn person_names(people: &[&Person]) -> String {
  let mut names: Vec<&str> = Vec::new();
  for person in people {
    let name = person.full_name.as_deref().or(person.login.as_deref()).unwrap_or("a reviewer");
    if !names.contains(&name) {
      names.push(name);
    }
  }
  names.join(", ")
}
