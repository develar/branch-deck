//! `bd robot`: the status of the Patronus robot of a virtual branch, as `bd safe-push` finds it, for an agent that
//! watches a robot the user started or restarted, and, when it failed or fails while it runs, why and which commits of
//! the checked-out branch it tested (`spec/branch/safe-push-failure-prompt.spec.md`): the same failure and commit map
//! as the app's Copy Prompt for Agent. Read only: no repository lock and no sync; the commit map compares the virtual
//! branch as the last sync built it.

use crate::robot_list::list_robots;
use crate::safe_push_command::{RobotSummary, patronus_error, patronus_service, robot_summary};
use bd_core::branch_view::read_grouping;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::{CliError, ErrorCode};
use bd_core::output::{SchemaVersion, ShortIds};
use bd_core::stack_summary::{WholeStack, whole_stack_of};
use git_ops::branch_names::to_final_branch_name;
use patronus::service::PatronusService;
use patronus::teamcity_cli::{PROCESS_TIMEOUT, TeamCityCli};
use repository_view::branch_table::stack_ancestors;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use sync_core::patronus_start::{find_branch_robot, find_stack_robot, is_robot_active};
use sync_core::robot_commits::robot_failure_report;
use sync_core::stack_branch::read_stack_branches;
use sync_types::patronus::{PatronusRobot, PatronusRobotDetails, PatronusRobotStatus};
use sync_types::robot_failure::{FailedBuild, FailedCheck, OmittedFailures, RobotCommit, RobotCommitState, RobotFailure, RobotFailureKind, RobotFailureProblem};
use tokio::runtime::Runtime;

#[derive(Debug, Default, clap::Args)]
pub struct RobotArgs {
  /// The branch the robot runs on
  pub branch: String,
  /// Also the failed builds of a failure: their problems, failed tests with messages and, through the `teamcity` CLI
  /// when it is logged in, the log tails; a few seconds
  #[arg(long)]
  pub full: bool,
  /// This robot instead of the newest one of the branch
  #[arg(long, value_name = "ID")]
  pub robot: Option<String>,
  /// The robot of the whole stack of the branch, as `bd safe-push --whole-stack` starts it: on the stack branch
  /// `{prefix}/stack/<root>` of its top-level root, or for a straight chain on its last branch
  #[arg(long)]
  pub whole_stack: bool,
}

/// `bd robot`
#[derive(Debug, Serialize, JsonSchema)]
pub struct RobotOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub branch: String,
  /// The robot of the branch (a running one first, then the newest of the last 7 days), or the one of `--robot`;
  /// `null` when the branch has none. A newer robot of a branch stacked on this one, or of the whole stack, wins: it
  /// builds this branch too. With `--whole-stack`, the robot of the whole stack
  pub robot: Option<RobotState>,
  /// The branch whose robot `robot` is, when a branch stacked on this one ran it; left out for the branch's own
  #[serde(skip_serializing_if = "Option::is_none")]
  pub robot_branch: Option<String>,
  /// The root of the stack whose stack branch `robot` runs on (`{prefix}/stack/<root>`): a Safe Push of the whole stack,
  /// which builds every branch of it; left out for a robot of a branch
  #[serde(skip_serializing_if = "Option::is_none")]
  pub stack_root: Option<String>,
  /// Why the robot failed, or fails while it runs (a required check failed); left out otherwise
  #[serde(skip_serializing_if = "Option::is_none")]
  pub failure: Option<FailureOutput>,
  /// With a failure: the commits of the branch against those the robot tested, newest first, by the ids of the
  /// checked-out branch (the agent fixes those; no id the CI reports is local); left out for a robot of the whole stack,
  /// whose stack branch has no mapping notes
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub commits: Vec<RobotCommitOutput>,
  /// The robot's commit is not in the repository (started on another machine, or pruned): every commit is `unknown`
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub source_missing: bool,
  /// The checked-out branch has commits of the branch that the virtual branch lacks (not synced since): they are not
  /// in `commits`
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub virtual_behind_head: bool,
  /// With a failure: the checked-out branch, whose commits `commits` names; left out when HEAD is detached
  #[serde(skip_serializing_if = "Option::is_none")]
  pub head_branch: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RobotState {
  #[serde(flatten)]
  pub summary: RobotSummary,
  /// Not finished yet (`running` or `canceling`): run `bd robot` again later
  pub running: bool,
  /// The merge profile, which decides the checks; left out for the default checks of the target
  #[serde(skip_serializing_if = "Option::is_none")]
  pub profile: Option<RobotProfile>,
  /// The branch of `origin` the robot pushes into, e.g. `master`
  pub target: String,
  /// Unix seconds
  pub started_at: u32,
  /// Unix seconds; left out while it runs
  #[serde(skip_serializing_if = "Option::is_none")]
  pub finished_at: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cancellation_reason: Option<String>,
  /// The GitHub pull request whose `/patronus` comment started the robot
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pull_request: Option<RobotPullRequest>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RobotProfile {
  /// What `bd safe-push --profile` takes
  pub id: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RobotPullRequest {
  pub number: u32,
  pub url: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FailureOutput {
  pub kind: FailureKind,
  /// The commit `TeamCity` built: the robot's commit rebased onto the target; a full id, not a local commit
  #[serde(skip_serializing_if = "Option::is_none")]
  pub test_revision: Option<String>,
  /// The commit of the target the robot rebased onto, a full id
  #[serde(skip_serializing_if = "Option::is_none")]
  pub target_revision: Option<String>,
  /// The failed checks by their last attempt; of a running robot also the running checks with a failed build
  pub checks: Vec<FailedCheckOutput>,
  /// The problems of severity error, e.g. a conflict of the rebase onto the target
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<ProblemOutput>,
  /// With `--full`: the failed builds of the failed checks (never a composite one), those that failed not only on tests
  /// first
  #[serde(skip_serializing_if = "Option::is_none")]
  pub builds: Option<Vec<FailedBuildOutput>>,
  /// What the limits left out (5 builds, 40 test names, 20 test messages, each list whole when only one more; cut
  /// texts)
  pub omitted: OmittedOutput,
}

/// What failed, which decides the first step
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
  /// Checks failed: fix the branch
  Checks,
  /// Every failed check only failed to start its builds: the CI, not the branch; the user restarts the robot
  Infrastructure,
  /// Patronus detected a massive failure of tests or builds: the branch may break a core module, or the target is
  /// broken
  Massive,
  /// No check failed, but Patronus reports an error, e.g. the rebase onto the target conflicted (`bd update`)
  Patronus,
  /// Nothing failed and no error is reported: open the robot page
  Unknown,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FailedCheckOutput {
  /// e.g. "Fast Tests"
  pub name: String,
  /// The `TeamCity` build configuration
  #[serde(skip_serializing_if = "Option::is_none")]
  pub build_configuration_id: Option<String>,
  /// The number of the last attempt, from 1
  pub attempt: u32,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub attempt_limit: Option<u32>,
  /// The `TeamCity` build id of the attempt (not the build number): `teamcity run log <build_id>`
  pub build_id: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub build_url: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub failed_tests: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub failed_builds: Option<u32>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub failed_to_start: Option<u32>,
  /// Still running: more builds of it may fail
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub running: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProblemOutput {
  pub title: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub subsystem: Option<String>,
  /// Markdown, cut to 4 KB
  #[serde(skip_serializing_if = "Option::is_none")]
  pub details: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FailedBuildOutput {
  /// The build configuration name, e.g. "Compile"
  pub name: String,
  /// e.g. "IntelliJ / Tests"
  #[serde(skip_serializing_if = "Option::is_none")]
  pub project: Option<String>,
  pub build_id: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub build_url: Option<String>,
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub failed_to_start: bool,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub problems: Vec<BuildProblemOutput>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub failed_tests: Vec<FailedTestOutput>,
  /// The last messages of the build log, through the `teamcity` CLI; left out without it, and for a build that failed
  /// only on tests
  #[serde(skip_serializing_if = "Option::is_none")]
  pub log_tail: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BuildProblemOutput {
  /// e.g. `TC_COMPILATION_ERROR`, `TC_EXIT_CODE`, `TC_FAILED_TESTS`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub kind: Option<String>,
  pub details: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FailedTestOutput {
  /// e.g. `com.intellij.FooTest.testBar`
  pub name: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub url: Option<String>,
  /// The failure message and stack trace, cut to 60 lines; left out beyond the messages read
  #[serde(skip_serializing_if = "Option::is_none")]
  pub message: Option<String>,
  /// The test failed in an earlier build already (`TeamCity`'s `firstFailed`, through the `teamcity` CLI): likely not
  /// caused by the branch; left out when unknown
  #[serde(skip_serializing_if = "Option::is_none")]
  pub first_failed_before: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OmittedOutput {
  /// Failed builds beyond the limit: never 1, a list one longer than its limit is given whole
  pub builds: u32,
  /// Failed tests beyond the limit of names: never 1, as `builds`
  pub tests: u32,
  /// Failed tests listed without their message
  pub messages: u32,
  /// Some text (a message, problem details, a log) was cut
  pub truncated: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RobotCommitOutput {
  /// The commit of the checked-out branch; left out for a commit only the robot had (`removed`, `landed`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub commit: Option<String>,
  /// The subject of the commit of the checked-out branch, with its `(prefix)`; of a commit only the robot had, without
  pub subject: String,
  /// Unix seconds
  pub author_time: u32,
  /// The virtual branch of the commit: the robot's, or an ancestor of a stacked branch (its robot builds the whole
  /// stack); left out for a commit only the robot had
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  pub state: CommitState,
}

/// A commit of the branch against the robot's commits
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CommitState {
  /// The robot tested this commit as it is
  Tested,
  /// A copy on a newer baseline with the same changes: tested
  Rebased,
  /// Changed (amended) since the robot ran: not tested as it is
  Changed,
  /// Added after the robot ran: not tested
  Added,
  /// The robot had it, the branch no longer has it
  Removed,
  /// The robot had it, and the baseline has it now
  Landed,
  /// The robot's commits are unknown (`source_missing`)
  Unknown,
}

pub fn robot(context: &RepoContext, args: &RobotArgs, environment: &Environment) -> Result<RobotOutput, CliError> {
  let name = args.branch.as_str();
  // the name must be one a virtual branch can have
  to_final_branch_name(&context.prefix, name).map_err(|e| CliError::new(ErrorCode::InvalidArguments, format!("{e:#}")))?;
  let service = patronus_service(environment)?;
  let space = context
    .space_repository()?
    .ok_or_else(|| CliError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus has no robots of it"))?;
  let runtime = context.runtime()?;
  let grouping = read_grouping(context, None)?;
  let has_row = grouping.branches.iter().any(|branch| branch.name == name);
  let mut output = RobotOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    robot: None,
    robot_branch: None,
    stack_root: None,
    failure: None,
    commits: Vec::new(),
    source_missing: false,
    virtual_behind_head: false,
    head_branch: None,
  };

  // the branch and those stacked on it: the robot of a descendant builds this branch too
  let descendants: Vec<&str> = grouping
    .branches
    .iter()
    .map(|branch| branch.name.as_str())
    .filter(|candidate| stack_ancestors(&grouping.branches, candidate).iter().any(|ancestor| ancestor == name))
    .collect();
  // the stack branches as they stand, without a sync (no git): the whole stack of a stack that forks
  let stacks = if has_row {
    read_stack_branches(&context.prefix, &grouping, &context.cache(), context.config()?.resolve_mode()?)
  } else {
    Vec::new()
  };
  let whole = if args.whole_stack {
    Some(whole_stack_of(&grouping.branches, &stacks, name)?)
  } else {
    None
  };
  // the stack that forks the branch is a branch of: the robot of the whole stack builds it too
  let stack = stacks.iter().find(|stack| stack.members.iter().any(|member| member == name));
  let robot_id = if let Some(id) = &args.robot {
    id.clone()
  } else {
    let robots = list_robots(context, &service, &space)?;
    let local_tips: HashMap<&str, &str> = grouping.listed.iter().map(|branch| (branch.name.as_str(), branch.tip.as_str())).collect();
    let robot_of = |branch: &str| {
      let full = to_final_branch_name(&context.prefix, branch).ok()?;
      find_branch_robot(&robots, branch, local_tips.get(full.as_str()).copied(), grouping.remote_tips.get(&full).map(String::as_str))
    };
    let stack_robot = stack.and_then(|stack| find_stack_robot(&robots, &stack.root, stack.tip.as_deref(), grouping.remote_tips.get(&stack.branch_name).map(String::as_str)));
    let found = match &whole {
      Some(WholeStack::Forking(_)) => stack_robot.map(|robot| (None, robot)),
      Some(WholeStack::Chain { leaf, .. }) => robot_of(leaf).map(|robot| ((leaf != name).then_some(leaf.as_str()), robot)),
      None => {
        let own = robot_of(name);
        // the newest robot of a descendant or of the whole stack, when it started after the branch's own
        descendants
          .iter()
          .filter_map(|branch| robot_of(branch).map(|robot| (Some(*branch), robot)))
          .chain(stack_robot.map(|robot| (None, robot)))
          .filter(|(_, robot)| own.is_none_or(|own| robot.start_time > own.start_time))
          .max_by_key(|(_, robot)| robot.start_time)
          .or_else(|| own.map(|robot| (None, robot)))
      }
    };
    match found {
      Some((branch, robot)) => {
        output.robot_branch = branch.map(str::to_string);
        robot.id.clone()
      }
      None if has_row => return Ok(output),
      // a branch that landed has no row, but its robot still tells so: a name with neither is unknown
      None => return Err(unknown_branch(name, &grouping.baseline_branch)),
    }
  };
  let details = runtime
    .block_on(service.robot_details(&context.prefix, &robot_id, context.patronus_new_api))
    .map_err(patronus_error)?;
  check_robot_of(&details.robot, name, &descendants, stack.map(|stack| stack.root.as_str()), whole.as_ref())?;
  if let Some(other) = details.robot.branch_name.as_deref().filter(|other| *other != name) {
    output.robot_branch = Some(other.to_string());
  }
  output.stack_root.clone_from(&details.robot.stack_root);
  output.robot = Some(robot_state(context, &details));

  let Some(mut failure) = failure(runtime, &service, context, &details, args.full, environment)? else {
    return Ok(output);
  };
  // a robot of a temporary ref (the IDE, the CLI) was matched by the tip of this branch: its commits are this branch's.
  // A robot of the whole stack built the commits of every member of it
  let ancestors = if let Some(root) = failure.robot.stack_root.as_deref() {
    stacks.iter().find(|stack| stack.root == root).map_or_default(|stack| stack.members.clone())
  } else {
    let robot_branch = failure.robot.branch_name.get_or_insert_with(|| name.to_string()).clone();
    stack_ancestors(&grouping.branches, &robot_branch)
  };
  let report = robot_failure_report(&context.git, &context.repository, &context.prefix, &ancestors, &grouping.baseline_branch, failure)?;
  if let Some(map) = report.commits {
    let short = context.short_ids(map.commits.iter().filter_map(|commit| commit.local_id.as_deref()));
    output.commits = map.commits.iter().map(|commit| commit_output(commit, &short)).collect();
    output.source_missing = map.source_missing;
    output.virtual_behind_head = map.virtual_behind_head;
  }
  output.head_branch = report.head_branch;
  output.failure = Some(failure_output(report.failure));
  Ok(output)
}

/// A robot of `--robot` must build the branch: its own, of a branch stacked on it, or of the whole stack it is a branch
/// of (`stack_root`); with `--whole-stack`, the robot of the whole stack
fn check_robot_of(robot: &PatronusRobot, name: &str, descendants: &[&str], stack_root: Option<&str>, whole: Option<&WholeStack<'_>>) -> Result<(), CliError> {
  let id = &robot.id;
  let builds = match (&robot.stack_root, &robot.branch_name, whole) {
    (Some(root), _, Some(WholeStack::Forking(update))) => *root == update.root,
    (Some(root), _, _) => stack_root == Some(root.as_str()),
    (None, Some(other), Some(WholeStack::Chain { leaf, .. })) => other == leaf,
    (None, Some(_), Some(WholeStack::Forking(_))) => false,
    (None, Some(other), None) => other == name || descendants.contains(&other.as_str()),
    // a temporary ref, matched by its tip
    (None, None, _) => true,
  };
  if builds {
    return Ok(());
  }
  let message = match (&robot.stack_root, &robot.branch_name) {
    (Some(root), _) => format!("Robot {id} runs on the whole {root} stack, not on {name}: `bd robot {root} --whole-stack --robot {id}`"),
    (None, Some(other)) => format!("Robot {id} runs on {other}, not on {name}: `bd robot {other} --robot {id}`"),
    (None, None) => format!("Robot {id} does not run on {name}"),
  };
  Err(CliError::new(ErrorCode::InvalidArguments, message))
}

fn unknown_branch(name: &str, baseline: &str) -> CliError {
  CliError::new(
    ErrorCode::UnknownBranch,
    format!("No branch named {name:?} among the commits ahead of {baseline}, and no robot of it"),
  )
}

/// The failure of a robot that failed, or that runs with a failed required check; `None` otherwise. Fast from the
/// details just read (no request), or with `full` the failed builds too
fn failure(
  runtime: &Runtime,
  service: &PatronusService,
  context: &RepoContext,
  details: &PatronusRobotDetails,
  full: bool,
  environment: &Environment,
) -> Result<Option<RobotFailure>, CliError> {
  let robot = &details.robot;
  let fast = runtime.block_on(service.robot_failure(&context.prefix, &robot.id));
  let failed = match robot.status {
    PatronusRobotStatus::Failure => true,
    PatronusRobotStatus::Running => details.failing || fast.as_ref().is_ok_and(|failure| !failure.checks.is_empty()),
    _ => false,
  };
  if !failed {
    return Ok(None);
  }
  if !full {
    return fast.map(Some).map_err(patronus_error);
  }
  let teamcity = runtime.block_on(async {
    match &environment.teamcity {
      Some(binary) => TeamCityCli::open(binary.clone(), PROCESS_TIMEOUT).await,
      None => TeamCityCli::discover().await,
    }
  });
  runtime
    .block_on(service.robot_failure_full(&context.prefix, &robot.id, teamcity.as_ref()))
    .map(Some)
    .map_err(patronus_error)
}

fn robot_state(context: &RepoContext, details: &PatronusRobotDetails) -> RobotState {
  let robot = &details.robot;
  RobotState {
    summary: robot_summary(context, robot),
    running: is_robot_active(robot),
    profile: details.profile_id.as_ref().map(|id| RobotProfile {
      id: id.clone(),
      name: details.profile_name.clone(),
    }),
    target: robot.target_branch.clone(),
    started_at: robot.start_time,
    finished_at: robot.finish_time,
    cancellation_reason: robot.cancellation_reason.clone(),
    pull_request: robot.pull_request.as_ref().map(|link| RobotPullRequest {
      number: link.number,
      url: link.url.clone(),
    }),
  }
}

fn failure_output(failure: RobotFailure) -> FailureOutput {
  let RobotFailure {
    kind,
    test_revision,
    target_revision,
    checks,
    problems,
    builds,
    omitted,
    ..
  } = failure;
  FailureOutput {
    kind: match kind {
      RobotFailureKind::Checks => FailureKind::Checks,
      RobotFailureKind::Infrastructure => FailureKind::Infrastructure,
      RobotFailureKind::Massive => FailureKind::Massive,
      RobotFailureKind::Patronus => FailureKind::Patronus,
      RobotFailureKind::Unknown => FailureKind::Unknown,
    },
    test_revision,
    target_revision,
    checks: checks.into_iter().map(check_output).collect(),
    problems: problems.into_iter().map(problem_output).collect(),
    builds: builds.map(|builds| builds.into_iter().map(build_output).collect()),
    omitted: omitted_output(&omitted),
  }
}

fn check_output(check: FailedCheck) -> FailedCheckOutput {
  FailedCheckOutput {
    name: check.name,
    build_configuration_id: check.build_configuration_id,
    attempt: check.attempt,
    attempt_limit: check.attempt_limit,
    build_id: check.build_id,
    build_url: check.build_url,
    failed_tests: check.failed_tests,
    failed_builds: check.failed_builds,
    failed_to_start: check.failed_to_start,
    running: check.running,
  }
}

fn problem_output(problem: RobotFailureProblem) -> ProblemOutput {
  ProblemOutput {
    title: problem.title,
    subsystem: problem.subsystem,
    details: problem.details,
  }
}

fn build_output(build: FailedBuild) -> FailedBuildOutput {
  FailedBuildOutput {
    name: build.name,
    project: build.project,
    build_id: build.build_id,
    build_url: build.build_url,
    failed_to_start: build.failed_to_start,
    problems: build
      .problems
      .into_iter()
      .map(|problem| BuildProblemOutput {
        kind: problem.kind,
        details: problem.details,
      })
      .collect(),
    failed_tests: build
      .failed_tests
      .into_iter()
      .map(|test| FailedTestOutput {
        name: test.name,
        url: test.url,
        message: test.message,
        first_failed_before: test.first_failed_before,
      })
      .collect(),
    log_tail: build.log_tail,
  }
}

const fn omitted_output(omitted: &OmittedFailures) -> OmittedOutput {
  OmittedOutput {
    builds: omitted.builds,
    tests: omitted.tests,
    messages: omitted.messages,
    truncated: omitted.truncated,
  }
}

fn commit_output(commit: &RobotCommit, short: &ShortIds) -> RobotCommitOutput {
  RobotCommitOutput {
    commit: commit.local_id.as_deref().map(|id| short.get(id).to_string()),
    subject: commit.subject.clone(),
    author_time: commit.author_time,
    branch: commit.branch.clone(),
    state: match commit.state {
      RobotCommitState::Tested => CommitState::Tested,
      RobotCommitState::Rebased => CommitState::Rebased,
      RobotCommitState::Changed => CommitState::Changed,
      RobotCommitState::Added => CommitState::Added,
      RobotCommitState::Removed => CommitState::Removed,
      RobotCommitState::Landed => CommitState::Landed,
      RobotCommitState::Unknown => CommitState::Unknown,
    },
  }
}
