//! `bd safe-push`: start a Patronus robot on a virtual branch, as the app's Safe Push form does: a Safe Push (rebase onto
//! the target, the checks, the push into the target) or a Dry Run (the checks only), with a merge profile. Before a Safe
//! Push the code-owner review is checked; unlike the form, which only warns, missing approvals refuse the start
//! (`approvals_missing`): Patronus would fail the robot with "Changes require review approval". A branch the remote
//! lacks commits of (by content, as the sync's remote status compares) is pushed first, parents first, as `bd push`
//! pushes it; one only rebuilt on a newer baseline is not: the robot takes the remote branch. The start is never repeated. With `--check` nothing is pushed or
//! started: the profiles, the review and what stands in the way, for the user to approve. The approvals count in the
//! GitHub pull request of the branch (Space merge requests are deprecated): `--create-pull-request` opens it, and the
//! Safe Push then starts from it with a `/patronus` comment.
//!
//! The sync, the checks before the push and the push run under the repository lock; the start, which can wait up to a
//! minute for the answer to a pull request comment, after it.

use crate::check_command::{CheckWarning, dangling_reference_message, dangling_references_of};
use crate::context::{Environment, RepoContext};
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use crate::push_command::{PullRequestDraft, PushArgs, PushedBranch, github_repository, not_synced_reason, push_locked, unpushed_pull_request_draft};
use crate::push_hold::{PushHolds, StackRobots};
use crate::sync_command::{RemoteSummary, SyncOutput, SyncedBranch, lock, sync_locked};
use git_ops::model::to_final_branch_name;
use github::client::{GITHUB_URL, GitHubClient, GitHubError};
use github::service::{GitHubFailure, GitHubService};
use github::token::TokenChain;
use patronus::client::PatronusClient;
use patronus::review_readiness::evaluates_pull_request;
use patronus::service::PatronusService;
use patronus::token::{TokenLookup, find_token};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_core::check::{StackIndependence, stack_independence};
use sync_core::patronus::{SpaceRepository, target_branch_of};
use sync_core::patronus_command::{post_patronus_command, read_patronus_command_reply};
use sync_core::patronus_start::{allows_push_mode, find_landing_ancestor, find_landing_descendant, is_push_into_robot_risky, is_start_blocked, push_mode_name, start_refusal};
use sync_core::pull_request_create::{CreatePullRequestParams, create_pull_request as core_create_pull_request, fill_pull_request};
use sync_core::pull_requests::{GitHubRepository, GitHubWriteError, failure_message, list_pull_requests};
use sync_core::review_readiness::{ReadinessRepository, branch_review_readiness};
use sync_types::patronus::{
  MissingApprovalsReason, MissingReviewGroup, PatronusAvailability, PatronusCommandReply, PatronusCommandSubject, PatronusMergeRequest, PatronusProfile, PatronusPushMode,
  PatronusReviewRequirement, PatronusRobot, PatronusRobotStatus, Person, ReviewReadiness, ReviewReadinessBranch, ReviewStart, ReviewerApprovalState, UnavailableReadinessReason,
  UnknownReadinessReason,
};
use sync_types::pull_request::{PullRequest, PullRequestState, ReviewerState};
use tokio::runtime::Runtime;

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
  /// A Dry Run: the checks without the push into the target; the review is not checked
  #[arg(long)]
  pub dry_run: bool,
  /// The merge profile, by id or name; `none` for the default checks. Default: the profile of the last robot of the
  /// branch
  #[arg(long, value_name = "PROFILE")]
  pub profile: Option<String>,
  /// Sync, then print the profiles, the review and what stands in the way of a start; nothing is pushed or started
  #[arg(long)]
  pub check: bool,
  /// Start a Safe Push although the code owners have not approved it: Patronus fails it unless they approve meanwhile
  #[arg(long, conflicts_with = "dry_run")]
  pub without_approvals: bool,
  /// Push the branch and open its GitHub pull request, where the code owners approve, assigned to the user; no robot
  /// starts. The Safe Push starts from it then
  #[arg(long, conflicts_with_all = ["dry_run", "check", "without_approvals", "profile"])]
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
  /// Start a stacked branch whose parents are not in the target yet: its Safe Push lands them with it
  #[arg(long)]
  pub land_stack: bool,
  /// Start although the branch removes a declaration that a commit of another branch still changes the use of
  /// (`dangling_reference`): the build fails unless the reference is a false alarm
  #[arg(long)]
  pub allow_dangling: bool,
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
  /// The merge profile the robot starts with; `null` for the default checks, also for a start from a pull request
  /// comment, which can't choose one
  pub profile: Option<ProfileSummary>,
  /// The merge profiles of the target that allow the mode, never the emergency ones: what `--profile` takes
  pub profiles: Vec<ProfileSummary>,
  /// The code-owner review the Safe Push needs; `null` for a dry run
  pub review: Option<ReviewSummary>,
  /// The remote lacks commits of the branch or of the branches it is built on, by content: the start pushes them
  /// first. False for a remote branch with the same commits, only rebuilt on an older baseline: the robot takes it as it
  /// is (Patronus rebases it), nothing is pushed
  pub push_needed: bool,
  /// With `--check`: what refuses a start, each with the error code the start would fail with
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub refusals: Vec<Refusal>,
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

/// The code-owner review of the branch, as the pre-check of Patronus answers it
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewSummary {
  /// The code owners still have to approve: a Safe Push fails without these approvals
  pub required: bool,
  /// The branch has too many commits to check (500), as the IDE skips it
  pub skipped: bool,
  /// `space_merge_request`, or `pull_request` for branches reviewed on GitHub: where the approvals count
  pub counts_in: String,
  /// An open Space merge request of the branch whose approvals count; a start through the API links it
  pub merge_request: Option<MergeRequestSummary>,
  /// The groups whose approval is missing; when Patronus evaluated the pull request of the start, every group the
  /// changes need, each with `approved`
  pub groups: Vec<ReviewGroupSummary>,
  /// Who approved the pull request the Safe Push starts from with an approval that counts: the members Patronus counts
  /// when it evaluated the pull request, else every approver (an older Patronus doesn't say whose count)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub approved_on_github: Vec<String>,
  /// What the review means for the start: `not_required`, `approved`, `missing` (refused as `approvals_missing`), or
  /// `unknown` (Patronus doesn't say whether the approvals of the pull request count, or the branch is too long: the
  /// robot decides)
  pub readiness: String,
  /// Why: `approvals_missing`, `no_pull_request_context` (a start through the API counts no approval of a pull
  /// request), `no_eligible_reviewer` (a group has nobody who can approve) for `missing`; `approvals_not_evaluated` or
  /// `skipped` for `unknown`
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub readiness_reason: Option<String>,
  /// The code owners of the missing groups asked to review the pull request who haven't approved: the start waits for
  /// them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub waiting_for: Vec<PersonSummary>,
  /// Approvers of the pull request Patronus doesn't count (no eligible code owner of the files, e.g. a bot)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub not_counted: Vec<PersonSummary>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewGroupSummary {
  pub name: String,
  pub reviewers: Vec<ReviewerSummary>,
  /// The files the group reviews: the ones Patronus names, else the ones its `OWNERSHIP` entries own; empty while
  /// neither says
  pub paths: Vec<String>,
  /// The group reviews every file of the branch
  pub branch_wide: bool,
  /// Whether the pull request has the approvals of the group; only when Patronus evaluated it
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub approved: Option<bool>,
  /// How many more approvals count for the group; only when Patronus says
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub needed: Option<u32>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewerSummary {
  /// The Space username; `null` for a member without one (a bot member)
  pub username: Option<String>,
  pub full_name: String,
  /// The GitHub login, for `--reviewer`; `null` when Patronus doesn't say
  pub github_login: Option<String>,
  pub available: bool,
  /// Patronus suggests asking this reviewer
  pub suggested: bool,
  /// What the review of the member in the pull request counts for: `approved`, `author_self_approved`, `reviewing`
  /// (asked, or the latest review comments or requests changes), `not_added` or `not_eligible`; only when Patronus
  /// evaluated the pull request
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub approval_state: Option<String>,
}

/// A person of the review: the full name Patronus knows, the GitHub login when known
#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct PersonSummary {
  pub login: Option<String>,
  pub full_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct MergeRequestSummary {
  /// e.g. `IJ-CR-7`
  pub key: String,
  pub url: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct RobotSummary {
  pub id: String,
  pub name: String,
  /// `running`, `canceling`, `canceled`, `failure`, `success` or `unknown`
  pub status: String,
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

/// The answer of the pre-check and what it means for the start
pub(crate) struct Review {
  pub(crate) requirement: PatronusReviewRequirement,
  pub(crate) readiness: ReviewReadiness,
}

/// The facts a start is decided on, read under the repository lock
struct Plan {
  full_name: String,
  mode: PatronusPushMode,
  space: SpaceRepository,
  target: String,
  github: Option<GitHubRepository>,
  profiles: Vec<PatronusProfile>,
  profile: Option<PatronusProfile>,
  review: Option<Review>,
  pull_request: Option<PullRequest>,
  push_needed: bool,
  refusals: Vec<Refusal>,
  warnings: Vec<CheckWarning>,
  /// The running Safe Push of an older commit that the start replaces: a push would fail it, and without a push the new
  /// robot would run beside it; canceled first with `--cancel-running`, as the app's form cancels it
  risky_robot: Option<PatronusRobot>,
}

pub fn safe_push(context: &RepoContext, args: &SafePushArgs, environment: &Environment) -> Result<SafePushOutput, CliError> {
  let mode = if args.dry_run { PatronusPushMode::DryRun } else { PatronusPushMode::RebaseAutosquash };
  let service = patronus_service(environment)?;
  let github = github_service(context, environment);
  let runtime = patronus_runtime()?;

  let lock = lock(context)?;
  let synced = sync_locked(context)?;
  let plan = plan(context, args, &service, &github, &runtime, &synced, mode)?;
  let mut output = SafePushOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    mode: mode_name(mode),
    target: plan.target.clone(),
    profile: plan.profile.as_ref().map(profile_summary),
    profiles: plan.profiles.iter().map(profile_summary).collect(),
    review: plan.review.as_ref().map(|review| review_summary(review, plan.github.is_some(), plan.pull_request.as_ref())),
    push_needed: plan.push_needed,
    refusals: Vec::new(),
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
  };
  if args.check {
    output.refusals = plan.refusals;
    return Ok(output);
  }
  if let Some(refusal) = plan.refusals.first().cloned() {
    let mut error = CliError::new(refusal.code, refusal.message);
    if refusal.code == ErrorCode::ApprovalsMissing {
      error.review = output.review.take().map(Box::new);
    }
    return Err(error);
  }

  if let Some(robot) = &plan.risky_robot {
    let canceled = runtime.block_on(service.cancel(&context.prefix, &robot.id)).map_err(patronus_error)?;
    output.canceled = Some(robot_summary(context, &canceled));
  }
  // by content: a branch whose remote branch has its commits, only rebuilt on an older baseline, is not pushed; the
  // robot takes the remote branch and Patronus rebases it, and a push would move the head of its pull request for nothing
  let (remote, draft) = if plan.push_needed {
    // the robot of the branch is canceled above; the start lands the ancestors its push takes along
    let pushed = push_locked(
      context,
      &PushArgs {
        branches: vec![args.branch.clone()],
        ..PushArgs::default()
      },
      &synced,
      &PushHolds::default(),
    )?;
    output.pushed = pushed.branches;
    let draft = output
      .pushed
      .iter()
      .find(|pushed| pushed.name == args.branch)
      .map(|pushed| pushed.pull_request.clone())
      .ok_or_else(|| CliError::new(ErrorCode::GitError, format!("{} was not pushed", args.branch)))?;
    (pushed.remote, draft)
  } else {
    unpushed_pull_request_draft(context, &synced, &args.branch)?
  };
  drop(lock);

  if args.create_pull_request {
    if output.pull_request.is_none() {
      output.pull_request = Some(create_pull_request(context, args, &github, &runtime, &plan, &remote, &draft)?);
    }
    return Ok(output);
  }

  if let Some(pull_request) = &plan.pull_request {
    let (start, robot) = start_from_pull_request(context, &github, &service, &runtime, &plan, pull_request)?;
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

/// Everything the start is decided on, before anything is pushed: the repository on Space, the robots of the user,
/// the stack, the profiles, the pull request and the review
fn plan(
  context: &RepoContext,
  args: &SafePushArgs,
  service: &PatronusService,
  github_service: &GitHubService,
  runtime: &Runtime,
  synced: &SyncOutput,
  mode: PatronusPushMode,
) -> Result<Plan, CliError> {
  let name = args.branch.as_str();
  let branch = synced
    .branches
    .iter()
    .find(|branch| branch.name == name)
    .ok_or_else(|| CliError::new(ErrorCode::UnknownBranch, format!("No branch named {name:?}")))?;
  if let Some(reason) = not_synced_reason(branch) {
    return Err(CliError::new(
      ErrorCode::PushRefused,
      format!("Nothing was pushed or started: {name} is not synced ({reason})"),
    ));
  }
  let full_name = to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
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

  // the stack, root first, and the robots of the user, each matched to its branch as the app does
  let stack = StackRobots::new(context, synced, robots(runtime, service, &space, &context.prefix)?);
  let ancestors = stack.ancestors_of(name);
  let order = &stack.order;
  let ancestors_of = |name: &str| stack.ancestors_of(name);
  let robot_of = |name: &str| stack.robot_of(name);
  let tip_of = |name: &str| stack.tip_of(name);
  let synced_by_name: HashMap<&str, &SyncedBranch> = synced.branches.iter().map(|branch| (branch.name.as_str(), branch)).collect();
  let local_tip = tip_of(name);
  let robot = robot_of(name);
  // the remote branch while it has the commits of the branch by content: a robot on it runs on the branch as it is
  let pushed_tip = synced_by_name
    .get(name)
    .and_then(|branch| branch.remote.as_ref())
    .filter(|remote| !needs_push(Some(remote)))
    .and_then(|remote| remote.head_id.as_deref());

  let mut refusals = Vec::new();
  let pushes = mode != PatronusPushMode::DryRun;
  // a dry run lands nothing: it waits for no robot of the stack
  // a robot of a side branch that lands an ancestor counts too; the branch's own robot is the one a start replaces
  let span_of = |ancestor: &str| find_landing_descendant(ancestor, order.iter().map(String::as_str), ancestors_of, robot_of).filter(|landing| landing.branch != name);
  let landing_descendant = if pushes {
    find_landing_descendant(name, order.iter().map(String::as_str), ancestors_of, robot_of)
  } else {
    None
  };
  // a robot that lands the branch itself says so: it lands its ancestors too
  let landing_ancestor = if pushes {
    find_landing_ancestor(&ancestors, robot_of, span_of).filter(|landing| landing_descendant.as_ref().is_none_or(|it| it.robot.id != landing.robot.id))
  } else {
    None
  };
  if let Some(message) = start_refusal(robot, local_tip.as_deref(), pushed_tip, landing_ancestor.as_ref(), landing_descendant.as_ref(), name) {
    refusals.push(Refusal {
      code: ErrorCode::RobotRunning,
      message,
    });
  }

  let push_needed = std::iter::once(name)
    .chain(ancestors.iter().map(String::as_str))
    .any(|name| synced_by_name.get(name).is_none_or(|branch| needs_push(branch.remote.as_ref())));
  // a Safe Push of an older commit (a robot on the commits as they are refuses the start above): the push fails it at
  // the end; without a push a Safe Push replaces it, as the app's form does, and a dry run or a pull request leaves it
  let starts_safe_push = pushes && !args.create_pull_request;
  let risky_robot = robot
    .filter(|robot| is_push_into_robot_risky(robot, name) && !is_start_blocked(robot, local_tip.as_deref(), pushed_tip))
    .filter(|_| push_needed || starts_safe_push)
    .cloned();
  if let Some(robot) = &risky_robot
    && !args.cancel_running
  {
    let why = if push_needed { "the push fails it at the end" } else { "the start would run beside it" };
    refusals.push(Refusal {
      code: ErrorCode::RobotRunning,
      message: format!(
        "A {} of an older commit of {name} is running: {why}. Pass --cancel-running to cancel it first ({})",
        push_mode_name(robot.push_mode),
        robot.url
      ),
    });
  }
  if pushes && !ancestors.is_empty() && !args.land_stack && !args.create_pull_request {
    refusals.push(Refusal {
      code: ErrorCode::StackNotLanded,
      message: format!(
        "{name} is built on {}, which aren't in {target} yet. Safe Push {} first, then the next: after a merge, the next sync moves the branches built on it onto {target}. Or pass --land-stack: the Safe Push of {name} lands {} with it{}",
        join_names(&ancestors),
        ancestors[0],
        join_names(&ancestors),
        stack_independence_hint(context, name)
      ),
    });
  }

  // a removed declaration that another branch's commit still changes the use of: git merges it, the build fails
  let built: Vec<&str> = std::iter::once(name).chain(ancestors.iter().map(String::as_str)).collect();
  let (dangling, warnings) = dangling_references_of(context, &built)?;
  if !dangling.is_empty() && !args.allow_dangling {
    refusals.push(Refusal {
      code: ErrorCode::DanglingReference,
      message: dangling_reference_message(name, &dangling),
    });
  }

  let options = runtime
    .block_on(service.start_options(&space.project_key, &space.name, &target, robot.map(|robot| robot.id.as_str())))
    .map_err(patronus_error)?;
  if !options.supported {
    return Err(CliError::new(
      ErrorCode::PatronusRefused,
      format!("Patronus does not protect {target}: a robot can't be started into it"),
    ));
  }
  let profiles: Vec<PatronusProfile> = options.profiles.into_iter().filter(|profile| allows_push_mode(profile, mode)).collect();

  let pull_request = match &github {
    Some(repository) => open_pull_request(context, github_service, runtime, repository, name, &target),
    None => None,
  };
  let profile = match args.profile.as_deref() {
    Some(profile) if pull_request.is_some() && !profile.eq_ignore_ascii_case("none") => {
      return Err(CliError::new(
        ErrorCode::InvalidArguments,
        format!(
          "{name} starts from pull request #{}: a /patronus comment starts the default checks of {target} and can't choose a merge profile; drop --profile",
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

  let review = if pushes {
    let repository = ReadinessRepository {
      space: &space,
      github: github.as_ref(),
      target: &target,
      baseline: &synced.baseline,
      branch_prefix: &context.prefix,
    };
    let branch = ReviewReadinessBranch {
      branch_name: name.to_string(),
      tip: local_tip.unwrap_or_else(|| format!("refs/heads/{full_name}")),
      start: pull_request
        .as_ref()
        .map_or(ReviewStart::Rest, |pull_request| ReviewStart::PullRequest { number: pull_request.number }),
      reviewers: pull_request.as_ref().map_or_default(|pull_request| pull_request.reviewers.clone()),
      author: pull_request.as_ref().and_then(|pull_request| pull_request.author.clone()),
    };
    let review = review_readiness(context, service, runtime, &repository, &branch)?;
    // Patronus judges the approvals of the pull request the robot starts from; an older Patronus that doesn't say
    // whose count leaves the start to the robot
    if matches!(review.readiness, ReviewReadiness::Missing { .. }) && !args.without_approvals && !args.create_pull_request {
      refusals.push(Refusal {
        code: ErrorCode::ApprovalsMissing,
        message: approvals_missing_message(name, &review, github.as_ref(), pull_request.as_ref()),
      });
    }
    Some(review)
  } else {
    None
  };
  if args.create_pull_request && github.is_none() {
    return Err(CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY));
  }

  Ok(Plan {
    full_name,
    mode,
    space,
    target,
    github,
    profiles,
    profile,
    review,
    pull_request,
    push_needed,
    refusals,
    warnings,
    risky_robot,
  })
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
  let user_agent = format!("Branch Deck CLI/{} ({}; {})", crate::version::VERSION, std::env::consts::OS, std::env::consts::ARCH);
  Ok(PatronusService::new(PatronusClient::new(environment.patronus_url.clone(), &user_agent), move |now| {
    find_token(home.as_deref(), space_token.clone(), now)
  }))
}

/// The runtime the requests to Patronus (and GitHub) run on
pub(crate) fn patronus_runtime() -> Result<Runtime, CliError> {
  tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .map_err(|e| CliError::new(ErrorCode::GitError, format!("Cannot start the Patronus runtime: {e}")))
}

fn unix_now() -> u32 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |elapsed| u32::try_from(elapsed.as_secs()).unwrap_or(u32::MAX))
}

/// The user's robots of the last 7 days in the repository
pub(crate) fn robots(runtime: &Runtime, service: &PatronusService, space: &SpaceRepository, prefix: &str) -> Result<Vec<PatronusRobot>, CliError> {
  robots_since(runtime, service, space, prefix, None)
}

fn robots_since(runtime: &Runtime, service: &PatronusService, space: &SpaceRepository, prefix: &str, since: Option<u32>) -> Result<Vec<PatronusRobot>, CliError> {
  let result = runtime.block_on(service.robots(&space.name, prefix, since, false));
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

/// The remote lacks commits of the branch, or has commits a push replaces
fn needs_push(remote: Option<&RemoteSummary>) -> bool {
  remote.is_none_or(|remote| !remote.exists || remote.unpushed > 0 || remote.behind > 0 || remote.amended > 0 || remote.restacked > 0)
}

/// The open pull request of the branch into the target, which a Safe Push starts from (the approvals of a pull request
/// count only for a robot its comment started); one into another base (a stacked branch into its parent's branch)
/// starts through the API, as Patronus would merge it there. Without a GitHub token or an answer of GitHub, through the
/// API too.
fn open_pull_request(context: &RepoContext, github: &GitHubService, runtime: &Runtime, repository: &GitHubRepository, name: &str, target: &str) -> Option<PullRequest> {
  let result = runtime.block_on(list_pull_requests(github, &context.repository, repository, &context.prefix));
  let pull_request = result
    .pull_requests
    .get(name)
    .filter(|pull_request| pull_request.state == PullRequestState::Open && (pull_request.base_ref_name.is_empty() || pull_request.base_ref_name == target));
  if pull_request.is_none() {
    tracing::debug!(availability = ?result.availability, "no open pull request of the branch into the target");
  }
  pull_request.cloned()
}

/// The GitHub service of `bd`: the token of `GH_TOKEN`/`GITHUB_TOKEN`, `gh auth token` or the git credentials
pub(crate) fn github_service(context: &RepoContext, environment: &Environment) -> GitHubService {
  let user_agent = format!("Branch Deck CLI/{} ({}; {})", crate::version::VERSION, std::env::consts::OS, std::env::consts::ARCH);
  GitHubService::new(
    GitHubClient::with_urls(environment.github_api_url.clone(), GITHUB_URL, &user_agent),
    TokenChain::new(environment.github_tokens.clone(), context.git.clone()),
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
/// approvals count: the GitHub pull request of the branch, or a Space merge request found open
fn approvals_missing_message(name: &str, review: &Review, github: Option<&GitHubRepository>, pull_request: Option<&PullRequest>) -> String {
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
      "Patronus counts code-owner approvals in the GitHub pull request of the branch: `bd safe-push {name} --create-pull-request` pushes the branch and opens it, and the Safe Push starts from it once they approved"
    ),
    (None, _, Some(merge_request)) => format!("Waiting for approval in {} ({})", merge_request.review_key, merge_request.url),
    (None, _, None) => {
      format!("Patronus counts code-owner approvals in the GitHub pull request of the branch. {UNKNOWN_GITHUB_REPOSITORY}, then `bd safe-push {name} --create-pull-request`")
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

/// Opens the pull request of the pushed branch, as the `gh pr create` that `bd push` prints: into its parent's branch
/// or the target, with the `bd describe` text, else filled from the commits as `--fill` does (one commit: its subject
/// and body; several: the branch name and their subjects), assigned to the user and with the `--reviewer` logins asked.
/// A pull request that already exists is found instead and left as it is.
fn create_pull_request(
  context: &RepoContext,
  args: &SafePushArgs,
  github: &GitHubService,
  runtime: &Runtime,
  plan: &Plan,
  remote: &str,
  draft: &PullRequestDraft,
) -> Result<PullRequestStart, CliError> {
  let Some(repository) = &plan.github else {
    return Err(CliError::new(ErrorCode::InvalidArguments, UNKNOWN_GITHUB_REPOSITORY));
  };
  let (title, body) = match &draft.title {
    Some(title) => (title.clone(), draft.body.clone().unwrap_or_default()),
    None => fill_pull_request(&context.git, &context.repository, remote, &draft.head, &draft.base)?
      .ok_or_else(|| CliError::new(ErrorCode::PushRefused, format!("No commits between {remote}/{} and {}", draft.base, draft.head)))?,
  };
  let params = CreatePullRequestParams {
    repository_path: context.repository.clone(),
    branch_prefix: context.prefix.clone(),
    branch_name: args.branch.clone(),
    head: draft.head.clone(),
    base: draft.base.clone(),
    title,
    body,
    reviewers: args.reviewers.clone(),
    assign_self: !args.no_assign,
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

/// "Air Plugin Release Team (requested: Ilya Korennoy)", else "(suggested: Alice)", as the Safe Push form names a group
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
  let robots = robots_since(runtime, service, &plan.space, &context.prefix, Some(posted_at.saturating_sub(COMMENT_CLOCK_SLACK)))?;
  let robot = robots
    .into_iter()
    .filter(|robot| robot.pull_request.as_ref().is_some_and(|link| link.number == pull_request.number))
    .max_by_key(|robot| robot.start_time);
  Ok((start, robot))
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

fn review_summary(review: &Review, reviewed_on_github: bool, pull_request: Option<&PullRequest>) -> ReviewSummary {
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
    status: match robot.status {
      PatronusRobotStatus::Running => "running",
      PatronusRobotStatus::Canceling => "canceling",
      PatronusRobotStatus::Canceled => "canceled",
      PatronusRobotStatus::Failure => "failure",
      PatronusRobotStatus::Success => "success",
      PatronusRobotStatus::Unknown => "unknown",
    }
    .to_string(),
    mode: mode_name(robot.push_mode),
    commit: robot.source_revision.as_deref().map(|revision| short.get(revision).to_string()),
    url: robot.url.clone(),
  }
}

/// What the trial build of the branch as a root says, as a sentence after the refusal: it builds alone, or it needs
/// commits of its ancestors. The build is on the base the sync builds on, not on the newest target. Empty when the check
/// fails or names no commit
fn stack_independence_hint(context: &RepoContext, name: &str) -> String {
  let independence = match stack_independence(&context.git, &context.repository, &context.prefix, name, &context.cache()) {
    Ok(independence) => independence,
    Err(error) => {
      tracing::debug!(%error, "cannot check whether {name} builds alone");
      return String::new();
    }
  };
  match independence {
    StackIndependence::Builds { uses, .. } if !uses.is_empty() => {
      let first = &uses[0];
      let short = context.short_ids([first.commit.id.as_str(), first.declared_by.id.as_str()]);
      format!(
        ". {name} applies on the baseline alone, but {} uses {}, which {} of {} adds: it needs its stack to compile (found by names, not by a compiler)",
        short.get(&first.commit.id),
        first.name,
        short.get(&first.declared_by.id),
        first.declared_by.branch
      )
    }
    StackIndependence::Builds { symbols: true, .. } => {
      format!(". {name} builds alone on the baseline and uses no name its stack adds: `bd unstack {name}` makes it a root (not checked by a compiler)")
    }
    StackIndependence::Builds { .. } => {
      format!(". {name} builds alone on the baseline: `bd unstack {name}` makes it a root (lines only: compile dependencies are not checked)")
    }
    StackIndependence::Needs { parent_commits, movable, .. } if !parent_commits.is_empty() => {
      let short = context.short_ids(parent_commits.iter().map(|commit| commit.id.as_str()));
      let ids: Vec<String> = parent_commits.iter().map(|commit| format!("{} ({})", short.get(&commit.id), commit.branch)).collect();
      let them = if ids.len() == 1 { "it" } else { "them" };
      if movable {
        format!(". {name} needs {}: move {them} into the branch, then `bd unstack {name}`", join_names(&ids))
      } else {
        format!(
          ". {name} needs {}, and other branches need {them} too: a move would make those need {name}",
          join_names(&ids)
        )
      }
    }
    StackIndependence::Needs { .. } => String::new(),
  }
}

/// "auth", "auth and login", "auth, login and oauth"
fn join_names(names: &[String]) -> String {
  match names {
    [] => String::new(),
    [only] => only.clone(),
    [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
  }
}
