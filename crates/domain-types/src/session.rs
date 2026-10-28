//! What the session of a repository sends a window (docs/design/rust-domain-authority.md, "Представление: снимок и
//! патчи"): a snapshot of its view (`RepositoryView`) and the patches that follow it (`ViewPatch`). The table is cut
//! into parts that change for different reasons: of each branch its head and its commits (with a grouping), its sync
//! state (with its status and progress), the copies of its commits (with each commit the sync reports) and its remote
//! status, its structure (its stack, whether it is parked and the section of the table it is a row of, with a
//! grouping); the stack branches by root; the sections below the table. A patch carries a part only when it differs
//! from the one sent last, so the page keeps the identity of every part it did not get.
//!
//! The names on the wire are the ones the page reads today (`branch-sync.ts`). The statuses are data: the texts, the
//! marks of what to expand and the badges are the page's presentation of them. A fact a service or a computation
//! answers is a `Fact` with its `FactState` (the rule of a fact without an answer), and the facts of the remote services
//! are parts of the view beside the table (`FactsView`: the robots of Patronus by id), as is what the session derives
//! of them and the table (`PatronusPart`: the robot of each row and what the robots land, landed and hold, with the
//! landing needs, keyed by branch and stack), and so are the operations of the remote that run or wait in the session
//! (`RunningOperation`, by id); a window tells the session what it shows and needs (`Demand`) and asks it to read a fact
//! again (`RefreshFactsParams`).

use crate::actions::ActionState;
use crate::branch_integration::BranchIntegrationInfo;
use crate::commit::Commit;
use crate::conflict::WordResolutionSummary;
use crate::error::ApiError;
use crate::issue_navigation::IssueNavigationConfig;
use crate::landed::LandedBranch;
use crate::landing::{LandDecisions, Landed, LandedPullRequest};
use crate::next_step::NextStepView;
use crate::operation::{OperationId, RunningOperation};
use crate::patronus::{PatronusAvailability, PatronusReviewRequirement, PatronusRobot, PatronusRobotDetails, PatronusStartOptions, Person, ReviewReadiness, ReviewReadinessBranch};
use crate::pull_request::{BranchRules, PullRequest, PullRequestAvailability, PullRequestView};
use crate::reason::Reason;
use crate::repository_view::{BranchPatronus, PatronusView, RobotLandedBranch, RobotRestart, StackPatronus};
use crate::review_request::{ReviewEntry, ReviewRequest};
use crate::rewrite::TableWitness;
use crate::sign_in::AuthStatus;
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
  /// The window shows the Reviews view: the review requests are read with the rest of Reviews
  #[serde(default)]
  pub reviews: bool,
  /// The pull requests the review windows of the window show: their actions stay in the view while a window shows them,
  /// also once no listing has them any more
  #[serde(default)]
  pub review_windows: Vec<u32>,
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
  /// The sign-ins of the app's accounts (`FactsView::auth`): read when a window opens the session, on a focus by the
  /// rule of the lists, and at once after a sign-in or a sign-out of any window (`AfterWrite`, which the host asks of
  /// every session)
  Auth,
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
  /// `bd` changed the repository: it touched `$GIT_COMMON_DIR/branch-deck/stamp` after a mutation, which
  /// `operations::registry_watch` saw; runs when the inputs changed, as a focus does
  Stamp,
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
  /// Git refused the last sync before it built anything: a rebase, merge, cherry-pick or revert in progress
  /// (`operation_in_progress`), or a detached HEAD (`detached_head`). Nothing failed; the table is the one of the sync
  /// before, and the next sync starts once git finished (a focus sees the change)
  Deferred {
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
  /// The names of the branches in the order of the table, as the sync grouped them (newest first); the rows of each
  /// section are this order split by the section of each branch (`BranchStructure::section`), which the view sends
  /// once, never a second list of the same names
  pub order: Vec<String>,
  pub branches: BTreeMap<String, BranchView>,
  /// The stack branches the last sync reported, by the root of their stack
  pub stacks: BTreeMap<String, StackBranchUpdate>,
  /// What the pushes of each branch send, as the rules order them over the table and the holds of the robots
  /// (`domain_policy::push`); a branch whose pushes send it alone is not listed
  pub pushes: BTreeMap<String, BranchPush>,
  /// The open pull request of each subject (a branch, `stack:<root>` the whole stack) as the rules read it: where it
  /// goes, a draft, the code owners Request Review… offers (`domain_policy::pull_request`, `domain_policy::review`); a
  /// subject without an open pull request is not listed
  pub open_pull_requests: BTreeMap<String, PullRequestView>,
  /// What a Safe Push and a Dry Run of each subject (a branch, `stack:<root>` the whole stack) do now and what stands in
  /// their way, the decisions of their default intents (`domain_policy::land::decide`); none before the Patronus view
  pub lands: BTreeMap<String, LandDecisions>,
  /// What landed each subject (a branch, `stack:<root>` the whole stack), as `domain_policy::landed::landed_state`
  /// decides it over the landing report, the Patronus view and the pull requests: the fetched baseline, merged Safe
  /// Pushes (also a part of it), its pull request merged with its commits; a subject nothing landed is not listed
  pub landed_states: BTreeMap<String, Landed>,
  /// The next step and the node of each subject (a branch, `stack:<root>` the whole stack of a root that forks), as
  /// `domain_policy::next_step` and `domain_policy::landing_node` decide them over the table and the facts; none before
  /// the first sync
  pub next_steps: BTreeMap<String, NextStepView>,
  /// The actions of each subject (a branch, `stack:<root>` the whole stack of a root, `robot:<id>` a robot of the user)
  /// and whether each can run now, as `domain_policy::actions` decides them over the table, its next steps and the facts;
  /// none before the first sync
  pub actions: BTreeMap<String, Vec<ActionState>>,
  /// The subjects (a branch, `stack:<root>` the whole stack) whose robot needs the user, as `domain_policy::attention`
  /// decides it over the rows as they are and the robots: it failed on the subject as it is and nothing else holds the
  /// subject. The landing overview, Attention and the Dock badge count them; none before the first sync
  pub attention: BTreeMap<String, RobotAttention>,
  /// The operations of the remote that run or wait in the session, by id, as `operations::registry` admitted them
  pub operations: BTreeMap<OperationId, RunningOperation>,
  pub sections: TableSections,
  /// Every open pull request of the repository the user takes part in beside the requests by name, by section, as
  /// `domain_policy::reviews` orders them over the review requests and the table
  pub reviews: ReviewsView,
  /// What the robots of Patronus land, landed and hold of the table, as the session derives it
  pub patronus: PatronusPart,
  /// What the remote services answered, beside the table
  pub facts: FactsView,
  /// The last news of the session (at most `NEWS_KEPT`), oldest first: a window tells each once, by its id, and none
  /// that its first view had already
  pub news: Vec<News>,
}

/// The robot of a subject that needs the user (`RepositoryView::attention`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotAttention {
  /// The robot of the subject's row (`robotId` of the Patronus view), by id
  pub robot_id: String,
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
  /// The sign-ins of the app's accounts and where their tokens come from (`remote_facts::sign_in`): one status of the
  /// app, which every session reads by its rule (`domain_policy::refresh::auth_due`)
  pub auth: Fact<AuthStatus>,
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

/// Reviews (spec/repository/reviews.spec.md) as the session derives it: how the rest of Reviews beside the requests by
/// name stands (`head`: `Unknown` while no window shows the view and nothing was read, `Loading` until its first answer,
/// then as the review requests), its sections in their order, the empty ones left out, each pull request in the first
/// section that has it, and when GitHub gave them
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewsView {
  pub head: Fact<PullRequestAvailability>,
  pub sections: Vec<ReviewSection>,
  /// Unix seconds of the last answer with them, 0 = never
  pub checked_at: u32,
}

/// A section of Reviews after the requests by name (Your Turn's rows lead it)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReviewSectionKind {
  /// Added by link
  Linked,
  /// Requested from the user's teams
  Team,
  /// Reviewed by the user
  Reviewed,
  /// The user's own pull requests
  Authored,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewSection {
  pub kind: ReviewSectionKind,
  pub entries: Vec<ReviewEntry>,
  /// How many match the search of the section, also beyond the 50 GitHub returns; the entries of a linked section
  pub total_count: u32,
  /// The pull requests of the section whose move is the user's, in its order (`domain_policy::reviews::is_users_move`)
  pub moves: Vec<u32>,
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
  /// A robot known running or canceling succeeded and pushed (not a dry run): it merged what it tested. Told once the
  /// Patronus view of the robots it merged in stands, with the open pull requests of the branches it landed whole besides
  /// its own (`closes`, `domain_policy::landed::pull_requests_to_close`), which the app closes
  RobotMerged {
    #[serde(rename = "robotId")]
    robot_id: String,
    closes: Vec<LandedPullRequest>,
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
  /// The approvals of a subject that waited for them arrived (`domain_policy::news::approval_news`): whose
  /// approval counts now in the groups that waited (none named: the code owners), in the pull request of its start
  ApprovalsArrived {
    subject: String,
    #[serde(rename = "pullRequest")]
    pull_request: u32,
    approvers: Vec<Person>,
    groups: Vec<String>,
  },
  /// Someone else pushed to the remote branch of `branch`, which the session fetched when its pull request showed the
  /// new head (`moved_remote`): `count` commits of others (`others_behind`) the branch lacks at the remote tip `tip`, by
  /// `authors` (newest first, each once). Told once per tip
  RemoteCommits { branch: String, tip: String, count: u32, authors: Vec<String> },
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
  /// The branch of origin the baseline tracks, which Patronus pushes into; none before the first grouping and for a
  /// baseline that is no branch of origin
  pub target_branch: Option<String>,
  /// The commit HEAD pointed at when the commits were grouped; empty before the first grouping
  pub head: String,
  /// The checked out branch whose commits were grouped (`main`); empty before the first grouping
  pub branch: String,
  /// The tips of the refs of Branch Deck the commits were grouped with
  pub store_tips: StoreTips,
}

impl TableHeader {
  /// What a rewrite asked on this table names it by (`RewriteIntent`): HEAD and the store tips of the grouping
  #[must_use]
  pub fn witness(&self) -> TableWitness {
    TableWitness {
      head: self.head.clone(),
      store_tips: self.store_tips.clone(),
    }
  }
}

/// The commits of the refs of Branch Deck that a grouping reads and the rewrites write
/// (`refs/branch-deck/issue-groups`, `stack-parents`, `parked`); `None` for a ref that does not exist
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StoreTips {
  pub issue_groups: Option<String>,
  pub stack_parents: Option<String>,
  pub parked: Option<String>,
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
  /// Where the branch is among the others: its stack and whether it is parked; it changes only with a grouping (and,
  /// of the root of a whole stack, with its stack branch)
  pub structure: BranchStructure,
}

/// Where a branch is among the branches of the table, as the rules of `domain_policy::stack` and `domain_policy::parked`
/// decide it for the app and `bd` alike (`repository_model::derive`)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchStructure {
  /// The branch of the table it stacks on, its declared parent; absent for a root or a parent the table lacks
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub parent: Option<String>,
  /// The branches stacked directly on it, in the order of the table
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub children: Vec<String>,
  /// The branches it is built on, root first: the chain of its effective bases among the branches of the table
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub ancestors: Vec<String>,
  /// The branches stacked on it, directly or further down, each after its parent
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub descendants: Vec<String>,
  /// The main line of its stack below it: at each fork the child with the longest chain (lane 0 of the stack graph),
  /// empty when nothing is stacked on it. A Safe Push of its last branch lands the branch and the line in one robot
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub main_line: Vec<String>,
  /// Why it stays local: its own line of the parked branches, or a parked branch it is built on; absent otherwise
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub parked: Option<Parked>,
  /// Of the root of a stack that forks: the first member of its stack branch that is parked, which keeps the stack
  /// branch local; absent otherwise
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack_parked: Option<ParkedMember>,
  /// Of the root of a stack: its whole stack, which one Safe Push lands (`domain_policy::stack::whole_stack_of`, by
  /// declared parent); absent for a branch alone and for a branch that heads no stack
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub whole_stack: Option<WholeStackView>,
  /// The table it is a row of, in the order of the table (`RepositoryView::order`); absent for the main one
  #[serde(skip_serializing_if = "BranchSection::is_main")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub section: BranchSection,
}

/// The whole stack of a root (spec/branch/whole-stack.spec.md): the stack branch of a stack that forks, or the last
/// branch of a straight chain, which carries every commit of it
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum WholeStackView {
  /// The stack forks: its stack branch `{prefix}/stack/<root>` lands it (what of it is left to land:
  /// `StackLanding::rest`); `not_built` why the sync did not build it, the stack lands in pieces then
  Forking {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "specta", specta(optional))]
    not_built: Option<Reason>,
  },
  /// A straight chain of two branches or more: `leaf`, its last branch that is not parked, lands it
  Chain { leaf: String },
}

/// Why a branch is parked (spec/branch/parked-branches.spec.md)
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Parked {
  /// The note of its line, or of the line of `with`; empty without one
  pub note: String,
  /// The parked branch it is built on, whose commits it carries; `None` when its own line parks it
  pub with: Option<String>,
}

/// A member of a whole stack and why it is parked
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ParkedMember {
  pub member: String,
  pub parked: Parked,
}

/// The table a branch is a row of: the main one, or the Parked card of the branches kept local
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum BranchSection {
  #[default]
  Main,
  Parked,
}

impl BranchSection {
  #[must_use]
  pub const fn is_main(&self) -> bool {
    matches!(self, Self::Main)
  }
}

/// What the pushes of a branch send besides the branch, root first, as `domain_policy::push::push_plan` orders them over
/// the table and the holds of the robots: the branch goes last in each, and an empty list sends it alone. A list absent
/// is the one of its structure, most of the time: the view sends only what differs (`RepositoryView::pushes`)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchPush {
  /// Push Stack: the branches it is built on that have work for the remote, root first, up to the nearest one a running
  /// Safe Push holds; one a merged Safe Push landed whole, or the fetched baseline has whole, is left out. Absent while
  /// it is every branch it is built on (`BranchStructure::ancestors`)
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack: Option<Vec<String>>,
  /// The push of the push button and of an action that needs the branch on the remote (a pull request, a review
  /// request): as `stack`, starting above the nearest branch whose push drops remote commits, which asks first in its
  /// row; absent while none of `stack` asks first (`stack` then). A Safe Push start pushes what its decision does (the
  /// `Push` effect, `domain_policy::push::start_push`)
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub action: Option<Vec<String>>,
  /// The push button pushes the stack, `action` and then the branch: the branch has work for the remote and asks nothing
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub button: bool,
  /// Push with Children; absent for a branch nothing but parked branches is stacked on
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub children: Option<ChildrenPush>,
  /// Push Up to Here of the commits of the branch the remote lacks; absent while every one of them offers it, it sends
  /// the branches of Push Stack first (`stack`) and nothing stops it
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub up_to: Option<PushUpTo>,
}

/// Push Up to Here of a branch (`domain_policy::push::up_to_push`): the branches it is built on that have work for the
/// remote go first, whole, root first, then the branch up to the commit, in one atomic push. A field absent is the
/// common answer, so most branches send none
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PushUpTo {
  /// The copies of the commits of the branch that offer it, oldest first: of its own commits the remote lacks, those no
  /// merged Safe Push landed; absent while it is every one of them (`RemoteStatusUpdate::unpushed_commits`). The last
  /// commit the remote lacks pushes the whole branch
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub commits: Option<Vec<String>>,
  /// The branches it is built on that it pushes first, whole, root first; absent while they are those of Push Stack
  /// (`BranchPush::stack`)
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub ancestors: Option<Vec<String>>,
  /// Why it can't push now: the branch stays local, a Safe Push holds it, the sync did not build it, it was rewritten
  /// since its push, or a branch it is built on is held, not built or asks first
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub blocker: Option<Reason>,
}

/// What Push with Children of a branch sends, in one atomic push, parents first
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ChildrenPush {
  /// The branch while it has work for the remote, then the branches stacked on it that have work, each after its
  /// parent: none parked, none a Safe Push holds or a merged one landed, none whose push drops remote commits. Absent
  /// while it is the branch and every branch stacked on it (`BranchStructure::descendants`)
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub order: Option<Vec<String>>,
  /// The branches stacked on it whose push drops remote commits they lack: left out, each asks first in its row
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub ask_first: Vec<String>,
  /// Of the root of a stack that forks: what the push does with the stack branch of the whole stack
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack_branch: Option<StackBranchAlong>,
  /// It pushes more than the branch itself: a branch stacked on it, or the stack branch along
  #[serde(skip_serializing_if = "std::ops::Not::not")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub offers: bool,
}

/// What Push with Children of the root of a stack that forks does with the stack branch of its whole stack
/// (`domain_policy::push::stack_branch_along`): `along` pushes it in the same atomic push, after the branches, so the
/// pull request of the whole stack follows them; any other kind leaves it as it is, and says why
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum StackBranchAlong {
  /// Built, on the remote or with its pull request open, its remote lacking commits of it
  Along,
  /// On the remote as it is
  UpToDate,
  /// Neither on the remote nor with an open pull request: the pull request or the Safe Push of the whole stack puts it
  /// there
  Unpublished,
  /// The sync did not build it: never pushed
  NotBuilt { reason: Reason },
  /// Its remote branch has commits it lacks: a push would drop them, as a child that asks first in its row
  DropsRemote,
  /// Its push is held (`StackLanding::hold`): a member is parked, every branch of the stack landed, its own robot runs,
  /// or the sync did not build it
  Held { reason: Reason },
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
  /// `description_title` split at the issue key it starts with, which a row that shows the key leaves out
  /// (`sync_utils::issue_pattern::issue_lead`)
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub description_lead: Option<crate::commit::IssueLead>,
  /// The issue key the name starts with (`IJPL-1` of `IJPL-1` and of `IJPL-1-cache`), which the row shows in the name
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub name_issue_key: Option<String>,
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
  /// has no remote status, without its structure it is a root of the main table with nothing stacked on it
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
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub pushes: BTreeMap<String, BranchPush>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_pushes: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub open_pull_requests: BTreeMap<String, PullRequestView>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_open_pull_requests: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub lands: BTreeMap<String, LandDecisions>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_lands: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub landed_states: BTreeMap<String, Landed>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_landed_states: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub next_steps: BTreeMap<String, NextStepView>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_next_steps: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub actions: BTreeMap<String, Vec<ActionState>>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_actions: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub attention: BTreeMap<String, RobotAttention>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_attention: Vec<String>,
  #[serde(skip_serializing_if = "BTreeMap::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub operations: BTreeMap<OperationId, RunningOperation>,
  #[serde(skip_serializing_if = "Vec::is_empty")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub removed_operations: Vec<OperationId>,
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
  pub reviews: Option<ReviewsView>,
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
/// with the keys that went, the landings of the merged robots and the restarts of the robots when they differ
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
  /// The restarts of the robots, whole, when they differ
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub restarts: Option<BTreeMap<String, RobotRestart>>,
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
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub auth: Option<Fact<AuthStatus>>,
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
  #[serde(skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub structure: Option<BranchStructure>,
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
