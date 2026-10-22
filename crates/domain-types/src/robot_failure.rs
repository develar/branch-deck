//! Why a Patronus robot failed, and which commits of the branch it checked: what the prompt for an agent says
//! (`spec/branch/safe-push-failure-prompt.spec.md`). The app and `bd robot` build it with the same code.

use crate::patronus::PatronusRobot;
use serde::{Deserialize, Serialize};

/// A failed or failing robot as an agent needs it to fix the branch
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotFailure {
  pub robot: PatronusRobot,
  /// Still running (a required check failed, `robot_is_failing`): more checks may fail
  pub running: bool,
  /// The commit `TeamCity` built: the source revision rebased onto the target (`testReferenceRevision`); null before the
  /// rebase or when Patronus doesn't say
  pub test_revision: Option<String>,
  /// The commit of the target branch the robot rebased onto (`targetRevision`)
  pub target_revision: Option<String>,
  pub kind: RobotFailureKind,
  /// The failed checks, the last attempt of each; for a running robot also the running checks with a failed build
  pub checks: Vec<FailedCheck>,
  /// The problems of severity error, e.g. a conflict of the rebase
  pub problems: Vec<RobotFailureProblem>,
  /// The failed builds of the checks with their problems and failed tests: only the full version, null in the fast one
  pub builds: Option<Vec<FailedBuild>>,
  /// What the limits left out
  pub omitted: OmittedFailures,
}

/// What failed, which decides the first step: a restart for builds that never started, a fix otherwise
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RobotFailureKind {
  /// Checks failed
  Checks,
  /// Every failed check failed only to start its builds and ran no failing test: the CI, not the branch; a restart is
  /// the first step
  Infrastructure,
  /// Patronus detected a massive failure of tests or builds (`MASSIVE_*_FAILURE_DETECTOR`): the branch may break a
  /// core module, or the target is broken
  Massive,
  /// No check failed, but Patronus reports an error, e.g. the rebase onto the target conflicted
  Patronus,
  /// Nothing failed and no error is reported
  Unknown,
}

/// A failed check, by its last attempt
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FailedCheck {
  /// The profile name of the check, else its build configuration name, e.g. "Fast Tests"
  pub name: String,
  /// The `TeamCity` build configuration, e.g. `BazelCommunityBuild_Aggregator`
  pub build_configuration_id: Option<String>,
  /// The number of the attempt, from 1
  pub attempt: u32,
  pub attempt_limit: Option<u32>,
  /// The `TeamCity` build of the attempt (the build id, not the build number)
  pub build_id: String,
  pub build_url: Option<String>,
  pub failed_tests: Option<u32>,
  pub failed_builds: Option<u32>,
  pub failed_to_start: Option<u32>,
  /// Still running: the attempt has a failed build, others may follow
  pub running: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotFailureProblem {
  pub title: String,
  pub subsystem: Option<String>,
  /// Markdown, cut to the limit
  pub details: Option<String>,
}

/// A failed build of the dependency chain of a check (never a composite one)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FailedBuild {
  /// The build configuration name, e.g. "Compile"
  pub name: String,
  /// The full project name, e.g. "IntelliJ / Tests"
  pub project: Option<String>,
  pub build_id: String,
  pub build_url: Option<String>,
  pub failed_to_start: bool,
  pub problems: Vec<BuildProblem>,
  pub failed_tests: Vec<FailedTest>,
  /// The last messages of the build log, through the `teamcity` CLI; null without it, or for a build that failed only
  /// on tests
  pub log_tail: Option<String>,
}

/// A problem TeamCity reports for a build, e.g. a compilation error
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BuildProblem {
  /// e.g. `TC_COMPILATION_ERROR`, `TC_EXIT_CODE`, `TC_FAILED_TESTS`
  pub kind: Option<String>,
  /// Cut to the limit
  pub details: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FailedTest {
  /// The full name, e.g. `com.intellij.FooTest.testBar`
  pub name: String,
  pub url: Option<String>,
  /// The failure message and stack trace, cut to the limit; null beyond the number of messages read
  pub message: Option<String>,
  /// The test failed in an earlier build than this one (`TeamCity`'s `firstFailed`): likely not caused by the branch;
  /// null when unknown
  pub first_failed_before: Option<bool>,
}

/// What the limits of a failure left out
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OmittedFailures {
  /// Failed builds beyond the limit
  pub builds: u32,
  /// Failed tests beyond the limit of names
  pub tests: u32,
  /// Failed tests listed without their message
  pub messages: u32,
  /// Some text (a message, problem details, a log) was cut
  pub truncated: bool,
}

/// The commits of a branch against those its robot checked, by the hashes of the checked-out branch: the virtual
/// branch copies the commits, Patronus rebases them, so no hash `TeamCity` reports is local. A stacked branch has the
/// commits of its ancestors too: its robot builds the whole stack.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotCommitMap {
  /// Newest first: the commits of the virtual branch, then those the robot had and the branch no longer has
  pub commits: Vec<RobotCommit>,
  /// The robot's source revision is not in the repository (started on another machine, or pruned): every commit is
  /// `unknown`
  pub source_missing: bool,
  /// The checked-out branch has commits of the branch the virtual branch lacks (not synced since): they are not listed
  pub virtual_behind_head: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotCommit {
  /// The commit of the checked-out branch; null for a commit only the robot had (`removed`, `landed`)
  pub local_id: Option<String>,
  /// The subject of the commit of the checked-out branch, with its `(prefix)`; for a commit only the robot had, the
  /// subject of the virtual copy, without it
  pub subject: String,
  /// Unix seconds
  pub author_time: u32,
  /// The virtual branch of the commit (without the namespace): the branch of the robot, or an ancestor of a stacked
  /// branch, whose commits the robot built too; null for a commit only the robot had
  pub branch: Option<String>,
  pub state: RobotCommitState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RobotCommitState {
  /// The robot checked this commit as it is
  Tested,
  /// A copy on a newer baseline with the same changes: checked
  Rebased,
  /// Changed (amended) since the robot ran
  Changed,
  /// Added after the robot ran: not checked
  Added,
  /// The robot had it, the branch no longer has it
  Removed,
  /// The robot had it, and it is in the baseline now (merged meanwhile, e.g. by the Safe Push of a parent)
  Landed,
  /// The robot's commits are unknown (`source_missing`)
  Unknown,
}

/// The failure of a robot and the commit map of its branch, for the prompt
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotFailureReport {
  pub failure: RobotFailure,
  /// null when the branch has no row (archived) or the robot has no source revision
  pub commits: Option<RobotCommitMap>,
  /// The checked-out branch, whose commits the agent fixes; null when HEAD is detached
  pub head_branch: Option<String>,
}

/// The commit map of a running robot, for the commit rows of the branches it tests: which commits it covers
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetRobotCommitMapParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// The virtual branch of the robot, without the `{prefix}/virtual/` namespace; null for the robot of a whole stack
  pub branch_name: Option<String>,
  /// The root of the stack whose stack branch the robot tests; null for the robot of a branch
  pub stack_root: Option<String>,
  pub source_revision: String,
  /// For the robot of a branch, the branches it is built on, root first; for the robot of a whole stack, its members,
  /// the root first (as `GetRobotFailureParams`)
  pub ancestors: Vec<String>,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline_branch: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetRobotFailureParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub robot_id: String,
  /// The virtual branch of the robot, without the `{prefix}/virtual/` namespace; null without a row
  pub branch_name: Option<String>,
  /// The branches it is built on, root first (the chain of `stack.effective_base`): the robot builds their commits too;
  /// empty for a root branch
  pub ancestors: Vec<String>,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline_branch: String,
  /// The full version: the failed builds, their problems and failed tests with messages, and the log tails
  pub full: bool,
}
