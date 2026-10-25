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
  /// another branch or on a temporary ref (a Safe Push from the IDE or the CLI), matched by its revision instead, and
  /// for a robot of the whole stack (`stack_root`)
  pub branch_name: Option<String>,
  /// The root of the stack whose stack branch (`{prefix}/stack/<root>`) the robot was started on: a Safe Push of the
  /// whole stack, which lands every branch of it. It belongs to no branch: never matched by its revision
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack_root: Option<String>,
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
  /// The profile skips the code-owner review approval check (Patronus writes it into the options of the robot); None:
  /// not written yet, unknown
  pub skips_review: Option<bool>,
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
  pub const fn is_finished(self) -> bool {
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

/// A running robot whose status the app knows
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct KnownRobotStatus {
  pub id: String,
  pub status: PatronusRobotStatus,
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
  /// A required check failed while the robot runs: Patronus warns that it will fail (it may run on for an hour)
  pub failing: bool,
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

/// The subsystem of the problem of a robot that failed for missing code-owner approvals
pub const REVIEW_APPROVAL_SUBSYSTEM: &str = "OWNERSHIP_REVIEW_APPROVAL_CHECK";

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
  /// The profile skips the code-owner review approval check: known for an emergency profile and for the profile of the
  /// last robot (its options say it); None: Patronus doesn't tell before a start
  pub skips_review: Option<bool>,
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
  /// The emergency profiles (they skip all checks): only a Safe Push without review offers them, after a warning
  pub emergency_profiles: Vec<PatronusProfile>,
  /// The profile of the last robot of the branch, if it is still offered
  pub default_profile_id: Option<String>,
}

/// A robot of the selected repository
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusRobotParams {
  pub repository_path: String,
  pub robot_id: String,
  /// The cancel of the robot a start or a push replaces or restarts (`start_guard::check_cancel`): a robot that finished
  /// meanwhile is answered as it is, already finished, instead of "not running", and the start or the push goes on
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub replaced: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StartPatronusRobotParams {
  pub repository_path: String,
  pub baseline_branch: String,
  /// The virtual branch, without the `{prefix}/virtual/` namespace; it must be pushed. With `stack`, the root of the
  /// stack
  pub branch_name: String,
  /// The whole stack of the root `branch_name`: the robot starts from its stack branch `{prefix}/stack/<root>`, which
  /// must be pushed, and lands every branch of the stack
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack: bool,
  pub mode: PatronusPushMode,
  pub profile_id: Option<String>,
  /// The merge request whose approvals the review check of the robot counts; null without one
  pub merge_request: Option<PatronusMergeRequest>,
  /// What the user started anyway (Start Anyway); without it the refusals of `trial_check::landing_gate` fail the start
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub allow: LandingAllowances,
}

/// The overrides of a refusal of `trial_check::landing_gate`, each by the user: `--allow-dangling` and `--allow-missing`
/// of `bd safe-push`, Start Anyway of the app
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingAllowances {
  /// Start although a commit proves a dangling reference
  #[serde(default)]
  pub allow_dangling: bool,
  /// Start although what lands uses a name a branch outside it adds
  #[serde(default)]
  pub allow_missing: bool,
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
  /// The groups whose approval is missing, with who can approve; with an answer about a pull request
  /// (`PatronusReviewerGroup::approved` set) every group the changes need, the approved ones too
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
  /// The approvals the group needs; only in an answer about a pull request, of an ownership rule
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub required_approvals: Option<u32>,
  /// The approvals that count (a self-approval too), may be more than `required_approvals`; as `required_approvals`
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub collected_approvals: Option<u32>,
  /// Whether the pull request has the approvals of the group; only in an answer about a pull request
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub approved: Option<bool>,
  /// The author's own approval counts; as `required_approvals`
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub self_approval_allowed: Option<bool>,
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
  /// The Space username; null only in an answer about a pull request, for a member without one (a bot member)
  pub username: Option<String>,
  /// The GitHub login a review of a pull request can be asked from; null when Patronus doesn't say
  pub github_login: Option<String>,
  /// Not absent and not opted out of reviews
  pub available: bool,
  /// Patronus suggests them for this change
  pub suggested: bool,
  /// What the review of the member in the pull request counts for; only in an answer about a pull request
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub approval_state: Option<ReviewerApprovalState>,
}

/// What Patronus makes of the review of a group member in the pull request, as the robot started from it would
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReviewerApprovalState {
  /// The latest review approves; it counts, a stale one too
  Approved,
  /// The author, whose own approval the group allows
  AuthorSelfApproved,
  /// Asked to review, or the latest review comments, requests changes or was dismissed
  Reviewing,
  /// Neither asked nor reviewed
  NotAdded,
  /// The approval of the member can't count (the author where self-approval isn't allowed, an unavailable member)
  NotEligible,
}

impl ReviewerApprovalState {
  /// The review counts as an approval of the group
  pub const fn counts(self) -> bool {
    matches!(self, Self::Approved | Self::AuthorSelfApproved)
  }
}

/// How the robot of a Safe Push would start, which decides whose approvals count
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ReviewStart {
  /// From the `/patronus` comment of the open pull request into the target: its approvals count
  PullRequest { number: u32 },
  /// Through the API: no pull request is linked, no approval of one counts
  Rest,
}

/// A person of a review: a group member Patronus names (the full name, a GitHub login when it knows one) or a reviewer
/// of the pull request outside every group (the login only)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Person {
  pub login: Option<String>,
  pub full_name: Option<String>,
}

/// Whether a Safe Push of the branch, started as `ReviewStart` says, would pass the code-owner review: Patronus decides
/// (`patronus::review_readiness`), the app and `bd` render it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ReviewReadiness {
  /// The changes need no approval
  NotRequired,
  /// Every group the changes need approved the pull request
  Approved { groups: Vec<ApprovedReviewGroup> },
  /// A Safe Push would fail for approvals: the groups still short of them
  Missing {
    groups: Vec<MissingReviewGroup>,
    /// Approvers of the pull request Patronus doesn't count: no eligible member of a group (a bot, a colleague)
    not_counted: Vec<Person>,
    reason: MissingApprovalsReason,
  },
  /// Patronus doesn't tell whether the approvals of the pull request count
  Unknown { reason: UnknownReadinessReason },
  /// Patronus didn't answer
  Unavailable { reason: UnavailableReadinessReason, message: String },
}

/// A group whose approval is there, with the members Patronus counts
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ApprovedReviewGroup {
  pub name: String,
  pub approved_by: Vec<Person>,
}

/// A group whose approval is missing; its files are those of the group of the same name in the requirement
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct MissingReviewGroup {
  pub name: String,
  /// How many more approvals count; null when Patronus doesn't say
  pub needed: Option<u32>,
  /// Members whose approval counts already (a group that needs two)
  pub approved_by: Vec<Person>,
  /// Members asked to review in the pull request who haven't approved
  pub requested: Vec<Person>,
  /// Members Patronus suggests for the change
  pub suggested: Vec<Person>,
  /// Members whose latest review of the pull request requests changes
  pub changes_requested_by: Vec<Person>,
}

/// Why the approvals are missing
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum MissingApprovalsReason {
  /// The code owners haven't approved yet
  ApprovalsMissing,
  /// A start through the API counts no approval of a pull request: the answer without one is the verdict
  NoPullRequestContext,
  /// A missing group has no member whose approval could count
  NoEligibleReviewer,
}

/// Why Patronus can't tell
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UnknownReadinessReason {
  /// The pull request has approvals, and Patronus answers without saying whose count (an older server, or the
  /// evaluation of pull requests is off)
  ApprovalsNotEvaluated,
  /// The branch has too many commits to check, as in the IDE; the robot checks them anyway
  Skipped,
}

/// Why Patronus didn't answer
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UnavailableReadinessReason {
  /// No Space token, an expired one, or Patronus rejected it
  NotLoggedIn,
  /// Patronus failed or could not be reached, or is backed off after a failure
  Error,
}

/// A branch whose review readiness the app asks, at the tip it shows
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewReadinessBranch {
  /// The virtual branch, without the `{prefix}/virtual/` namespace; with `source`, only the key the answer comes back
  /// with (the app's `stack:<root>`)
  pub branch_name: String,
  /// The branch the robot would start from, without `refs/heads/`, when it is not the virtual branch of `branch_name`:
  /// the stack branch `{prefix}/stack/<root>` of the whole stack, whose commits `tip` names. Patronus is asked about
  /// it; null for the virtual branch
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub source: Option<String>,
  /// The full commit id the answer is about; the commits are those of it the baseline lacks
  pub tip: String,
  pub start: ReviewStart,
  /// The reviewers of the pull request of the start, as the app shows them; empty for a start through the API
  #[serde(default)]
  pub reviewers: Vec<crate::pull_request::Reviewer>,
  /// The author of the pull request of the start
  #[serde(default)]
  pub author: Option<String>,
}

impl ReviewReadinessBranch {
  /// The branch started from `pull_request`, the open pull request the caller chose as the robot's start, whose
  /// reviewers and author Patronus evaluates; without one through the API (`Rest`), which counts no approval. The one
  /// way a start of `bd` is built (`plan()` of `bd safe-push`, `bd request-review`, the scenario facts); the app's is
  /// `inputOf` of `src/stores/review-readiness.ts`
  #[must_use]
  pub fn starting_from(branch_name: String, source: Option<String>, tip: String, pull_request: Option<&crate::pull_request::PullRequest>) -> Self {
    Self {
      branch_name,
      source,
      tip,
      start: pull_request.map_or(ReviewStart::Rest, |pull_request| ReviewStart::PullRequest { number: pull_request.number }),
      reviewers: pull_request.map_or_default(|pull_request| pull_request.reviewers.clone()),
      author: pull_request.and_then(|pull_request| pull_request.author.clone()),
    }
  }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetBranchReviewReadinessParams {
  pub repository_path: String,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline_branch: String,
  pub branches: Vec<ReviewReadinessBranch>,
  /// Ask Patronus again instead of the cached answer (the user waits for approvals)
  #[serde(default)]
  pub recheck: bool,
}

/// The answer for one branch of `GetBranchReviewReadinessParams`, keyed by its input
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchReviewReadiness {
  pub branch_name: String,
  pub tip: String,
  /// What Patronus answered; null when it didn't
  pub requirement: Option<PatronusReviewRequirement>,
  pub readiness: ReviewReadiness,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetPatronusTargetBranchParams {
  pub repository_path: String,
  pub baseline_branch: String,
}

/// Whose pull request a `/patronus` comment starts: the text of a merge depends on it
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PatronusCommandSubject {
  /// The pull request of a virtual branch of the user: the merge keeps the branch, the sync archives it once merged
  #[default]
  OwnBranch,
  /// A pull request of someone else the user reviewed: the merge deletes its branch, as a merge on GitHub does
  ReviewedPullRequest,
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
  #[serde(default)]
  pub subject: PatronusCommandSubject,
  /// `owner/name` of the pull request when the review requests know it; else the mirror of the Space repository
  #[serde(default)]
  pub github_repository: Option<String>,
  /// The branch of the user the comment starts (`OwnBranch`, required there): the checks of `start_patronus_robot` run
  /// on it before the comment (`RemoteActions::check_start_from_pull_request`)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub start: Option<PullRequestStartBranch>,
}

/// The branch of the user a `/patronus` comment on its pull request starts, as `start_patronus_robot` names it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PullRequestStartBranch {
  /// The virtual branch, without the `{prefix}/virtual/` namespace; with `stack`, the root of the whole stack
  pub branch_name: String,
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack: bool,
  /// What the user started anyway (Start Anyway)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub allow: LandingAllowances,
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
  /// As in `PostPatronusCommandParams`
  #[serde(default)]
  pub github_repository: Option<String>,
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
