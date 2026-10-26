//! `bd safe-push`: start a Patronus robot on a virtual branch, as the app's Safe Push does: a Safe Push (rebase onto
//! the target, the checks, the push into the target) or a Dry Run (the checks only), with a merge profile. A stacked
//! branch carries the commits of the branches it is built on, so its Safe Push lands them with it, in one robot
//! (`lands`); with `--check` the trial build of the branch alone says whether it could land without them (`alone`), a
//! second way the user chooses (`bd unstack`), never the lead. Before a Safe Push the code-owner review is checked;
//! unlike the form, which only warns, missing approvals refuse the start (`approvals_missing`): Patronus would fail the
//! robot with "Changes require review approval". A branch the remote lacks commits of (by content, as the sync's
//! remote status compares) is pushed first, parents first, as `bd push` pushes it; one only rebuilt on a newer baseline
//! is not: the robot takes the remote branch. The start is never repeated. With `--check` nothing is pushed or started:
//! the profiles, the review and what stands in the way, for the user to approve. The approvals count in the GitHub pull
//! request of the branch (Space merge requests are deprecated): `--create-pull-request` opens it, into the target as
//! every pull request of a virtual branch (or takes the open one of the branch, whatever its base), and the Safe Push
//! then starts from it with a `/patronus` comment. An open pull request of the branch, or of a branch the push takes
//! along, whose base is a virtual branch is retargeted to the target first (`pull_request_retarget`).
//!
//! Before the lock, the target's remote branch is fetched, and the check before the push replays what the robot lands
//! onto its tip, as Patronus rebases it (`sync_core::target_conflict`): a conflict refuses the start
//! (`target_conflict`), and `bd update` resolves it locally. A failed fetch doesn't stop the check: it replays onto the
//! last fetched tip and says so (`target_check`).
//!
//! The sync, the checks before the push and the commits and leases of the push run under the repository lock; the
//! network push of those commits (`<commit>:refs/heads/<name>`, leased) and the start, which can wait up to a minute for
//! the answer to a pull request comment, after it.

use crate::landed::{LandedBranch, every_branch_landed, landed_branches};
use crate::pull_request_retarget::{RetargetedPullRequest, refusal_warning};
use crate::push_command::{
  LANDED_NEXT, PullRequestDraft, PushArgs, PushedBranch, REMOTE_REFUSED_NEXT, WOULD_REJECT_NEXT, github_repository, not_synced_reason, plan_push, push_size_limit, pushed_branch,
  refuse_an_unapplied_name, stack_pull_request_draft, unknown_or_unapplied, unpushed_pull_request_draft,
};
use crate::push_hold::{PushHolds, refusing_subject, robot_command};
use crate::robot_list::keep_listed;
use crate::session_facts::{SessionFacts, listed_robots, robots_since};
use bd_core::bd_command::shell_word;
use bd_core::check_summary::{CheckConflict, CheckWarning, DanglingReferenceSummary, dangling_reference_message, dangling_references_in, session_of_sync};
use bd_core::context::{Environment, RepoContext};
use bd_core::error::{CliError, ErrorBranch, NextStep, regroup_error};
use bd_core::next_step_summary::NextStepSummary;
use bd_core::output::SchemaVersion;
use bd_core::review_summary::{MergeRequestSummary, PersonSummary, ReviewGroupSummary, ReviewSummary, ReviewerSummary};
use bd_core::services::{BdServices, cli_user_agent, github_service, patronus_service};
use bd_core::stack_summary::{WholeStackSummary, stack_branch_of, whole_stack, whole_stack_summary};
use bd_core::sync_command::{LandedRemoteCommitSummary, RemoteSummary, SyncOutput, landed_commits, lock, sync_locked};
use domain_policy::facts::{Repository, Robots};
use domain_policy::holds::is_active;
use domain_policy::land::{SubjectFacts, TargetConflictFact, allows_mode, table_blocker, unaccepted};
use domain_policy::landed::LandedFacts;
use domain_policy::next_step::PatronusStanding;
use domain_policy::parked::whole_stack_parked;
use domain_policy::pull_request::{RulesAnswer, dismisses_approvals};
use domain_policy::push::needs_push;
use domain_policy::review::{CountsIn, ReviewState, review_state};
use domain_policy::stack::WholeStack;
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::ErrorCode;
use domain_types::landing::{ApprovalsSkip, Confirmation, Land, LandingFix, LandingRefusal, PlannedEffect, ProfileChoice, StackCommit, StartRoute, Subject};
use domain_types::operation::{Intent, Plan};
use domain_types::patronus::{
  LandingAllowances, MissingApprovalsReason, MissingReviewGroup, PatronusCommandReply, PatronusCommandSubject, PatronusMergeRequest, PatronusProfile, PatronusPushMode,
  PatronusReviewRequirement, PatronusRobot, PatronusRobotStatus, PatronusStartOptions, Person, ReviewReadiness, ReviewerApprovalState, UnavailableReadinessReason,
  UnknownReadinessReason,
};
use domain_types::pull_request::{BranchRules, PullRequest, ReviewerState};
use domain_types::reason::Reason;
use domain_types::session::{Fact, FactState, LandedSection, ParkedMember, PullRequestsView, SubjectReadiness};
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use github::client::GitHubError;
use github::service::{GitHubFailure, GitHubService};
use history_rewrite::repository_lock::RepositoryLock;
use operations::land::{LandHost, one_shot_plan};
use operations::needs::{FactNeed, needs};
use operations::push::OverMoved;
use patronus::review_readiness::evaluates_pull_request;
use patronus::service::PatronusService;
use remote_facts::fact_sources::FactSources;
use remote_facts::patronus_command::{post_patronus_command, read_patronus_command_reply};
use remote_facts::pull_request_create::{CreatePullRequestParams, create_pull_request as core_create_pull_request};
use remote_facts::pull_requests::{GitHubWriteError, failure_message};
use repository_model::branch_table::{BranchTable, RemoteFact};
use repository_model::derive::{LandingFacts, StepSources, grouped_rows, landed_states, next_steps, open_pull_requests_view};
use repository_model::facts::{FactRecord, Needs};
use repository_model::one_shot::OneShot;
use repository_model::push_guard::parked_words;
use repository_model::start_guard::{stack_conflict_words, stack_not_built_words, start_refusal_text};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::{SpaceRepository, target_branch_of};
use sync_core::patronus_start::{join_names, push_mode_name};
use sync_core::push::{LeasedBranch, LeasedPush, PreparedPush, PushTarget};
use sync_core::push_lease::{LeaseSource, PushOptions, PushRef};
use sync_core::target_conflict::{TargetCheck, TargetConflict, TargetFetch, TargetOutcome, check_target, fetch_target};
use sync_utils::timestamp::unix_now;
use tokio::runtime::Runtime;
use trial_check::check::{CheckSession, StackIndependence};
use trial_check::landing_gate::check_landing_gate;
use youtrack::client::YouTrackClient;
use youtrack::service::YouTrackService;

/// How often the answer of Patronus to a pull request comment is read, and for how long, as the app does
const COMMENT_REPLY_INTERVAL: Duration = Duration::from_secs(3);
const COMMENT_REPLY_TIMEOUT: Duration = Duration::from_secs(60);
/// A robot started by a comment is listed from a little before the comment: the clocks of GitHub and Patronus differ
const COMMENT_CLOCK_SLACK: u32 = 120;

#[derive(Debug, Default, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "command-line flags")]
pub struct SafePushArgs {
  /// The branch the robot runs on
  pub branch: String,
  /// Start a Patronus Dry Run: the robot runs the checks without the push into the target; the review is not checked.
  /// It pushes the branch and starts a robot, as a Safe Push does: only after the user approves
  #[arg(long)]
  pub patronus_dry_run: bool,
  /// The merge profile, by id or name; `none` for the default checks. Default: the profile of the last robot of the
  /// branch
  #[arg(long, value_name = "PROFILE")]
  pub profile: Option<String>,
  /// Sync, then print the profiles, the review and what stands in the way of a start; nothing is pushed or started
  #[arg(long)]
  pub check: bool,
  /// Start a Safe Push although the code owners have not approved it: Patronus fails it unless they approve meanwhile
  #[arg(long, conflicts_with = "patronus_dry_run")]
  pub without_approvals: bool,
  /// Push the branch and open its GitHub pull request, where the code owners approve, assigned to the user; no robot
  /// starts. The Safe Push starts from it then
  #[arg(long, conflicts_with_all = ["patronus_dry_run", "check", "without_approvals", "profile"])]
  pub create_pull_request: bool,
  /// With `--create-pull-request`: ask this GitHub login to review it (repeatable); `github_login` of the `review`
  /// reviewers of `--check` names the suggested code owners
  #[arg(long = "reviewer", value_name = "LOGIN", requires = "create_pull_request")]
  pub reviewers: Vec<String>,
  /// With `--create-pull-request`: don't assign the pull request to the user
  #[arg(long, requires = "create_pull_request")]
  pub no_assign: bool,
  /// Cancel a running Safe Push of an older commit of the branch first: the push would fail it, or, with nothing to push,
  /// the Safe Push replaces it
  #[arg(long)]
  pub cancel_running: bool,
  /// Obsolete and without effect: the Safe Push of a stacked branch always lands the branches it is built on with it.
  /// Accepted so that an older command line still runs
  #[arg(long, hide = true)]
  pub land_stack: bool,
  /// Start although the branch removes a declaration that a commit of another branch still changes the use of
  /// (`dangling_reference`): the build fails unless the reference is a false alarm
  #[arg(long)]
  pub allow_dangling: bool,
  /// Start although a commit of what lands uses a name that a commit of another branch adds (`missing_declaration`):
  /// the build fails unless the name is a false alarm
  #[arg(long)]
  pub allow_missing: bool,
  /// Start although GitHub didn't answer the rules of the target (`rules_unknown`: no token, no access, an error) for a
  /// push of new commits into a pull request someone approved: the push may dismiss the approvals Patronus counts, and
  /// the robot fails without them
  #[arg(long)]
  pub allow_unknown_rules: bool,
  /// Start although Patronus couldn't answer the code-owner review (`review_unknown`: its pre-check failed): the robot
  /// checks the approvals when it starts and fails without them
  #[arg(long, conflicts_with = "patronus_dry_run")]
  pub allow_unknown_review: bool,
  /// Start although the push drops commits of a remote branch that the branch and the baseline lack (`push_refused`,
  /// `dropped_commits`: someone's work): the start pushes them away with `--force`
  #[arg(long)]
  pub force: bool,
  /// Land the whole stack of the branch in one robot: from the stack branch `{prefix}/stack/<root>` of its top-level
  /// root, which the sync builds of every commit of the stack, and only it is pushed; its pull request is the stack's. A
  /// straight chain has none: its last branch carries the whole stack, and the robot starts from it
  #[arg(long)]
  pub whole_stack: bool,
}

/// `bd safe-push`
#[derive(Debug, Serialize, JsonSchema)]
pub struct SafePushOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub branch: String,
  /// `safe_push` or `dry_run`
  pub mode: String,
  /// The branch of `origin` the robot pushes into, e.g. `master`
  pub target: String,
  /// What the robot takes, root first, ending with the branch: the branches it is built on (none for a root), then the
  /// branch itself, whose virtual branch carries all their commits; with `--whole-stack`, every branch of the stack, each
  /// after its parent. A Safe Push lands them all into the target in one robot, a dry run checks them; with
  /// `--create-pull-request`, what its pull request holds. A branch a merged Safe Push landed all of is left out (the
  /// branch itself always named): its commits are in the target. Name them to the user
  pub lands: Vec<String>,
  /// With `--whole-stack`: the stack it lands whole and the branch the robot starts from
  #[serde(skip_serializing_if = "Option::is_none")]
  pub whole_stack: Option<WholeStackSummary>,
  /// The merge profile the robot starts with; `null` for the default checks, also for a start from a pull request
  /// comment, which can't choose one
  pub profile: Option<ProfileSummary>,
  /// The merge profiles of the target that allow the mode, never the emergency ones: what `--profile` takes
  pub profiles: Vec<ProfileSummary>,
  /// The code-owner review the Safe Push needs; `null` for a dry run
  pub review: Option<ReviewSummary>,
  /// What the GitHub rules of the target do to the approvals of the pull request the robot starts from: read only for a
  /// Safe Push that pushes new commits into a pull request someone approved, else absent
  #[serde(skip_serializing_if = "Option::is_none")]
  pub rules: Option<RulesSummary>,
  /// The remote lacks commits of the branch or of the branches it is built on, by content: the start pushes them
  /// first. False for a remote branch with the same commits, only rebuilt on an older baseline: the robot takes it as it
  /// is (Patronus rebases it), nothing is pushed
  pub push_needed: bool,
  /// The remote branches the start pushes that exist: what the push replaces of each (`remote.amended`, `restacked`,
  /// `landed` with `landed_commits`: older versions, no --force needed) and drops (`remote.behind` with
  /// `dropped_commits`, which refuses the start with `push_refused`)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub remotes: Vec<ErrorBranch>,
  /// With `--check`: the next step of the subject as the app's row shows it (`domain_policy::next_step`) over what the
  /// check read: the row of the table, the step, the fact it waits for, its pull request, why a Safe Push or a Dry Run
  /// can't start now
  #[serde(skip_serializing_if = "Option::is_none")]
  pub next_step: Option<NextStepSummary>,
  /// With `--check`: what refuses a start, each with the error code the start would fail with
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub refusals: Vec<Refusal>,
  /// Whether Patronus can rebase what the robot lands onto the newest target; `null` with `--create-pull-request`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub target_check: Option<TargetCheckSummary>,
  /// With `--check`, for a stacked branch: the trial build of the branch alone, as a root, whether it could land
  /// without the branches it is built on. A second way for the user to choose, never the lead; a start runs no trial
  #[serde(skip_serializing_if = "Option::is_none")]
  pub alone: Option<AloneBuild>,
  /// What may break the build though no commit proves it, as `bd check` warns: `possible_dangling_reference`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub warnings: Vec<CheckWarning>,
  /// The branches the start pushed, parents first; empty with `--check` and without `push_needed`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub pushed: Vec<PushedBranch>,
  /// The running Safe Push of an older commit that `--cancel-running` canceled: before the push, or before the start of a
  /// Safe Push that has nothing to push
  #[serde(skip_serializing_if = "Option::is_none")]
  pub canceled: Option<RobotSummary>,
  /// `api`, or `pull_request` for a `/patronus` comment on the open pull request of the branch; `null` when nothing
  /// was started
  pub started_via: Option<String>,
  /// The started robot; `null` with `--check` and `--create-pull-request`, and while Patronus hasn't listed the robot
  /// of a comment yet
  pub robot: Option<RobotSummary>,
  /// The pull request the robot starts from; with `--check`, the one it would start from; with
  /// `--create-pull-request`, the one it opened (or found open)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pull_request: Option<PullRequestStart>,
  /// The open pull requests of the branch and of the branches its push takes along whose base was a virtual branch,
  /// retargeted to the target before the push (with `--check`: the ones a start would retarget). Name them to the user,
  /// and the `approved_by` of each
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub retargeted: Vec<RetargetedPullRequest>,
  /// A retarget of a pull request of a branch taken along that GitHub refused, with the `gh pr edit` that does it
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub retarget_warnings: Vec<String>,
  /// The plan `refusals` and the start tell (`operations::land`, the decision of `domain_policy::land::decide`), not
  /// printed: the scenario table compares it with the app's (`crates/bd-remote/src/scenario_test.rs`)
  #[serde(skip)]
  #[schemars(skip)]
  pub plan: Option<Plan>,
  /// With `--check`: the next step `next_step` tells, not printed (the scenario table)
  #[serde(skip)]
  #[schemars(skip)]
  pub step: Option<domain_types::next_step::NextStep>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ProfileSummary {
  pub id: String,
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub description: Option<String>,
}

/// What a start would be refused with
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Refusal {
  pub code: ErrorCode,
  pub message: String,
  /// The commands that act on it, as the error of the start names them (`error.next_steps`)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub next_steps: Vec<NextStep>,
}

impl Refusal {
  fn new(code: ErrorCode, message: impl Into<String>) -> Self {
    Self {
      code,
      message: message.into(),
      next_steps: Vec::new(),
    }
  }

  /// The refusal with one more of its `next_steps`, see `CliError::with_next_step`
  fn with_next_step(mut self, command: impl Into<String>, needs_approval: bool) -> Self {
    self.next_steps.push(NextStep::new(command, needs_approval));
    self
  }
}

/// The replay of what the robot lands onto the newest target, as Patronus rebases it before its checks: a plain rebase,
/// with no resolution by words
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TargetCheckSummary {
  /// `clean`: every commit applies; `conflict`: a commit conflicts, refused as `target_conflict`; `not_run`: the check
  /// did not run to its end (`message` says why), and it holds nothing back
  pub result: String,
  /// The tip of the target the commits were replayed onto; `null` with `not_run`
  pub target_tip: Option<String>,
  /// Whether the fetch of the target before the check succeeded; `false`: the check used the last fetched tip, and
  /// `message` says why the fetch failed
  pub fetched: bool,
  /// With `conflict` and `not_run`, or after a failed fetch: what it means, to tell the user
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
}

/// The trial build of a stacked branch alone (`stack_independence` in `crates/trial-check/src/check.rs`): the branch as a
/// root on the base the sync builds on, not on the newest target; it writes git objects only and unstacks nothing
#[derive(Debug, Serialize, JsonSchema)]
pub struct AloneBuild {
  /// `builds`: it applies alone, and `bd unstack <branch>` (only after the user approves) makes it a root that lands
  /// alone, unless `uses` names a name of its stack; `needs`: a commit of it conflicts without the branches it is built
  /// on; `unknown`: the trial failed, `message` says why
  pub result: String,
  /// The parent the branch is stacked on
  pub parent: String,
  /// With `builds`: the names commits of the branch use that commits of the branches it is built on add, one per name:
  /// it applies alone but would not compile (found by names, not by a compiler)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub uses: Vec<AloneUse>,
  /// With `builds`: whether the names were read (`branchdeck.symbols on`): then an empty `uses` says that it uses no
  /// name of its stack; `false`: lines only, a compile dependency on the stack is not seen
  #[serde(skip_serializing_if = "Option::is_none")]
  pub symbols: Option<bool>,
  /// With `needs`: the first commit of the branch that conflicts, and its files
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflict: Option<CheckConflict>,
  /// With `needs`: the commits of the branches it is built on that it needs, and what these need, oldest first, by the
  /// line dependencies: what a move into the branch would move; empty when no commit of them explains the conflict
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub needs: Vec<StackCommitSummary>,
  /// With `needs`: whether `needs` can move into the branch (then `bd unstack <branch>`); `false`: other branches need
  /// them too, and a move would make those need this one
  #[serde(skip_serializing_if = "Option::is_none")]
  pub movable: Option<bool>,
  /// What it means, in a sentence to tell the user
  pub message: String,
}

/// A use, by a commit of the branch, of a name a commit of a branch it is built on adds
#[derive(Debug, Serialize, JsonSchema)]
pub struct AloneUse {
  pub identifier: String,
  /// The commit of the branch that uses it
  pub used_by: String,
  /// The commit that adds it
  pub declared_by: String,
  /// The branch of `declared_by`
  pub declared_in: String,
}

/// A commit ahead of the baseline and its branch
#[derive(Debug, Serialize, JsonSchema)]
pub struct StackCommitSummary {
  pub commit: String,
  /// The subject without its prefix
  pub subject: String,
  pub branch: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RobotState {
  Running,
  Canceling,
  Canceled,
  Failure,
  Success,
  Unknown,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RobotSummary {
  pub id: String,
  pub name: String,
  pub state: RobotState,
  /// `safe_push`, `dry_run`, or another mode of a robot started elsewhere
  pub mode: String,
  /// The commit Patronus took
  pub commit: Option<String>,
  /// The robot page; open it for the user
  pub url: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PullRequestStart {
  pub number: u32,
  pub url: String,
  /// `--create-pull-request` opened it
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub created: bool,
  /// The posted `/patronus` comment; `null` with `--check`
  pub comment_url: Option<String>,
  /// `accepted`, or `pending` while Patronus hasn't answered the comment within a minute; `null` with `--check`
  pub reply: Option<String>,
  /// `--create-pull-request`: the logins GitHub asked for a review
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub reviewers: Vec<String>,
  /// `--create-pull-request`: the logins GitHub assigned
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub assignees: Vec<String>,
  /// `--create-pull-request`: what failed after the pull request was opened (a reviewer GitHub refused, the assignee),
  /// or that one already existed and was left as it is
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub warnings: Vec<String>,
}

/// What the rules of the target do to the approvals of the pull request a Safe Push pushes new commits into
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RulesSummary {
  /// The rules dismiss the approvals of a pull request on a push of new commits; `null` when GitHub didn't answer
  pub dismiss_stale_reviews_on_push: Option<bool>,
  /// What it means for the start, in words: tell the user when the push dismisses approvals or the rules are unknown
  pub message: String,
}

/// The answer of the pre-check and what it means for the start
pub(crate) struct Review {
  pub(crate) requirement: PatronusReviewRequirement,
  pub(crate) readiness: ReviewReadiness,
}

/// A start as `prepare` planned it, with the facts `bd` tells it with, read under the repository lock
struct Prepared {
  /// The branch the robot runs on (for a straight chain with `--whole-stack`, its last branch), or the root of the
  /// whole stack
  name: String,
  /// The branch Patronus starts from, without `refs/heads/`
  full_name: String,
  mode: PatronusPushMode,
  space: SpaceRepository,
  target: String,
  github: Option<GitHubRepository>,
  /// The reads of the facts the plan read with: the robot a `/patronus` comment started is listed through them
  sources: Arc<dyn FactSources>,
  profiles: Vec<PatronusProfile>,
  profile: Option<PatronusProfile>,
  /// The code-owner review Patronus answered, with what it was asked for; `None` when not asked (a dry run)
  review: Option<SubjectReadiness>,
  /// The rules of the target for a push into the approved pull request the robot starts from
  rules: Option<RulesSummary>,
  /// Why the approvals don't hold the start back (`domain_policy::review::skips_approvals`); `None` when a `missing`
  /// readiness refuses it
  approvals_skipped_by: Option<ApprovalsSkip>,
  /// The open pull request the robot starts from: into the target, or for a branch into a virtual branch (retargeted
  /// first, `retargets`); one into another base refuses a start whose approvals count (`pull_request_other_base`), and
  /// a start that skips them goes through the API
  pull_request: Option<PullRequest>,
  /// The open pull requests into a virtual branch of the branch and of the branches its push takes along
  retargets: Vec<RetargetedPullRequest>,
  push_needed: bool,
  /// The branches the start pushes, root first (the decision's `Push`, `domain_policy::push::start_push`): only those, as
  /// the app's start pushes them, so an ancestor a running Safe Push holds stays as its robot took it; of a whole stack
  /// its root, whose stack branch is the only one pushed
  push: Vec<String>,
  /// The push replaces commits the remote has that the branch lacks (`--force`, the decision's `ForcePushDrops`)
  force: bool,
  /// The remote branches the start pushes, with their remote status
  remotes: Vec<ErrorBranch>,
  /// What the robot lands: the branches it is built on, root first, then the branch; or every branch of the stack
  lands: Vec<String>,
  refusals: Vec<Refusal>,
  /// With `--check`: the next step of the subject over what the check read, as the app's row shows it, with its words
  next_step: Option<(domain_types::next_step::NextStep, NextStepSummary)>,
  /// What `refusals`, the effects and the route above are of: the plan of the start (`operations::land`)
  decision: Plan,
  /// With `--check`, for a stacked branch: its trial build alone
  alone: Option<AloneBuild>,
  warnings: Vec<CheckWarning>,
  /// The running Safe Push of an older commit that the start replaces (the decision's `CancelRobot`): a push would fail
  /// it, and without a push the new robot would run beside it; canceled first with `--cancel-running`, as the app's
  /// form cancels it
  cancel: Option<PatronusRobot>,
  /// The ids of the robots that can't be the one a `/patronus` comment starts: the user's robots that had finished when
  /// listed before the start, and the robot the start replaces (`cancel`)
  known_robots: HashSet<String>,
  /// The whole stack of a stack that forks: its stack branch, the only branch pushed
  stack: Option<StackPush>,
  /// The replay onto the newest target and what it means; `None` with `--create-pull-request`
  target_check: Option<(TargetCheck, TargetCheckSummary)>,
}

/// The `skipped_by` of the `review`: why the approvals don't hold a Safe Push back
const fn approvals_skip_name(skip: ApprovalsSkip) -> &'static str {
  match skip {
    ApprovalsSkip::Profile => "profile",
    ApprovalsSkip::WithoutApprovals => "without_approvals",
  }
}

/// The stack branch a Safe Push of the whole stack pushes and starts from
struct StackPush {
  /// The branch on the remote, the same name as the local stack branch, `{prefix}/stack/<root>`
  full_name: String,
  /// Its tip as the sync built it; `None` without one
  tip: Option<String>,
  /// The remote head its remote status compared with, the lease of the push; `None` without a remote branch
  lease: Option<String>,
  /// The remote commits that landed in the baseline, which the push replaces
  replaced_landed: Vec<LandedRemoteCommitSummary>,
  /// Its pull request, as the text of the whole stack opens it
  draft: PullRequestDraft,
}

/// What a start lands, and from which branch: a branch with the branches it is built on starts from its virtual
/// branch (the whole stack of a straight chain is its last branch), the whole stack of a stack that forks from its
/// stack branch. One value, so the subject and the branch it starts from can't disagree
enum Start<'a> {
  Branch(String),
  WholeStack(&'a StackBranchUpdate),
}

impl Start<'_> {
  fn subject(&self) -> Subject {
    match self {
      Self::Branch(name) => Subject::Branch { name: name.clone() },
      Self::WholeStack(update) => Subject::WholeStack { root: update.root.clone() },
    }
  }
}

/// `bd safe-push <name>`, with `--whole-stack` for the whole stack of a stack that forks
fn safe_push_again(context: &RepoContext, name: &str, whole_stack: bool) -> String {
  let whole_stack = if whole_stack { " --whole-stack" } else { "" };
  format!("{} safe-push {}{whole_stack}", context.bd, shell_word(name))
}

/// The start as `args` asked it, to run again: the mode, the profile and the pull request it opens, never a flag that
/// starts anyway (`--allow-*`, `--without-approvals`), which only the user decides
fn start_again(context: &RepoContext, args: &SafePushArgs, name: &str, whole_stack: bool) -> String {
  let mut command = safe_push_again(context, name, whole_stack);
  if args.patronus_dry_run {
    command.push_str(" --patronus-dry-run");
  }
  if let Some(profile) = &args.profile {
    let _ = write!(command, " --profile {}", shell_word(profile));
  }
  if args.create_pull_request {
    command.push_str(" --create-pull-request");
    for reviewer in &args.reviewers {
      let _ = write!(command, " --reviewer {}", shell_word(reviewer));
    }
    if args.no_assign {
      command.push_str(" --no-assign");
    }
  }
  command
}

/// What to do with an unapplied branch instead of its Safe Push
const UNAPPLIED_SAFE_PUSH: &str = "nothing was pushed or started: reapply it to land it";

pub fn safe_push(context: &RepoContext, args: &SafePushArgs, environment: &Environment) -> Result<SafePushOutput, CliError> {
  let mode = if args.patronus_dry_run {
    PatronusPushMode::DryRun
  } else {
    PatronusPushMode::RebaseAutosquash
  };
  // an unapplied branch is refused as one also without a token, which a Safe Push of it never needs; a branch of HEAD
  // costs no process for it
  let service = Arc::new(patronus_service(environment).map_err(|error| refuse_an_unapplied_name(context, &args.branch, UNAPPLIED_SAFE_PUSH).err().unwrap_or(error))?);
  let github = Arc::new(github_service(context, environment));
  let youtrack = youtrack_service(environment);
  let runtime = context.runtime()?;

  // the target moves every minute: its remote branch is fetched for the target conflict check, before the lock, which
  // no fetch runs under; a pull request starts no robot and needs no check
  let target_fetch = (!args.create_pull_request).then(|| fetch_target(&context.git, &context.repository));
  let lock = lock(context)?;
  let synced = sync_locked(context)?;
  // a check writes nothing after its sync: the trial builds and Patronus don't keep the other bd writers waiting
  let mut lock = (!args.check).then_some(lock);
  let whole_stack = whole_stack_arg(context, args, &synced)?;
  let start = start_of(args, &synced, whole_stack.as_ref());
  let plan = plan(context, args, &service, &github, &synced, mode, &start, target_fetch)?;
  let mut output = start_output(
    context,
    args,
    mode,
    whole_stack.as_ref().map(|whole| (whole, synced.report.stack_branches.as_slice())),
    &plan,
  );
  if args.check {
    return Ok(check_output(output, plan));
  }
  if let Some(refusal) = plan.refusals.first().cloned() {
    let mut error = CliError::new(refusal.code, refusal.message);
    for step in refusal.next_steps {
      error.push_next_step(step);
    }
    if refusal.code == ErrorCode::PushRefused {
      error.branches = plan.remotes.iter().filter(|branch| branch.remote.drops_remote_commits()).cloned().collect();
    }
    if refusal.code == ErrorCode::ApprovalsMissing {
      error.review = output.review.take().map(Box::new);
    }
    if let (ErrorCode::StackConflict, Start::WholeStack(update)) = (refusal.code, &start)
      && let Some(conflict) = &update.conflict
    {
      error.commits = vec![context.short_ids(std::iter::once(conflict.commit.as_str())).get(&conflict.commit).to_string()];
      error.paths.clone_from(&conflict.files);
    }
    if let (ErrorCode::TargetConflict, Some((check, _))) = (refusal.code, &plan.target_check)
      && let TargetOutcome::Conflict(conflict) = &check.outcome
    {
      let commit = original_commit(&synced, conflict);
      error.commits = vec![context.short_ids(std::iter::once(commit)).get(commit).to_string()];
      error.paths.clone_from(&conflict.files);
    }
    return Err(error);
  }

  // the pull requests go into the target before anything is pushed or canceled: the one the robot starts from must,
  // one of a branch taken along that GitHub refuses is a warning
  if let Some(repository) = &plan.github {
    for retarget in &plan.retargets {
      match runtime.block_on(operations::pull_request::retarget(
        &github,
        &context.repository,
        repository,
        retarget.number,
        &retarget.new_base,
      )) {
        Ok(()) => output.retargeted.push(retarget.clone()),
        Err(error) if retarget.branch == plan.name => {
          let mut error = github_write_error(error);
          error.message = format!(
            "Nothing was pushed or started: cannot retarget pull request #{} of {} from {} to {}: {}",
            retarget.number, retarget.branch, retarget.old_base, retarget.new_base, error.message
          );
          return Err(error);
        }
        Err(error) => output.retarget_warnings.push(refusal_warning(retarget, repository, error)),
      }
    }
  }
  if let Some(robot) = &plan.cancel {
    let canceled = runtime.block_on(service.cancel(&context.prefix, &robot.id)).map_err(patronus_error)?;
    output.canceled = Some(robot_summary(context, &canceled));
  }
  // by content: a branch whose remote branch has its commits, only rebuilt on an older baseline, is not pushed; the
  // robot takes the remote branch and Patronus rebases it, and a push would move the head of its pull request for nothing
  let draft = match (&plan.stack, plan.push_needed) {
    // the whole stack: only its stack branch, leased; its members stay as they are on the remote
    (Some(stack), true) => {
      output.pushed = vec![push_stack_branch(context, &synced, &plan.name, stack, &mut lock)?];
      stack.draft.clone()
    }
    (Some(stack), false) => stack.draft.clone(),
    (None, true) => {
      // the robot of the branch is canceled above; the start pushes what its decision pushes, the ancestors it lands
      // with it up to the nearest one a running Safe Push holds, each alone, as the app's start pushes them
      let args = PushArgs {
        branches: plan.push.clone(),
        alone: true,
        force: plan.force,
        ..PushArgs::default()
      };
      let holds = PushHolds::default();
      let planned = plan_push(context, &args, &synced, &holds, &HashSet::new())?;
      // the commits and the leases are fixed: the network push holds no lock
      drop(lock.take());
      let pushed = planned.send(context, &args, &synced, None, Some(&youtrack))?;
      output.pushed = pushed.branches;
      match output.pushed.iter().find(|pushed| pushed.name == plan.name).and_then(|pushed| pushed.pull_request.clone()) {
        Some(draft) => draft,
        // on the remote as it is, only an ancestor pushed
        None => unpushed_pull_request_draft(context, &synced, &plan.name, Some(&youtrack))?,
      }
    }
    (None, false) => unpushed_pull_request_draft(context, &synced, &plan.name, Some(&youtrack))?,
  };
  drop(lock);

  if args.create_pull_request {
    if output.pull_request.is_none() {
      output.pull_request = Some(create_pull_request(context, args, &github, runtime, &plan, &draft)?);
    }
    return Ok(output);
  }

  if let Some(pull_request) = &plan.pull_request {
    let (start, robot) = start_from_pull_request(context, &github, runtime, &plan, pull_request)?;
    output.pull_request = Some(start);
    output.robot = robot.map(|robot| robot_summary(context, &robot));
    output.started_via = Some("pull_request".to_string());
  } else {
    let merge_request = plan.review.as_ref().and_then(|review| review.requirement.as_ref()?.merge_request.clone());
    let robot = runtime
      .block_on(service.start(
        &plan.space.name,
        &context.prefix,
        &format!("refs/heads/{}", plan.full_name),
        &plan.target,
        plan.mode,
        plan.profile.as_ref().map(|profile| profile.id.clone()),
        merge_request,
      ))
      .map_err(patronus_error)?;
    output.robot = Some(robot_summary(context, &robot));
    output.started_via = Some("api".to_string());
  }
  Ok(output)
}

/// The whole stack `--whole-stack` lands, from the sync (`whole_stack`: the rule of the app, by declared parent); a
/// stack branch the sync doesn't know is `stack_not_built`, which the next sync builds
fn whole_stack_arg<'s>(context: &RepoContext, args: &SafePushArgs, synced: &'s SyncOutput) -> Result<Option<WholeStack<'s>>, CliError> {
  if !args.whole_stack {
    return Ok(None);
  }
  let report = &synced.report;
  whole_stack(&report.branches, &report.grouping.parked.entries, &report.stack_branches, &args.branch)
    .map(Some)
    .map_err(|error| match error.code {
      ErrorCode::StackNotBuilt => error.with_next_step(format!("{} sync", context.bd), false),
      _ => error,
    })
}

/// What the start lands, and from which branch: the branch; of the whole stack, a stack that forks from its stack
/// branch, a straight chain from its last branch
fn start_of<'a>(args: &SafePushArgs, synced: &'a SyncOutput, whole_stack: Option<&WholeStack<'_>>) -> Start<'a> {
  match whole_stack {
    Some(WholeStack::Chain { leaf, .. }) => Start::Branch((*leaf).to_string()),
    Some(WholeStack::Forking { root }) => match stack_branch_of(&synced.report.stack_branches, root) {
      Some(update) => Start::WholeStack(update),
      None => Start::Branch(args.branch.clone()),
    },
    _ => Start::Branch(args.branch.clone()),
  }
}

/// The output of a start as `plan` decides it, before anything is pushed or started
fn start_output(context: &RepoContext, args: &SafePushArgs, mode: PatronusPushMode, whole_stack: Option<(&WholeStack, &[StackBranchUpdate])>, plan: &Prepared) -> SafePushOutput {
  SafePushOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    mode: mode_name(mode),
    target: plan.target.clone(),
    lands: plan.lands.clone(),
    whole_stack: whole_stack.map(|(whole, stacks)| whole_stack_summary(whole, stacks, &context.prefix)),
    profile: plan.profile.as_ref().map(profile_summary),
    profiles: plan.profiles.iter().map(profile_summary).collect(),
    review: plan
      .review
      .as_ref()
      .map(|review| review_summary(review, plan.github.is_some(), plan.pull_request.as_ref(), plan.approvals_skipped_by)),
    rules: plan.rules.clone(),
    push_needed: plan.push_needed,
    remotes: plan.remotes.clone(),
    refusals: Vec::new(),
    next_step: None,
    target_check: plan.target_check.as_ref().map(|(_, summary)| summary.clone()),
    alone: None,
    warnings: plan.warnings.clone(),
    pushed: Vec::new(),
    canceled: None,
    started_via: None,
    robot: None,
    pull_request: plan.pull_request.as_ref().map(|pull_request| PullRequestStart {
      number: pull_request.number,
      url: pull_request.url.clone(),
      created: false,
      comment_url: None,
      reply: None,
      reviewers: Vec::new(),
      assignees: Vec::new(),
      warnings: Vec::new(),
    }),
    retargeted: Vec::new(),
    retarget_warnings: Vec::new(),
    plan: Some(plan.decision.clone()),
    step: None,
  }
}

/// The output of `--check`: `output` with what refuses a start, the trial build alone and the retargets a start would do
fn check_output(mut output: SafePushOutput, plan: Prepared) -> SafePushOutput {
  output.refusals = plan.refusals;
  (output.step, output.next_step) = plan.next_step.unzip();
  output.alone = plan.alone;
  output.retargeted = plan.retargets;
  output
}

/// The remote of the stack branch: Patronus pushes from `origin`, whose remote status the sync computed
const STACK_REMOTE: &str = "origin";

/// What a start lands as the sync left it, for the reads of the start and the words of its refusals
struct Landing {
  /// The branch the robot runs on, or the root of the whole stack
  name: String,
  /// "auth", or "the whole auth stack"
  what: String,
  /// The branch Patronus starts from, without `refs/heads/`
  full_name: String,
  local_tip: Option<String>,
  /// What the start lands as the table has it, landed or not: the branches it is built on, root first, then the branch;
  /// every branch of the whole stack
  built: Vec<String>,
  /// The remote branches the push changes, with their remote status
  remotes: Vec<ErrorBranch>,
  stack: Option<StackPush>,
}

/// What `start` lands and the remote branches its push changes, as the sync left them: a branch with the branches it is
/// built on, the whole stack of a stack that forks from its stack branch, the only branch pushed
fn landing_of(context: &RepoContext, synced: &SyncOutput, table: &BranchTable, start: &Start<'_>) -> Result<Landing, CliError> {
  match start {
    Start::Branch(name) => {
      let full_name = to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
      let built: Vec<String> = table.ancestors_of(name).into_iter().chain(std::iter::once(name.clone())).collect();
      // what the push replaces and drops of each remote branch: one that drops commits the baseline lacks refuses the
      // start (`Reason::DropsRemote`), with the push that drops them
      let remotes = built
        .iter()
        .filter_map(|branch| synced.branches.iter().find(|synced| synced.name == *branch))
        .filter_map(|branch| {
          let remote = branch.remote.clone().filter(|remote| remote.exists && remote.needs_push())?;
          Some(ErrorBranch {
            name: branch.name.clone(),
            remote,
          })
        })
        .collect();
      Ok(Landing {
        name: name.clone(),
        what: name.clone(),
        full_name,
        local_tip: table.tip_of(name).map(str::to_string),
        built,
        remotes,
        stack: None,
      })
    }
    Start::WholeStack(update) => {
      let root = update.root.as_str();
      let remote = update.remote.as_ref();
      let short = context.short_ids(remote.into_iter().flat_map(RemoteSummary::ids));
      let remotes = remote
        .filter(|remote| remote.remote_exists && needs_push(remote))
        .map(|remote| ErrorBranch {
          name: update.branch_name.clone(),
          remote: RemoteSummary::new(remote, usize::try_from(update.commit_count).unwrap_or(usize::MAX), &short),
        })
        .into_iter()
        .collect();
      let base = target_branch_of(synced.report.upstream.as_ref()).unwrap_or_default();
      Ok(Landing {
        name: root.to_string(),
        what: Subject::whole_stack(root).words(),
        full_name: update.branch_name.clone(),
        local_tip: update.tip.clone(),
        built: update.members.clone(),
        remotes,
        stack: Some(StackPush {
          full_name: update.branch_name.clone(),
          tip: update.tip.clone(),
          lease: remote.and_then(|remote| remote.remote_tip.clone()),
          replaced_landed: remote.map_or_default(|remote| landed_commits(remote, &short)),
          draft: stack_pull_request_draft(context, &synced.report, base, update)?,
        }),
      })
    }
  }
}

/// The table of `bd`'s sync as the rules read it: its sync compared every remote branch it could, so a branch or a stack
/// branch without a remote status is one the remote lacks, which the push creates (`domain_policy::land::decide` waits
/// for a remote nobody compared yet)
fn compared_table(context: &RepoContext, synced: &SyncOutput) -> BranchTable {
  let mut table = synced.branch_table(&context.prefix);
  for fact in table.remote.branches.values_mut().filter(|fact| !fact.known) {
    fact.known = true;
    fact.up_to_date = false;
  }
  for stack in &table.stack_branches {
    table.remote.stacks.entry(stack.root.clone()).or_insert_with(|| RemoteFact {
      known: true,
      ..RemoteFact::default()
    });
  }
  table
}

/// The subject part of the decision of `start` (`LandingFacts::branch_subject`, `stack_subject`) over `pull_requests` and
/// the landing report of the sync
fn subject_facts<'a>(
  context: &RepoContext,
  synced: &SyncOutput,
  landing: &'a LandingFacts<'a>,
  start: &'a Start<'_>,
  pull_requests: Option<&PullRequestsView>,
) -> Result<SubjectFacts<'a>, CliError> {
  let baseline = &synced.landed_section;
  Ok(match start {
    Start::Branch(name) => SubjectFacts::Branch(
      landing
        .branch_subject(name, pull_requests, baseline)
        .ok_or_else(|| unknown_or_unapplied(context, name, UNAPPLIED_SAFE_PUSH))?,
    ),
    Start::WholeStack(update) => SubjectFacts::WholeStack(landing.stack_subject(update, pull_requests, baseline)),
  })
}

/// What the table alone refuses of a start (`domain_policy::land::table_blocker`), as the error of the command: the sync
/// did not build the branch, it stays local, the stack branch holds a parked member
fn table_error(context: &RepoContext, synced: &SyncOutput, start: &Start<'_>, reason: &Reason) -> CliError {
  let report = &synced.report;
  if let Reason::CantPush { branch, .. } = reason {
    let detail = synced
      .branches
      .iter()
      .find(|synced| synced.name == *branch)
      .and_then(|synced| not_synced_reason(report, synced))
      .unwrap_or_default();
    CliError::new(
      ErrorCode::PushRefused,
      format!("Nothing was pushed or started: {branch} is not synced ({detail}); fix it first, `bd sync` says what holds it"),
    )
    .with_next_step(format!("{} sync", context.bd), false)
  } else {
    let words = match (reason, start) {
      (Reason::StackParked { .. }, Start::WholeStack(update)) => {
        let repo = Repository::of_grouping(&report.branches, &report.grouping.parked.entries);
        let member = whole_stack_parked(&repo, update.members.iter().map(String::as_str)).map_or_default(|ParkedMember { member, parked }| parked_words(&member, &parked));
        format!("{} holds what is parked: {member}; land the stack in pieces", Subject::whole_stack(&update.root).words())
      }
      _ => start_refusal_text(reason, &start.subject().words(), &[]),
    };
    CliError::new(
      ErrorCode::Parked,
      format!("Nothing was pushed or started: {words}; it stays local until the user unparks it: tell the user, never unpark it yourself"),
    )
  }
}

/// "Nothing was pushed or started: the whole auth stack doesn't build as one branch: …": the commit and its files
/// (`stack_conflict_words`, the app's words too), and the way out
fn stack_conflict_message(context: &RepoContext, synced: &SyncOutput, update: &StackBranchUpdate) -> String {
  let Some(conflict) = &update.conflict else {
    return format!("Nothing was pushed or started: the stack branch of {} conflicts; `bd sync` names the commit", update.root);
  };
  let short = context.short_ids(std::iter::once(conflict.commit.as_str()));
  format!(
    "Nothing was pushed or started: {}. Land it in pieces: `bd safe-push <branch> --check` lands a branch with what it is built on; or regroup so that the commits it needs come before it",
    stack_conflict_words(&synced.report.branches, update, &|id| short.get(id).to_string())
  )
}

/// The branch of the commit of the whole stack that conflicts with the commits before it
fn conflict_branch<'a>(synced: &'a SyncOutput, update: &StackBranchUpdate) -> Option<&'a str> {
  let conflict = update.conflict.as_ref()?;
  synced
    .report
    .branches
    .iter()
    .find(|branch| branch.commits.iter().any(|commit| commit.id == conflict.commit))
    .map(|branch| branch.name.as_str())
}

/// `bd push <root> --with-children --force`: the push that replaces the remote stack branch of `root`
fn stack_force_push(context: &RepoContext, root: &str) -> String {
  format!("{} push {} --with-children --force", context.bd, shell_word(root))
}

/// Why a stack branch that is not built can't land (`stack_not_built_words`, the app's words too), and the way out
fn stack_not_built_message(update: &StackBranchUpdate) -> String {
  format!("Nothing was pushed or started: {}. `bd sync` says what to fix", stack_not_built_words(update))
}

/// What the build of what lands misses: the first missing declaration with its fix, and the count of the rest, which
/// `bd check` lists (`missing_declarations`)
fn missing_declaration_message(context: &RepoContext, missing: &[LandingRefusal]) -> String {
  let short = context.short_ids(missing.iter().flat_map(|refusal| match refusal {
    LandingRefusal::MissingDeclaration { used_by, declared_by, .. } => vec![used_by.id.as_str(), declared_by.id.as_str()],
    LandingRefusal::DanglingReference { .. } => Vec::new(),
  }));
  let named = |commit: &StackCommit| format!("{} ({})", short.get(&commit.id), commit.subject);
  let Some(LandingRefusal::MissingDeclaration {
    branch,
    identifier,
    file,
    line,
    used_by,
    declared_by,
    fix,
    ..
  }) = missing.first()
  else {
    return String::new();
  };
  let adds = if declared_by.branch.is_empty() {
    format!("the unassigned commit {} adds", named(declared_by))
  } else {
    format!("{} of {} adds", named(declared_by), declared_by.branch)
  };
  let fix = match fix {
    LandingFix::Restack { branch: root, onto } => format!(
      "`{} restack {root} {onto}` lands {onto} with it, after the user approves, or move one of the two",
      context.bd
    ),
    LandingFix::MoveCommit { commit, into } => format!("Move {} into {into}", short.get(&commit.id)),
  };
  let more = match missing.len() - 1 {
    0 => String::new(),
    rest => format!(" ({rest} more: bd check)"),
  };
  format!(
    "Nothing was pushed or started: {} of {branch} uses {identifier} ({file}:{line}), which {adds}; git merges it, but the build fails. {fix}{more}; --allow-missing starts anyway, only when the user says it is a false alarm",
    named(used_by)
  )
}

/// Everything the start is decided on, before anything is pushed: what the table alone refuses, then the repository on
/// Space, the robots of the user, what lands and its trial, the profiles and the pull requests; then the plan of the
/// start (`operations::land::prepare_one_shot`), which reads the review and the rules of the target as its first
/// decision waits for them and decides by `domain_policy::land::decide`, as the app's `prepare` does, told in the words
/// of `bd`
#[expect(clippy::too_many_arguments, reason = "the services and the sync of the command")]
#[expect(clippy::too_many_lines, reason = "the reads of the start in their order, each a few lines")]
fn plan(
  context: &RepoContext,
  args: &SafePushArgs,
  service: &Arc<PatronusService>,
  github_service: &Arc<GitHubService>,
  synced: &SyncOutput,
  mode: PatronusPushMode,
  start: &Start<'_>,
  target_fetch: Option<TargetFetch>,
) -> Result<Prepared, CliError> {
  let subject = start.subject();
  if let Start::Branch(name) = start
    && !synced.branches.iter().any(|branch| branch.name == *name)
  {
    return Err(unknown_or_unapplied(context, name, UNAPPLIED_SAFE_PUSH));
  }
  let table = compared_table(context, synced);
  // what the table alone refuses, before Patronus or GitHub is asked: the sync did not build it, it stays local
  {
    let unasked = OneShot::default();
    let landing = unasked.landing(&table);
    if let Some(reason) = table_blocker(&subject_facts(context, synced, &landing, start, None)?) {
      return Err(table_error(context, synced, start, &reason));
    }
  }
  let landing = landing_of(context, synced, &table, start)?;
  // the Space repository, the target and the GitHub repository from the push URL of origin and what the sync read
  let space = context
    .space_repository()?
    .ok_or_else(|| CliError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't push it"))?;
  let target = target_branch_of(synced.report.upstream.as_ref()).ok_or_else(|| {
    CliError::new(
      ErrorCode::NotOnSpace,
      format!("The baseline {} is not a branch of origin: Patronus pushes into a branch of origin", synced.baseline),
    )
  })?;
  let github = github_repository(context, &synced.report)?;
  let mut land = Land {
    subject: subject.clone(),
    mode,
    profile: ProfileChoice::Default,
    without_approvals: args.without_approvals,
    create_pull_request: args.create_pull_request,
    allow: LandingAllowances {
      allow_dangling: args.allow_dangling,
      allow_missing: args.allow_missing,
      allow_force: args.force,
      allow_unknown_rules: args.allow_unknown_rules,
      allow_unknown_review: args.allow_unknown_review,
      cancel_running: args.cancel_running,
    },
  };

  // what the start reads of the remote, as the session of the app reads it (`operations::needs`): the robots of the
  // user, each matched to its branch (or the whole stack) and what the merged ones landed of it by content, the pull
  // requests, the merge profiles of the target, the profile of the last robot of the subject when the list names none;
  // with --check the problems of its failed robot. One that had finished before a comment is never the one it starts
  let services = BdServices {
    patronus: Some(Arc::clone(service)),
    space: Some(space.clone()),
    github_repository: github.clone(),
    target: Some(target.clone()),
    ..BdServices::new(context, Arc::clone(github_service))
  };
  let mut facts = SessionFacts::new(context, synced.table.clone(), table.clone(), services.sources());
  let mut first_needs = needs(&Intent::Land(land.clone()));
  if args.check {
    first_needs.push(FactNeed::FailedRobot(subject.clone()));
  }
  facts.read(context, &first_needs)?;
  keep_listed(context, service, &space, listed_robots(&facts.session)?);
  let mut known_robots: HashSet<String> = facts.session.robots().iter().filter(|robot| !is_active(robot)).map(|robot| robot.id.clone()).collect();
  let landed = landed_branches(&facts.session);
  let name = landing.name.as_str();

  // what lands, in one trial: a removed declaration that another branch's commit still changes the use of, a name that a
  // branch outside it adds; git merges either, the build fails. The landing needs of a branch are its gate's
  let trial = session_of_sync(context, &synced.report)?;
  let built: Vec<&str> = landing.built.iter().map(String::as_str).collect();
  let landed_commits: HashSet<String> = landed.values().flat_map(|landed| landed.landed.iter().cloned()).collect();
  let gate = check_landing_gate(&trial, &subject, &landed_commits).map_err(regroup_error)?;
  let (dangling, warnings) = dangling_references_in(context, &trial, &gate.report, &built)?;
  let gate_refusals = gate.refusals(&trial);
  if let Start::Branch(name) = start {
    let needs = if gate_refusals.is_empty() {
      Needs::new()
    } else {
      Needs::from([((*name).clone(), gate_refusals.clone())])
    };
    facts.session.landing_needs(|_| Ok(needs));
  }
  // Patronus rebases what it lands onto the newest target before any check, and fails the robot on a conflict
  let rerun = format!("bd safe-push {name}{}", if landing.stack.is_some() { " --whole-stack" } else { "" });
  let target_check = target_fetch.map(|fetch| {
    let fetch_error = fetch.into_error();
    let check = match landing.local_tip.as_deref() {
      Some(tip) => check_target(&context.git, &context.repository, &synced.baseline, &synced.report.baseline_commit, tip, fetch_error),
      None => TargetCheck {
        baseline: synced.baseline.clone(),
        fetch_error,
        outcome: TargetOutcome::NotRun {
          error: format!("{} has no tip", landing.full_name),
        },
      },
    };
    let summary = target_check_summary(context, synced, &landing.what, &rerun, &check);
    (check, summary)
  });
  // the second way, for the user to choose: the branch alone, in the same trial session; a start never asks
  let alone = match (&landing.stack, landing.built.len().checked_sub(2).and_then(|parent| landing.built.get(parent))) {
    (None, Some(parent)) if args.check => Some(alone_build(context, &trial, name, parent)),
    _ => None,
  };
  let conflict = target_check.as_ref().and_then(|(check, _)| match &check.outcome {
    TargetOutcome::Conflict(conflict) => Some(TargetConflictFact {
      commit: &conflict.commit.id,
      files: &conflict.files,
    }),
    _ => None,
  });
  let host = LandHost {
    table: &table,
    baseline: &synced.landed_section,
    target: &target,
    github: github.is_some(),
    gate: &gate_refusals,
    target_conflict: conflict,
  };

  // the profile `--profile` names among those of the target that allow the mode; a comment can't choose one, it starts
  // the default checks of the target
  let profiles: Vec<PatronusProfile> = profiles_of(&facts.session)?.profiles.iter().filter(|profile| allows_mode(profile, mode)).cloned().collect();
  if args.profile.as_deref().is_some_and(|profile| !profile.eq_ignore_ascii_case("none"))
    && let Some(StartRoute::Comment { number }) = one_shot_plan(&facts.session, &land, &host).as_ref().and_then(Plan::start_route)
  {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!(
        "{} starts from pull request #{number}: a /patronus comment starts the default checks of {target} and can't choose a merge profile; drop --profile",
        landing.what
      ),
    ));
  }
  land.profile = profile_choice(args, &profiles, &target, mode)?;
  if args.create_pull_request && github.is_none() {
    return Err(CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY));
  }
  // the plan of the start over what was read, as the app's session plans it (`operations::land`): what its first decision
  // waits for is read next (the review of the subject with the pull request the robot starts from, of the whole stack
  // its stack branch; the rules of the target for a push into an approved pull request: one GET, only then), and it is
  // decided again
  let decision = facts
    .prepare(context, &land, &host)?
    .ok_or_else(|| unknown_or_unapplied(context, &landing.name, UNAPPLIED_SAFE_PUSH))?;
  if let Some(Reason::Unprotected { target }) = decision.blockers.iter().find(|reason| matches!(reason, Reason::Unprotected { .. })) {
    return Err(CliError::new(
      ErrorCode::PatronusRefused,
      format!("Patronus does not protect {target}: a robot can't be started into it"),
    ));
  }
  let session = &facts.session;
  // the review and the rules as the plan read them: none it didn't wait for
  let readiness = subject_readiness(session, &subject)?;
  let rules = session.facts().rules.get(&target).map(FactRecord::fact);
  let pull_requests = session.facts().pull_requests.view();
  let start_pull_request = decision.pull_request.and_then(|number| pull_request_of(pull_requests, start, number)).cloned();
  // the next step of the subject over what the start read, the problems of its failed robot with them (whether it
  // failed for the approvals), as the session reads them of its failed robots
  let next_step = args.check.then(|| {
    let read = CheckRead {
      table: &table,
      session,
      landed: &synced.landed_section,
      readiness,
      target: &target,
    };
    check_next_step(context, &read, start, &landing.what)
  });
  let (refusals, rules) = {
    let told = Told {
      context,
      args,
      synced,
      session,
      landed: &landed,
      start,
      landing: &landing,
      rerun: &rerun,
      dangling: &dangling,
      target: &target,
      github: github.as_ref(),
      target_check: target_check.as_ref().map(|(_, summary)| summary),
      readiness: readiness.and_then(|fact| fact.value.as_ref()),
      rules,
      start_pull_request: start_pull_request.as_ref(),
      pushes: decision.effects.iter().any(|effect| matches!(effect, PlannedEffect::Push { .. })),
    };
    (told.refusals(&decision, land.allow), told.rules_summary())
  };
  // what the start does once nothing refuses it: the retargets, the robot it replaces, the push, the route
  let cancel = decision.effects.iter().find_map(|effect| match effect {
    PlannedEffect::CancelRobot { robot_id } => session.robot(robot_id).cloned(),
    _ => None,
  });
  known_robots.extend(cancel.iter().map(|robot| robot.id.clone()));
  let (push, force) = decision
    .effects
    .iter()
    .find_map(|effect| match effect {
      PlannedEffect::Push { branches, force } => Some((branches.clone(), Some(*force))),
      _ => None,
    })
    .unwrap_or_default();
  let retargets = decision
    .effects
    .iter()
    .filter_map(|effect| match effect {
      PlannedEffect::Retarget { branch, number, to, .. } => pull_requests
        .pull_requests
        .get(branch)
        .filter(|pull_request| pull_request.number == *number)
        .map(|pull_request| RetargetedPullRequest::of(branch, pull_request, to)),
      _ => None,
    })
    .collect();
  let profile = match decision.start_route() {
    Some(StartRoute::Api { profile }) => profile.clone(),
    _ => None,
  };
  let Landing {
    name,
    full_name,
    mut remotes,
    stack,
    ..
  } = landing;
  // of the branches it lands, the ones it pushes: a held ancestor's remote stays as it is
  if stack.is_none() {
    remotes.retain(|remote| push.contains(&remote.name));
  }
  Ok(Prepared {
    name,
    full_name,
    mode,
    space,
    target,
    github,
    sources: facts.sources(),
    profiles,
    profile,
    review: readiness.and_then(|fact| fact.value.clone()),
    rules,
    approvals_skipped_by: decision.approvals_skipped_by,
    pull_request: start_pull_request,
    retargets,
    push_needed: force.is_some(),
    push,
    force: force.unwrap_or(false),
    remotes,
    lands: decision.lands.clone(),
    refusals,
    next_step: next_step.flatten(),
    decision,
    alone,
    warnings,
    cancel,
    known_robots,
    stack,
    target_check,
  })
}

/// The merge profiles of the target as the session read them, or why Patronus gave none
fn profiles_of(session: &OneShot) -> Result<&PatronusStartOptions, CliError> {
  let fact = session.facts().profiles.fact();
  match (&fact.value, &fact.state) {
    (Some(options), _) => Ok(options),
    (None, FactState::Unavailable { reason }) => Err(patronus_error(reason.message.clone())),
    (None, _) => Err(CliError::new(ErrorCode::PatronusError, "Patronus gave no merge profiles of the target")),
  }
}

/// The profile `--profile` chooses among `profiles`, the ones of `target` that allow the mode: the default without it,
/// none for `none`
fn profile_choice(args: &SafePushArgs, profiles: &[PatronusProfile], target: &str, mode: PatronusPushMode) -> Result<ProfileChoice, CliError> {
  match args.profile.as_deref() {
    None => Ok(ProfileChoice::Default),
    Some(profile) if profile.eq_ignore_ascii_case("none") => Ok(ProfileChoice::None),
    Some(wanted) => profiles
      .iter()
      .find(|profile| profile.id == wanted || profile.name.eq_ignore_ascii_case(wanted))
      .map(|profile| ProfileChoice::Id { id: profile.id.clone() })
      .ok_or_else(|| {
        let names = profiles.iter().map(|profile| format!("{} ({})", profile.id, profile.name)).collect::<Vec<_>>().join(", ");
        CliError::new(
          ErrorCode::InvalidArguments,
          format!("No merge profile {wanted:?} of {target} allows a {}: {names}, or none", push_mode_name(mode)),
        )
      }),
  }
}

/// The review of the subject as the session read it, asked again with the pull request the robot starts from (never a
/// kept answer, which it writes for the app), the paths from the object cache, then the `OWNERSHIP` files for the
/// groups Patronus names without their files. A pre-check that failed is an answer (`Unavailable`), which the decision
/// asks the user to accept (`review_unknown`, D14); a Patronus that doesn't let the user in is `not_logged_in`; `None`
/// when nothing was asked (no tip)
pub(crate) fn subject_readiness<'s>(session: &'s OneShot, subject: &Subject) -> Result<Option<&'s Fact<SubjectReadiness>>, CliError> {
  let Some(fact) = session.facts().readiness.fact(&subject.key()) else {
    return Ok(None);
  };
  match (&fact.value, &fact.state) {
    (None, FactState::Unavailable { reason }) => Err(CliError::new(reason.code, reason.message.clone())),
    (None, _) => Ok(None),
    (
      Some(SubjectReadiness {
        requirement: None,
        readiness: ReviewReadiness::Unavailable {
          reason: UnavailableReadinessReason::NotLoggedIn,
          message,
        },
        ..
      }),
      _,
    ) => Err(CliError::new(ErrorCode::NotLoggedIn, message.clone())),
    (Some(SubjectReadiness { requirement: None, readiness, .. }), _) if !matches!(readiness, ReviewReadiness::Unavailable { .. }) => {
      Err(CliError::new(ErrorCode::PatronusError, "Patronus gave no answer for the review"))
    }
    (Some(_), _) => Ok(Some(fact)),
  }
}

/// What `bd safe-push --check` read, for the next step of its subject
struct CheckRead<'a> {
  table: &'a BranchTable,
  /// The robots with the problems of the failed robot of the subject, the Patronus view with the landing needs of a
  /// branch (its gate's), the pull requests
  session: &'a OneShot,
  /// The landing report of the sync
  landed: &'a LandedSection,
  /// The readiness of the subject, read only when the start waits for it
  readiness: Option<&'a Fact<SubjectReadiness>>,
  target: &'a str,
}

/// The next step of the subject of `start` over what the check read (`next_steps`, the rule of the app's rows): Patronus
/// answered (it listed the robots), the problems of the failed robot of the subject as the session holds them (whether
/// it failed for the approvals), the landing needs of a branch are its gate's
fn check_next_step(context: &RepoContext, read: &CheckRead<'_>, start: &Start<'_>, what: &str) -> Option<(domain_types::next_step::NextStep, NextStepSummary)> {
  let table = read.table;
  let rows = grouped_rows(table);
  let key = start.subject().key();
  let view = read.session.view();
  let robots = read.session.facts().robots.view();
  let patronus = PatronusStanding::of(&robots.head, &FactState::Fresh);
  let pull_requests = read.session.facts().pull_requests.view();
  let subject_readiness = |subject: &str| read.readiness.and_then(|fact| fact.value.as_ref()).filter(|_| subject == key).map(|value| &value.readiness);
  let open = open_pull_requests_view(
    read.target,
    |name| table.branch(name).is_some(),
    |root| table.stack(root).is_some(),
    pull_requests,
    subject_readiness,
    &context.prefix,
  );
  let robots_of = Robots::new(read.session.robots());
  let landed = LandedFacts {
    baseline: read.landed,
    view: Some(view),
    robots: &robots_of,
  };
  let landed = landed_states(table, &table.remote, &landed, pull_requests);
  let readiness = |subject: &str| read.readiness.filter(|_| subject == key);
  let sources = StepSources {
    table,
    remote: &table.remote,
    rows: &rows,
    view,
    robots,
    patronus: &patronus,
    landing_needs: &FactState::Fresh,
    pull_requests,
    open_pull_requests: &open,
    landed: &landed,
    readiness: &readiness,
    target: read.target,
    view_stacks: None,
  };
  let mut steps = next_steps(&sources);
  steps.remove(&key).map(|view| {
    let summary = NextStepSummary::new(&view.step, what, read.session.robots());
    (view.step, summary)
  })
}

/// The open pull request `number` of the subject of `start`: the branch's, or the whole stack's (its stack branch's)
fn pull_request_of<'p>(pull_requests: &'p PullRequestsView, start: &Start<'_>, number: u32) -> Option<&'p PullRequest> {
  match start {
    Start::Branch(name) => pull_requests.pull_requests.get(name),
    Start::WholeStack(update) => pull_requests.stack_pull_requests.get(&update.root),
  }
  .filter(|pull_request| pull_request.number == number)
}

/// What `bd` says of a decision of `domain_policy::land::decide`: the facts the words of its refusals read
struct Told<'a> {
  context: &'a RepoContext,
  args: &'a SafePushArgs,
  synced: &'a SyncOutput,
  session: &'a OneShot,
  landed: &'a HashMap<String, LandedBranch>,
  start: &'a Start<'a>,
  landing: &'a Landing,
  rerun: &'a str,
  dangling: &'a [(String, DanglingReferenceSummary)],
  target: &'a str,
  github: Option<&'a GitHubRepository>,
  target_check: Option<&'a TargetCheckSummary>,
  readiness: Option<&'a SubjectReadiness>,
  rules: Option<&'a Fact<BranchRules>>,
  start_pull_request: Option<&'a PullRequest>,
  /// The start pushes first
  pushes: bool,
}

impl Told<'_> {
  /// The refusals of the plan `decision`: each blocker in its order, the refusals of the landing gate one per kind, then
  /// each confirmation `allow` doesn't accept (`domain_policy::land::unaccepted`), naming the flag that accepts it
  fn refusals(&self, decision: &Plan, allow: LandingAllowances) -> Vec<Refusal> {
    let missing: Vec<LandingRefusal> = decision
      .blockers
      .iter()
      .filter_map(|reason| match reason {
        Reason::LandingRefused { refusal } if matches!(**refusal, LandingRefusal::MissingDeclaration { .. }) => Some((**refusal).clone()),
        _ => None,
      })
      .collect();
    let (mut dangling_told, mut missing_told) = (false, false);
    let mut refusals = Vec::new();
    for reason in &decision.blockers {
      refusals.push(match reason {
        Reason::LandingRefused { refusal } if matches!(**refusal, LandingRefusal::DanglingReference { .. }) => {
          if std::mem::replace(&mut dangling_told, true) {
            continue;
          }
          self.dangling_refusal()
        }
        Reason::LandingRefused { .. } if !std::mem::replace(&mut missing_told, true) => self.missing_refusal(&missing),
        Reason::LandingRefused { .. } => continue,
        reason => self.refusal(reason),
      });
    }
    refusals.extend(unaccepted(&decision.confirmations, &allow).into_iter().map(|confirmation| self.unaccepted(confirmation)));
    refusals
  }

  fn subject(&self) -> Subject {
    self.start.subject()
  }

  const fn stack(&self) -> Option<&StackBranchUpdate> {
    match self.start {
      Start::WholeStack(update) => Some(update),
      Start::Branch(_) => None,
    }
  }

  /// A blocker of the start in the words of `bd`, with the commands that act on it
  fn refusal(&self, reason: &Reason) -> Refusal {
    let context = self.context;
    let (name, what) = (self.landing.name.as_str(), self.landing.what.as_str());
    match reason {
      Reason::Landed { .. } | Reason::StackLanded { .. } => {
        let landed_whole = |branch: &str| self.landed.get(branch).filter(|landed| landed.is_whole());
        let words = match self.stack() {
          None => landed_whole(name).map(|landed| landed.reason(name)),
          Some(_) => self
            .landing
            .built
            .iter()
            .map(|member| landed_whole(member))
            .collect::<Option<Vec<&LandedBranch>>>()
            .map(|landed| every_branch_landed(what, &landed)),
        }
        .unwrap_or_else(|| start_refusal_text(reason, what, self.session.robots()));
        Refusal::new(
          ErrorCode::Landed,
          format!("Nothing was pushed or started: {words}: nothing of it is left to land; {LANDED_NEXT}"),
        )
        .with_next_step(format!("{} update", context.bd), true)
      }
      // the fetched baseline has it, or GitHub merged its pull request with its commits (D13): a robot lands nothing
      Reason::LandedIn { .. } | Reason::PullRequestMerged { .. } => Refusal::new(
        ErrorCode::Landed,
        format!(
          "Nothing was pushed or started: {}: nothing of it is left to land; {LANDED_NEXT}",
          start_refusal_text(reason, what, self.session.robots())
        ),
      )
      .with_next_step(format!("{} update", context.bd), true),
      Reason::StackNotBuilt { state, .. } => match (self.stack(), state) {
        // the branch of the commit that conflicts builds with what it is built on: a piece to land
        (Some(update), StackBranchState::Conflict) => Refusal::new(ErrorCode::StackConflict, stack_conflict_message(context, self.synced, update)).with_next_step(
          format!("{} safe-push {} --check", context.bd, shell_word(conflict_branch(self.synced, update).unwrap_or(name))),
          false,
        ),
        (Some(update), _) => Refusal::new(ErrorCode::StackNotBuilt, stack_not_built_message(update)).with_next_step(format!("{} sync", context.bd), false),
        (None, _) => Refusal::new(ErrorCode::StackNotBuilt, start_refusal_text(reason, what, self.session.robots())),
      },
      Reason::DropsRemote { branch, behind } => self.drops_refusal(branch, *behind),
      Reason::TargetConflict { .. } => Refusal::new(ErrorCode::TargetConflict, self.target_check.and_then(|summary| summary.message.clone()).unwrap_or_default())
        .with_next_step(format!("{} update", context.bd), true),
      Reason::Draft { .. } | Reason::Unprotected { .. } => Refusal::new(ErrorCode::PatronusRefused, start_refusal_text(reason, what, self.session.robots())),
      Reason::OtherBase { number, base, target } => {
        let message = format!(
          "Nothing was pushed or started: pull request #{number} of {what} goes into {base}, and Patronus merges a pull request into its base: its approvals count for no Safe Push into {target}. Retarget it to {target} on GitHub (`gh pr edit {number} --base {target}`) after the user approves, then run it again; --without-approvals starts through the API without them, only when the user says so"
        );
        let retarget = match self.github {
          Some(repository) => format!("gh pr edit {number} --repo {} --base {}", repository.full_name(), shell_word(target)),
          None => format!("gh pr edit {number} --base {}", shell_word(target)),
        };
        // the retarget, the user's to approve; --without-approvals is a suppression, only the message names it
        Refusal::new(ErrorCode::PullRequestOtherBase, message).with_next_step(retarget, true)
      }
      Reason::ApprovalsMissing { .. } => self.approvals_refusal(reason),
      Reason::DismissesApprovals { .. } => {
        let (fix, steps) = self.push_then_review();
        let message = self.dismissed_words(&fix);
        steps
          .iter()
          .fold(Refusal::new(ErrorCode::PushDismissesApprovals, message), |refusal, step| refusal.with_next_step(step, true))
      }
      Reason::CantPush { .. } => Refusal::new(ErrorCode::PushRefused, start_refusal_text(reason, what, self.session.robots())),
      Reason::Parked { .. } | Reason::StackParked { .. } => Refusal::new(ErrorCode::Parked, start_refusal_text(reason, what, self.session.robots())),
      Reason::LandingRefused { refusal } => Refusal::new(ErrorCode::MissingDeclaration, refusal.reason()),
      Reason::NoStackBranch { .. } => Refusal::new(ErrorCode::StackNotBuilt, start_refusal_text(reason, what, self.session.robots())),
      Reason::Blocked { .. } | Reason::LastCommit { .. } | Reason::Descendant { .. } | Reason::Ancestor { .. } | Reason::Held { .. } | Reason::StackHeld { .. } => {
        let robot = robot_command(context, &refusing_subject(reason, &self.subject()));
        Refusal::new(ErrorCode::RobotRunning, start_refusal_text(reason, what, self.session.robots())).with_next_step(robot, false)
      }
    }
  }

  /// The push of `branch` drops `behind` commits of its remote branch the baseline lacks: the push that replaces them,
  /// or the start that pushes them away (`--force`), each the user's to approve
  fn drops_refusal(&self, branch: &str, behind: u32) -> Refusal {
    let context = self.context;
    let (name, what) = (self.landing.name.as_str(), self.landing.what.as_str());
    let forced = format!("{} --force", start_again(context, self.args, name, self.stack().is_some()));
    if let Some(update) = self.stack() {
      let message = format!(
        "Nothing was pushed or started: the remote branch {} has {behind} commits {what} doesn't; `bd push {name} --with-children --force` replaces them, or `--force` pushes them away with the start, after the user approves",
        update.branch_name
      );
      Refusal::new(ErrorCode::PushRefused, message)
        .with_next_step(stack_force_push(context, name), true)
        .with_next_step(forced, true)
    } else {
      let push = self
        .synced
        .branches
        .iter()
        .find(|synced| synced.name == branch)
        .and_then(|synced| synced.push.clone())
        .unwrap_or_else(|| format!("{} push {} --force", context.bd, shell_word(branch)));
      let message = format!(
        "Nothing was pushed or started: {branch}: the remote branch has {behind} commits the branch doesn't, and the baseline neither (`dropped_commits`); `{push}` replaces them, or `--force` pushes them away with the start, after the user approves"
      );
      Refusal::new(ErrorCode::PushRefused, message).with_next_step(push, true).with_next_step(forced, true)
    }
  }

  /// The proven dangling references of what lands, in one refusal: the branch that removes the declaration names them,
  /// the fixes a trial build proved are its commands, each a rewrite
  fn dangling_refusal(&self) -> Refusal {
    // the whole stack is built on nothing: the branch that removes the declaration names it
    let of = match self.stack() {
      Some(_) => self.dangling.first().map_or(self.landing.name.as_str(), |(owner, _)| owner.as_str()),
      None => self.landing.name.as_str(),
    };
    let fixes = self.dangling.iter().filter_map(|(_, reference)| reference.fix.as_ref()?.command.clone());
    fixes.fold(Refusal::new(ErrorCode::DanglingReference, dangling_reference_message(of, self.dangling)), |refusal, fix| {
      refusal.with_next_step(fix, true)
    })
  }

  /// The missing declarations of what lands, in one refusal: the first with its fix, the restacks that land the branches
  /// that declare them
  fn missing_refusal(&self, missing: &[LandingRefusal]) -> Refusal {
    let restacks = missing.iter().filter_map(|refusal| match refusal {
      LandingRefusal::MissingDeclaration {
        fix: LandingFix::Restack { branch, onto },
        ..
      } => Some(format!("{} restack {branch} {onto}", self.context.bd)),
      _ => None,
    });
    restacks.fold(
      Refusal::new(ErrorCode::MissingDeclaration, missing_declaration_message(self.context, missing)),
      |refusal, restack| refusal.with_next_step(restack, true),
    )
  }

  /// The code owners haven't approved: where the approvals count (`domain_policy::review::review_state`), and without a
  /// pull request the one `--create-pull-request` opens, a push the user approves
  fn approvals_refusal(&self, reason: &Reason) -> Refusal {
    let what = self.landing.what.as_str();
    let Some(readiness) = self.readiness else {
      return Refusal::new(ErrorCode::ApprovalsMissing, start_refusal_text(reason, what, self.session.robots()));
    };
    let state = review_state(Some(&readiness.readiness), readiness.requirement.as_ref(), self.github.is_some(), self.start_pull_request);
    let ReviewState::Missing { groups, counts_in } = state else {
      return Refusal::new(ErrorCode::ApprovalsMissing, start_refusal_text(reason, what, self.session.robots()));
    };
    let refusal = Refusal::new(
      ErrorCode::ApprovalsMissing,
      approvals_missing_message(what, self.rerun, &readiness.readiness, groups, counts_in),
    );
    if self.github.is_some() && self.start_pull_request.is_none() {
      refusal.with_next_step(
        format!("{} --create-pull-request", safe_push_again(self.context, &self.landing.name, self.stack().is_some())),
        true,
      )
    } else {
      refusal
    }
  }

  /// What to do when the push dismisses the approvals: push first, then ask for the review again, with the commands
  fn push_then_review(&self) -> (String, Vec<String>) {
    let (bd, name) = (&self.context.bd, self.landing.name.as_str());
    let number = self.start_pull_request.map_or(0, |pull_request| pull_request.number);
    match self.stack() {
      Some(_) => (
        format!("push first (`bd push {name} --with-children`), then ask the code owners for the review of #{number} again"),
        vec![format!("{bd} push {} --with-children", shell_word(name))],
      ),
      None => (
        format!("push first (`bd push {name}`), then ask for the review again (`bd request-review {name}`)"),
        vec![format!("{bd} push {}", shell_word(name)), format!("{bd} request-review {}", shell_word(name))],
      ),
    }
  }

  /// "Nothing was pushed or started: the push dismisses the approvals of pull request #7 that Patronus counts …"
  fn dismissed_words(&self, fix: &str) -> String {
    let number = self.start_pull_request.map_or(0, |pull_request| pull_request.number);
    let target = self.target;
    format!(
      "Nothing was pushed or started: the push dismisses the approvals of pull request #{number} that Patronus counts (the rules of {target} dismiss stale reviews on push), and the robot would fail without them; {fix}"
    )
  }

  /// A confirmation `bd` accepts only by its flag, refused without it
  fn unaccepted(&self, confirmation: &Confirmation) -> Refusal {
    let context = self.context;
    let (name, what, target) = (self.landing.name.as_str(), self.landing.what.as_str(), self.target);
    match confirmation {
      Confirmation::CancelsRobot { robot_id } => {
        let robot = self.session.robot(robot_id);
        let why = if self.pushes { "the push fails it at the end" } else { "the start would run beside it" };
        let message = format!(
          "A {} of an older commit of {what} is running: {why}. Once the user approved, --cancel-running cancels it first ({})",
          robot.map_or("Safe Push", |robot| push_mode_name(robot.push_mode)),
          robot.map_or("", |robot| robot.url.as_str())
        );
        Refusal::new(ErrorCode::RobotRunning, message)
          .with_next_step(robot_command(context, &self.subject()), false)
          .with_next_step(format!("{} --cancel-running", start_again(context, self.args, name, self.stack().is_some())), true)
      }
      Confirmation::ReviewUnknown { message } => Refusal::new(
        ErrorCode::ReviewUnknown,
        format!(
          "Nothing was pushed or started: Patronus couldn't answer the code-owner review of {what} ({message}); show it to the user, --allow-unknown-review starts anyway only when the user says so, and the robot checks the approvals when it starts"
        ),
      ),
      Confirmation::RulesUnknown { number } => Refusal::new(
        ErrorCode::RulesUnknown,
        format!(
          "Nothing was pushed or started: {}; show it to the user, --allow-unknown-rules starts anyway only when the user says so",
          self.rules_unknown_words(*number)
        ),
      ),
      other => Refusal::new(ErrorCode::Internal, format!("{what} asks the user first: {other:?}; {target}")),
    }
  }

  /// "Cannot check whether the push keeps the approvals of pull request #7: GitHub didn't answer the rules of master (…)"
  fn rules_unknown_words(&self, number: u32) -> String {
    let why = match self.rules.map(|rules| &rules.state) {
      Some(FactState::Unavailable { reason }) => format!(" ({})", reason.message),
      _ => String::new(),
    };
    format!(
      "Cannot check whether the push keeps the approvals of pull request #{number}: GitHub didn't answer the rules of {}{why}",
      self.target
    )
  }

  /// What the rules of the target do to the approvals of the pull request the robot starts from, once they were read
  /// (`domain_policy::pull_request::dismisses_approvals`): told in `rules`, whatever the decision made of them
  fn rules_summary(&self) -> Option<RulesSummary> {
    let rules = self.rules?;
    let pull_request = self.start_pull_request?;
    let (number, target) = (pull_request.number, self.target);
    let counted = self.readiness.is_some_and(|readiness| matches!(readiness.readiness, ReviewReadiness::Approved { .. }));
    Some(match dismisses_approvals(Some(rules), pull_request) {
      RulesAnswer::Dismiss if counted => RulesSummary {
        dismiss_stale_reviews_on_push: Some(true),
        message: self.dismissed_words(&self.push_then_review().0),
      },
      RulesAnswer::Dismiss => RulesSummary {
        dismiss_stale_reviews_on_push: Some(true),
        message: format!("The push dismisses the approvals of pull request #{number} (the rules of {target} dismiss stale reviews on push); Patronus counts none of them"),
      },
      RulesAnswer::Keep => RulesSummary {
        dismiss_stale_reviews_on_push: Some(false),
        message: format!("The push keeps the approvals of pull request #{number}: the rules of {target} don't dismiss stale reviews"),
      },
      RulesAnswer::Unknown { .. } | RulesAnswer::Loading => RulesSummary {
        dismiss_stale_reviews_on_push: None,
        message: self.rules_unknown_words(number),
      },
    })
  }
}

/// What the target conflict check found, in words: the conflicting commit by the id of the checked out branch, its
/// files, the commits of the target that change them, and the way out
fn target_check_summary(context: &RepoContext, synced: &SyncOutput, what: &str, rerun: &str, check: &TargetCheck) -> TargetCheckSummary {
  let baseline = &check.baseline;
  let fetch_note = check
    .fetch_error
    .as_ref()
    .map(|error| format!("The fetch of {baseline} failed, so the check used the last fetched {baseline}: {error}"));
  let (result, target_tip, message) = match &check.outcome {
    TargetOutcome::Clean => ("clean", Some(synced.report.baseline_commit.as_str()), fetch_note),
    TargetOutcome::Conflict(conflict) => {
      let message = target_conflict_message(context, synced, what, rerun, conflict);
      let message = match fetch_note {
        Some(note) => format!("{message}. {note}"),
        None => message,
      };
      ("conflict", Some(conflict.target_tip.as_str()), Some(message))
    }
    TargetOutcome::NotRun { error } => {
      let message = format!("The check whether Patronus can rebase {what} onto {baseline} did not run: {error}. It holds nothing back");
      let message = match fetch_note {
        Some(note) => format!("{message}. {note}"),
        None => message,
      };
      ("not_run", None, Some(message))
    }
  };
  TargetCheckSummary {
    result: result.to_string(),
    target_tip: target_tip.map(|tip| context.short_ids(std::iter::once(tip)).get(tip).to_string()),
    fetched: check.fetch_error.is_none(),
    message,
  }
}

/// "Nothing was pushed or started: Patronus can't rebase auth onto origin/master: …": the commit and its files, the
/// commits of the target that change them (hash, author, subject), and the way out
fn target_conflict_message(context: &RepoContext, synced: &SyncOutput, what: &str, rerun: &str, conflict: &TargetConflict) -> String {
  let baseline = &synced.baseline;
  let commit = original_commit(synced, conflict);
  let short = context.short_ids(std::iter::once(commit).chain(conflict.target_commits.iter().map(|target| target.id.as_str())));
  let more = usize::try_from(conflict.file_count).unwrap_or(usize::MAX).saturating_sub(conflict.files.len());
  let files = if more > 0 {
    format!("{} and {more} more files", conflict.files.join(", "))
  } else {
    join_names(&conflict.files)
  };
  let commits: Vec<String> = conflict
    .target_commits
    .iter()
    .map(|target| format!("{} ({}: {})", short.get(&target.id), target.author, target.subject))
    .collect();
  let changed_by = if commits.is_empty() { String::new() } else { format!(": {}", commits.join(", ")) };
  format!(
    "Nothing was pushed or started: Patronus can't rebase {what} onto {baseline}: {} ({}) conflicts in {files}, which {baseline} changed after the base of {what}{changed_by}. `bd update` (the app's Update Project) brings the checked out branch onto {baseline} and resolves the conflict locally; then run `{rerun}` again",
    short.get(commit),
    conflict.commit.subject
  )
}

/// The commit of the checked out branch that the conflicting copy of the virtual branch copies, by its author time and
/// its subject without the prefix; the copy itself when none matches
fn original_commit<'a>(synced: &'a SyncOutput, conflict: &'a TargetConflict) -> &'a str {
  let copy = &conflict.commit;
  synced
    .report
    .branches
    .iter()
    .flat_map(|branch| &branch.commits)
    .find(|commit| commit.author_timestamp == copy.author_time && commit.stripped_subject == copy.subject)
    .map_or(copy.id.as_str(), |commit| commit.id.as_str())
}

/// Pushes the stack branch alone, leased on the remote head its remote status compared with, as `bd push` pushes a
/// branch (`operations::push::send`): its members stay as they are on the remote. `lock` is released before the network push, which
/// sends the tip the sync left
fn push_stack_branch(context: &RepoContext, synced: &SyncOutput, root: &str, stack: &StackPush, lock: &mut Option<RepositoryLock>) -> Result<PushedBranch, CliError> {
  let tip = stack.tip.clone().ok_or_else(|| {
    CliError::new(
      ErrorCode::StackNotBuilt,
      format!("Nothing was pushed or started: {} has no tip: `bd sync` builds it, then start again", stack.full_name),
    )
    .with_next_step(format!("{} sync", context.bd), false)
  })?;
  let push = PushRef {
    full_name: stack.full_name.clone(),
    tip,
    lease: stack.lease.clone(),
    source: if stack.lease.is_some() { LeaseSource::Expected } else { LeaseSource::Absent },
  };
  let target = PushTarget {
    repository_path: &context.repository,
    remote: STACK_REMOTE,
    baseline: &synced.baseline,
    my_email: None,
    size_limit: push_size_limit(context)?,
  };
  let leased = vec![LeasedBranch {
    name: root.to_string(),
    push,
    parent_ref: None,
  }];
  let prepared = PreparedPush::leased(&target, leased, vec![true], PushOptions { atomic: true, dry_run: false });
  drop(lock.take());
  // a remote stack branch that moved since the sync without commits the stack branch lacks: nobody's work is lost, so
  // the push goes again over what the remote has now, once, as `bd push --with-children` and the app's start do (D4); a
  // remote with someone's commits, or one that moves again, is told
  let sent = operations::push::send(&context.git, prepared, OverMoved::Once, &mut |_| {}).map_err(|e| CliError::new(ErrorCode::NetworkError, format!("{e:#}")))?;
  let over = sent.moved_to(0).cloned();
  let statuses = match sent.answer {
    LeasedPush::Pushed(statuses) => statuses,
    LeasedPush::RemoteMoved { remote_statuses, .. } => {
      let short = context.short_ids(remote_statuses.iter().flat_map(RemoteSummary::ids));
      let mut error = CliError::new(
        ErrorCode::LeaseRejected,
        format!(
          "The remote rejected {}: it is not where the lease expects it. Nothing was pushed or started; bd fetched it, and `branches` says what the remote has now (`dropped_commits`). Show them to the user; `bd push {root} --with-children --force` replaces them, after the user approves",
          stack.full_name
        ),
      )
      .with_next_step(stack_force_push(context, root), true);
      error.branches = remote_statuses
        .iter()
        .map(|status| ErrorBranch {
          name: stack.full_name.clone(),
          remote: RemoteSummary::new(status, 0, &short),
        })
        .collect();
      return Err(error);
    }
    LeasedPush::Refused { summary } => {
      return Err(CliError::new(
        ErrorCode::PushRefused,
        format!("Nothing was pushed or started: the remote refused the push ({summary}). {REMOTE_REFUSED_NEXT}"),
      ));
    }
    LeasedPush::WouldReject => {
      return Err(CliError::new(
        ErrorCode::LeaseRejected,
        format!("The remote would reject {}: {WOULD_REJECT_NEXT}", stack.full_name),
      ));
    }
    LeasedPush::TooLarge(too_large) => return Err(CliError::from(&too_large)),
  };
  let push = &sent.prepared.branches()[0].push;
  let short = context.short_ids(std::iter::once(push.tip.as_str()).chain(push.lease.as_deref()));
  let replaced_landed = match &over {
    Some(moved) => landed_commits(moved, &context.short_ids(RemoteSummary::ids(moved))),
    None => stack.replaced_landed.clone(),
  };
  Ok(PushedBranch {
    pushed_over: over.is_some(),
    ..pushed_branch(root, push, &statuses, stack.draft.clone(), &short, replaced_landed)
  })
}

/// The service of `bd`: the token of the Safe Push CLI (`./safePush.cmd`), else `SPACE_TOKEN`; `not_logged_in` without
/// either, before anything else runs
/// A failed Patronus request: its answer
pub(crate) fn patronus_error(message: String) -> CliError {
  if message.contains("safePush.cmd") {
    return CliError::new(ErrorCode::NotLoggedIn, message);
  }
  CliError::new(ErrorCode::PatronusError, message)
}

/// The YouTrack of `bd`: `YOUTRACK_BASE_URL` and `YOUTRACK_TOKEN`, else the app's sign-in, only read: a renewal is the
/// app's (`YouTrackService::without_renewal`)
pub(crate) fn youtrack_service(environment: &Environment) -> YouTrackService {
  YouTrackService::without_renewal(
    YouTrackClient::new(&cli_user_agent()),
    environment.youtrack_environment.clone(),
    environment.youtrack_token_file.clone(),
  )
}

/// A failed GitHub write of `sync_core`
fn github_write_error(error: GitHubWriteError) -> CliError {
  match error {
    GitHubWriteError::GitHub(failure) => github_error(&failure),
    GitHubWriteError::Refused(message) => CliError::new(ErrorCode::PatronusRefused, message),
  }
}

/// A failed GitHub request: `not_logged_in` without a token GitHub accepts
pub(crate) fn github_error(failure: &GitHubFailure) -> CliError {
  let code = match failure {
    GitHubFailure::SignedOut
    | GitHubFailure::Failed {
      error: GitHubError::Unauthorized, ..
    } => ErrorCode::NotLoggedIn,
    GitHubFailure::Failed { .. } => ErrorCode::NetworkError,
  };
  CliError::new(code, failure_message(failure))
}

/// The GitHub repository of the branches is not known: a pull request can't be opened, its approvals can't count
pub(crate) const UNKNOWN_GITHUB_REPOSITORY: &str =
  "The GitHub repository of the branches is unknown: the Space settings name no GitHub mirror; run `git config branchdeck.githubRepository <owner>/<name>`";

/// "The code owners haven't approved auth: …", with the missing `groups`, the approvals Patronus doesn't count, and where
/// the approvals count (`counts_in`, `domain_policy::review::review_state`): the GitHub pull request of the branch, or a
/// Space merge request found open. `name` says what lands ("auth", "the whole auth stack"), `rerun` is the command that
/// starts it ("bd safe-push auth")
fn approvals_missing_message(name: &str, rerun: &str, readiness: &ReviewReadiness, groups: &[MissingReviewGroup], counts_in: CountsIn) -> String {
  let (not_counted, reason) = match readiness {
    ReviewReadiness::Missing { not_counted, reason, .. } => (not_counted.as_slice(), Some(*reason)),
    _ => (&[][..], None),
  };
  let named = groups.iter().map(format_missing_group).collect::<Vec<_>>().join(", ");
  let needed = if named.is_empty() { String::new() } else { format!(" Needs review by {named}.") };
  let not_counted = if not_counted.is_empty() {
    String::new()
  } else {
    let them = if not_counted.len() == 1 { "it" } else { "them" };
    format!(
      " {} approved, but no eligible code owner of these files: Patronus doesn't count {them}.",
      join_names(&not_counted.iter().map(person_name).collect::<Vec<_>>())
    )
  };
  let no_eligible = if reason == Some(MissingApprovalsReason::NoEligibleReviewer) {
    " No member of a group can approve it (Patronus names none eligible): ask the group to add a reviewer."
  } else {
    ""
  };
  let counts = match counts_in {
    CountsIn::PullRequest { number, url } => format!("Patronus counts code-owner approvals in pull request #{number} ({url}): once they approved it there, run it again"),
    CountsIn::NewPullRequest => format!(
      "Patronus counts code-owner approvals in the GitHub pull request of the branch: `{rerun} --create-pull-request` pushes the branch and opens it, and the Safe Push starts from it once they approved"
    ),
    CountsIn::MergeRequest(merge_request) => format!("Waiting for approval in {} ({})", merge_request.review_key, merge_request.url),
    CountsIn::UnknownRepository => {
      format!("Patronus counts code-owner approvals in the GitHub pull request of the branch. {UNKNOWN_GITHUB_REPOSITORY}, then `{rerun} --create-pull-request`")
    }
  };
  format!(
    "The code owners haven't approved {name}: a Safe Push fails without these approvals. Nothing was started.{needed}{not_counted}{no_eligible} {counts}. `review` lists the groups and their files; show them to the user, then run again once the code owners approved; --without-approvals starts anyway, only when the user says so"
  )
}

/// The reviewers whose latest review of the pull request approves it and whose approval counts: by the evaluation of
/// Patronus when it has one (the members it counts, a self-approval too), else every approver
fn github_approvers(requirement: &PatronusReviewRequirement, pull_request: &PullRequest) -> Vec<String> {
  if evaluates_pull_request(requirement) {
    let mut logins: Vec<String> = requirement
      .groups
      .iter()
      .flat_map(|group| &group.reviewers)
      .filter(|member| member.approval_state.is_some_and(ReviewerApprovalState::counts))
      .filter_map(|member| member.github_login.clone())
      .collect();
    logins.sort_unstable_by_key(|login| login.to_lowercase());
    logins.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    return logins;
  }
  pull_request
    .reviewers
    .iter()
    .filter(|reviewer| reviewer.state == ReviewerState::Approved)
    .map(|reviewer| reviewer.login.clone())
    .collect()
}

/// Opens the pull request of the pushed branch, as the `gh pr create` that `bd push` prints: into the target, a stacked
/// branch's with the commits of the branches it is built on, with the text of its draft (`pull_request_text`: the
/// `bd describe` text, else the text of its commits), assigned to the user and with the `--reviewer` logins asked. A
/// pull request that already exists is found instead and left as it is.
fn create_pull_request(
  context: &RepoContext,
  args: &SafePushArgs,
  github: &GitHubService,
  runtime: &Runtime,
  plan: &Prepared,
  draft: &PullRequestDraft,
) -> Result<PullRequestStart, CliError> {
  let Some(repository) = &plan.github else {
    return Err(CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY));
  };
  let (title, body) = (draft.title.clone(), draft.body.clone().unwrap_or_default());
  let params = CreatePullRequestParams {
    repository_path: context.repository.clone(),
    branch_name: plan.name.clone(),
    head: draft.head.clone(),
    base: draft.base.clone(),
    title,
    body,
    reviewers: args.reviewers.clone(),
    assign_self: !args.no_assign,
    // pushed right before: a mirror copies the branch to GitHub a moment later
    wait_for_mirror: plan.push_needed,
  };
  let created = runtime.block_on(core_create_pull_request(github, repository, &params)).map_err(|error| match error {
    GitHubWriteError::GitHub(failure) => github_error(&failure),
    GitHubWriteError::Refused(message) => CliError::new(ErrorCode::PushRefused, message),
  })?;
  let mut warnings = created.warnings;
  if !created.created && (!args.reviewers.is_empty() || !args.no_assign) {
    warnings.push(format!(
      "pull request #{} already existed: it was left as it is, without the assignee and the reviewers",
      created.pull_request.number
    ));
  }
  Ok(PullRequestStart {
    number: created.pull_request.number,
    url: created.pull_request.url,
    created: created.created,
    comment_url: None,
    reply: None,
    reviewers: if created.created {
      created.pull_request.reviewers.into_iter().map(|reviewer| reviewer.login).collect()
    } else {
      Vec::new()
    },
    assignees: created.assignees,
    warnings,
  })
}

/// "Air Plugin Release Team (requested: Ilya Korennoy)", else "(suggested: Alice)", as the Safe Push popover names a group
fn format_missing_group(group: &MissingReviewGroup) -> String {
  let (label, people) = if group.requested.is_empty() {
    ("suggested", &group.suggested)
  } else {
    ("requested", &group.requested)
  };
  let needed = group.needed.filter(|needed| *needed > 1).map(|needed| format!("{needed} approvals"));
  let names = (!people.is_empty()).then(|| format!("{label}: {}", people.iter().map(person_name).collect::<Vec<_>>().join(", ")));
  let details: Vec<String> = needed.into_iter().chain(names).collect();
  if details.is_empty() {
    group.name.clone()
  } else {
    format!("{} ({})", group.name, details.join("; "))
  }
}

/// The full name Patronus knows, else the login
fn person_name(person: &Person) -> String {
  person.full_name.clone().or_else(|| person.login.clone()).unwrap_or_default()
}

/// Posts `/patronus merge --keep-source-branch` (or `dry-run`) on the pull request, waits up to a minute for the answer
/// of Patronus, then finds the robot it started
fn start_from_pull_request(
  context: &RepoContext,
  github: &GitHubService,
  runtime: &Runtime,
  plan: &Prepared,
  pull_request: &PullRequest,
) -> Result<(PullRequestStart, Option<PatronusRobot>), CliError> {
  let Some(repository) = &plan.github else {
    return Err(CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY));
  };
  let posted_at = unix_now();
  let comment = runtime
    .block_on(post_patronus_command(
      github,
      &context.repository,
      repository,
      pull_request.number,
      plan.mode,
      PatronusCommandSubject::OwnBranch,
    ))
    .map_err(github_write_error)?;
  let mut start = PullRequestStart {
    number: pull_request.number,
    url: pull_request.url.clone(),
    created: false,
    comment_url: Some(comment.url),
    reply: Some("pending".to_string()),
    reviewers: Vec::new(),
    assignees: Vec::new(),
    warnings: Vec::new(),
  };
  let deadline = Instant::now() + COMMENT_REPLY_TIMEOUT;
  loop {
    match runtime
      .block_on(read_patronus_command_reply(github, &context.repository, repository, pull_request.number, &comment.id))
      .map_err(github_write_error)?
    {
      PatronusCommandReply::Accepted => break,
      PatronusCommandReply::Rejected { reason } => {
        return Err(CliError::new(
          ErrorCode::PatronusRefused,
          format!(
            "Patronus rejected the comment on pull request #{}: {}",
            pull_request.number,
            reason.unwrap_or_else(|| "no reason given yet; see the pull request".to_string())
          ),
        ));
      }
      PatronusCommandReply::Pending if Instant::now() >= deadline => return Ok((start, None)),
      PatronusCommandReply::Pending => std::thread::sleep(COMMENT_REPLY_INTERVAL),
    }
  }
  start.reply = Some("accepted".to_string());
  let robots = robots_since(context, &*plan.sources, posted_at.saturating_sub(COMMENT_CLOCK_SLACK))?;
  Ok((start, comment_robot(robots, pull_request.number, plan.mode, &plan.known_robots)))
}

/// The robot a `/patronus` comment on pull request `number` started in `mode`, as the app matches it
/// (`findCommentRobot`): of that pull request, a dry run for a dry run and a Safe Push for a Safe Push, and none of
/// `known` (the robots known before the comment: a canceled Dry Run or the replaced Safe Push of the same pull request a
/// minute before is never the new robot); the newest of them
pub(crate) fn comment_robot(robots: Vec<PatronusRobot>, number: u32, mode: PatronusPushMode, known: &HashSet<String>) -> Option<PatronusRobot> {
  let dry_run = mode == PatronusPushMode::DryRun;
  robots
    .into_iter()
    .filter(|robot| robot.pull_request.as_ref().is_some_and(|link| link.number == number))
    .filter(|robot| (robot.push_mode == PatronusPushMode::DryRun) == dry_run && !known.contains(&robot.id))
    .max_by_key(|robot| robot.start_time)
}

pub(crate) fn mode_name(mode: PatronusPushMode) -> String {
  match mode {
    PatronusPushMode::DryRun => "dry_run",
    PatronusPushMode::RebaseAutosquash => "safe_push",
    PatronusPushMode::Rebase => "safe_push_rebase",
    PatronusPushMode::SquashAll => "safe_push_squash",
    PatronusPushMode::Merge => "safe_merge",
    PatronusPushMode::Unknown => "unknown",
  }
  .to_string()
}

fn profile_summary(profile: &PatronusProfile) -> ProfileSummary {
  ProfileSummary {
    id: profile.id.clone(),
    name: profile.name.clone(),
    description: profile.description.clone(),
  }
}

fn merge_request_summary(merge_request: &PatronusMergeRequest) -> MergeRequestSummary {
  MergeRequestSummary {
    key: merge_request.review_key.clone(),
    url: merge_request.url.clone(),
  }
}

fn review_summary(review: &SubjectReadiness, reviewed_on_github: bool, pull_request: Option<&PullRequest>, skipped_by: Option<ApprovalsSkip>) -> ReviewSummary {
  // a pre-check that failed answered no requirement: nothing of it is known
  let unanswered = PatronusReviewRequirement {
    required: false,
    merge_request: None,
    groups: Vec::new(),
    skipped: false,
  };
  let requirement = review.requirement.as_ref().unwrap_or(&unanswered);
  let missing_groups: &[MissingReviewGroup] = match &review.readiness {
    ReviewReadiness::Missing { groups, .. } => groups,
    _ => &[],
  };
  let (readiness, readiness_reason) = readiness_names(&review.readiness);
  ReviewSummary {
    required: requirement.required,
    skipped: requirement.skipped,
    counts_in: if reviewed_on_github { "pull_request" } else { "space_merge_request" }.to_string(),
    merge_request: requirement.merge_request.as_ref().map(merge_request_summary),
    groups: requirement
      .groups
      .iter()
      .map(|group| ReviewGroupSummary {
        name: group.name.clone(),
        reviewers: group
          .reviewers
          .iter()
          .map(|reviewer| ReviewerSummary {
            username: reviewer.username.clone(),
            full_name: reviewer.full_name.clone(),
            github_login: reviewer.github_login.clone(),
            available: reviewer.available,
            suggested: reviewer.suggested,
            approval_state: reviewer.approval_state.map(approval_state_name),
          })
          .collect(),
        paths: group.paths.clone(),
        branch_wide: group.branch_wide,
        approved: group.approved,
        needed: missing_groups.iter().find(|missing| missing.name == group.name).and_then(|missing| missing.needed),
      })
      .collect(),
    approved_on_github: pull_request.map_or_else(Vec::new, |pull_request| github_approvers(requirement, pull_request)),
    readiness: readiness.to_string(),
    readiness_reason: readiness_reason.map(str::to_string),
    skipped_by: skipped_by.map(|skipped_by| approvals_skip_name(skipped_by).to_string()),
    waiting_for: missing_groups.iter().flat_map(|group| &group.requested).map(person_summary).collect(),
    not_counted: match &review.readiness {
      ReviewReadiness::Missing { not_counted, .. } => not_counted.iter().map(person_summary).collect(),
      _ => Vec::new(),
    },
  }
}

/// The readiness and its reason in the words of the JSON output
pub(crate) const fn readiness_names(readiness: &ReviewReadiness) -> (&'static str, Option<&'static str>) {
  match readiness {
    ReviewReadiness::NotRequired => ("not_required", None),
    ReviewReadiness::Approved { .. } => ("approved", None),
    ReviewReadiness::Missing { reason, .. } => (
      "missing",
      Some(match reason {
        MissingApprovalsReason::ApprovalsMissing => "approvals_missing",
        MissingApprovalsReason::NoPullRequestContext => "no_pull_request_context",
        MissingApprovalsReason::NoEligibleReviewer => "no_eligible_reviewer",
      }),
    ),
    ReviewReadiness::Unknown { reason } => (
      "unknown",
      Some(match reason {
        UnknownReadinessReason::ApprovalsNotEvaluated => "approvals_not_evaluated",
        UnknownReadinessReason::Skipped => "skipped",
      }),
    ),
    ReviewReadiness::Unavailable { .. } => ("unavailable", None),
  }
}

fn approval_state_name(state: ReviewerApprovalState) -> String {
  match state {
    ReviewerApprovalState::Approved => "approved",
    ReviewerApprovalState::AuthorSelfApproved => "author_self_approved",
    ReviewerApprovalState::Reviewing => "reviewing",
    ReviewerApprovalState::NotAdded => "not_added",
    ReviewerApprovalState::NotEligible => "not_eligible",
  }
  .to_string()
}

fn person_summary(person: &Person) -> PersonSummary {
  PersonSummary {
    login: person.login.clone(),
    full_name: person.full_name.clone(),
  }
}

pub(crate) fn robot_summary(context: &RepoContext, robot: &PatronusRobot) -> RobotSummary {
  let short = context.short_ids(robot.source_revision.as_deref());
  RobotSummary {
    id: robot.id.clone(),
    name: robot.name.clone(),
    state: match robot.status {
      PatronusRobotStatus::Running => RobotState::Running,
      PatronusRobotStatus::Canceling => RobotState::Canceling,
      PatronusRobotStatus::Canceled => RobotState::Canceled,
      PatronusRobotStatus::Failure => RobotState::Failure,
      PatronusRobotStatus::Success => RobotState::Success,
      PatronusRobotStatus::Unknown => RobotState::Unknown,
    },
    mode: mode_name(robot.push_mode),
    commit: robot.source_revision.as_deref().map(|revision| short.get(revision).to_string()),
    url: robot.url.clone(),
  }
}

/// The trial build of the stacked branch `name` alone, as a root on the base the sync built on (not the newest target),
/// in the trial session of the dangling references; `parent` is the branch it is built on, for a trial that failed
fn alone_build(context: &RepoContext, session: &CheckSession, name: &str, parent: &str) -> AloneBuild {
  let independence = match session.stack_independence(name) {
    Ok(independence) => independence,
    Err(error) => {
      tracing::debug!(%error, "cannot check whether {name} builds alone");
      return AloneBuild {
        result: "unknown".to_string(),
        parent: parent.to_string(),
        uses: Vec::new(),
        symbols: None,
        conflict: None,
        needs: Vec::new(),
        movable: None,
        message: format!("The trial build of {name} alone failed: {error}"),
      };
    }
  };
  match independence {
    StackIndependence::Builds { parent, uses, symbols } => {
      let short = context.short_ids(uses.iter().flat_map(|stack_use| [stack_use.commit.id.as_str(), stack_use.declared_by.id.as_str()]));
      let message = match uses.first() {
        Some(first) => format!(
          "{name} applies on the baseline alone, but {} uses {}, which {} of {} adds: it needs its stack to compile (found by names, not by a compiler)",
          short.get(&first.commit.id),
          first.name,
          short.get(&first.declared_by.id),
          first.declared_by.branch
        ),
        None if symbols => {
          format!("{name} builds alone on the baseline and uses no name its stack adds: `bd unstack {name}` makes it a root that lands alone (not checked by a compiler)")
        }
        None => format!("{name} builds alone on the baseline: `bd unstack {name}` makes it a root that lands alone (lines only: compile dependencies are not checked)"),
      };
      AloneBuild {
        result: "builds".to_string(),
        parent,
        uses: uses
          .iter()
          .map(|stack_use| AloneUse {
            identifier: stack_use.name.clone(),
            used_by: short.get(&stack_use.commit.id).to_string(),
            declared_by: short.get(&stack_use.declared_by.id).to_string(),
            declared_in: stack_use.declared_by.branch.clone(),
          })
          .collect(),
        symbols: Some(symbols),
        conflict: None,
        needs: Vec::new(),
        movable: None,
        message,
      }
    }
    StackIndependence::Needs {
      parent,
      child_commit,
      files,
      parent_commits,
      movable,
    } => {
      let short = context.short_ids(std::iter::once(child_commit.id.as_str()).chain(parent_commits.iter().map(|commit| commit.id.as_str())));
      let ids: Vec<String> = parent_commits.iter().map(|commit| format!("{} ({})", short.get(&commit.id), commit.branch)).collect();
      let them = if ids.len() == 1 { "it" } else { "them" };
      let message = if ids.is_empty() {
        format!(
          "{name} doesn't apply alone: {} conflicts in {} without the branches it is built on, and no commit of theirs explains it by lines",
          short.get(&child_commit.id),
          join_names(&files)
        )
      } else if movable {
        format!("{name} needs {}: move {them} into the branch, then `bd unstack {name}`", join_names(&ids))
      } else {
        format!("{name} needs {}, and other branches need {them} too: a move would make those need {name}", join_names(&ids))
      };
      AloneBuild {
        result: "needs".to_string(),
        parent,
        uses: Vec::new(),
        symbols: None,
        conflict: Some(CheckConflict {
          commit: short.get(&child_commit.id).to_string(),
          subject: child_commit.subject,
          files,
          missing_commits: Vec::new(),
        }),
        needs: parent_commits
          .into_iter()
          .map(|commit| StackCommitSummary {
            commit: short.get(&commit.id).to_string(),
            subject: commit.subject,
            branch: commit.branch,
          })
          .collect(),
        movable: Some(movable),
        message,
      }
    }
  }
}
