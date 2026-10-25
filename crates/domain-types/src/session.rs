//! What the session of a repository sends a window (docs/design/rust-domain-authority.md, "Представление: снимок и
//! патчи"): a snapshot of its view (`RepositoryView`) and the patches that follow it (`ViewPatch`). The table is cut
//! into parts that change for different reasons: of each branch its head and its commits (with a grouping), its sync
//! state (with its status and progress), the copies of its commits (with each commit the sync reports) and its remote
//! status; the stack branches by root; the sections below the table. A patch carries a part only when it differs from the one sent last, so the page
//! keeps the identity of every part it did not get.
//!
//! The names on the wire are the ones the page reads today (`branch-sync.ts`). The statuses are data: the texts, the
//! marks of what to expand and the badges are the page's presentation of them. A fact a service or a computation
//! answers is a `Fact` with its `FactState` (the rule of a fact without an answer), and the facts of the remote services
//! are parts of the view beside the table (`FactsView`: the robots of Patronus by id), as is what the session derives
//! of them and the table (`PatronusPart`: the robot of each row and what the robots land, landed and hold, with the
//! landing needs, keyed by branch and stack); a window tells the session what it shows and needs (`Demand`) and asks it
//! to read a fact again (`RefreshFactsParams`).

use crate::branch_integration::BranchIntegrationInfo;
use crate::commit::Commit;
use crate::conflict::WordResolutionSummary;
use crate::error::ApiError;
use crate::issue_navigation::IssueNavigationConfig;
use crate::landed::LandedBranch;
use crate::patronus::{PatronusAvailability, PatronusReviewRequirement, PatronusRobot, PatronusRobotDetails, PatronusStartOptions, Person, ReviewReadiness, ReviewReadinessBranch};
use crate::pull_request::{BranchRules, PullRequest, PullRequestAvailability};
use crate::repository_view::{BranchPatronus, PatronusView, RobotLandedBranch, StackPatronus};
use crate::review_request::ReviewRequest;
use crate::stack::{BranchStack, StackBranchUpdate};
use crate::update_project::UpdateCheck;
use crate::{BranchError, BranchOrigin, BranchSyncStatus, CommitSyncStatus, RemoteStatusUpdate, UnappliedBranchInfo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// The version of what a window holds of a session. `epoch` is random per process and session, `seq` grows by one with
/// each patch. A patch applies only to the view at its `base`; any other view (a lost patch, another epoch) asks for a
/// snapshot. Both are `u32`, a number in TypeScript
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Version {
  pub epoch: u32,
  pub seq: u32,
}

impl Version {
  /// The version after this one, in the same epoch
  #[must_use]
  pub const fn next(self) -> Self {
    Self {
      epoch: self.epoch,
      seq: self.seq.wrapping_add(1),
    }
  }
}

/// A fact of a session that a service or a computation answers (docs/design/rust-domain-authority.md, "Факты"): the
/// value of the last answer, how it stands now, the number of the observation that answered it (`seq`, from the one
/// counter of the session: an answer applies only when its observation is newer) and when it answered (Unix seconds).
/// `value` is `None` until the first answer
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Fact<T> {
  pub value: Option<T>,
  pub state: FactState,
  pub seq: u32,
  pub observed_at: u32,
}

impl<T> Default for Fact<T> {
  /// A fact without an answer yet
  fn default() -> Self {
    Self {
      value: None,
      state: FactState::Loading,
      seq: 0,
      observed_at: 0,
    }
  }
}

/// How a fact stands. A fact without an answer is never "no": a read that fails is `Unavailable` with its reason, never
/// an empty value, and never a `Loading` that lasts
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum FactState {
  /// No answer yet: from the open of the session (or a change of what it is of) until the first one
  #[default]
  Loading,
  /// The value is the answer of the newest observation
  Fresh,
  /// The value is an earlier answer: the newest read failed (`reason`), or what the fact is of changed and it is being
  /// read or computed again (`None`); nothing waits for it
  Stale {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(optional))]
    reason: Option<ApiError>,
  },
  /// No answer: the read failed before any answer came
  Unavailable { reason: ApiError },
  /// Nothing asks for it (no window demands it)
  Unknown,
}

/// What a window shows and needs of a session (docs/design/rust-domain-authority.md, "Demand"), the one thing the page
/// tells of itself: the facts it asks for, and how often. The session keeps the demand of each window by its
/// subscription and asks for the facts of all of them; the lists name branches by name, robots by id
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Demand {
  /// The window has the focus
  pub focused: bool,
  /// The machine is online (`navigator.onLine`): offline, no poll asks
  pub online: bool,
  /// The branches whose rows the window shows with a step that waits for the review readiness (the page's rule of a
  /// step until the next step is decided in Rust)
  pub visible_branches: Vec<String>,
  /// The whole stacks whose island headers the window shows with such a step, by root
  pub visible_stacks: Vec<String>,
  /// The subjects a surface needs a fact of now, asked at once beside whatever is in flight (the Safe Push popover and
  /// its click, the "Create pull request" dialog): a branch by name, the whole stack of a root as `stack:<root>`
  /// (`stack_readiness_key`)
  pub needed: Vec<String>,
  /// The robots whose details a surface shows (the robot popover, the preview, a row of the landing overview)
  pub robot_details: Vec<String>,
  /// The robots of no branch the window started and follows until they finish (the Safe Push of a reviewed pull
  /// request): polled with the robots of the branches
  pub watched_robots: Vec<String>,
}

/// The demand of a window, by its subscription to the session (`ViewSubscription::id`)
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SetDemandParams {
  pub subscription: u32,
  pub demand: Demand,
}

/// A family of the facts of a session, which a window asks to read again (`RefreshFactsParams`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FactFamily {
  /// The user's robots of Patronus (`FactsView::robots`)
  Robots,
  /// Why the failed robots of the table failed (`RobotsView::needs_review`): Check Approvals Again asks those Patronus
  /// didn't give again; their reads come as patches, the refresh answers at once
  RobotProblems,
  /// The GitHub pull requests of the branches and whole stacks (`FactsView::pull_requests`)
  PullRequests,
  /// The rules of the bases of the approved pull requests (`FactsView::rules`): those GitHub didn't give are asked again
  BranchRules,
  /// The merge profiles of the target of the baseline (`FactsView::profiles`), asked when unknown, after a failure and
  /// a minute after the last read
  Profiles,
  /// The review readiness of a subject (`FactsView::readiness`): Check Approvals Again (`Manual` with the subject) asks
  /// Patronus again instead of the kept answer; the answer is the version after its patch
  Readiness,
  /// The pull requests of others that wait for the user's review (`FactsView::review_requests`): the refresh button
  /// asks them (`Manual`); a request opened on GitHub, where the user may answer it, makes the next focus ask at once
  /// (`AfterWrite`, answered at once)
  ReviewRequests,
}

/// Why a window asks a fact again
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum FactRefreshReason {
  /// The refresh button: always asks, the robots of the full window
  Manual,
  /// An action of the window changed the remote or needs the fact now: the robot a `/patronus` comment started, a
  /// merge Patronus closed a pull request of, a start that reads the rules and the profiles, a form that opens; asks
  /// when the rules of the family say so for a write (`domain_policy::refresh`), and once more after a read in flight
  AfterWrite,
}

/// Asks the session of a repository to read a fact again (`refresh_facts`); the answer is the version of the view after
/// the patch of that read, null when nothing was asked (the rules held it back, no session is open)
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RefreshFactsParams {
  pub repository_path: String,
  pub family: FactFamily,
  pub reason: FactRefreshReason,
  /// The subject of a family read by subject: the readiness of a branch, `stack:<root>` of a whole stack
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub subject: Option<String>,
}

/// What the readiness key of a whole stack starts with: `stack:<root>` (a ref name has no `:`, so no branch is one)
pub const STACK_READINESS_PREFIX: &str = "stack:";

/// The readiness key of the whole stack of `root` (`stackReadinessKey` of the page)
#[must_use]
pub fn stack_readiness_key(root: &str) -> String {
  format!("{STACK_READINESS_PREFIX}{root}")
}

/// The root of the readiness key of a whole stack; `None` for a branch's
#[must_use]
pub fn stack_root_of_readiness_key(key: &str) -> Option<&str> {
  key.strip_prefix(STACK_READINESS_PREFIX)
}

/// Why a session syncs (`repository_model::sync_scheduler` decides what runs)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SyncReason {
  /// ⌘R, the Sync button: always runs
  Manual,
  /// A window gained focus: runs when the inputs changed
  Focus,
  /// A rewrite, its undo or a fetch changed the branches: always runs, as a quiet follow-up while one is in flight
  AfterRewrite,
  /// A window opened the session: runs when the inputs changed, or joins the run in flight
  Open,
}

/// How the request of a sync ended, when it did not fail
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SyncOutcome {
  /// A run served the request and succeeded
  Synced,
  /// Nothing a sync reads changed since the last successful sync: no run
  Unchanged,
  /// A focus of a session that no window shows: nothing ran
  Skipped,
}

/// The answer of a request of a sync: how it ended, and the version of the view after the last patch of the run that
/// served it. The patch and the answer travel apart, so a window whose view is older waits for the patch that brings
/// it to `version`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SyncAnswer {
  pub outcome: SyncOutcome,
  pub version: Version,
}

/// The answer of a command whose result the session of its repository applied to the table (a push): its own outcome,
/// and the version of the view after the patch of that change, which a window waits for as it waits for the last patch
/// of a sync (`SyncAnswer`); `None` when no session of the repository is open (no window shows it). A command with no
/// outcome of its own (the deletion of an archived or unapplied branch) answers the version alone
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ViewAnswer<T> {
  pub outcome: T,
  pub version: Option<Version>,
}

impl<T> ViewAnswer<T> {
  /// The answer with the outcome `convert` makes of this one's, at the same version
  ///
  /// # Errors
  /// What `convert` failed with
  pub fn try_map<U, E>(self, convert: impl FnOnce(T) -> Result<U, E>) -> Result<ViewAnswer<U>, E> {
    Ok(ViewAnswer {
      outcome: convert(self.outcome)?,
      version: self.version,
    })
  }
}

/// A window's subscription to the patches of a session: its id (`unsubscribe_view`) and the view at the subscription,
/// which the first patch on the channel follows. A patch may reach the window before this answer: the window keeps it
/// until it has the view
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ViewSubscription {
  pub id: u32,
  pub view: RepositoryView,
}

/// Whether a sync runs, and what it does
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SyncState {
  #[default]
  Idle,
  Running {
    step: SyncStep,
  },
  /// The last sync failed; the table is the one of the sync before
  Failed {
    error: ApiError,
  },
}

/// The step of a running sync, as its events tell it
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum SyncStep {
  /// The commits ahead of the baseline are read and grouped
  Grouping,
  /// The branches are grouped and are being built
  Building,
  /// The branches and the stack branches are built; the detection of the archived branches or the update check still
  /// runs (both run beside the build, and no event says which one is left)
  Detection,
}

/// The snapshot of the view of a session: the branch table, what the robots land, landed and hold of it, the facts of
/// the remote services and their news
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RepositoryView {
  pub version: Version,
  pub sync: SyncState,
  pub header: TableHeader,
  /// The names of the branches in the order of the table, as the sync grouped them (newest first)
  pub order: Vec<String>,
  pub branches: BTreeMap<String, BranchView>,
  /// The stack branches the last sync reported, by the root of their stack
  pub stacks: BTreeMap<String, StackBranchUpdate>,
  pub sections: TableSections,
  /// What the robots of Patronus land, landed and hold of the table, as the session derives it
  pub patronus: PatronusPart,
  /// What the remote services answered, beside the table
  pub facts: FactsView,
  /// The last news of the session (at most `NEWS_KEPT`), oldest first: a window tells each once, by its id, and none
  /// that its first view had already
  pub news: Vec<News>,
}

/// How many news a view keeps, for a window that lost a patch and asks a snapshot
pub const NEWS_KEPT: usize = 32;

/// The Patronus view of the session (`repository_model::facts::derive_patronus`, the rule `bd` reads too): the robot of
/// each branch and whole stack and what the robots land, landed and hold of the table of the last sync, on the remote
/// branches as the session knows them (a push moves them), with the landing needs of each branch
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusPart {
  /// How the view stands: `Loading` until Patronus answered once and the view of that answer is derived, `Stale` while
  /// the view of newer robots, a newer table or a push is derived (nothing waits for it), `Unavailable` when the
  /// derivation failed without an earlier view, `Stale` with the reason when it failed after one
  pub state: FactState,
  /// How the landing needs of `view` stand: `Loading` while they are computed for the table and what the view names
  /// landed (the needs of a branch are `unknown` meanwhile), `Fresh` once they are, `Unavailable` when the computation
  /// failed for them
  pub landing_needs: FactState,
  pub view: PatronusView,
}

/// The facts of the remote services a session keeps, each with how it stands (docs/design/rust-domain-authority.md,
/// "Факты")
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FactsView {
  pub robots: RobotsView,
  pub pull_requests: PullRequestsView,
  /// By branch, the rules of the bases of the open pull requests someone approved: what a push of new commits does to
  /// the approvals; a base the session has not read yet has none
  pub rules: BTreeMap<String, Fact<BranchRules>>,
  /// The merge profiles of the target the baseline pushes to (`targetBranch`), without the default of the last robot
  /// (the page's until the policy moves into Rust): `Unknown` until a start or its popover asks for them
  pub profiles: Fact<PatronusStartOptions>,
  /// By subject (a branch, `stack:<root>`), the code-owner review a Safe Push would meet: `Loading` until its first
  /// answer, `Stale` while it is asked again for other inputs (another tip, start or reviews, Check Approvals Again),
  /// `Unavailable` when it can't be asked (Patronus not available, a read that failed); a subject nothing asked has none
  pub readiness: BTreeMap<String, Fact<SubjectReadiness>>,
  /// By robot id, the details of the robots a window shows them of (`Demand::robot_details`: the robot popover, a row of
  /// the landing overview, a toast that names the failed checks): the checks, the problems, the merge profile and, of a
  /// running robot, the estimate of the CI; `Loading` until the first answer, read again when the robot's status
  /// changes and every `RUNNING_DETAILS_REFRESH` while it runs; a robot no window demands any more has none
  pub robot_details: BTreeMap<String, Fact<PatronusRobotDetails>>,
  pub review_requests: ReviewRequestsView,
}

/// The pull requests of others that wait for the user's review ("Your Turn"), as the session listed them: the
/// availability of the last answer with how the listing stands (`head`: `Loading` until the first answer; `Fresh` while
/// GitHub lists them, and for a repository not on GitHub; `Stale` with the reason when GitHub failed for a while after a
/// listing, whose requests stay; `Unavailable` when none stands: a failure before any listing, no token, a token GitHub
/// rejects, no access), the requests waiting the longest first, how many wait in all, and when GitHub gave them
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequestsView {
  pub head: Fact<PullRequestAvailability>,
  /// Waiting the longest first, then by number; those of the last listing, kept through a failure, none without a token
  /// GitHub accepts or off GitHub
  pub requests: Vec<ReviewRequest>,
  /// All that wait, also beyond the 50 GitHub returns
  pub total_count: u32,
  /// Unix seconds of the last answer with the requests, 0 = never: how old the rows are after a failure
  pub checked_at: u32,
}

/// The review readiness of a subject as Patronus answered it, with what it was asked for: the tip, the start and the
/// reviews of the pull request of the start (`ask.branch_name` is the subject)
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SubjectReadiness {
  pub ask: ReviewReadinessBranch,
  /// What Patronus answered; null when it didn't
  pub requirement: Option<PatronusReviewRequirement>,
  pub readiness: ReviewReadiness,
}

/// The GitHub pull requests of the virtual branches and the whole stacks, as the session listed them and the app's
/// writes left them: the availability of the last answer with how the listing stands (`head`: `Loading` until the first
/// answer; `Fresh` while GitHub lists them, and for a repository not on GitHub; `Stale` with the reason when GitHub
/// failed for a while after a listing, which stays; `Unavailable` when no listing stands: a failure before any, or
/// GitHub refused the token), the pull requests by branch and by the root of the stack. A failed listing is never an
/// empty one: without a standing listing no pull request of a branch is known
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PullRequestsView {
  pub head: Fact<PullRequestAvailability>,
  /// By virtual branch name (without the `{prefix}/virtual/` namespace)
  pub pull_requests: BTreeMap<String, PullRequest>,
  /// By the root of the stack whose stack branch (`{prefix}/stack/<root>`) is the head; never a branch's
  pub stack_pull_requests: BTreeMap<String, PullRequest>,
}

/// The user's robots of Patronus in the repository, as the session listed and polled them: what Patronus said of the
/// repository and how the fact stands (`head`: `Loading` until the first answer, `Stale` with the robots of an earlier
/// answer after a failure, `Unavailable` after a failure without one), the robots by id, and what the session asked of
/// them after its answers: which running ones a failed required check dooms, why the failed robots of the table failed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotsView {
  pub head: Fact<RobotsHead>,
  pub robots: BTreeMap<String, PatronusRobot>,
  /// The running robots a failed required check dooms (Patronus' warning `robot_is_failing`), by id, sorted
  pub failing: Vec<String>,
  /// By id, the failed robots of the table whose problems were read: whether they failed for missing code-owner
  /// approvals
  pub needs_review: BTreeMap<String, bool>,
  /// By id, the failed robots of the table whose problems Patronus didn't give, with why
  pub problems_failed: BTreeMap<String, String>,
}

/// A transition the session saw in the answers of a remote service, which a window tells once (a toast, an OS
/// notification); `id` grows by one per news of a session
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct News {
  pub id: u32,
  pub event: NewsEvent,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NewsEvent {
  /// A robot known running or canceling succeeded and pushed (not a dry run): it merged what it tested
  RobotMerged {
    #[serde(rename = "robotId")]
    robot_id: String,
  },
  /// A robot known running or canceling failed, told once why is known: for missing code-owner approvals
  /// (`needsReview`), or not; null when the session doesn't read its problems (a robot of no branch of the table) or
  /// Patronus didn't give them
  RobotFailed {
    #[serde(rename = "robotId")]
    robot_id: String,
    #[serde(rename = "needsReview")]
    needs_review: Option<bool>,
  },
  /// Pull requests of others asked the user for a review since the last listing (`numbers`, of the requests of
  /// `FactsView::review_requests`): a request whose number and request time the listing before lacked, a re-request
  /// too; never one back to the user after their answer, and nothing in the first listing of a session a window
  /// watches anew
  ReviewRequested { numbers: Vec<u32> },
  /// The approvals of a subject that waited for them arrived (`domain_policy::readiness::approval_news`): whose
  /// approval counts now in the groups that waited (none named: the code owners), in the pull request of its start
  ApprovalsArrived {
    subject: String,
    #[serde(rename = "pullRequest")]
    pull_request: u32,
    approvers: Vec<Person>,
    groups: Vec<String>,
  },
}

/// What the last answer of Patronus said of the repository
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotsHead {
  /// Whether the repository is on Space, the Safe Push CLI logged in, Patronus answering
  pub availability: PatronusAvailability,
  /// `owner/name` of the GitHub repository the branches are mirrored to, as the last list named it
  /// (`PatronusRobotsResult::github_repository`)
  pub github_repository: Option<String>,
}

/// What the table as a whole was grouped and checked with
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableHeader {
  /// The baseline the sync compared with (`origin/master`); empty before the first grouping
  pub baseline_branch: String,
  /// Stacked branches are on (`branchdeck.stacks`)
  pub stacks_enabled: bool,
  /// Issue key -> branch name: the issue groups the commits were grouped with
  pub issue_groups: BTreeMap<String, String>,
  /// Child -> parent: the stack parents the branches were placed with
  pub stack_parents: BTreeMap<String, String>,
  /// Branch -> note: the parked branches
  pub parked: BTreeMap<String, String>,
  /// The issue links of the IntelliJ IDEA configuration, the last a sync found
  pub issue_navigation_config: Option<IssueNavigationConfig>,
}

/// The check of the checked out branch against the last fetched baseline
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BaselineStatus {
  pub baseline_branch: String,
  /// `None` for a local baseline, which has nothing to update from
  pub update: Option<UpdateCheck>,
  /// Why the check failed, e.g. a rebase in progress
  pub check_error: Option<String>,
}

/// A branch of the table, in the parts that change for different reasons
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchView {
  pub head: BranchHead,
  /// Newest first, as the sync grouped them; they change only with a grouping
  pub commits: Vec<Commit>,
  pub sync: BranchSync,
  /// What the sync made of each commit, in the order of `commits`
  pub copies: Vec<CommitSync>,
  pub remote: BranchRemote,
}

/// What the grouping says of a branch; a field without a value is absent
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchHead {
  pub name: String,
  pub latest_commit_time: u32,
  pub summary: String,
  /// The title of the branch's description (`bd describe`); `None` without one
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub description_title: Option<String>,
  pub all_commits_have_issue_references: bool,
  /// Most frequent author email in this branch's commits
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub my_email: Option<String>,
  pub origin: BranchOrigin,
  /// The issue the branch belongs to
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub issue_key: Option<String>,
  /// Where the branch sits in its stack; `None` without stacked branches
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack: Option<BranchStack>,
}

/// What the sync made of a commit: its virtual copy; the fields are absent until the sync reported the commit
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CommitSync {
  /// The virtual copy the sync wrote or kept
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub hash: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub status: Option<CommitSyncStatus>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub error: Option<BranchError>,
  /// The conflict of git that the copy resolved by words (`branchdeck.resolveByWords`)
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub resolved: Option<WordResolutionSummary>,
}

/// The status of a branch in the table: `Syncing` until the sync reports how it ended
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum BranchStatus {
  Syncing,
  Created,
  Updated,
  Unchanged,
  Error,
  MergeConflict,
  AnalyzingConflict,
  Blocked,
}

impl From<BranchSyncStatus> for BranchStatus {
  fn from(status: BranchSyncStatus) -> Self {
    match status {
      BranchSyncStatus::Created => Self::Created,
      BranchSyncStatus::Updated => Self::Updated,
      BranchSyncStatus::Unchanged => Self::Unchanged,
      BranchSyncStatus::Error => Self::Error,
      BranchSyncStatus::MergeConflict => Self::MergeConflict,
      BranchSyncStatus::AnalyzingConflict => Self::AnalyzingConflict,
      BranchSyncStatus::Blocked => Self::Blocked,
    }
  }
}

/// What the sync made of a branch
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchSync {
  pub status: BranchStatus,
  /// The commits the sync reported while the branch was `Syncing` (the progress of its row)
  pub processed_count: u32,
  pub has_error: bool,
  /// The error of the branch or of its last failed commit
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub error_details: Option<BranchError>,
  /// Why a `Blocked` branch was not rebuilt (its parent can't be built on); `None` otherwise
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub blocked_reason: Option<String>,
}

impl BranchSync {
  /// The state of a branch the sync has not reported yet
  pub const SYNCING: Self = Self {
    status: BranchStatus::Syncing,
    processed_count: 0,
    has_error: false,
    error_details: None,
    blocked_reason: None,
  };
}

/// What the remote has of a branch
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchRemote {
  /// The last remote status a sync computed; `None` until one did, kept while the branch is rebuilt
  pub status: Option<RemoteStatusUpdate>,
}

/// The sections below the branch table
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableSections {
  /// The commits no prefix and no issue key groups
  pub unassigned: Vec<Commit>,
  pub archived: ArchivedSection,
  /// The unapplied branches, newest first
  pub unapplied: Vec<UnappliedBranchInfo>,
  pub landed: LandedSection,
}

/// The archived branches: their names as the sync found them, and the integration each detection reported
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ArchivedSection {
  /// In the order the sync reported them (the page sorts them by the integration date)
  pub names: Vec<String>,
  /// The branches a sync of this session archived and that are still archived, oldest mark first
  pub newly_archived: Vec<String>,
  /// The integration of the archived branches whose detection reported
  pub integration: BTreeMap<String, BranchIntegrationInfo>,
}

/// The check of the checked out branch against the last fetched baseline and the commits that landed or are landing
/// with its update, which the sync reports together from the update check it runs beside its build
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandedSection {
  /// Whether the checked out branch can be updated onto the last fetched baseline; `None` until a sync reported it
  pub baseline: Option<BaselineStatus>,
  /// Local commits that the fetched baseline already has: the update drops them
  pub landing: Vec<LandedBranch>,
  /// Commits that landed with an update and that the user has not dismissed yet
  pub landed: Vec<LandedBranch>,
}

/// The change of a view from `base` to `next` (`next` is `base.next()`): the parts that differ from the ones sent last,
/// each whole. A window applies it only to the view at `base`
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ViewPatch {
  pub base: Version,
  pub next: Version,
  pub changes: TableChanges,
}

/// The parts of a `RepositoryView` a patch replaces; a part absent is unchanged. The keyed parts (branches, stacks, the
/// integration of the archived branches) carry only the keys that changed and the keys removed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TableChanges {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub sync: Option<SyncState>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub header: Option<TableHeader>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub order: Option<Vec<String>>,
  /// The branches whose parts changed. A branch the view lacks comes with its head and its commits; without its sync
  /// part it is `BranchSync::SYNCING`, without its copies none of its commits is reported, without its remote part it
  /// has no remote status
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub branches: BTreeMap<String, BranchChange>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_branches: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stacks: BTreeMap<String, StackBranchUpdate>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_stacks: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub unassigned: Option<Vec<Commit>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub archived: Option<ArchivedChanges>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub unapplied: Option<Vec<UnappliedBranchInfo>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub landed: Option<LandedSection>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub patronus: Option<PatronusChange>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub facts: Option<FactsChanges>,
  /// The news since the patch before, oldest first
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub news: Vec<News>,
}

/// What changed of the Patronus view: its states when they differ, the branches and the stacks whose facts changed
/// with the keys that went, the landings of the merged robots when they differ
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusChange {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub state: Option<FactState>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub landing_needs: Option<FactState>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub branches: BTreeMap<String, BranchPatronus>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stacks: BTreeMap<String, StackPatronus>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub landed_by_robot: Option<BTreeMap<String, Vec<RobotLandedBranch>>>,
  /// The branches and the stacks the view no longer lists
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_branches: Vec<String>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_stacks: Vec<String>,
}

/// The parts of the facts that changed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FactsChanges {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub robots: Option<RobotsChange>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub pull_requests: Option<PullRequestsChange>,
  /// All the rules, when one of them changed: a handful of bases
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub rules: Option<BTreeMap<String, Fact<BranchRules>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub profiles: Option<Fact<PatronusStartOptions>>,
  /// The readiness of the subjects that changed, and the subjects that went
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub readiness: BTreeMap<String, Fact<SubjectReadiness>>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_readiness: Vec<String>,
  /// The details of the robots that changed, and the robots no window demands any more
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub robot_details: BTreeMap<String, Fact<PatronusRobotDetails>>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_robot_details: Vec<String>,
  /// The whole part when it changed: at most 50 requests
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub review_requests: Option<ReviewRequestsView>,
}

/// What changed of the pull requests: the head when it differs, the pull requests that changed and the keys that went
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PullRequestsChange {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub head: Option<Fact<PullRequestAvailability>>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub pull_requests: BTreeMap<String, PullRequest>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_pull_requests: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack_pull_requests: BTreeMap<String, PullRequest>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_stack_pull_requests: Vec<String>,
}

/// What changed of the robots: the head when it differs, the robots that changed and the ids that went
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotsChange {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub head: Option<Fact<RobotsHead>>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub robots: BTreeMap<String, PatronusRobot>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_robots: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub failing: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub needs_review: Option<BTreeMap<String, bool>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub problems_failed: Option<BTreeMap<String, String>>,
}

impl TableChanges {
  /// Nothing changed
  #[must_use]
  pub fn is_empty(&self) -> bool {
    self == &Self::default()
  }
}

/// The parts of one branch that changed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchChange {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub head: Option<BranchHead>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub commits: Option<Vec<Commit>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub sync: Option<BranchSync>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub copies: Option<Vec<CommitSync>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub remote: Option<BranchRemote>,
}

/// The parts of the archived section that changed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ArchivedChanges {
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub names: Option<Vec<String>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub newly_archived: Option<Vec<String>>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub integration: BTreeMap<String, BranchIntegrationInfo>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_integration: Vec<String>,
}
