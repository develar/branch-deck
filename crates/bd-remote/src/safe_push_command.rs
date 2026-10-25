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
use crate::patronus_facts::{PatronusFacts, table_of};
use crate::pull_request_retarget::{RetargetedPullRequest, is_virtual_branch_base, refusal_warning, retarget_one, virtual_base_retargets};
use crate::push_command::{
  PullRequestDraft, PushArgs, PushedBranch, github_repository, not_synced_reason, push_locked, push_size_limit, pushed_branch, refuse_an_unapplied_name, stack_pull_request_draft,
  unknown_or_unapplied, unpushed_pull_request_draft,
};
use crate::push_hold::PushHolds;
use crate::robot_list::list_robots;
use bd_core::check_summary::{CheckConflict, CheckWarning, dangling_reference_message, dangling_references_in, session_of_sync};
use bd_core::context::{Environment, RepoContext};
use bd_core::error::{CliError, ErrorBranch, regroup_error};
use bd_core::output::SchemaVersion;
use bd_core::review_summary::{MergeRequestSummary, PersonSummary, ReviewGroupSummary, ReviewSummary, ReviewerSummary};
use bd_core::stack_summary::{WholeStack, WholeStackSummary, whole_stack_of};
use bd_core::sync_command::{LandedRemoteCommitSummary, RemoteSummary, SyncOutput, SyncedBranch, landed_commits, lock, sync_locked};
use domain_types::branch_names::to_final_branch_name;
use domain_types::error::ErrorCode;
use domain_types::landing::{LandingFix, LandingRefusal, StackCommit, Subject};
use domain_types::patronus::{
  MissingApprovalsReason, MissingReviewGroup, PatronusAvailability, PatronusCommandReply, PatronusCommandSubject, PatronusMergeRequest, PatronusProfile, PatronusPushMode,
  PatronusReviewRequirement, PatronusRobot, PatronusRobotStatus, Person, ReviewReadiness, ReviewReadinessBranch, ReviewerApprovalState, UnavailableReadinessReason,
  UnknownReadinessReason,
};
use domain_types::pull_request::{BranchRules, PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult, ReviewerState};
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use github::client::{GITHUB_URL, GitHubClient, GitHubError};
use github::service::{GitHubFailure, GitHubService};
use github::token::TokenChain;
use history_rewrite::repository_lock::RepositoryLock;
use patronus::client::PatronusClient;
use patronus::review_readiness::evaluates_pull_request;
use patronus::service::PatronusService;
use patronus::token::{TokenLookup, find_token};
use remote_facts::branch_rules::read_branch_rules;
use remote_facts::patronus_command::{post_patronus_command, read_patronus_command_reply};
use remote_facts::pull_request_create::{CreatePullRequestParams, create_pull_request as core_create_pull_request};
use remote_facts::pull_requests::{GitHubWriteError, failure_message, list_pull_requests};
use remote_facts::review_readiness::{ReadinessRepository, branch_review_readiness};
use repository_model::branch_table::BranchTable;
use repository_model::start_guard::{stack_conflict_words, stack_not_built_words, start_refusal_text};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_core::github_repository::GitHubRepository;
use sync_core::parked::parked_state;
use sync_core::patronus::{SpaceRepository, target_branch_of};
use sync_core::patronus_start::{allows_push_mode, is_push_into_robot_risky, is_robot_active, join_names, push_mode_name};
use sync_core::push::{LeasedBranch, LeasedPush, PushTarget, push_leased};
use sync_core::push_lease::{LeaseSource, PushOptions, PushRef};
use sync_core::remote_status::has_work_for_remote;
use sync_core::target_conflict::{TargetCheck, TargetConflict, TargetFetch, TargetOutcome, check_target, fetch_target};
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

/// The facts a start is decided on, read under the repository lock
struct Plan {
  /// The branch the robot runs on (for a straight chain with `--whole-stack`, its last branch), or the root of the
  /// whole stack
  name: String,
  /// The branch Patronus starts from, without `refs/heads/`
  full_name: String,
  mode: PatronusPushMode,
  space: SpaceRepository,
  target: String,
  github: Option<GitHubRepository>,
  profiles: Vec<PatronusProfile>,
  profile: Option<PatronusProfile>,
  review: Option<Review>,
  /// The rules of the target for a push into the approved pull request the robot starts from
  rules: Option<RulesSummary>,
  /// Why the approvals don't hold the start back; `None` when a `missing` readiness refuses it
  approvals_skipped_by: Option<ApprovalsSkippedBy>,
  /// The open pull request the robot starts from: into the target, or for a branch into a virtual branch (retargeted
  /// first, `retargets`)
  pull_request: Option<PullRequest>,
  /// The open pull request into another base (a release branch): left as it is, the start goes through the API, and
  /// `--create-pull-request` takes it instead of opening a second one
  other_base_pull_request: Option<PullRequest>,
  /// The open pull requests into a virtual branch of the branch and of the branches its push takes along
  retargets: Vec<RetargetedPullRequest>,
  push_needed: bool,
  /// The remote branches the start pushes, with their remote status
  remotes: Vec<ErrorBranch>,
  /// What the robot lands: the branches it is built on, root first, then the branch; or every branch of the stack
  lands: Vec<String>,
  refusals: Vec<Refusal>,
  /// With `--check`, for a stacked branch: its trial build alone
  alone: Option<AloneBuild>,
  warnings: Vec<CheckWarning>,
  /// The running Safe Push of an older commit that the start replaces: a push would fail it, and without a push the new
  /// robot would run beside it; canceled first with `--cancel-running`, as the app's form cancels it
  risky_robot: Option<PatronusRobot>,
  /// The ids of the robots that can't be the one a `/patronus` comment starts: the user's robots that had finished when
  /// listed before the start, and the robot the start replaces (`risky_robot`)
  known_robots: HashSet<String>,
  /// The whole stack of a stack that forks: its stack branch, the only branch pushed
  stack: Option<StackPush>,
  /// The replay onto the newest target and what it means; `None` with `--create-pull-request`
  target_check: Option<(TargetCheck, TargetCheckSummary)>,
}

/// Why the approvals don't hold a Safe Push back, the `skipped_by` of the `review`
#[derive(Debug, Clone, Copy)]
enum ApprovalsSkippedBy {
  /// A start through the API with a merge profile known to skip the code-owner review (the options of the last robot,
  /// or the list with the new API), as the app's Safe Push without review starts; a `/patronus` comment has no profile
  Profile,
  /// `--without-approvals`
  WithoutApprovals,
}

impl ApprovalsSkippedBy {
  /// What a Safe Push skips the approvals for, if anything: never a dry run (no review) or a pull request it opens
  /// (no start)
  fn of(args: &SafePushArgs, pushes: bool, pull_request: Option<&PullRequest>, profile: Option<&PatronusProfile>) -> Option<Self> {
    if !pushes || args.create_pull_request {
      None
    } else if args.without_approvals {
      Some(Self::WithoutApprovals)
    } else if pull_request.is_none() && profile.is_some_and(|profile| profile.skips_review == Some(true)) {
      Some(Self::Profile)
    } else {
      None
    }
  }

  const fn name(self) -> &'static str {
    match self {
      Self::Profile => "profile",
      Self::WithoutApprovals => "without_approvals",
    }
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
  let service = patronus_service(environment).map_err(|error| refuse_an_unapplied_name(context, &args.branch, UNAPPLIED_SAFE_PUSH).err().unwrap_or(error))?;
  let github = github_service(context, environment);
  let youtrack = youtrack_service(environment);
  let runtime = context.runtime()?;

  // the target moves every minute: its remote branch is fetched for the target conflict check, before the lock, which
  // no fetch runs under; a pull request starts no robot and needs no check
  let target_fetch = (!args.create_pull_request).then(|| fetch_target(&context.git, &context.repository));
  let lock = lock(context)?;
  let synced = sync_locked(context)?;
  // a check writes nothing after its sync: the trial builds and Patronus don't keep the other bd writers waiting
  let mut lock = (!args.check).then_some(lock);
  let whole_stack = whole_stack_arg(args, &synced)?;
  let start = start_of(args, whole_stack.as_ref());
  let plan = plan(context, args, &service, &github, runtime, &synced, mode, &start, target_fetch)?;
  let mut output = start_output(context, args, mode, whole_stack.as_ref(), &plan);
  if args.check {
    return Ok(check_output(output, plan));
  }
  if let Some(refusal) = plan.refusals.first().cloned() {
    let mut error = CliError::new(refusal.code, refusal.message);
    if refusal.code == ErrorCode::PushRefused {
      error.branches = plan.remotes.iter().filter(|branch| branch.remote.behind > 0).cloned().collect();
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
      match retarget_one(context, &github, runtime, repository, retarget) {
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
  if let Some(robot) = &plan.risky_robot {
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
      // the robot of the branch is canceled above; the start lands the ancestors its push takes along
      let pushed = push_locked(
        context,
        &PushArgs {
          branches: vec![plan.name.clone()],
          ..PushArgs::default()
        },
        &synced,
        &PushHolds::default(),
        &HashSet::new(),
        Some(&youtrack),
        &mut lock,
      )?;
      output.pushed = pushed.branches;
      output
        .pushed
        .iter()
        .find(|pushed| pushed.name == plan.name)
        .and_then(|pushed| pushed.pull_request.clone())
        .ok_or_else(|| CliError::new(ErrorCode::GitError, format!("{} was not pushed", plan.name)))?
    }
    (None, false) => unpushed_pull_request_draft(context, &synced, &plan.name, Some(&youtrack))?,
  };
  drop(lock);

  if args.create_pull_request {
    if output.pull_request.is_none() {
      // one open pull request per branch: the one into another base is the branch's, GitHub would open a second one
      output.pull_request = Some(match &plan.other_base_pull_request {
        Some(existing) => other_base_pull_request_start(existing, &plan.target),
        None => create_pull_request(context, args, &github, runtime, &plan, &draft)?,
      });
    }
    return Ok(output);
  }

  if let Some(pull_request) = &plan.pull_request {
    let (start, robot) = start_from_pull_request(context, &github, &service, runtime, &plan, pull_request)?;
    output.pull_request = Some(start);
    output.robot = robot.map(|robot| robot_summary(context, &robot));
    output.started_via = Some("pull_request".to_string());
  } else {
    let merge_request = plan.review.as_ref().and_then(|review| review.requirement.merge_request.clone());
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

/// The whole stack `--whole-stack` lands, from the sync
fn whole_stack_arg<'s>(args: &SafePushArgs, synced: &'s SyncOutput) -> Result<Option<WholeStack<'s>>, CliError> {
  if !args.whole_stack {
    return Ok(None);
  }
  whole_stack_of(&synced.report.branches, &synced.report.stack_branches, &args.branch).map(Some)
}

/// What the start lands, and from which branch: the branch; of the whole stack, a stack that forks from its stack
/// branch, a straight chain from its last branch
fn start_of<'a>(args: &SafePushArgs, whole_stack: Option<&'a WholeStack<'_>>) -> Start<'a> {
  match whole_stack {
    None => Start::Branch(args.branch.clone()),
    Some(WholeStack::Chain { leaf, .. }) => Start::Branch(leaf.clone()),
    Some(WholeStack::Forking(update)) => Start::WholeStack(update),
  }
}

/// The output of a start as `plan` decides it, before anything is pushed or started
fn start_output(context: &RepoContext, args: &SafePushArgs, mode: PatronusPushMode, whole_stack: Option<&WholeStack>, plan: &Plan) -> SafePushOutput {
  SafePushOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    mode: mode_name(mode),
    target: plan.target.clone(),
    lands: plan.lands.clone(),
    whole_stack: whole_stack.map(|whole| whole.summary(&context.prefix)),
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
  }
}

/// The output of `--check`: `output` with what refuses a start, the trial build alone and the retargets a start would do
fn check_output(mut output: SafePushOutput, plan: Plan) -> SafePushOutput {
  output.refusals = plan.refusals;
  output.alone = plan.alone;
  output.retargeted = plan.retargets;
  output
}

/// The remote of the stack branch: Patronus pushes from `origin`, whose remote status the sync computed
const STACK_REMOTE: &str = "origin";

/// What a start lands and the robots in its way, before the profiles, the pull request and the review
struct Landing<'r> {
  /// The branch the robot runs on, or the root of the whole stack
  name: String,
  /// "auth", or "the whole auth stack"
  subject: String,
  /// The branch Patronus starts from, without `refs/heads/`
  full_name: String,
  local_tip: Option<String>,
  /// The robot of the branch, or of the whole stack
  robot: Option<&'r PatronusRobot>,
  /// The robot runs on the commit the start would take (`start_blocked` of the repository view)
  start_blocked: bool,
  /// A push into it fails this robot of an older commit
  is_risky: bool,
  lands: Vec<String>,
  push_needed: bool,
  /// The branches the push takes along, whose pull requests into a virtual branch are retargeted too
  pushed_along: Vec<String>,
  /// The remote branches the push changes, with their remote status
  remotes: Vec<ErrorBranch>,
  refusals: Vec<Refusal>,
  stack: Option<StackPush>,
}

/// The start of a branch: it lands the branches it is built on with it; a robot that lands it or a part of it refuses
/// the start, and so does the Safe Push of the whole stack it is a branch of
fn branch_landing<'r>(context: &RepoContext, synced: &SyncOutput, table: &BranchTable, facts: &'r PatronusFacts, name: &str, pushes: bool) -> Result<Landing<'r>, CliError> {
  let full_name = to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  let ancestors = table.ancestors_of(name);
  let synced_by_name: HashMap<&str, &SyncedBranch> = synced.branches.iter().map(|branch| (branch.name.as_str(), branch)).collect();
  let local_tip = table.tip_of(name).map(str::to_string);
  let robot = facts.branch_robot(name);

  let mut refusals = Vec::new();
  // the robot part of the decision, as the repository view tells it: a robot on the last commit, a robot of a branch
  // built on it or of its whole stack that lands it, a robot that lands an ancestor; a dry run waits for no robot of
  // the stack
  let facts_of = facts.branch(name);
  if let Some(refusal) = facts_of.and_then(|branch| if pushes { branch.start.as_ref() } else { branch.dry_run.as_ref() }) {
    refusals.push(Refusal {
      code: ErrorCode::RobotRunning,
      message: start_refusal_text(refusal, name, &facts.robots),
    });
  }

  let needs_push_of = |name: &str| synced_by_name.get(name).is_none_or(|branch| needs_push(branch.remote.as_ref()));
  let push_needed = std::iter::once(name).chain(ancestors.iter().map(String::as_str)).any(needs_push_of);
  let pushed_along = ancestors.iter().filter(|ancestor| needs_push_of(ancestor)).cloned().collect();
  // the virtual branch of a stacked branch carries the commits of its ancestors: one robot lands them all, and the
  // branches built on it stay
  let lands: Vec<String> = ancestors.iter().cloned().chain(std::iter::once(name.to_string())).collect();
  // what the push replaces and drops of each remote branch: one that would drop commits the baseline lacks refuses the
  // start, as `bd push` refuses it, with the push that drops them
  let remotes: Vec<ErrorBranch> = lands
    .iter()
    .filter_map(|branch| synced_by_name.get(branch.as_str()))
    .filter_map(|branch| {
      let remote = branch.remote.clone().filter(|remote| remote.exists && remote.has_work())?;
      Some(ErrorBranch {
        name: branch.name.clone(),
        remote,
      })
    })
    .collect();
  for branch in remotes.iter().filter(|branch| branch.remote.behind > 0) {
    let push = synced_by_name
      .get(branch.name.as_str())
      .and_then(|synced| synced.push.as_deref())
      .unwrap_or("bd push --force");
    refusals.push(Refusal {
      code: ErrorCode::PushRefused,
      message: format!(
        "Nothing was pushed or started: {}: the remote branch has {} commits the branch doesn't, and the baseline neither (`dropped_commits`); `{push}` replaces them, after the user approves",
        branch.name, branch.remote.behind
      ),
    });
  }
  Ok(Landing {
    name: name.to_string(),
    subject: name.to_string(),
    full_name,
    is_risky: robot.is_some_and(|robot| is_push_into_robot_risky(robot, &Subject::Branch { name: name.to_string() })),
    local_tip,
    robot,
    start_blocked: facts_of.is_some_and(|branch| branch.start_blocked),
    lands,
    push_needed,
    pushed_along,
    remotes,
    refusals,
    stack: None,
  })
}

/// The start of the whole stack, from its stack branch: only a `built` one has the stack's commits as they are; a robot
/// of a branch of it lands a part of it and refuses the start, as the robot of the whole stack refuses theirs
fn stack_landing<'r>(context: &RepoContext, synced: &SyncOutput, facts: &'r PatronusFacts, update: &StackBranchUpdate, pushes: bool) -> Result<Landing<'r>, CliError> {
  let root = update.root.as_str();
  let whole_stack = Subject::WholeStack { root: root.to_string() };
  let subject = whole_stack.words();
  let mut refusals = Vec::new();
  match update.state {
    StackBranchState::Built => {}
    StackBranchState::Conflict => refusals.push(Refusal {
      code: ErrorCode::StackConflict,
      message: stack_conflict_message(context, synced, update),
    }),
    StackBranchState::Blocked | StackBranchState::Stale => refusals.push(Refusal {
      code: ErrorCode::StackNotBuilt,
      message: stack_not_built_message(update),
    }),
  }
  let remote = update.remote.as_ref();
  let has_work = remote.is_none_or(has_work_for_remote);
  let short = context.short_ids(remote.into_iter().flat_map(RemoteSummary::ids));
  let remotes: Vec<ErrorBranch> = remote
    .filter(|remote| remote.remote_exists && has_work)
    .map(|remote| ErrorBranch {
      name: update.branch_name.clone(),
      remote: RemoteSummary::new(remote, usize::try_from(update.commit_count).unwrap_or(usize::MAX), &short),
    })
    .into_iter()
    .collect();
  if let Some(remote) = remote.filter(|remote| remote.commits_behind > 0) {
    refusals.push(Refusal {
      code: ErrorCode::PushRefused,
      message: format!(
        "Nothing was pushed or started: the remote branch {} has {} commits {subject} doesn't; `bd push {root} --with-children --force` replaces them, after the user approves",
        update.branch_name, remote.commits_behind
      ),
    });
  }
  let local_tip = update.tip.clone();
  let robot = facts.stack_robot(root);
  // a robot on the commits of the stack branch, or of a branch of the stack, which lands a part of it
  let facts_of = facts.stack(root);
  if let Some(refusal) = facts_of.and_then(|stack| if pushes { stack.start.as_ref() } else { stack.dry_run.as_ref() }) {
    refusals.push(Refusal {
      code: ErrorCode::RobotRunning,
      message: start_refusal_text(refusal, &subject, &facts.robots),
    });
  }
  let base = target_branch_of(synced.report.upstream.as_ref()).unwrap_or_default();
  Ok(Landing {
    name: root.to_string(),
    subject,
    full_name: update.branch_name.clone(),
    is_risky: robot.is_some_and(|robot| is_push_into_robot_risky(robot, &whole_stack)),
    local_tip,
    robot,
    start_blocked: facts_of.is_some_and(|stack| stack.start_blocked),
    lands: update.members.clone(),
    push_needed: has_work,
    pushed_along: Vec::new(),
    remotes,
    refusals,
    stack: Some(StackPush {
      full_name: update.branch_name.clone(),
      tip: update.tip.clone(),
      lease: remote.and_then(|remote| remote.remote_tip.clone()),
      replaced_landed: remote.map_or_default(|remote| landed_commits(remote, &short)),
      draft: stack_pull_request_draft(context, &synced.report, base, update)?,
    }),
  })
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
    "Nothing was pushed or started: {} of {branch} uses {identifier} ({file}:{line}), which {adds}; git merges it, but the build fails. {fix}{more}; --allow-missing starts anyway",
    named(used_by)
  )
}

/// Everything the start is decided on, before anything is pushed: the repository on Space, the robots of the user,
/// the stack, the profiles, the pull request and the review
#[expect(clippy::too_many_arguments, reason = "the services and the sync of the command")]
fn plan(
  context: &RepoContext,
  args: &SafePushArgs,
  service: &PatronusService,
  github_service: &GitHubService,
  runtime: &Runtime,
  synced: &SyncOutput,
  mode: PatronusPushMode,
  start: &Start<'_>,
  target_fetch: Option<TargetFetch>,
) -> Result<Plan, CliError> {
  let subject = start.subject();
  if let Start::Branch(name) = start {
    let branch = synced
      .branches
      .iter()
      .find(|branch| branch.name == *name)
      .ok_or_else(|| unknown_or_unapplied(context, name, UNAPPLIED_SAFE_PUSH))?;
    if let Some(reason) = not_synced_reason(&synced.report, branch) {
      return Err(CliError::new(
        ErrorCode::PushRefused,
        format!("Nothing was pushed or started: {name} is not synced ({reason})"),
      ));
    }
    to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  }
  // a parked branch stays local, and so does what carries its commits: a branch built on it, the whole stack
  let report = &synced.report;
  let parked = match start {
    Start::Branch(name) => parked_state(&report.branches, &report.grouping.parked.entries, name).map(|parked| parked.reason(name)),
    Start::WholeStack(update) => update
      .members
      .iter()
      .find_map(|member| parked_state(&report.branches, &report.grouping.parked.entries, member).map(|parked| parked.reason(member)))
      .map(|reason| {
        format!(
          "{} holds what is parked: {reason}; land the stack in pieces",
          Subject::WholeStack { root: update.root.clone() }.words()
        )
      }),
  };
  if let Some(reason) = parked {
    return Err(CliError::new(
      ErrorCode::Parked,
      format!("Nothing was pushed or started: {reason}; it stays local until the user unparks it"),
    ));
  }
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

  // the stack, root first, and the robots of the user, each matched to its branch (or the whole stack) as the app does;
  // what the merged ones landed of it, by content
  let user_robots = list_robots(context, service, &space)?;
  let table = table_of(context, synced);
  // a robot that had finished before the comment is never the one it starts; one that ran is the replaced one, or the
  // start was refused
  let mut known_robots: HashSet<String> = user_robots.iter().filter(|robot| !is_robot_active(robot)).map(|robot| robot.id.clone()).collect();
  let facts = PatronusFacts::read(context, &table, user_robots);
  let landed = landed_branches(&facts);
  let pushes = mode != PatronusPushMode::DryRun;
  let landing = match start {
    Start::Branch(name) => branch_landing(context, synced, &table, &facts, name, pushes)?,
    Start::WholeStack(update) => stack_landing(context, synced, &facts, update, pushes)?,
  };
  let Landing {
    name,
    subject: what,
    full_name,
    local_tip,
    robot,
    start_blocked,
    is_risky,
    lands,
    push_needed,
    pushed_along,
    remotes,
    mut refusals,
    stack: stack_push,
  } = landing;
  let name = name.as_str();
  let landed_whole = |branch: &str| landed.get(branch).filter(|landed| landed.is_whole());
  // a merged Safe Push landed all of it: its commits are in the target, a robot lands nothing, before any other reason
  let landed_refusal = match &stack_push {
    None => landed_whole(name).map(|landed| format!("Nothing was pushed or started: {}: nothing of it is left to land", landed.reason(name))),
    Some(_) => lands
      .iter()
      .map(|member| landed_whole(member))
      .collect::<Option<Vec<&LandedBranch>>>()
      .map(|landed| format!("Nothing was pushed or started: {}: nothing of it is left to land", every_branch_landed(&what, &landed))),
  };
  if let Some(message) = landed_refusal {
    refusals.insert(0, Refusal { code: ErrorCode::Landed, message });
  }

  // a Safe Push of an older commit (a robot on the commits as they are refuses the start above): the push fails it at
  // the end; without a push a Safe Push replaces it, as the app's form does, and a dry run or a pull request leaves it
  let starts_safe_push = pushes && !args.create_pull_request;
  let risky_robot = robot.filter(|_| is_risky && !start_blocked).filter(|_| push_needed || starts_safe_push).cloned();
  if let Some(robot) = &risky_robot
    && !args.cancel_running
  {
    let why = if push_needed { "the push fails it at the end" } else { "the start would run beside it" };
    refusals.push(Refusal {
      code: ErrorCode::RobotRunning,
      message: format!(
        "A {} of an older commit of {what} is running: {why}. Pass --cancel-running to cancel it first ({})",
        push_mode_name(robot.push_mode),
        robot.url
      ),
    });
  }

  // what lands, in one trial: a removed declaration that another branch's commit still changes the use of, a name that
  // a branch outside it adds; git merges either, the build fails
  let session = session_of_sync(context, &synced.report)?;
  let built: Vec<&str> = lands.iter().map(String::as_str).collect();
  let landed_commits: HashSet<String> = landed.values().flat_map(|landed| landed.landed.iter().cloned()).collect();
  let gate = check_landing_gate(&session, &subject, &landed_commits).map_err(regroup_error)?;
  let (dangling, warnings) = dangling_references_in(context, &session, &gate.report, &built)?;
  if !dangling.is_empty() && !args.allow_dangling {
    // the whole stack is built on nothing: the branch that removes the declaration names it
    let of = match &stack_push {
      Some(_) => dangling.first().map_or(name, |(owner, _)| owner.as_str()),
      None => name,
    };
    refusals.push(Refusal {
      code: ErrorCode::DanglingReference,
      message: dangling_reference_message(of, &dangling),
    });
  }
  let missing: Vec<LandingRefusal> = gate
    .refusals(&session)
    .into_iter()
    .filter(|refusal| matches!(refusal, LandingRefusal::MissingDeclaration { .. }))
    .collect();
  if !missing.is_empty() && !args.allow_missing {
    refusals.push(Refusal {
      code: ErrorCode::MissingDeclaration,
      message: missing_declaration_message(context, &missing),
    });
  }
  // Patronus rebases what it lands onto the newest target before any check, and fails the robot on a conflict
  let rerun = if stack_push.is_some() {
    format!("bd safe-push {name} --whole-stack")
  } else {
    format!("bd safe-push {name}")
  };
  let target_check = target_fetch.map(|fetch| {
    let fetch_error = fetch.into_error();
    let check = match local_tip.as_deref() {
      Some(tip) => check_target(&context.git, &context.repository, &synced.baseline, &synced.report.baseline_commit, tip, fetch_error),
      None => TargetCheck {
        baseline: synced.baseline.clone(),
        fetch_error,
        outcome: TargetOutcome::NotRun {
          error: format!("{full_name} has no tip"),
        },
      },
    };
    let summary = target_check_summary(context, synced, &what, &rerun, &check);
    if let TargetOutcome::Conflict(_) = &check.outcome {
      refusals.push(Refusal {
        code: ErrorCode::TargetConflict,
        message: summary.message.clone().unwrap_or_default(),
      });
    }
    (check, summary)
  });

  // the second way, for the user to choose: the branch alone, in the same trial session; a start never asks
  let alone = match (&stack_push, lands.len().checked_sub(2).and_then(|parent| lands.get(parent))) {
    (None, Some(parent)) if args.check => Some(alone_build(context, &session, name, parent)),
    _ => None,
  };

  // with the new API the list names the profile of the last robot: nothing more to read for the default
  let last_profile = robot.filter(|_| context.patronus_new_api).and_then(|robot| robot.profile.clone());
  let options = runtime
    .block_on(service.start_options(&space.project_key, &space.name, &target, robot.map(|robot| robot.id.as_str()), last_profile))
    .map_err(patronus_error)?;
  if !options.supported {
    return Err(CliError::new(
      ErrorCode::PatronusRefused,
      format!("Patronus does not protect {target}: a robot can't be started into it"),
    ));
  }
  let profiles: Vec<PatronusProfile> = options.profiles.into_iter().filter(|profile| allows_push_mode(profile, mode)).collect();

  // the open pull requests of the branch and of the ancestors its push takes along: one into a virtual branch goes into
  // the target as every pull request of a virtual branch, one into another base may be meant. The whole stack's is the
  // pull request of its stack branch, which no retarget touches
  let listed = match &github {
    Some(repository) => open_pull_requests(context, github_service, runtime, repository),
    None => PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
  };
  let own = match &stack_push {
    Some(_) => listed.stack_pull_requests.get(name),
    None => listed.pull_requests.get(name),
  };
  let (pull_request, other_base_pull_request) = match own.filter(|pull_request| pull_request.state == PullRequestState::Open) {
    Some(open) if open.base_ref_name.is_empty() || open.base_ref_name == target || (stack_push.is_none() && is_virtual_branch_base(&open.base_ref_name, &context.prefix)) => {
      (Some(open.clone()), None)
    }
    other => (None, other.cloned()),
  };
  let retargets = match &stack_push {
    Some(_) => Vec::new(),
    None => virtual_base_retargets(
      &listed.pull_requests,
      pushed_along.iter().map(String::as_str).chain(std::iter::once(name)),
      &context.prefix,
      &target,
    ),
  };
  let profile = match args.profile.as_deref() {
    Some(profile) if pull_request.is_some() && !profile.eq_ignore_ascii_case("none") => {
      return Err(CliError::new(
        ErrorCode::InvalidArguments,
        format!(
          "{what} starts from pull request #{}: a /patronus comment starts the default checks of {target} and can't choose a merge profile; drop --profile",
          pull_request.as_ref().map_or(0, |pull_request| pull_request.number)
        ),
      ));
    }
    Some(profile) if profile.eq_ignore_ascii_case("none") => None,
    Some(wanted) => Some(
      profiles
        .iter()
        .find(|profile| profile.id == wanted || profile.name.eq_ignore_ascii_case(wanted))
        .cloned()
        .ok_or_else(|| {
          let names = profiles.iter().map(|profile| format!("{} ({})", profile.id, profile.name)).collect::<Vec<_>>().join(", ");
          CliError::new(
            ErrorCode::InvalidArguments,
            format!("No merge profile {wanted:?} of {target} allows a {}: {names}, or none", push_mode_name(mode)),
          )
        })?,
    ),
    // a comment can't choose one: the default checks
    None if pull_request.is_some() => None,
    None => options
      .default_profile_id
      .as_deref()
      .and_then(|id| profiles.iter().find(|profile| profile.id == id))
      .cloned(),
  };
  if let Some(pull_request) = &pull_request
    && pull_request.is_draft
    && pushes
  {
    refusals.push(Refusal {
      code: ErrorCode::PatronusRefused,
      message: format!("Pull request #{} is a draft: Patronus merges it once it is ready for review", pull_request.number),
    });
  }

  // a stack branch that is not built has no commits to review: the refusal above says why
  let reviewable = stack_push.as_ref().is_none_or(|stack| {
    stack.tip.is_some()
      && refusals
        .iter()
        .all(|refusal| refusal.code != ErrorCode::StackConflict && refusal.code != ErrorCode::StackNotBuilt)
  });
  let approvals_skipped_by = ApprovalsSkippedBy::of(args, pushes, pull_request.as_ref(), profile.as_ref());
  let review = if pushes && reviewable {
    let repository = ReadinessRepository {
      space: &space,
      github: github.as_ref(),
      target: &target,
      baseline: &synced.baseline,
      branch_prefix: &context.prefix,
    };
    // the whole stack is asked as its stack branch
    let branch = ReviewReadinessBranch::starting_from(
      name.to_string(),
      stack_push.as_ref().map(|stack| stack.full_name.clone()),
      local_tip.unwrap_or_else(|| format!("refs/heads/{full_name}")),
      pull_request.as_ref(),
    );
    let review = review_readiness(context, service, runtime, &repository, &branch)?;
    // Patronus judges the approvals of the pull request the robot starts from; an older Patronus that doesn't say
    // whose count leaves the start to the robot. A profile that skips the review starts without them, as the app's
    // Safe Push without review does
    if matches!(review.readiness, ReviewReadiness::Missing { .. }) && approvals_skipped_by.is_none() && !args.create_pull_request {
      refusals.push(Refusal {
        code: ErrorCode::ApprovalsMissing,
        message: approvals_missing_message(&what, &rerun, &review, github.as_ref(), pull_request.as_ref()),
      });
    }
    Some(review)
  } else {
    None
  };
  if args.create_pull_request && github.is_none() {
    return Err(CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY));
  }
  // a push of new commits into a pull request someone approved, which the robot starts from: the rules of the target may
  // dismiss the approvals with the push, as the app's popover asks; one GET, only here
  let approved = pull_request
    .as_ref()
    .filter(|pull_request| pushes && push_needed && !args.create_pull_request && pull_request.reviewers.iter().any(|reviewer| reviewer.state == ReviewerState::Approved));
  let rules = match (approved, &github) {
    (Some(pull_request), Some(repository)) => {
      let read = runtime.block_on(read_branch_rules(github_service, &context.repository, repository, &target));
      let counted = review.as_ref().is_some_and(|review| matches!(review.readiness, ReviewReadiness::Approved { .. }));
      let fix = if stack_push.is_some() {
        format!(
          "push first (`bd push {name} --with-children`), then ask the code owners for the review of #{} again",
          pull_request.number
        )
      } else {
        format!("push first (`bd push {name}`), then ask for the review again (`bd request-review {name}`)")
      };
      let (summary, refusal) = rules_check(read, pull_request.number, &target, counted, args.allow_unknown_rules, &fix);
      refusals.extend(refusal);
      Some(summary)
    }
    _ => None,
  };

  Ok(Plan {
    name: name.to_string(),
    full_name,
    mode,
    space,
    target,
    github,
    profiles,
    profile,
    review,
    rules,
    approvals_skipped_by,
    pull_request,
    other_base_pull_request,
    retargets,
    push_needed,
    remotes,
    // what merged Safe Pushes landed of it is in the target: the robot lands the rest, the branch itself always named
    lands: lands
      .into_iter()
      .filter(|branch| (branch == name && stack_push.is_none()) || landed_whole(branch).is_none())
      .collect(),
    refusals,
    alone,
    warnings,
    known_robots: {
      known_robots.extend(risky_robot.iter().map(|robot| robot.id.clone()));
      known_robots
    },
    risky_robot,
    stack: stack_push,
    target_check,
  })
}

/// What the rules of `target` mean for a push of new commits into the approved pull request #`number`: the approvals
/// Patronus counts (`counted`) dismissed by it refuse the start (`push_dismisses_approvals`, `fix` says what to do),
/// other approvals dismissed are said; rules GitHub didn't answer refuse it (`rules_unknown`) unless `allow_unknown`,
/// never read as "keeps the approvals"
fn rules_check(read: Result<BranchRules, GitHubFailure>, number: u32, target: &str, counted: bool, allow_unknown: bool, fix: &str) -> (RulesSummary, Option<Refusal>) {
  match read {
    Ok(rules) if rules.dismiss_stale_reviews_on_push && counted => {
      let message = format!(
        "Nothing was pushed or started: the push dismisses the approvals of pull request #{number} that Patronus counts (the rules of {target} dismiss stale reviews on push), and the robot would fail without them; {fix}"
      );
      let summary = RulesSummary {
        dismiss_stale_reviews_on_push: Some(true),
        message: message.clone(),
      };
      (
        summary,
        Some(Refusal {
          code: ErrorCode::PushDismissesApprovals,
          message,
        }),
      )
    }
    Ok(rules) => {
      let message = if rules.dismiss_stale_reviews_on_push {
        format!("The push dismisses the approvals of pull request #{number} (the rules of {target} dismiss stale reviews on push); Patronus counts none of them")
      } else {
        format!("The push keeps the approvals of pull request #{number}: the rules of {target} don't dismiss stale reviews")
      };
      let summary = RulesSummary {
        dismiss_stale_reviews_on_push: Some(rules.dismiss_stale_reviews_on_push),
        message,
      };
      (summary, None)
    }
    Err(failure) => {
      let unknown = format!(
        "Cannot check whether the push keeps the approvals of pull request #{number}: GitHub didn't answer the rules of {target} ({})",
        failure_message(&failure)
      );
      let summary = RulesSummary {
        dismiss_stale_reviews_on_push: None,
        message: unknown.clone(),
      };
      let refusal = (!allow_unknown).then(|| Refusal {
        code: ErrorCode::RulesUnknown,
        message: format!("Nothing was pushed or started: {unknown}; ask the user, --allow-unknown-rules starts anyway"),
      });
      (summary, refusal)
    }
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
/// branch (`push_leased`): its members stay as they are on the remote. `lock` is released before the network push, which
/// sends the tip the sync left
fn push_stack_branch(context: &RepoContext, synced: &SyncOutput, root: &str, stack: &StackPush, lock: &mut Option<RepositoryLock>) -> Result<PushedBranch, CliError> {
  let tip = stack
    .tip
    .clone()
    .ok_or_else(|| CliError::new(ErrorCode::StackNotBuilt, format!("Nothing was pushed or started: {} has no tip", stack.full_name)))?;
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
  let leased = [LeasedBranch {
    name: root.to_string(),
    push: push.clone(),
    parent_ref: None,
  }];
  drop(lock.take());
  let pushed =
    push_leased(&context.git, &target, &leased, PushOptions { atomic: true, dry_run: false }, None).map_err(|e| CliError::new(ErrorCode::NetworkError, format!("{e:#}")))?;
  let statuses = match pushed {
    LeasedPush::Pushed(statuses) => statuses,
    LeasedPush::RemoteMoved { remote_statuses, .. } => {
      let short = context.short_ids(remote_statuses.iter().flat_map(RemoteSummary::ids));
      let mut error = CliError::new(
        ErrorCode::LeaseRejected,
        format!(
          "The remote rejected {}: it is not where the lease expects it. Nothing was pushed or started; bd fetched it, and `branches` says what the remote has now (`dropped_commits`). Show them to the user; `bd push {root} --with-children --force` replaces them, after the user approves",
          stack.full_name
        ),
      );
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
        format!("Nothing was pushed or started: the remote refused the push ({summary})"),
      ));
    }
    LeasedPush::WouldReject => return Err(CliError::new(ErrorCode::LeaseRejected, format!("The remote would reject {}", stack.full_name))),
    LeasedPush::TooLarge(too_large) => return Err(CliError::from(&too_large)),
  };
  let short = context.short_ids(std::iter::once(push.tip.as_str()).chain(push.lease.as_deref()));
  Ok(pushed_branch(root, &push, &statuses, stack.draft.clone(), &short, stack.replaced_landed.clone()))
}

/// The service of `bd`: the token of the Safe Push CLI (`./safePush.cmd`), else `SPACE_TOKEN`; `not_logged_in` without
/// either, before anything else runs
pub(crate) fn patronus_service(environment: &Environment) -> Result<PatronusService, CliError> {
  let home = environment.home.clone();
  let space_token = environment.space_token.clone();
  if !matches!(find_token(home.as_deref(), space_token.clone(), unix_now()), TokenLookup::Found(_)) {
    return Err(CliError::new(
      ErrorCode::NotLoggedIn,
      "No Space token for Patronus, or it expired: run ./safePush.cmd once to log in, or set SPACE_TOKEN",
    ));
  }
  let user_agent = cli_user_agent();
  let mut client = PatronusClient::new(environment.patronus_url.clone(), &user_agent);
  if let Some(delay) = environment.remote_retry_delay {
    client = client.with_retry_delay(delay);
  }
  Ok(PatronusService::new(client, move |now| find_token(home.as_deref(), space_token.clone(), now)))
}

fn unix_now() -> u32 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |elapsed| u32::try_from(elapsed.as_secs()).unwrap_or(u32::MAX))
}

/// The user's robots of the last 7 days in the repository
pub(crate) fn robots(runtime: &Runtime, service: &PatronusService, space: &SpaceRepository, prefix: &str, new_api: bool) -> Result<Vec<PatronusRobot>, CliError> {
  robots_since(runtime, service, space, prefix, new_api, None)
}

fn robots_since(runtime: &Runtime, service: &PatronusService, space: &SpaceRepository, prefix: &str, new_api: bool, since: Option<u32>) -> Result<Vec<PatronusRobot>, CliError> {
  let result = runtime.block_on(service.robots(&space.name, prefix, since, false, new_api));
  match result.availability {
    PatronusAvailability::Available => Ok(result.robots),
    PatronusAvailability::NotLoggedIn => Err(CliError::new(
      ErrorCode::NotLoggedIn,
      "Patronus rejected the Space token: run ./safePush.cmd once to log in again",
    )),
    PatronusAvailability::Error { message } => Err(CliError::new(ErrorCode::PatronusError, message)),
    PatronusAvailability::NotPatronus => Err(CliError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't push it")),
  }
}

/// A failed Patronus request: its answer
pub(crate) fn patronus_error(message: String) -> CliError {
  if message.contains("safePush.cmd") {
    return CliError::new(ErrorCode::NotLoggedIn, message);
  }
  CliError::new(ErrorCode::PatronusError, message)
}

/// The remote lacks commits of the branch, or has commits a push replaces or drops
fn needs_push(remote: Option<&RemoteSummary>) -> bool {
  remote.is_none_or(RemoteSummary::has_work)
}

/// The pull requests of the branches, by virtual branch name, and of the stack branches, by root (one search). A Safe
/// Push starts from the open one of the branch (or of the stack branch) into the target (the approvals of a pull request count only for a robot its comment started), or into a
/// virtual branch once it is retargeted; one into another base starts through the API, as Patronus would merge it
/// there. Without a GitHub token or an answer of GitHub, through the API too.
pub(crate) fn open_pull_requests(context: &RepoContext, github: &GitHubService, runtime: &Runtime, repository: &GitHubRepository) -> PullRequestsResult {
  let result = runtime.block_on(list_pull_requests(github, &context.repository, repository, &context.prefix));
  if result.pull_requests.is_empty() && result.stack_pull_requests.is_empty() {
    tracing::debug!(availability = ?result.availability, "no pull requests of the branches");
  }
  result
}

/// `--create-pull-request` of a branch whose open pull request goes into another base than the target: it is taken as
/// it is, and the warning says where Patronus would merge it
fn other_base_pull_request_start(existing: &PullRequest, target: &str) -> PullRequestStart {
  PullRequestStart {
    number: existing.number,
    url: existing.url.clone(),
    created: false,
    comment_url: None,
    reply: None,
    reviewers: Vec::new(),
    assignees: Vec::new(),
    warnings: vec![format!(
      "pull request #{} already existed and goes into {}: Patronus merges a pull request into its base, so its approvals count only for a Safe Push into {}; retarget it to {target} on GitHub (`gh pr edit {} --base {target}`) for a Safe Push into {target}",
      existing.number, existing.base_ref_name, existing.base_ref_name, existing.number
    )],
  }
}

/// How `bd` names itself to Patronus, GitHub and YouTrack
pub(crate) fn cli_user_agent() -> String {
  format!("Branch Deck CLI/{} ({}; {})", bd_core::version::VERSION, std::env::consts::OS, std::env::consts::ARCH)
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

/// The GitHub service of `bd`: the token of `GH_TOKEN`/`GITHUB_TOKEN`, `gh auth token` or the git credentials
pub(crate) fn github_service(context: &RepoContext, environment: &Environment) -> GitHubService {
  let user_agent = cli_user_agent();
  let mut client = GitHubClient::with_urls(environment.github_api_url.clone(), GITHUB_URL, &user_agent);
  if let Some(delay) = environment.remote_retry_delay {
    client = client.with_retry_delay(delay);
  }
  GitHubService::new(client, TokenChain::new(environment.github_tokens.clone(), context.git.clone()))
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

/// The review of the commits of the virtual branch at its local tip: one `git log`, the paths from the object cache,
/// the pre-check with the pull request it starts from (asked again, never a kept answer, which it writes for the app),
/// then the `OWNERSHIP` files for the groups Patronus names without their files. A pre-check that failed is an error.
pub(crate) fn review_readiness(
  context: &RepoContext,
  service: &PatronusService,
  runtime: &Runtime,
  repository: &ReadinessRepository<'_>,
  branch: &ReviewReadinessBranch,
) -> Result<Review, CliError> {
  let cache = context.cache();
  let answer = runtime
    .block_on(branch_review_readiness(
      &context.git,
      &context.repository,
      &cache,
      service,
      repository,
      std::slice::from_ref(branch),
      true,
    ))?
    .pop()
    .ok_or_else(|| CliError::new(ErrorCode::PatronusError, "Patronus gave no answer for the review"))?;
  match (answer.requirement, answer.readiness) {
    (Some(requirement), readiness) => Ok(Review { requirement, readiness }),
    (None, ReviewReadiness::Unavailable { reason, message }) => Err(match reason {
      UnavailableReadinessReason::NotLoggedIn => CliError::new(ErrorCode::NotLoggedIn, message),
      UnavailableReadinessReason::Error => CliError::new(ErrorCode::PatronusError, message),
    }),
    (None, _) => Err(CliError::new(ErrorCode::PatronusError, "Patronus gave no answer for the review")),
  }
}

/// The GitHub repository of the branches is not known: a pull request can't be opened, its approvals can't count
pub(crate) const UNKNOWN_GITHUB_REPOSITORY: &str =
  "The GitHub repository of the branches is unknown: the Space settings name no GitHub mirror; run `git config branchdeck.githubRepository <owner>/<name>`";

/// "The code owners haven't approved auth: …", with the groups, the approvals Patronus doesn't count, and where the
/// approvals count: the GitHub pull request of the branch, or a Space merge request found open. `name` says what lands
/// ("auth", "the whole auth stack"), `rerun` is the command that starts it ("bd safe-push auth")
fn approvals_missing_message(name: &str, rerun: &str, review: &Review, github: Option<&GitHubRepository>, pull_request: Option<&PullRequest>) -> String {
  let ReviewReadiness::Missing { groups, not_counted, reason } = &review.readiness else {
    return String::new();
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
  let no_eligible = if *reason == MissingApprovalsReason::NoEligibleReviewer {
    " No member of a group can approve it (Patronus names none eligible): ask the group to add a reviewer."
  } else {
    ""
  };
  let counts = match (github, pull_request, &review.requirement.merge_request) {
    (Some(_), Some(pull_request), _) => format!(
      "Patronus counts code-owner approvals in pull request #{} ({}): once they approved it there, run it again",
      pull_request.number, pull_request.url
    ),
    (Some(_), None, _) => format!(
      "Patronus counts code-owner approvals in the GitHub pull request of the branch: `{rerun} --create-pull-request` pushes the branch and opens it, and the Safe Push starts from it once they approved"
    ),
    (None, _, Some(merge_request)) => format!("Waiting for approval in {} ({})", merge_request.review_key, merge_request.url),
    (None, _, None) => {
      format!("Patronus counts code-owner approvals in the GitHub pull request of the branch. {UNKNOWN_GITHUB_REPOSITORY}, then `{rerun} --create-pull-request`")
    }
  };
  format!(
    "The code owners haven't approved {name}: a Safe Push fails without these approvals. Nothing was started.{needed}{not_counted}{no_eligible} {counts}. `review` lists the groups and their files; show them to the user, then run again once approved, or with --without-approvals to start anyway"
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
  plan: &Plan,
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
  service: &PatronusService,
  runtime: &Runtime,
  plan: &Plan,
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
  let robots = robots_since(
    runtime,
    service,
    &plan.space,
    &context.prefix,
    context.patronus_new_api,
    Some(posted_at.saturating_sub(COMMENT_CLOCK_SLACK)),
  )?;
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

fn review_summary(review: &Review, reviewed_on_github: bool, pull_request: Option<&PullRequest>, skipped_by: Option<ApprovalsSkippedBy>) -> ReviewSummary {
  let requirement = &review.requirement;
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
    skipped_by: skipped_by.map(|skipped_by| skipped_by.name().to_string()),
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
