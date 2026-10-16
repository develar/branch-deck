//! `bd safe-push`: start a Patronus robot on a virtual branch, as the app's Safe Push form does: a Safe Push (rebase onto
//! the target, the checks, the push into the target) or a Dry Run (the checks only), with a merge profile. Before a Safe
//! Push the code-owner review is checked; unlike the form, which only warns, missing approvals refuse the start
//! (`approvals_missing`): Patronus would fail the robot with "Changes require review approval". The branch is pushed
//! first, parents first, as `bd push` pushes it; the start is never repeated. With `--check` nothing is pushed or
//! started: the profiles, the review and what stands in the way, for the user to approve. The approvals count in the
//! GitHub pull request of the branch (Space merge requests are deprecated): `--create-pull-request` opens it, and the
//! Safe Push then starts from it with a `/patronus` comment.
//!
//! The sync, the checks before the push and the push run under the repository lock; the start, which can wait up to a
//! minute for the answer to a pull request comment, after it.

use crate::context::{Environment, RepoContext};
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use crate::push_command::{PullRequestDraft, PushArgs, PushedBranch, not_synced_reason, push_locked};
use crate::status::read_grouping;
use crate::sync_command::{RemoteSummary, SyncOutput, SyncedBranch, lock, sync_locked};
use git_ops::model::to_final_branch_name;
use github::client::{GITHUB_URL, GitHubClient, GitHubError};
use github::service::{GitHubFailure, GitHubService, RequestKind};
use github::token::TokenChain;
use patronus::client::PatronusClient;
use patronus::review::ReviewSubject;
use patronus::service::PatronusService;
use patronus::token::{TokenLookup, find_token};
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use sync_core::grouping::branch_tips;
use sync_core::patronus::{SpaceRepository, find_space_repository, find_target_branch};
use sync_core::patronus_command::{post_patronus_command, read_patronus_command_reply};
use sync_core::patronus_review::{attribute_ownership_paths, read_review_input, skipped_review};
use sync_core::patronus_start::{allows_push_mode, find_branch_robot, find_landing_ancestor, find_landing_descendant, is_push_into_robot_risky, push_mode_name, start_refusal};
use sync_core::pull_requests::{GitHubRepository, GitHubWriteError, failure_message, list_pull_requests, resolve_github_repository};
use sync_types::patronus::{
  PatronusAvailability, PatronusCommandReply, PatronusCommandSubject, PatronusMergeRequest, PatronusProfile, PatronusPushMode, PatronusReviewRequirement, PatronusReviewerGroup,
  PatronusRobot, PatronusRobotStatus,
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
  /// Push the branch and open its GitHub pull request (`gh pr create`), where the code owners approve; no robot starts.
  /// The Safe Push starts from it then
  #[arg(long, conflicts_with_all = ["dry_run", "check", "without_approvals", "profile"])]
  pub create_pull_request: bool,
  /// Cancel a running Safe Push of an older commit of the branch first: the push would fail it
  #[arg(long)]
  pub cancel_running: bool,
  /// Start a stacked branch whose parents are not in the target yet: its Safe Push lands them with it
  #[arg(long)]
  pub land_stack: bool,
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
  /// The remote lacks commits of the branch or of the branches it is built on: the start pushes them first
  pub push_needed: bool,
  /// With `--check`: what refuses a start, each with the error code the start would fail with
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub refusals: Vec<Refusal>,
  /// The branches the start pushed, parents first; empty with `--check`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub pushed: Vec<PushedBranch>,
  /// The running Safe Push of an older commit that `--cancel-running` canceled
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
  /// The groups whose approval is missing
  pub groups: Vec<ReviewGroupSummary>,
  /// Who approved the pull request the Safe Push starts from: the robot counts these approvals, the check before it
  /// can't see them, so a Safe Push with any starts
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub approved_on_github: Vec<String>,
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
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ReviewerSummary {
  pub username: String,
  pub full_name: String,
  pub available: bool,
  /// Patronus suggests asking this reviewer
  pub suggested: bool,
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
  review: Option<PatronusReviewRequirement>,
  pull_request: Option<PullRequest>,
  push_needed: bool,
  refusals: Vec<Refusal>,
  /// The running Safe Push of an older commit a push would fail
  risky_robot: Option<PatronusRobot>,
}

pub fn safe_push(context: &RepoContext, args: &SafePushArgs, environment: &Environment) -> Result<SafePushOutput, CliError> {
  let mode = if args.dry_run { PatronusPushMode::DryRun } else { PatronusPushMode::RebaseAutosquash };
  let service = patronus_service(environment)?;
  let github = github_service(context, environment);
  let runtime = tokio::runtime::Builder::new_multi_thread()
    .enable_all()
    .build()
    .map_err(|e| CliError::new(ErrorCode::GitError, format!("Cannot start the Patronus runtime: {e}")))?;

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

  if plan.push_needed
    && let Some(robot) = &plan.risky_robot
  {
    let canceled = runtime.block_on(service.cancel(&context.prefix, &robot.id)).map_err(patronus_error)?;
    output.canceled = Some(robot_summary(context, &canceled));
  }
  let pushed = push_locked(
    context,
    &PushArgs {
      branches: vec![args.branch.clone()],
      ..PushArgs::default()
    },
    &synced,
  )?;
  output.pushed = pushed.branches;
  let remote = pushed.remote;
  drop(lock);

  if args.create_pull_request {
    if output.pull_request.is_none() {
      let draft = output
        .pushed
        .iter()
        .find(|pushed| pushed.name == args.branch)
        .map(|pushed| &pushed.pull_request)
        .ok_or_else(|| CliError::new(ErrorCode::GitError, format!("{} was not pushed", args.branch)))?;
      output.pull_request = Some(create_pull_request(context, &github, &runtime, &plan, &remote, draft)?);
    }
    return Ok(output);
  }

  if let Some(pull_request) = &plan.pull_request {
    let (start, robot) = start_from_pull_request(context, &github, &service, &runtime, &plan, pull_request)?;
    output.pull_request = Some(start);
    output.robot = robot.map(|robot| robot_summary(context, &robot));
    output.started_via = Some("pull_request".to_string());
  } else {
    let merge_request = plan.review.as_ref().and_then(|review| review.merge_request.clone());
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
  let space =
    find_space_repository(&context.git, &context.repository)?.ok_or_else(|| CliError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't push it"))?;
  let target = find_target_branch(&context.git, &context.repository, &synced.baseline)?.ok_or_else(|| {
    CliError::new(
      ErrorCode::NotOnSpace,
      format!("The baseline {} is not a branch of origin: Patronus pushes into a branch of origin", synced.baseline),
    )
  })?;
  let github = resolve_github_repository(&context.git, &context.repository)?;

  // the stack: the branches it is built on, root first
  let grouping = read_grouping(context, None)?;
  let parent_of = |name: &str| {
    grouping
      .placements
      .as_ref()
      .and_then(|placements| placements.get(name))
      .and_then(|placement| placement.effective_base.clone())
  };
  let ancestors_of = |name: &str| {
    let mut ancestors = Vec::new();
    let mut parent = parent_of(name);
    while let Some(ancestor) = parent {
      if ancestors.contains(&ancestor) {
        break;
      }
      parent = parent_of(&ancestor);
      ancestors.push(ancestor);
    }
    ancestors.reverse();
    ancestors
  };
  let ancestors = ancestors_of(name);
  let order: Vec<String> = grouping
    .forest
    .as_ref()
    .map_or_else(|| synced.branches.iter().map(|branch| branch.name.clone()).collect(), |forest| forest.order.clone());

  // the robots of the user, each matched to its branch as the app does
  let robots = robots(runtime, service, &space, &context.prefix)?;
  let tips = branch_tips(&context.git, &context.repository, &context.prefix)?;
  let synced_by_name: HashMap<&str, &SyncedBranch> = synced.branches.iter().map(|branch| (branch.name.as_str(), branch)).collect();
  let tip_of = |name: &str| to_final_branch_name(&context.prefix, name).ok().and_then(|full_name| tips.get(&full_name).cloned());
  let remote_tip_of = |name: &str| synced_by_name.get(name).and_then(|branch| branch.remote.as_ref()).and_then(|remote| remote.head_id.clone());
  let robot_of = |name: &str| find_branch_robot(&robots, name, tip_of(name).as_deref(), remote_tip_of(name).as_deref());
  let local_tip = tip_of(name);
  let robot = robot_of(name);

  let mut refusals = Vec::new();
  let pushes = mode != PatronusPushMode::DryRun;
  // a dry run lands nothing: it waits for no robot of the stack
  let landing_ancestor = if pushes { find_landing_ancestor(&ancestors, robot_of) } else { None };
  let landing_descendant = if pushes {
    find_landing_descendant(name, order.iter().map(String::as_str), ancestors_of, robot_of)
  } else {
    None
  };
  if let Some(message) = start_refusal(robot, local_tip.as_deref(), landing_ancestor.as_ref(), landing_descendant.as_ref(), name) {
    refusals.push(Refusal {
      code: ErrorCode::RobotRunning,
      message,
    });
  }

  let push_needed = std::iter::once(name)
    .chain(ancestors.iter().map(String::as_str))
    .any(|name| synced_by_name.get(name).is_none_or(|branch| needs_push(branch.remote.as_ref())));
  let risky_robot = robot
    .filter(|robot| is_push_into_robot_risky(robot, name) && robot.source_revision.as_deref() != local_tip.as_deref())
    .cloned();
  if let Some(robot) = &risky_robot
    && push_needed
    && !args.cancel_running
  {
    refusals.push(Refusal {
      code: ErrorCode::RobotRunning,
      message: format!(
        "A {} of an older commit of {name} is running: the push fails it at the end. Pass --cancel-running to cancel it first ({})",
        push_mode_name(robot.push_mode),
        robot.url
      ),
    });
  }
  if pushes && !ancestors.is_empty() && !args.land_stack && !args.create_pull_request {
    refusals.push(Refusal {
      code: ErrorCode::StackNotLanded,
      message: format!(
        "{name} is built on {}, which aren't in {target} yet. Safe Push {} first, then the next: after a merge, the next sync moves the branches built on it onto {target}. Or pass --land-stack: the Safe Push of {name} lands {} with it",
        join_names(&ancestors),
        ancestors[0],
        join_names(&ancestors)
      ),
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
    let requirement = review_requirement(context, service, runtime, &space, &target, &full_name, &synced.baseline, github.is_some())?;
    // the robot of a pull request counts its approvals, which the check before it can't see
    let approved_on_github = pull_request.as_ref().is_some_and(|pull_request| !github_approvers(pull_request).is_empty());
    if requirement.required && !approved_on_github && !args.without_approvals && !args.create_pull_request {
      refusals.push(Refusal {
        code: ErrorCode::ApprovalsMissing,
        message: approvals_missing_message(name, &requirement, github.as_ref(), pull_request.as_ref()),
      });
    }
    Some(requirement)
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
    risky_robot,
  })
}

/// The service of `bd`: the token of the Safe Push CLI (`./safePush.cmd`), else `SPACE_TOKEN`; `not_logged_in` without
/// either, before anything else runs
fn patronus_service(environment: &Environment) -> Result<PatronusService, CliError> {
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

fn unix_now() -> u32 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |elapsed| u32::try_from(elapsed.as_secs()).unwrap_or(u32::MAX))
}

/// The user's robots of the last 7 days in the repository
fn robots(runtime: &Runtime, service: &PatronusService, space: &SpaceRepository, prefix: &str) -> Result<Vec<PatronusRobot>, CliError> {
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
fn patronus_error(message: String) -> CliError {
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
fn github_service(context: &RepoContext, environment: &Environment) -> GitHubService {
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
fn github_error(failure: &GitHubFailure) -> CliError {
  let code = match failure {
    GitHubFailure::SignedOut
    | GitHubFailure::Failed {
      error: GitHubError::Unauthorized, ..
    } => ErrorCode::NotLoggedIn,
    GitHubFailure::Failed { .. } => ErrorCode::NetworkError,
  };
  CliError::new(code, failure_message(failure))
}

/// The review of the commits of the virtual branch: one `git log`, the pre-check (asked again, never a cached answer),
/// then the `OWNERSHIP` files for the groups Patronus names without their files
#[expect(clippy::too_many_arguments, reason = "the facts of one check")]
fn review_requirement(
  context: &RepoContext,
  service: &PatronusService,
  runtime: &Runtime,
  space: &SpaceRepository,
  target: &str,
  full_name: &str,
  baseline: &str,
  reviewed_on_github: bool,
) -> Result<PatronusReviewRequirement, CliError> {
  let Some(input) = read_review_input(&context.git, &context.repository, baseline, &format!("refs/heads/{full_name}"))? else {
    return Ok(skipped_review());
  };
  let subject = ReviewSubject {
    project_key: &space.project_key,
    repository: &space.name,
    target_branch: target,
    source_branch: full_name,
    base: input.base(),
    commits: &input.commits,
    reviewed_on_github,
  };
  let mut requirement = runtime.block_on(service.review_requirement(&subject, true)).map_err(patronus_error)?;
  attribute_ownership_paths(&context.git, &context.repository, baseline, &mut requirement, &input.paths);
  Ok(requirement)
}

/// The GitHub repository of the branches is not known: a pull request can't be opened, its approvals can't count
const UNKNOWN_GITHUB_REPOSITORY: &str =
  "The GitHub repository of the branches is unknown: the Space settings name no GitHub mirror; run `git config branchdeck.githubRepository <owner>/<name>`";

/// "The code owners haven't approved auth: …", with the groups and where the approvals count: the GitHub pull request
/// of the branch, or a Space merge request found open
fn approvals_missing_message(name: &str, requirement: &PatronusReviewRequirement, github: Option<&GitHubRepository>, pull_request: Option<&PullRequest>) -> String {
  let groups = requirement.groups.iter().map(format_reviewer_group).collect::<Vec<_>>().join(", ");
  let needed = if groups.is_empty() { String::new() } else { format!(" Needs review by {groups}.") };
  let counts = match (github, pull_request, &requirement.merge_request) {
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
    "The code owners haven't approved {name}: a Safe Push fails without these approvals. Nothing was started.{needed} {counts}. `review` lists the groups and their files; show them to the user, then run again once approved, or with --without-approvals to start anyway"
  )
}

/// The reviewers whose latest review of the pull request approves it
fn github_approvers(pull_request: &PullRequest) -> Vec<String> {
  pull_request
    .reviewers
    .iter()
    .filter(|reviewer| reviewer.state == ReviewerState::Approved)
    .map(|reviewer| reviewer.login.clone())
    .collect()
}

/// Opens the pull request of the pushed branch, as the `gh pr create` that `bd push` prints: into its parent's branch
/// or the target, with the `bd describe` text, else filled from the commits as `--fill` does (one commit: its subject
/// and body; several: the branch name and their subjects). A pull request that already exists is found instead.
fn create_pull_request(
  context: &RepoContext,
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
    None => fill_pull_request(context, remote, draft)?,
  };
  let request = serde_json::json!({"title": title, "body": body, "head": draft.head, "base": draft.base});
  let path = ["repos", repository.owner.as_str(), repository.name.as_str(), "pulls"];
  let (path, request) = (&path, &request);
  let created = runtime.block_on(github.ask(&context.repository, &repository.full_name(), RequestKind::Single, |client, token| async move {
    client.post::<GhCreatedPullRequest, _>(&token, path, request).await
  }));
  match created {
    Ok(created) => Ok(PullRequestStart {
      number: created.number,
      url: created.html_url,
      created: true,
      comment_url: None,
      reply: None,
    }),
    Err(GitHubFailure::Failed {
      error: GitHubError::Http { status: 422, message },
      ..
    }) if message.contains("already exists") => {
      let name = draft.head.rsplit("/virtual/").next().unwrap_or(&draft.head);
      let existing = open_pull_request(context, github, runtime, repository, name, &draft.base)
        .ok_or_else(|| CliError::new(ErrorCode::NetworkError, format!("{message}, but GitHub does not list it yet")))?;
      Ok(PullRequestStart {
        number: existing.number,
        url: existing.url,
        created: false,
        comment_url: None,
        reply: None,
      })
    }
    Err(failure) => Err(github_error(&failure)),
  }
}

/// The title and body `gh pr create --fill` takes from the commits `<remote>/<base>..<head>`: one `git log`
fn fill_pull_request(context: &RepoContext, remote: &str, draft: &PullRequestDraft) -> Result<(String, String), CliError> {
  let range = format!("refs/remotes/{remote}/{}..refs/heads/{}", draft.base, draft.head);
  let log = context
    .git
    .execute_command_raw(&["log", "--reverse", "--format=%s%x1f%b%x1e", &range, "--"], &context.repository)?;
  let commits: Vec<(&str, &str)> = log
    .split('\x1e')
    .map(|record| record.trim_start_matches('\n'))
    .filter(|record| !record.is_empty())
    .map(|record| record.split_once('\x1f').unwrap_or((record, "")))
    .collect();
  match commits.as_slice() {
    [] => Err(CliError::new(
      ErrorCode::PushRefused,
      format!("No commits between {remote}/{} and {}", draft.base, draft.head),
    )),
    [(subject, body)] => Ok(((*subject).to_string(), body.trim().to_string())),
    commits => {
      let name = draft.head.rsplit("/virtual/").next().unwrap_or(&draft.head);
      let body = commits.iter().map(|(subject, _)| format!("- **{subject}**")).collect::<Vec<_>>().join("\n");
      Ok((name.replace(['-', '_'], " "), body))
    }
  }
}

/// The answer to `POST /repos/{owner}/{repo}/pulls`
#[derive(serde::Deserialize)]
struct GhCreatedPullRequest {
  number: u32,
  html_url: String,
}

/// "Air Plugin Release Team (suggested: Alice)", as the Safe Push form names a group
fn format_reviewer_group(group: &PatronusReviewerGroup) -> String {
  let suggested: Vec<&str> = group
    .reviewers
    .iter()
    .filter(|reviewer| reviewer.suggested && reviewer.available)
    .map(|reviewer| reviewer.full_name.as_str())
    .collect();
  if suggested.is_empty() {
    group.name.clone()
  } else {
    format!("{} (suggested: {})", group.name, suggested.join(", "))
  }
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

fn mode_name(mode: PatronusPushMode) -> String {
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

fn review_summary(requirement: &PatronusReviewRequirement, reviewed_on_github: bool, pull_request: Option<&PullRequest>) -> ReviewSummary {
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
            available: reviewer.available,
            suggested: reviewer.suggested,
          })
          .collect(),
        paths: group.paths.clone(),
        branch_wide: group.branch_wide,
      })
      .collect(),
    approved_on_github: pull_request.map_or_else(Vec::new, github_approvers),
  }
}

fn robot_summary(context: &RepoContext, robot: &PatronusRobot) -> RobotSummary {
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

/// "auth", "auth and login", "auth, login and oauth"
fn join_names(names: &[String]) -> String {
  match names {
    [] => String::new(),
    [only] => only.clone(),
    [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
  }
}
