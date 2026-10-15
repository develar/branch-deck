//! Patronus robots (Safe Push, Safe Merge, Dry Run) of the user's virtual branches, as the app shows them.

use serde::{Deserialize, Serialize};

/// One Patronus run: it rebases the source branch onto the target, runs the checks and, unless it is a dry run, pushes
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusRobot {
  pub id: String,
  /// The subject of the first commit, or the title given at the start
  pub name: String,
  pub status: PatronusRobotStatus,
  pub push_mode: PatronusPushMode,
  /// The virtual branch (without the `{prefix}/virtual/` namespace) the robot was started on; null for a robot on
  /// another branch or on a temporary ref (a Safe Push from the IDE or the CLI), matched by its revision instead
  pub branch_name: Option<String>,
  /// The commit Patronus took from the source branch at the start
  pub source_revision: Option<String>,
  /// e.g. `master`
  pub target_branch: String,
  /// Unix seconds
  pub start_time: u32,
  /// Unix seconds, null while running
  pub finish_time: Option<u32>,
  pub cancellation_reason: Option<String>,
  /// The robot page in the Patronus web UI
  pub url: String,
  /// The GitHub pull request whose `/patronus` comment started the robot: Patronus reports in it and closes it once
  /// merged. A robot started otherwise has none.
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub pull_request: Option<PatronusPullRequestLink>,
  /// The merge profile the robot runs with, which decides its checks (and so how long it runs); null while unknown: a
  /// list doesn't say it, a poll, the details and a start do
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub profile: Option<PatronusRobotProfile>,
}

/// The merge profile of a robot, e.g. "Large Scale Change"; a known robot without one has `id` null (the default checks
/// of the target branch)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusRobotProfile {
  pub id: Option<String>,
  pub name: Option<String>,
}

/// A GitHub pull request a robot belongs to
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusPullRequestLink {
  pub number: u32,
  pub url: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PatronusRobotStatus {
  Running,
  /// Running, a cancel was asked for
  Canceling,
  Canceled,
  Failure,
  /// The checks passed; the changes are pushed unless it was a dry run
  Success,
  /// A status this version does not know
  Unknown,
}

impl PatronusRobotStatus {
  pub fn is_finished(self) -> bool {
    matches!(self, Self::Canceled | Self::Failure | Self::Success)
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PatronusPushMode {
  /// Only the checks: nothing is pushed
  DryRun,
  Rebase,
  RebaseAutosquash,
  SquashAll,
  Merge,
  Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusRobotsResult {
  pub availability: PatronusAvailability,
  /// The robots of the user in the repository, newest first; empty unless available
  pub robots: Vec<PatronusRobot>,
  /// `owner/name` of the GitHub repository the branches are mirrored to (the mirror of the committed Space settings, or
  /// `branchdeck.githubRepository`): their pull
  /// requests are reviewed there, and Patronus merges one from its `/patronus merge` comment. Only in the list of a
  /// repository, not in a poll.
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub github_repository: Option<String>,
}

/// No robots, and why
impl From<PatronusAvailability> for PatronusRobotsResult {
  fn from(availability: PatronusAvailability) -> Self {
    Self {
      availability,
      robots: Vec::new(),
      github_repository: None,
    }
  }
}

/// Whether Patronus can be asked for the repository
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PatronusAvailability {
  Available,
  /// The push remote is not a Space repository (`git.jetbrains.team`)
  NotPatronus,
  /// No Space token: the Safe Push CLI has not logged in, its token expired, or Patronus rejected it
  NotLoggedIn,
  /// Patronus failed or could not be reached
  Error {
    message: String,
  },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetPatronusRobotsParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// Unix seconds: only the robots started since then (an incremental refresh); null = the last 7 days
  pub since: Option<u32>,
  /// Asked by a timer or an event, not by the user: skipped while Patronus is backing off after failures
  pub background: bool,
}

/// A running robot whose status the app knows
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KnownRobotStatus {
  pub id: String,
  pub status: PatronusRobotStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PollPatronusRobotsParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub robots: Vec<KnownRobotStatus>,
}

/// What the chip popover shows beyond the list: the merge profile, the CI checks and the problems
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusRobotDetails {
  pub robot: PatronusRobot,
  /// null without a profile
  pub profile_id: Option<String>,
  /// e.g. "Large Scale Change"
  pub profile_name: Option<String>,
  /// null before Patronus started any check
  pub checks: Option<PatronusChecks>,
  /// The problems of severity error, e.g. "Changes require review approval"
  pub problems: Vec<PatronusProblem>,
  /// The estimate of the CI for a running robot; null for a finished one and before its checks run
  pub progress: Option<PatronusRobotProgress>,
}

/// How far a running robot is by the CI's own estimate of its running checks (`build-progress`), read only while its
/// details are open
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusRobotProgress {
  /// Seconds until the running check that ends last is done; null when no running check has an estimate or every one
  /// ran past it
  pub remaining_seconds: Option<u32>,
  /// That check, e.g. "Unit Tests", and how far it is, 0 to 100
  pub check_name: Option<String>,
  pub check_percent: Option<u32>,
  /// Seconds that check ran past its estimate, when every estimate is exceeded
  pub overtime_seconds: Option<u32>,
  /// Checks waiting in the build queue
  pub queued: u32,
  /// The CI thinks a running check hangs
  pub probably_hanging: bool,
}

/// The CI checks of a robot, counted
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusChecks {
  /// The build configurations of the first failed checks
  pub failed_names: Vec<String>,
  pub failed: u32,
  /// Queued or running
  pub running: u32,
  /// Passed or skipped
  pub passed: u32,
  pub canceled: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusProblem {
  pub title: String,
  /// e.g. `OWNERSHIP_REVIEW_APPROVAL_CHECK`
  pub subsystem: Option<String>,
}

/// A merge profile of the target branch, e.g. "Large Scale Change": which checks run
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusProfile {
  pub id: String,
  pub name: String,
  pub description: Option<String>,
  /// Empty: any mode
  pub allowed_push_modes: Vec<PatronusPushMode>,
}

/// What a start offers for a branch
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusStartOptions {
  /// The branch the robot pushes to, e.g. `master`
  pub target_branch: String,
  /// Patronus protects the target branch; false = a robot can't be started into it
  pub supported: bool,
  /// The profiles of the target branch the user may use; the emergency ones are left out
  pub profiles: Vec<PatronusProfile>,
  /// The profile of the last robot of the branch, if it is still offered
  pub default_profile_id: Option<String>,
}

/// A robot of the selected repository
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusRobotParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub robot_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetPatronusStartOptionsParams {
  pub repository_path: String,
  /// As the sync detected it, e.g. `origin/master`: the robot pushes to the branch it tracks
  pub baseline_branch: String,
  /// The last robot of the branch: its profile is the default
  pub last_robot_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StartPatronusRobotParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub baseline_branch: String,
  /// The virtual branch, without the `{prefix}/virtual/` namespace; it must be pushed
  pub branch_name: String,
  pub mode: PatronusPushMode,
  pub profile_id: Option<String>,
  /// The merge request whose approvals the review check of the robot counts; null without one
  pub merge_request: Option<PatronusMergeRequest>,
}

/// Whether the code owners still have to approve the branch before a Safe Push can merge it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusReviewRequirement {
  /// Approvals are missing: a Safe Push of the branch would fail its review check
  pub required: bool,
  /// The open merge request of the branch on Space; looked up only when approvals are required
  pub merge_request: Option<PatronusMergeRequest>,
  /// The groups whose approval is missing, with who can approve
  pub groups: Vec<PatronusReviewerGroup>,
  /// The branch has too many commits to check (as in the IDE); the robot checks them anyway
  pub skipped: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusMergeRequest {
  /// e.g. `IJ`
  pub project_key: String,
  /// e.g. `IJ-CR-221762`
  pub review_key: String,
  pub url: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusReviewerGroup {
  /// e.g. "Air Plugin Release Team"
  pub name: String,
  pub reviewers: Vec<PatronusReviewer>,
  /// The changed paths whose review the group owes, sorted; empty when `paths_source` is `Unknown` or the group
  /// reviews every change (`branch_wide`)
  pub paths: Vec<String>,
  /// Any change of the branch needs the group, not only the paths it owns
  pub branch_wide: bool,
  pub paths_source: ReviewPathsSource,
}

/// Who told which paths a reviewer group owes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReviewPathsSource {
  /// Patronus, from the rules it checked
  Patronus,
  /// The `OWNERSHIP` files of the baseline: the paths whose owner has the name of the group
  Ownership,
  /// Neither: Patronus sends no paths and no path is owned by the group by name (a delegated review, or other
  /// reviewers named by `review-rules.yaml`)
  Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusReviewer {
  pub full_name: String,
  pub username: String,
  /// Not absent and not opted out of reviews
  pub available: bool,
  /// Patronus suggests them for this change
  pub suggested: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusBranchParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline_branch: String,
  /// The virtual branch, without the `{prefix}/virtual/` namespace
  pub branch_name: String,
  /// Ask Patronus again instead of the cached answer (the user waits for approvals)
  #[serde(default)]
  pub recheck: bool,
}

/// Starts a robot from a GitHub pull request: the `/patronus` comment
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PostPatronusCommandParams {
  pub repository_path: String,
  pub pull_request_number: u32,
  /// `rebaseAutosquash` (`/patronus merge`) or `dryRun` (`/patronus dry-run`)
  pub mode: PatronusPushMode,
}

/// The posted command comment
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusCommandComment {
  /// The GitHub comment id; a string, as it outgrows 32 bits
  pub id: String,
  pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReadPatronusCommandReplyParams {
  pub repository_path: String,
  pub pull_request_number: u32,
  pub comment_id: String,
}

/// How Patronus answered a command comment
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PatronusCommandReply {
  /// No answer yet
  Pending,
  /// 👍: the robot started; it is in the list of robots, linked to the pull request
  Accepted,
  /// 👎: the command was not understood or the robot did not start, e.g. a draft pull request or a robot already
  /// running for the branch
  Rejected {
    /// The text of the bot's `⚠️` comment, null if it is not there yet
    reason: Option<String>,
  },
}
