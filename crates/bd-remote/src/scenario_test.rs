//! The scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` as a repository of the
//! bd-cli fixtures with the fake Patronus and the fake GitHub, and the one expectation (`expect`) both sides answer of
//! it. The app's side: the table its sync folds (`TableState`) and the facts its session holds of the same fakes
//! (`FactStore`, fed as the session's host feeds it), through `repository_model::derive` (`next_steps_of`,
//! `land_answer_of`). The side of `bd`: `safe_push()` with `--check` (the production path: its preamble, `plan()`, the
//! output of a check, with the decision and the step its words tell), then the start without `--check`

use crate::push_command::github_repository;
use crate::safe_push_command::{SafePushArgs, SafePushOutput, safe_push as safe_push_command};
use bd_cli::fixture_test::{PREFIX, commit_all, commit_changed_declaration, commit_dangling_reference, git, run_json, write};
use bd_cli::remote_fixture_test::{
  LANDED_SUBJECTS, MASTER_RULES, Space, dismissing_rules, evaluated_review, fake_github, land_auth_by_content, list_pull_requests, minutes_ago, pull_request_json,
  push_from_elsewhere_to, rebuild_on_a_newer_baseline, reviewed_on_github, robot, safe_push, stack_robot,
};
use bd_core::context::{Environment, RepoContext, RepoOptions};
use bd_core::error::CliError;
use bd_core::services::{github_service, patronus_service};
use bd_core::stack_summary::whole_stack;
use bd_core::sync_command::{lock, sync_locked};
use domain_policy::land::unaccepted;
use domain_policy::stack::{WholeStack, whole_stack_of};
use domain_types::branch_names::baseline_on_remote;
use domain_types::commit::Commit;
use domain_types::error::ApiError;
use domain_types::landing::{Land, LandDecision, PlannedEffect, ProfileChoice, StartRoute, Subject};
use domain_types::next_step::NextStep;
use domain_types::patronus::{LandingAllowances, PatronusAvailability, PatronusPushMode, PatronusRobotsResult};
use domain_types::pull_request::{PullRequestAvailability, PullRequestsResult};
use domain_types::session::{Demand, stack_readiness_key};
use domain_types::stack::BranchStack;
use domain_types::{ProgressReporter, SyncEvent};
use github::fake_github::{FakeAnswer, FakeGitHub};
use history_rewrite::commit_grouper::{CommitGrouper, GroupingInputs};
use history_rewrite::stack::place_branches;
use history_rewrite::store_lines::StoreLines;
use remote_facts::branch_rules::read_branch_rules;
use remote_facts::pull_requests::list_pull_requests as list_open_pull_requests;
use remote_facts::review_readiness::{ReadinessRepository, branch_review_readiness};
use repository_model::derive::{land_answer_of, landed_states_of, next_steps_of, repository, session_open_pull_requests, wanted_readiness};
use repository_model::facts::{AskedRobots, FactResult, FactStore, Observation, RobotsRead, Seq, derive_patronus, robots_digest};
use repository_model::table::TableState;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::SpaceRepository;
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::git_test_utils::TestRepo;
use trial_check::landing_gate::landing_needs_of;

/// The groups of the cases and their test functions here
const GROUPS: [&str; 7] = [
  "modes-and-routes",
  "profiles",
  "review",
  "next-step-rows",
  "stack-and-whole-stack",
  "landing",
  "discrepancies",
];

/// The ids of the registry of `docs/design/rust-domain-authority.md`: the group `discrepancies` has one entry of each
const REGISTRY: [&str; 16] = ["D1", "D2", "D3a", "D3b", "D4", "D5", "D6", "D7", "D8", "D9", "D10", "D11", "D12", "D13", "D14", "D15"];

/// A case of the table
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
  id: String,
  group: String,
  /// `exercised` (the default): both sides answer `given`; `recorded`: a discrepancy no decision of either side reaches,
  /// recorded by `today`
  #[serde(default)]
  kind: CaseKind,
  title: String,
  spec: Vec<String>,
  /// Of an entry of the registry only: the id of the registry it is the key of
  discrepancy: Option<String>,
  /// Of an entry of the registry: the stage of the design doc that closed the discrepancy; the entry stays the key of
  /// its id, and `today` says where each side does it now
  closed: Option<String>,
  /// Where each side does it today: `path#text` or `path:line#text` (a test, a function, a line), from the repository root
  today: Option<Today>,
  /// Why `bd` answers otherwise by design, beside its answer `bd`
  asymmetry: Option<Asymmetry>,
  given: Option<Given>,
  /// What both sides answer of `given`
  expect: Option<Expect>,
  /// What `bd` answers instead, only beside an `asymmetry`
  bd: Option<BdExpect>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum CaseKind {
  #[default]
  Exercised,
  Recorded,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Today {
  app: Vec<String>,
  bd: Vec<String>,
  /// What the pointers show
  #[expect(dead_code, reason = "words for the reader of the registry")]
  why: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asymmetry {
  #[expect(dead_code, reason = "its only value: a case of another kind fails to parse")]
  kind: AsymmetryKind,
  /// The spec or the doc that names the difference
  #[expect(dead_code, reason = "words for the reader of the table")]
  why: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum AsymmetryKind {
  ByDesign,
}

/// The one expectation of a case: `step` a subset of the next step of the subject (`step_answer`), `land` of the
/// decision of its start (`land_answer`); `null` where the side has none
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expect {
  step: Value,
  land: Value,
}

/// What `bd` answers instead of `expect`, by design: its own `step` or `land`, or the error of the command, which
/// refuses before it decides anything (the table alone refuses it, an argument)
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BdExpect {
  step: Option<Value>,
  land: Option<Value>,
  error: Option<String>,
}

/// A sync of the app that runs
#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum GivenSync {
  Running,
}

/// The closed vocabulary of a situation, the schema of the README
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
struct Given {
  mode: Mode,
  /// The branch the case starts
  branch: String,
  subject: GivenSubject,
  commits: Vec<String>,
  target: Target,
  remote: Remote,
  robot: Option<GivenRobots>,
  /// The app's sync runs, past its grouping: `bd` syncs itself before it plans, so nothing of it is a state of bd
  sync: Option<GivenSync>,
  /// The first commit of this branch changes a file that only an unassigned commit creates
  conflicts: Option<String>,
  not_on_space: Option<bool>,
  /// The open pull request #12 of the stack branch of the root
  stack_pull_request: Option<bool>,
  pull_request: Option<GivenPullRequest>,
  readiness: Readiness,
  profile: Option<GivenProfile>,
  flags: Vec<Flag>,
  /// What GitHub answers `GET rules/branches/master`
  rules: Rules,
  /// `self`, `builtOn` (its parent), or the name of a branch of the stack
  parked: Option<String>,
  stack_branch: Option<StackBranch>,
  landed: Option<Landed>,
  landing_needs: Option<bool>,
  /// The commits of a fixture that make a dangling reference, instead of `commits`
  dangling: Option<Dangling>,
}

impl Default for Given {
  fn default() -> Self {
    Self {
      mode: Mode::SafePush,
      branch: "auth".to_string(),
      subject: GivenSubject::Branch,
      commits: vec!["(auth) Add auth".to_string()],
      target: Target::Protected,
      remote: Remote::None,
      robot: None,
      sync: None,
      conflicts: None,
      not_on_space: None,
      stack_pull_request: None,
      pull_request: None,
      readiness: Readiness::NotRequired,
      profile: None,
      flags: Vec::new(),
      rules: Rules::Keep,
      parked: None,
      stack_branch: None,
      landed: None,
      landing_needs: None,
      dangling: None,
    }
  }
}

/// The rules of the target as GitHub answers them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Rules {
  /// No rule dismisses the approvals on a push
  Keep,
  /// The pull request rule dismisses stale reviews on a push
  Dismiss,
  /// GitHub fails the read (500)
  Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Mode {
  SafePush,
  DryRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum GivenSubject {
  Branch,
  WholeStack,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Target {
  Protected,
  Unprotected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Remote {
  None,
  UpToDate,
  Behind,
  Unpushed,
  RebuiltOnNewerBaseline,
}

/// One robot, or several
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GivenRobots {
  One(GivenRobot),
  Many(Vec<GivenRobot>),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct GivenRobot {
  state: RobotState,
  /// `tip`, `older`, `stack`, `ancestor:<branch>`, `child:<branch>`
  on: String,
  /// `approvals`: it failed for the approvals, as its problems say (the session reads them; `bd` doesn't)
  reason: Option<String>,
  /// Started by the comment on the pull request #7
  #[serde(default)]
  from_pull_request: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Dangling {
  /// `commit_dangling_reference`: `fake-agent` removes a constant that the tree still uses, `lane-document` changes the use
  Removed,
  /// `commit_changed_declaration`: `suspend` makes a function suspend, `journey` deletes the test that calls it
  Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum StackBranch {
  /// The last branch of the stack has a stack problem: the stack branch is not built
  NotBuilt,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Landed {
  /// origin/master has every commit
  InBaseline,
  /// `land_auth_by_content`: the baseline has the commits rebuilt, one added since
  ByContent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RobotState {
  Running,
  Failure,
  Canceled,
  Success,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
struct GivenPullRequest {
  /// The branch of the pull request: the case's branch by default
  of: Option<String>,
  base: String,
  draft: bool,
  approvers: Vec<String>,
  reviewers: Vec<String>,
  changes_requested: Vec<String>,
  state: PullRequestState,
}

impl Default for GivenPullRequest {
  fn default() -> Self {
    Self {
      of: None,
      base: "master".to_string(),
      draft: false,
      approvers: Vec::new(),
      reviewers: Vec::new(),
      changes_requested: Vec::new(),
      state: PullRequestState::Open,
    }
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum PullRequestState {
  Open,
  Closed,
  Merged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Readiness {
  NotRequired,
  Approved,
  Missing,
  Unknown,
  Unavailable,
  Unlisted,
  Space,
  /// The app's session didn't ask yet; bd always asks: the pre-check needs no approval
  NotAsked,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
struct GivenProfile {
  /// `--profile`: an id, or `none`
  chosen: Option<String>,
  /// The profile of the last robot: `lsc`, the one the fake names
  last_robot: Option<String>,
  skips_review: bool,
  /// The robot list names the last robot's profile (Settings → Patronus)
  new_api: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Flag {
  WithoutApprovals,
  CancelRunning,
  CreatePullRequest,
  AllowMissing,
  AllowDangling,
  AllowUnknownRules,
}

impl Given {
  fn has(&self, flag: Flag) -> bool {
    self.flags.contains(&flag)
  }

  /// The robots of the case, one or several
  fn robots(&self) -> &[GivenRobot] {
    match &self.robot {
      None => &[],
      Some(GivenRobots::One(robot)) => std::slice::from_ref(robot),
      Some(GivenRobots::Many(robots)) => robots,
    }
  }

  /// The values the adapter maps (the README says which); anything else fails the case
  fn check_mapped(&self) {
    assert!(
      !self.commits.is_empty() || self.dangling.is_some(),
      "given.commits [] goes with given.dangling, which brings its own"
    );
    if let Some(dangling) = self.dangling {
      let branch = match dangling {
        Dangling::Removed => "fake-agent",
        Dangling::Changed => "suspend",
      };
      assert!(
        self.commits.is_empty() && self.branch == branch,
        "given.dangling {dangling:?} brings its own commits: commits [], branch {branch}"
      );
    }
    if self.landed == Some(Landed::ByContent) {
      assert!(
        self.branch == "auth" && self.commits == LANDED_SUBJECTS,
        "given.landed byContent is auth of LANDED_SUBJECTS"
      );
    }
    if let Some(profile) = &self.profile {
      assert!(profile.last_robot.is_none() || self.robot.is_some(), "given.profile.lastRobot needs given.robot");
      assert!(
        profile.last_robot.as_deref().is_none_or(|id| id == "lsc"),
        "given.profile.lastRobot: the fake names only lsc"
      );
      assert!(
        !profile.skips_review || profile.last_robot.is_some(),
        "given.profile.skipsReview needs given.profile.lastRobot"
      );
      assert!(
        !profile.new_api || profile.skips_review,
        "given.profile.newApi: the list of the fake says skipsReview: true"
      );
    }
  }

  /// The branch of a commit and the parent it declares: `(auth/login) …` is login on auth
  fn group_of(subject: &str) -> (&str, Option<&str>) {
    let prefix = subject.strip_prefix('(').and_then(|rest| rest.split_once(')')).map_or("", |(prefix, _)| prefix);
    match prefix.rsplit_once('/') {
      Some((parent, name)) => (name, Some(parent)),
      None => (prefix, None),
    }
  }

  /// The parent `name` declares by its first commit
  fn parent_of(&self, name: &str) -> Option<&str> {
    self
      .commits
      .iter()
      .map(|subject| Self::group_of(subject))
      .find(|(branch, _)| *branch == name)
      .and_then(|(_, parent)| parent)
  }

  /// The root of the stack of `name`
  fn root_of<'a>(&'a self, name: &'a str) -> &'a str {
    let mut root = name;
    for _ in 0..self.commits.len() {
      match self.parent_of(root) {
        Some(parent) => root = parent,
        None => break,
      }
    }
    root
  }

  /// Stacked branches: a commit declares a parent, or the landing gate's names need a stack
  fn stacks(&self) -> bool {
    self.landing_needs.is_some() || self.commits.iter().any(|subject| Self::group_of(subject).1.is_some())
  }

  fn args(&self, check: bool) -> SafePushArgs {
    SafePushArgs {
      branch: self.branch.clone(),
      patronus_dry_run: self.mode == Mode::DryRun,
      profile: self.profile.as_ref().and_then(|profile| profile.chosen.clone()),
      check,
      without_approvals: self.has(Flag::WithoutApprovals),
      create_pull_request: self.has(Flag::CreatePullRequest),
      cancel_running: self.has(Flag::CancelRunning),
      allow_dangling: self.has(Flag::AllowDangling),
      allow_missing: self.has(Flag::AllowMissing),
      allow_unknown_rules: self.has(Flag::AllowUnknownRules),
      whole_stack: self.subject == GivenSubject::WholeStack,
      ..SafePushArgs::default()
    }
  }

  /// The command line of `args(false)`
  fn command_line(&self) -> Vec<String> {
    let args = self.args(false);
    let mut line = vec![self.branch.clone()];
    let flags = [
      (args.patronus_dry_run, "--patronus-dry-run"),
      (args.without_approvals, "--without-approvals"),
      (args.create_pull_request, "--create-pull-request"),
      (args.cancel_running, "--cancel-running"),
      (args.allow_dangling, "--allow-dangling"),
      (args.allow_missing, "--allow-missing"),
      (args.allow_unknown_rules, "--allow-unknown-rules"),
      (args.whole_stack, "--whole-stack"),
    ];
    line.extend(flags.into_iter().filter(|(on, _)| *on).map(|(_, flag)| flag.to_string()));
    if let Some(profile) = args.profile {
      line.extend(["--profile".to_string(), profile]);
    }
    line
  }

  /// What the user accepts by the flags of the case, the same for both sides: the popover's Start Anyway and its line
  /// of the robot it replaces are the flags of `bd`
  fn allowances(&self) -> LandingAllowances {
    LandingAllowances {
      allow_dangling: self.has(Flag::AllowDangling),
      allow_missing: self.has(Flag::AllowMissing),
      allow_force: false,
      allow_unknown_rules: self.has(Flag::AllowUnknownRules),
      allow_unknown_review: false,
      cancel_running: self.has(Flag::CancelRunning),
    }
  }

  /// The start of `subject` the case asks, as the app's click asks it with the choices of its popover
  fn land(&self, subject: Subject) -> Land {
    Land {
      subject,
      mode: match self.mode {
        Mode::SafePush => PatronusPushMode::RebaseAutosquash,
        Mode::DryRun => PatronusPushMode::DryRun,
      },
      profile: match self.profile.as_ref().and_then(|profile| profile.chosen.as_deref()) {
        None => ProfileChoice::Default,
        Some("none") => ProfileChoice::None,
        Some(id) => ProfileChoice::Id { id: id.to_string() },
      },
      without_approvals: self.has(Flag::WithoutApprovals),
      create_pull_request: self.has(Flag::CreatePullRequest),
      allow: self.allowances(),
    }
  }
}

/// The repository of a case, its fakes and the environment bd runs in
struct Situation {
  /// Serves the fakes while the case runs; its Patronus URL names the robot pages
  space: Space,
  environment: Environment,
  _github: Option<Arc<FakeGitHub>>,
}

/// The full name of the virtual branch of `name`
fn virtual_branch(name: &str) -> String {
  format!("{PREFIX}/virtual/{name}")
}

/// The pull request #7 of the case's branch (or of `of`) as the GitHub search lists it
fn pull_request_of(given: &GivenPullRequest, branch: &str) -> Value {
  let branch = given.of.as_deref().unwrap_or(branch);
  let approvers: Vec<&str> = given.approvers.iter().map(String::as_str).collect();
  let mut pull_request = pull_request_json(&approvers);
  let reviews = pull_request["latestReviews"]["nodes"].as_array_mut().unwrap();
  reviews.extend(
    given
      .changes_requested
      .iter()
      .map(|login| json!({"author": {"login": login}, "state": "CHANGES_REQUESTED"})),
  );
  let requests: Vec<Value> = given
    .reviewers
    .iter()
    .map(|login| json!({"requestedReviewer": {"__typename": "User", "login": login}}))
    .collect();
  pull_request["reviewRequests"] = json!({"nodes": requests});
  pull_request["author"] = json!({"login": "me"});
  pull_request["headRefName"] = json!(virtual_branch(branch));
  pull_request["isDraft"] = json!(given.draft);
  pull_request["baseRefName"] = json!(given.base);
  pull_request["state"] = json!(format!("{:?}", given.state).to_uppercase());
  pull_request
}

/// The open pull request #12 of the stack branch of `root` into master
fn stack_pull_request_of(root: &str) -> Value {
  let mut pull_request = pull_request_json(&[]);
  pull_request["number"] = json!(12);
  pull_request["url"] = json!("https://github.com/acme/monorepo/pull/12");
  pull_request["author"] = json!({"login": "me"});
  pull_request["headRefName"] = json!(format!("{PREFIX}/stack/{root}"));
  pull_request
}

/// The options of the robot `older` as `GET /robots/{id}` answers them: the profile of `profile.lastRobot`, and whether
/// it skips the review
fn add_profile_details(space: &Space, environment: &mut Environment, listed: &Value, profile: Option<&GivenProfile>) {
  let mut details = listed.clone();
  details["finishDateTime"] = json!(minutes_ago(120));
  details["options"] = match profile.filter(|profile| profile.last_robot.is_some()) {
    Some(profile) if profile.skips_review => {
      json!({"patronus.merge.profile.id": "lsc", "patronus.merge.profile.name": "Large Scale Change", "patronus.review.approval.check.skip": "true"})
    }
    Some(_) => json!({"patronus.merge.profile.id": "lsc", "patronus.merge.profile.name": "Large Scale Change"}),
    None => json!({}),
  };
  if profile.is_some_and(|profile| profile.new_api) {
    environment.settings.patronus_new_api = true;
    space.fake.list_profiles.store(true, Ordering::SeqCst);
  }
  space.fake.details.lock().unwrap().insert("older".to_string(), details);
}

/// The robots of the case as the list answers them, after the list `reviewed_on_github` left: `older` (`sourceRevision`
/// `0000`), the others on the tips the sync built of their branch or of the stack branch; the first the newest
fn add_robots(space: &Space, environment: &mut Environment, given: &Given) {
  let mut robots = space.fake.robots.lock().unwrap().take().unwrap_or_default();
  for (index, robot_of) in given.robots().iter().enumerate() {
    let state = format!("{:?}", robot_of.state).to_uppercase();
    let id = if robot_of.on == "older" { "older".to_string() } else { format!("robot-{index}") };
    let tip_of = |name: &str| space.local_tip(name);
    let mut listed = match robot_of.on.split_once(':') {
      Some((_, other)) => robot(&id, &state, other, &tip_of(&virtual_branch(other))),
      None if robot_of.on == "tip" => robot(&id, &state, &given.branch, &tip_of(&virtual_branch(&given.branch))),
      // the remote branch, which the sync rebuilt since on a newer baseline (`remote: rebuiltOnNewerBaseline`)
      None if robot_of.on == "remote" => robot(&id, &state, &given.branch, &space.remote_tip(&virtual_branch(&given.branch))),
      None if robot_of.on == "older" => robot(&id, &state, &given.branch, "0000"),
      None => {
        let root = given.root_of(&given.branch);
        stack_robot(&id, &state, &tip_of(&format!("{PREFIX}/stack/{root}")))
      }
    };
    // the first the newest, each next started a minute earlier, all after the robot `reviewed_on_github` left (180
    // minutes ago): the list (by start, the newest first) and the app's preference by start time are the same on every
    // run, where starts of the same second would tie by whatever order the fake keeps
    listed["startDateTime"] = json!(minutes_ago(170 + u32::try_from(index).unwrap()));
    if robot_of.state != RobotState::Running {
      // in the order of the list: the first finished first
      listed["finishDateTime"] = json!(minutes_ago(90 - u32::try_from(index).unwrap()));
    }
    if robot_of.from_pull_request {
      listed["githubPrUrl"] = json!("https://github.com/acme/monorepo/pull/7");
    }
    if robot_of.on == "older" {
      add_profile_details(space, environment, &listed, given.profile.as_ref());
    }
    // the problems the fake answers by default say it failed for the approvals
    if robot_of.reason.as_deref() != Some("approvals") {
      space.fake.problems.lock().unwrap().insert(id, json!([]));
    }
    robots.push(listed);
  }
  *space.fake.robots.lock().unwrap() = Some(robots);
}

/// The commits of the case on the baseline, as `commit_subjects` makes them (`f<i>.txt` with the subject), and what
/// `conflicts`, `landingNeeds` and `stackBranch` add
fn commit_given(space: &Space, given: &Given) {
  let repo = &space.repo;
  if given.stacks() {
    repo.set_config("branchdeck.stacks", "true").unwrap();
  }
  let mut time = 1_700_000_000;
  let mut commit = |subject: &str| {
    commit_all(repo, subject, time);
    time += 100;
  };
  if given.conflicts.is_some() {
    write(repo, "config.txt", "a\n");
    commit("Add config");
  }
  match given.dangling {
    Some(Dangling::Removed) => drop(commit_dangling_reference(repo)),
    Some(Dangling::Changed) => drop(commit_changed_declaration(repo)),
    None => {}
  }
  if given.landing_needs.is_some() {
    write(repo, "src/a/Util.kt", "package a\n\nfun controlUrl(): String = \"\"\n");
    commit("(codec) Add controlUrl");
  }
  let mut conflicted = false;
  for (index, subject) in given.commits.iter().enumerate() {
    write(repo, &format!("f{index}.txt"), subject);
    if !conflicted && given.conflicts.as_deref() == Some(Given::group_of(subject).0) {
      write(repo, "config.txt", "b\n");
      conflicted = true;
    }
    commit(subject);
  }
  if given.landing_needs.is_some() {
    write(repo, "src/b/Use.kt", "package b\n\nimport a.controlUrl\n\nfun start() = controlUrl()\n");
    commit(&format!("({}) Start with controlUrl", given.branch));
  }
  if given.stack_branch == Some(StackBranch::NotBuilt) {
    let last = given.commits.last().map(|subject| Given::group_of(subject).0).unwrap();
    write(repo, "loop.txt", "loop\n");
    commit(&format!("({last}/{last}) Loop"));
  }
}

/// A Safe Push landed every commit of the branch, and the baseline with them was fetched: origin/master has them rebuilt
/// (`cherry-pick -x`), HEAD has them as they were until Update Project
fn land_in_the_baseline(repo: &TestRepo) {
  let (head, branch) = (git(repo, &["rev-parse", "HEAD"]), git(repo, &["symbolic-ref", "--short", "HEAD"]));
  git(repo, &["checkout", "-q", "--detach", "refs/remotes/origin/master"]);
  git(repo, &["cherry-pick", "-x", &format!("refs/remotes/origin/master..{head}")]);
  git(repo, &["update-ref", "refs/remotes/origin/master", "HEAD"]);
  git(repo, &["checkout", "-q", &branch]);
}

/// `bd <args>` in the repository of the case, which succeeds
fn bd_ok(environment: &Environment, args: &[&str]) {
  let mut all_args = vec!["--prefix", PREFIX];
  all_args.extend_from_slice(args);
  let (exit_code, output) = run_json(environment, &all_args);
  assert_eq!(exit_code, 0, "bd {args:?}: {output}");
}

impl Situation {
  fn of(given: &Given) -> Self {
    given.check_mapped();
    let space = Space::new(&[]);
    commit_given(&space, given);
    let branch = given.branch.as_str();
    let mut pull_requests: Vec<Value> = given.pull_request.iter().map(|pull_request| pull_request_of(pull_request, branch)).collect();
    if given.stack_pull_request.is_some() {
      pull_requests.push(stack_pull_request_of(given.root_of(branch)));
    }
    // every repository but one only on Space is reviewed on GitHub
    let (mut environment, github) = if given.readiness == Readiness::Space {
      (space.environment(), None)
    } else {
      let (github, url) = fake_github(&space, &pull_requests);
      (reviewed_on_github(&space, &url), Some(github))
    };
    // GitHub retargets the pull request a start retargets
    if let (Some(github), Some(_)) = (&github, &given.pull_request) {
      github.answer("PATCH", "/repos/acme/monorepo/pulls/7", FakeAnswer::ok(json!({"number": 7})));
    }
    let review_required = !matches!(given.readiness, Readiness::NotRequired | Readiness::Unlisted | Readiness::NotAsked);
    space.fake.review_required.store(review_required, Ordering::SeqCst);
    let has_pull_request = given.pull_request.is_some();
    match given.readiness {
      Readiness::NotRequired | Readiness::Space | Readiness::NotAsked => {}
      Readiness::Approved => {
        assert!(has_pull_request, "given.readiness approved needs given.pullRequest");
        *space.fake.pull_request_review.lock().unwrap() = Some(evaluated_review(true, "APPROVED"));
      }
      Readiness::Missing => {
        if has_pull_request {
          *space.fake.pull_request_review.lock().unwrap() = Some(evaluated_review(false, "NOT_ADDED"));
        }
      }
      Readiness::Unknown => assert!(
        given.pull_request.as_ref().is_some_and(|pull_request| !pull_request.approvers.is_empty()),
        "given.readiness unknown needs a pull request with an approver"
      ),
      Readiness::Unavailable => {
        *space.fake.review_hook.lock().unwrap() = Some(Arc::new(|_| Some((500, json!({"message": "down"})))));
      }
      Readiness::Unlisted => {
        if let Some(github) = &github {
          github.answer("POST", "/graphql", FakeAnswer::status(500, json!({"message": "Server Error"})));
        }
      }
    }
    match (given.rules, &github) {
      (Rules::Keep, _) | (_, None) => {}
      (Rules::Dismiss, Some(github)) => github.answer("GET", MASTER_RULES, FakeAnswer::ok(dismissing_rules())),
      (Rules::Unknown, Some(github)) => github.answer("GET", MASTER_RULES, FakeAnswer::status(500, json!({"message": "Server Error"}))),
    }
    if given.target == Target::Unprotected {
      *space.fake.start_options.lock().unwrap() = Some(json!({"supported": false, "emergencySupported": false, "profiles": []}));
    }
    match given.landed {
      Some(Landed::InBaseline) => land_in_the_baseline(&space.repo),
      Some(Landed::ByContent) => {
        bd_ok(&environment, &["push", branch]);
        land_auth_by_content(&space.repo);
      }
      None => {}
    }
    match given.remote {
      Remote::None => {}
      Remote::UpToDate => bd_ok(&environment, &["push", branch]),
      Remote::Unpushed => {
        bd_ok(&environment, &["push", branch]);
        write(&space.repo, "more.txt", "more\n");
        commit_all(&space.repo, &format!("({branch}) More {branch}"), 1_700_009_000);
      }
      Remote::Behind => {
        bd_ok(&environment, &["push", branch]);
        push_from_elsewhere_to(&space.remote, &virtual_branch(branch), "Someone else");
        git(&space.repo, &["fetch", "-q", "origin"]);
      }
      Remote::RebuiltOnNewerBaseline => {
        // the remote branch has the commits by content
        bd_ok(&environment, &["push", branch]);
        rebuild_on_a_newer_baseline(&space.repo);
      }
    }
    if let Some(parked) = given.parked.as_deref() {
      let name = match parked {
        "self" => branch,
        "builtOn" => given.parent_of(branch).expect("given.parked builtOn needs a parent"),
        member => member,
      };
      bd_ok(&environment, &["sync"]);
      bd_ok(&environment, &["park", name]);
    }
    if given.robot.is_some() {
      // the robots ran on the tips the sync builds; a branch that fails to sync fails it
      let _ = run_json(&environment, &["--prefix", PREFIX, "sync"]);
      add_robots(&space, &mut environment, given);
    }
    // a merged pull request merged the commits the branch has: its head is the tip the sync builds
    if let Some(github) = github
      .as_ref()
      .filter(|_| given.pull_request.as_ref().is_some_and(|pull_request| pull_request.state == PullRequestState::Merged))
    {
      let _ = run_json(&environment, &["--prefix", PREFIX, "sync"]);
      let merged: Vec<Value> = pull_requests
        .iter()
        .map(|pull_request| {
          let mut pull_request = pull_request.clone();
          if pull_request["state"] == "MERGED" {
            pull_request["headRefOid"] = json!(space.local_tip(pull_request["headRefName"].as_str().unwrap()));
          }
          pull_request
        })
        .collect();
      list_pull_requests(github, &merged);
    }
    if given.not_on_space.is_some() {
      git(&space.repo, &["remote", "set-url", "origin", space.remote.path().to_str().unwrap()]);
    }
    Self {
      space,
      environment,
      _github: github,
    }
  }

  /// `safe_push()` with `--check`, the function the command runs: its output as `--check` prints it, or the error of
  /// the plan
  fn check(&self, given: &Given) -> Result<SafePushOutput, CliError> {
    safe_push_command(&self.context()?, &given.args(true), &self.environment)
  }

  fn context(&self) -> Result<RepoContext, CliError> {
    RepoContext::resolve(
      &self.environment,
      &RepoOptions {
        repo: None,
        prefix: Some(PREFIX),
        full_ids: false,
        no_cache: false,
      },
    )
  }

  /// `bd safe-push` without `--check`: `{started_via, canceled}` (whether it canceled a robot first) or `{error}`
  fn start(&self, given: &Given) -> Value {
    let line = given.command_line();
    let args: Vec<&str> = line.iter().map(String::as_str).collect();
    let (exit_code, output) = safe_push(&self.environment, &args);
    if exit_code == 0 {
      json!({"started_via": output["started_via"], "canceled": output["canceled"].is_object()})
    } else {
      json!({"error": output["error"]["code"]})
    }
  }
}

// ---- the app's side ----

/// A session of the app over the case's repository: the table its sync folded and the facts it holds, each read of
/// the same fakes as its host reads them (`remote_facts::fact_sources::FactSources` of the app) in the order its
/// scheduler asks them after a sync: the robots, the problems of the failed ones, the Patronus view with the landing
/// needs, the details of the robot of the subject (its profile, which a Safe Push reads), the pull requests, the rules
/// of the bases of the approved ones, the merge profiles of the target, the review readiness the start of the subject
/// needs
struct AppSession<'c> {
  context: &'c RepoContext,
  environment: &'c Environment,
  state: TableState,
  facts: FactStore,
  /// The GitHub repository the branches are mirrored to, as the host resolves it of the push URL
  github: Option<GitHubRepository>,
  seq: u32,
}

impl<'c> AppSession<'c> {
  /// The session after the app's sync of the repository of `context`
  fn synced(context: &'c RepoContext, environment: &'c Environment) -> Self {
    let mut synced = {
      let _lock = lock(context).unwrap();
      sync_locked(context).unwrap()
    };
    let table = synced.branch_table(&context.prefix);
    let baseline = table.baseline.clone();
    let mut session = Self {
      context,
      environment,
      state: std::mem::take(&mut synced.table),
      facts: FactStore::new(),
      github: github_repository(context, &synced.report).unwrap(),
      seq: 0,
    };
    let observation = session.observation();
    let _ = session.facts.table.answer(observation, Ok(Arc::new(table)));
    session.facts.table_committed(&baseline);
    session
  }

  fn observation(&mut self) -> Observation {
    self.seq += 1;
    Observation {
      seq: Seq(self.seq),
      inputs: 0,
      at: Duration::ZERO,
      observed_at: 1,
    }
  }

  fn apply(&mut self, result: FactResult) {
    let observation = self.observation();
    self.facts.apply(observation, result);
  }

  /// Reads every fact the start of the case reads, as the session's scheduler asks them
  fn read(&mut self, given: &Given) {
    let context = self.context;
    let runtime = context.runtime().unwrap();
    let service = patronus_service(self.environment).unwrap();
    let space = context.space_repository().unwrap();
    // the host names the GitHub repository the branches are mirrored to, as it resolves it
    let robots = match &space {
      Some(space) => PatronusRobotsResult {
        github_repository: self.github.as_ref().map(GitHubRepository::full_name),
        ..runtime.block_on(service.robots(&space.name, &context.prefix, None, false, context.patronus_new_api))
      },
      None => PatronusRobotsResult::from(PatronusAvailability::NotPatronus),
    };
    self.apply(FactResult::Robots {
      read: RobotsRead::List { full: true },
      result: robots,
    });
    let digest = robots_digest(&self.state, self.facts.robots.view(), AskedRobots { watched: &[], details: &[] });
    for read in digest.problems {
      let result = runtime.block_on(service.robot_problems(&read.id)).map_err(ApiError::internal);
      self.apply(FactResult::Problems { id: read.id, result });
    }
    self.derive_patronus();
    if let Some(id) = self.subject_robot(given) {
      self.facts.demand_details(std::slice::from_ref(&id));
      let result = runtime
        .block_on(service.robot_details(&context.prefix, &id, context.patronus_new_api))
        .map(Box::new)
        .map_err(ApiError::internal);
      self.apply(FactResult::RobotDetails { id, result });
      self.derive_patronus();
    }
    let github_service = github_service(context, self.environment);
    let baseline = self.state.header().baseline_branch.clone();
    let github = self.github.clone();
    let listed = match &github {
      Some(repository) => runtime.block_on(list_open_pull_requests(&github_service, &context.repository, repository, &context.prefix)),
      None => PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
    };
    self.apply(FactResult::PullRequests(listed));
    let committed = self.facts.table.value().map(|table| table.baseline.clone());
    if let Some(repository) = &github {
      for base in self.facts.pull_requests.approved_bases(committed.as_deref(), &context.prefix) {
        let result = runtime
          .block_on(read_branch_rules(&github_service, &context.repository, repository, &base))
          .map_err(|failure| ApiError::internal(format!("{failure:?}")));
        self.apply(FactResult::Rules { branch: base, result });
      }
    }
    if let Some(space) = &space {
      let result = runtime
        .block_on(service.start_options(&space.project_key, &space.name, baseline_on_remote(&baseline), None, None))
        .map_err(ApiError::internal);
      self.apply(FactResult::Profiles { baseline, result });
    }
    if given.readiness != Readiness::NotAsked
      && let Some(space) = &space
      && let Some((key, _)) = self.subject(given)
    {
      self.read_readiness(&key, space, github.as_ref());
    }
  }
}

impl AppSession<'_> {
  /// The Patronus view of the table, the robots and the remote branches the session knows, as its actor derives it
  /// once they changed (`facts::derive_patronus`), then the landing needs of what it names landed
  fn derive_patronus(&mut self) {
    let Some(derivation) = self.facts.begin_patronus(&self.state, Duration::ZERO) else {
      return;
    };
    let context = self.context;
    let view = derive_patronus(&context.git, &context.repository, &derivation.of(), &mut self.facts.memo.lock().unwrap());
    let observation = Observation {
      inputs: derivation.inputs,
      ..self.observation()
    };
    self.facts.patronus.answer(observation, Ok(view));
    let table = derivation.table;
    let mut compute = self.facts.patronus.publish(&table);
    while let Some((inputs, landed)) = compute {
      let needs = landing_needs_of(&context.git, &context.cache(), &context.repository, &context.prefix, &landed).map_err(|error| ApiError::internal(error.to_string()));
      compute = self.facts.patronus.needs_answer(inputs, needs).or_else(|| self.facts.patronus.publish(&table));
    }
  }

  /// The review readiness the session asks for the rows a window shows with a step that waits for it and for the
  /// subject `key` the click of its start needs now (`ReadinessFact::update`), of Patronus as its host asks it
  fn read_readiness(&mut self, key: &str, space: &SpaceRepository, github: Option<&GitHubRepository>) {
    let context = self.context;
    let demand = Demand {
      visible_branches: self.state.order().to_vec(),
      visible_stacks: self.state.stacks().map(|stack| stack.root.clone()).collect(),
      needed: vec![key.to_string()],
      ..Demand::default()
    };
    let wanted = wanted_readiness(&self.state, &self.facts, &context.prefix, &demand);
    self.facts.readiness.update(
      &self.state,
      self.facts.pull_requests.view(),
      self.facts.robots.view().head.value.as_ref(),
      &demand,
      &context.prefix,
      &wanted,
    );
    let digest = self.facts.readiness.digest().clone();
    if digest.subjects.is_empty() {
      return;
    }
    let target = baseline_on_remote(&digest.baseline);
    let repository = ReadinessRepository {
      space,
      github,
      target,
      baseline: &digest.baseline,
      branch_prefix: &context.prefix,
    };
    let asks: Vec<_> = digest.subjects.iter().map(|subject| subject.ask.clone()).collect();
    let service = patronus_service(self.environment).unwrap();
    let read = context.runtime().unwrap().block_on(branch_review_readiness(
      &context.git,
      &context.repository,
      &context.cache(),
      &service,
      &repository,
      &asks,
      true,
    ));
    let answers = digest.answers(read.map_err(|error| ApiError::internal(error.to_string())));
    self.apply(FactResult::Readiness(answers));
  }

  /// The subject the case starts as the view of the session has it, with its key: the branch; its whole stack, the
  /// stack branch of its root while the stack forks, else the last branch of the straight chain
  /// (`domain_policy::stack::whole_stack_of`); none for a branch the table lacks
  fn subject(&self, given: &Given) -> Option<(String, Subject)> {
    let name = given.branch.as_str();
    self.state.branch(name)?;
    if given.subject == GivenSubject::Branch {
      return Some((name.to_string(), Subject::branch(name)));
    }
    let repo = repository(&self.state);
    match whole_stack_of(&repo, |root| self.state.stack(root).is_some(), name)? {
      WholeStack::Chain { leaf, .. } => Some((leaf.to_string(), Subject::branch(leaf))),
      WholeStack::Forking { root } | WholeStack::Unbuilt { root, .. } => Some((stack_readiness_key(root), Subject::whole_stack(root))),
    }
  }

  /// The robot of the subject as the Patronus view matches it: its last robot, whose profile a Safe Push takes
  fn subject_robot(&self, given: &Given) -> Option<String> {
    let view = &self.facts.patronus.part().view;
    match self.subject(given)?.1 {
      Subject::Branch { name } => view.branches.get(&name)?.robot_id.clone(),
      Subject::WholeStack { root } => view.stacks.get(&root)?.robot_id.clone(),
    }
  }

  /// What the session derives of the case: the next step of the subject and the decision of its start
  fn answer(&self, given: &Given) -> (Value, Value) {
    let prefix = &self.context.prefix;
    let Some((key, subject)) = self.subject(given) else {
      return (Value::Null, Value::Null);
    };
    let open = session_open_pull_requests(&self.state, &self.facts, prefix);
    let landed = landed_states_of(&self.state, &self.facts);
    let steps = next_steps_of(&self.state, &self.facts, &open, &landed);
    let land = land_answer_of(&self.state, &self.facts, prefix, &given.land(subject)).map(|answer| answer.decision);
    (step_answer(steps.get(&key).map(|view| &view.step)), land_answer(land.as_ref()))
  }
}

/// Folds the events of a sync into the table of a session up to its grouping, then nothing: the table of a sync that
/// runs, past its grouping, before it built the branches
#[derive(Clone)]
struct UntilGrouped {
  table: Arc<Mutex<TableState>>,
  grouped: Arc<AtomicBool>,
}

impl ProgressReporter for UntilGrouped {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    if self.grouped.load(Ordering::SeqCst) {
      return Ok(());
    }
    let grouping = matches!(event, SyncEvent::BranchesGrouped { .. });
    self.table.lock().map_err(|e| anyhow::anyhow!("the table of the sync is poisoned: {e}"))?.apply(event);
    self.grouped.store(grouping, Ordering::SeqCst);
    Ok(())
  }
}

impl Situation {
  /// What the app answers of the case: its session synced the repository and read the facts of the start, then (`sync:
  /// running`) its next sync of a new commit of the branch runs past its grouping
  fn app(&self, given: &Given) -> (Value, Value) {
    let context = self.context().unwrap();
    let mut session = AppSession::synced(&context, &self.environment);
    session.read(given);
    if given.sync == Some(GivenSync::Running) {
      write(&self.space.repo, "syncing.txt", "syncing\n");
      commit_all(&self.space.repo, &format!("({}) Sync {}", given.branch, given.branch), 1_700_009_500);
      let table = Arc::new(Mutex::new(std::mem::take(&mut session.state)));
      table.lock().unwrap().begin_sync();
      let reporter = UntilGrouped {
        table: Arc::clone(&table),
        grouped: Arc::new(AtomicBool::new(false)),
      };
      let options = SyncOptions {
        object_cache: context.cache(),
        config: Some(context.config().unwrap()),
        ..SyncOptions::default()
      };
      context
        .runtime()
        .unwrap()
        .block_on(sync_branches(&context.git, &context.repository, &context.prefix, reporter, options))
        .unwrap();
      session.state = std::mem::take(&mut *table.lock().unwrap());
    }
    session.answer(given)
  }
}

// ---- what a case compares ----

/// What a case compares of a next step (`expect.step`): its row, its kind, the blocker of each start, of a rule the
/// code of its reason; `null` without one
fn step_answer(step: Option<&NextStep>) -> Value {
  let Some(step) = step else {
    return Value::Null;
  };
  let value = serde_json::to_value(step).unwrap();
  let blocker = |blocker: &Value| match blocker["code"].as_str() {
    None => Value::Null,
    Some("rule") => blocker["reason"]["code"].clone(),
    Some(code) => json!(code),
  };
  json!({
    "row": value["row"],
    "kind": value["kind"]["kind"],
    "safePush": blocker(&value["safePush"]),
    "dryRun": blocker(&value["dryRun"]),
  })
}

/// What a case compares of the decision of a start (`expect.land`): the codes of its blockers, the kinds of its
/// confirmations and of the facts it waits for, in order; how it starts (`api`, `api:<profile>`, `comment:<number>`,
/// `createPullRequest`), the branches its push pushes and whether by force, the pull requests it retargets, the robot
/// it cancels, what lands; `null` without a decision
fn land_answer(decision: Option<&LandDecision>) -> Value {
  let Some(decision) = decision else {
    return Value::Null;
  };
  let tags = |values: Vec<Value>, tag: &str| -> Vec<Value> { values.into_iter().map(|value| value[tag].clone()).collect() };
  let waits: Vec<Value> = decision
    .waits
    .iter()
    .map(|wait| {
      let fact = serde_json::to_value(&wait.fact).unwrap()["kind"].as_str().unwrap().to_string();
      json!(if wait.failed.is_some() { format!("{fact}:failed") } else { fact })
    })
    .collect();
  let start = if decision.effects.contains(&PlannedEffect::CreatePullRequest) {
    Some("createPullRequest".to_string())
  } else {
    decision.start_route().map(|route| match route {
      StartRoute::Api { profile: None } => "api".to_string(),
      StartRoute::Api { profile: Some(profile) } => format!("api:{}", profile.id),
      StartRoute::Comment { number } => format!("comment:{number}"),
    })
  };
  let push = decision.effects.iter().find_map(|effect| match effect {
    PlannedEffect::Push { branches, force } => Some((branches, *force)),
    _ => None,
  });
  let effects = |of: fn(&PlannedEffect) -> Option<Value>| -> Vec<Value> { decision.effects.iter().filter_map(of).collect() };
  json!({
    "blockers": tags(decision.blockers.iter().map(|reason| serde_json::to_value(reason).unwrap()).collect(), "code"),
    "confirmations": tags(decision.confirmations.iter().map(|confirmation| serde_json::to_value(confirmation).unwrap()).collect(), "kind"),
    "waits": waits,
    "start": start,
    "pushes": push.map(|(branches, _)| branches),
    "force": push.is_some_and(|(_, force)| force),
    "retargets": effects(|effect| match effect {
      PlannedEffect::Retarget { number, .. } => Some(json!(number)),
      _ => None,
    }),
    "cancels": effects(|effect| match effect {
      PlannedEffect::CancelRobot { robot_id } => Some(json!(robot_id)),
      _ => None,
    }).first(),
    "lands": decision.lands,
  })
}

/// The keys of a step a case names always: its row and its kind
const STEP_NAMED: [&str; 2] = ["row", "kind"];

/// The keys a case compares even where it leaves them out, with the answer it means then: no blocker of either start;
/// of a decision no blocker and no confirmation, nor a wait of one that nothing refuses, so one that appears fails
/// every case it appears in
fn defaults(column: &str, expected: &Map<String, Value>) -> Vec<(&'static str, Value)> {
  match column {
    "step" => vec![("safePush", Value::Null), ("dryRun", Value::Null)],
    _ if expected.get("blockers").and_then(Value::as_array).is_some_and(|blockers| !blockers.is_empty()) => {
      vec![("confirmations", json!([]))]
    }
    _ => vec![("blockers", json!([])), ("confirmations", json!([])), ("waits", json!([]))],
  }
}

/// Where `actual` answers otherwise than the subset `expected` of the column `column` (`step` or `land`), each named
/// with the side
fn differences(side: &str, column: &str, expected: &Value, actual: &Value) -> Vec<String> {
  match (expected, actual) {
    (Value::Null, Value::Null) => Vec::new(),
    (Value::Object(expected), Value::Object(actual)) => {
      let mut failures: Vec<String> = Vec::new();
      if column == "step" {
        failures.extend(STEP_NAMED.iter().filter(|key| !expected.contains_key(**key)).map(|key| format!("{column} names its {key}")));
      }
      let mut keys: Map<String, Value> = defaults(column, expected).into_iter().map(|(key, value)| (key.to_string(), value)).collect();
      keys.extend(expected.clone());
      for (key, value) in &keys {
        match actual.get(key) {
          None => failures.push(format!("{column}.{key} is no key of the answer")),
          Some(answered) if answered != value => failures.push(format!("{side}: {column}.{key}: expected {value}, answers {answered}")),
          Some(_) => {}
        }
      }
      if failures.is_empty() {
        failures
      } else {
        vec![format!("{} (all: {})", failures.join("; "), Value::Object(actual.clone()))]
      }
    }
    _ => vec![format!("{side}: {column}: expected {expected}, answers {actual}")],
  }
}

/// The start of `bd` without `--check` does what its decision says: it starts (through the API or from a pull request,
/// canceling the robot the decision cancels) when nothing refuses it and the flags accept every confirmation
/// (`domain_policy::land::unaccepted`), else it refuses
fn start_differences(situation: &Situation, given: &Given, decision: &LandDecision) -> Vec<String> {
  let starts = decision.blockers.is_empty() && decision.waits.is_empty() && unaccepted(&decision.confirmations, &given.allowances()).is_empty();
  let expected = if starts {
    let via = match decision.start_route() {
      Some(StartRoute::Api { .. }) if !decision.effects.contains(&PlannedEffect::CreatePullRequest) => "api",
      _ => "pull_request",
    };
    let cancels = decision.effects.iter().any(|effect| matches!(effect, PlannedEffect::CancelRobot { .. }));
    json!({"started_via": via, "canceled": cancels})
  } else {
    json!("refused")
  };
  let started = situation.start(given);
  let actual = if started.get("error").is_some() { json!("refused") } else { started };
  if actual == expected {
    Vec::new()
  } else {
    vec![format!("bd: the start without --check: expected {expected}, answers {actual}")]
  }
}

/// What the case's repository answers against its expectation: the app's side, then `bd` with `--check` (its decision
/// and its step), then its start without `--check`; the failures, each named with the case
fn answer(case: &Case) -> Vec<String> {
  let named = |failures: Vec<String>| -> Vec<String> { failures.into_iter().map(|failure| format!("{}: {failure} — {}", case.id, case.title)).collect() };
  assert!(!case.spec.is_empty(), "{}: a case names the requirements it pins", case.id);
  let broken: Vec<String> = case.spec.iter().filter_map(|spec| broken_spec(spec)).collect();
  if !broken.is_empty() {
    return named(broken);
  }
  let (given, expect) = match (case.kind, &case.given, &case.expect) {
    (CaseKind::Exercised, Some(given), Some(expect)) => (given, expect),
    (CaseKind::Recorded, None, None) if case.bd.is_none() && case.asymmetry.is_none() => return Vec::new(),
    _ => return named(vec!["an exercised case has given and expect, a recorded one neither, nor bd".to_string()]),
  };
  if case.asymmetry.is_some() != case.bd.is_some() {
    return named(vec!["bd answers otherwise only by design: an asymmetry, with the answer bd".to_string()]);
  }
  let situation = Situation::of(given);
  let (step, land) = situation.app(given);
  let mut failures = differences("app", "step", &expect.step, &step);
  failures.extend(differences("app", "land", &expect.land, &land));
  let bd = case.bd.as_ref();
  let bd_step = bd.and_then(|bd| bd.step.as_ref()).unwrap_or(&expect.step);
  let bd_land = bd.and_then(|bd| bd.land.as_ref()).unwrap_or(&expect.land);
  match (situation.check(given), bd.and_then(|bd| bd.error.as_deref())) {
    (Ok(output), None) => {
      failures.extend(differences("bd", "step", bd_step, &step_answer(output.step.as_ref())));
      failures.extend(differences("bd", "land", bd_land, &land_answer(output.decision.as_ref())));
      if let Some(decision) = &output.decision {
        failures.extend(start_differences(&situation, given, decision));
      }
    }
    (Ok(_), Some(code)) => failures.push(format!("bd: expected the error {code}, decides")),
    (Err(error), expected) => {
      let code = serde_json::to_value(error.code).unwrap();
      if expected.is_none_or(|expected| code != expected) {
        failures.push(format!("bd: expected {}, refuses with {code}: {}", expected.unwrap_or("its decision"), error.message));
      }
    }
  }
  named(failures)
}

fn cases() -> Vec<Case> {
  let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/../domain-policy/cases");
  let mut files: Vec<_> = std::fs::read_dir(directory)
    .unwrap()
    .map(|entry| entry.unwrap().path())
    .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
    .collect();
  files.sort();
  files
    .iter()
    .flat_map(|file| {
      let text = std::fs::read_to_string(file).unwrap();
      serde_json::from_str::<Vec<Case>>(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()))
    })
    .collect()
}

/// Every case of `group`, each in its own repository on its own thread, built once for both sides: one message names
/// every case that answers otherwise
fn assert_group(group: &str) {
  let cases = cases();
  let unknown: Vec<&str> = cases.iter().map(|case| case.group.as_str()).filter(|known| !GROUPS.contains(known)).collect();
  assert!(unknown.is_empty(), "groups without their test: {unknown:?}");
  let of_group: Vec<&Case> = cases.iter().filter(|case| case.group == group).collect();
  assert!(!of_group.is_empty(), "no case of {group}");
  let failures: Vec<String> = std::thread::scope(|scope| {
    #[expect(clippy::needless_collect, reason = "every case starts before the first is joined")]
    let running: Vec<_> = of_group.iter().map(|case| (case.id.as_str(), scope.spawn(|| answer(case)))).collect();
    running
      .into_iter()
      .flat_map(|(id, thread)| {
        thread.join().unwrap_or_else(|panic| {
          let message = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(ToString::to_string));
          vec![format!("{id}: panicked: {}", message.unwrap_or_default())]
        })
      })
      .collect()
  });
  assert!(
    failures.is_empty(),
    "{} of {} cases of {group} answer otherwise:\n{}",
    failures.len(),
    of_group.len(),
    failures.join("\n")
  );
}

#[test_log::test]
fn the_safe_push_mode_and_route_cases_answer_alike_in_the_app_and_bd() {
  assert_group("modes-and-routes");
}

#[test_log::test]
fn the_safe_push_profile_cases_answer_alike_in_the_app_and_bd() {
  assert_group("profiles");
}

#[test_log::test]
fn the_safe_push_review_cases_answer_alike_in_the_app_and_bd() {
  assert_group("review");
}

#[test_log::test]
fn the_next_step_row_cases_answer_alike_in_the_app_and_bd() {
  assert_group("next-step-rows");
}

#[test_log::test]
fn the_stack_and_whole_stack_cases_answer_alike_in_the_app_and_bd() {
  assert_group("stack-and-whole-stack");
}

#[test_log::test]
fn the_landing_cases_answer_alike_in_the_app_and_bd() {
  assert_group("landing");
}

const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// Where a pointer of `today` goes wrong: `path#text` without the text in the file, `path:line#text` without the text
/// on that line; a bare `path:line` says nothing of what is there and goes stale with any edit above it
fn broken_pointer(pointer: &str) -> Option<String> {
  let Some((place, name)) = pointer.split_once('#') else {
    return Some(format!("{pointer}: a pointer is path#text or path:line#text"));
  };
  let (path, line) = match place.rsplit_once(':') {
    Some((path, line)) if line.chars().all(|c| c.is_ascii_digit()) => (path, Some(line.parse::<usize>().unwrap_or(0))),
    _ => (place, None),
  };
  let Ok(text) = std::fs::read_to_string(format!("{ROOT}/{path}")) else {
    return Some(format!("{pointer}: no file {path}"));
  };
  match line {
    None if !text.contains(name) => Some(format!("{pointer}: {path} has no {name:?}")),
    Some(line) if !text.lines().nth(line.wrapping_sub(1)).is_some_and(|on| on.contains(name)) => Some(format!("{pointer}: line {line} of {path} has no {name:?}")),
    _ => None,
  }
}

/// The GitHub slug of a Markdown heading: lower case, the letters, digits, spaces and hyphens kept, spaces as hyphens
fn heading_slug(heading: &str) -> String {
  heading
    .trim()
    .to_lowercase()
    .chars()
    .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
    .map(|c| if c == ' ' { '-' } else { c })
    .collect()
}

/// Where an anchor of `spec` goes wrong: no file, no heading of the slug, no row `| <row> |` of a `#the-table:<row>`
fn broken_spec(spec: &str) -> Option<String> {
  let (path, anchor) = spec.split_once('#').map_or((spec, None), |(path, anchor)| (path, Some(anchor)));
  let Ok(text) = std::fs::read_to_string(format!("{ROOT}/{path}")) else {
    return Some(format!("{spec}: no file {path}"));
  };
  let anchor = anchor?;
  let (slug, row) = anchor.split_once(':').map_or((anchor, None), |(slug, row)| (slug, Some(row)));
  let headings: Vec<String> = text
    .lines()
    .filter_map(|line| line.strip_prefix('#').map(|rest| rest.trim_start_matches('#')))
    .filter(|rest| rest.starts_with(' '))
    .map(heading_slug)
    .collect();
  if !headings.iter().any(|heading| heading == slug) {
    return Some(format!("{spec}: {path} has no heading of the slug {slug:?}"));
  }
  match row {
    Some(row) if !text.lines().any(|line| line.starts_with(&format!("  | {row} |"))) => Some(format!("{spec}: {path} has no row {row:?} of the table")),
    _ => None,
  }
}

/// The group `discrepancies` is the registry: one entry of each id, a recorded one with its pointers; no case of another
/// group records a discrepancy, as its sides answer one expectation
fn registry_failures(cases: &[Case]) -> Vec<String> {
  let registry: Vec<&Case> = cases.iter().filter(|case| case.group == "discrepancies").collect();
  let mut failures = Vec::new();
  let ids: Vec<&str> = registry.iter().filter_map(|case| case.discrepancy.as_deref()).collect();
  for id in REGISTRY {
    let count = ids.iter().filter(|known| **known == id).count();
    if count != 1 {
      failures.push(format!("the registry has {count} entries of {id}"));
    }
  }
  failures.extend(ids.iter().filter(|id| !REGISTRY.contains(id)).map(|id| format!("{id} is no id of the registry")));
  for entry in &registry {
    let Some(id) = entry.discrepancy.as_deref() else {
      failures.push(format!("{}: an entry of the registry names its discrepancy", entry.id));
      continue;
    };
    if entry.kind == CaseKind::Recorded && entry.today.is_none() {
      failures.push(format!("{}: a recorded entry says where each side does it today", entry.id));
    }
    if let Some(today) = &entry.today {
      if today.app.is_empty() || today.bd.is_empty() {
        failures.push(format!("{}: today names a place of each side", entry.id));
      }
      failures.extend(
        today
          .app
          .iter()
          .chain(&today.bd)
          .filter_map(|pointer| broken_pointer(pointer))
          .map(|broken| format!("{}: {broken}", entry.id)),
      );
    }
    if entry.kind == CaseKind::Exercised && entry.closed.is_none() {
      failures.push(format!("{}: both sides answer an exercised entry alike: {id} is closed", entry.id));
    }
  }
  for case in cases.iter().filter(|case| case.group != "discrepancies") {
    if let Some(id) = &case.discrepancy {
      failures.push(format!("{}: {id} outside the registry: a case answers one expectation", case.id));
    }
    if case.today.is_some() {
      failures.push(format!("{}: only an entry of the registry says where each side does it", case.id));
    }
    if case.kind == CaseKind::Recorded {
      failures.push(format!("{}: only the registry records a case", case.id));
    }
    if case.closed.is_some() {
      failures.push(format!("{}: only an entry of the registry is closed", case.id));
    }
  }
  failures.extend(design_doc_failures(&registry));
  failures
}

/// The text of the design doc under `heading`, up to the next heading of its level or above
fn section<'a>(doc: &'a str, heading: &str) -> &'a str {
  let level = heading.chars().take_while(|c| *c == '#').count();
  let start = doc.find(&format!("\n{heading}\n")).unwrap_or_else(|| panic!("the design doc has no heading {heading:?}")) + 1;
  let rest = &doc[start + heading.len()..];
  let end = rest
    .match_indices("\n#")
    .map(|(at, _)| at + 1)
    .find(|at| {
      let line = &rest[*at..];
      let hashes = line.chars().take_while(|c| *c == '#').count();
      hashes <= level && line[hashes..].starts_with(' ')
    })
    .map_or(doc.len(), |at| start + heading.len() + at);
  &doc[start..end]
}

/// `text` names `id` as a word: `D1,` or `D1/D9`, not `D11`
fn names_id(text: &str, id: &str) -> bool {
  text
    .match_indices(id)
    .any(|(at, _)| !text[at + id.len()..].starts_with(|c: char| c.is_ascii_alphanumeric()))
}

/// The numbered items of a Markdown `text`, each with its continuation lines
fn numbered_items(text: &str) -> Vec<String> {
  let mut items: Vec<String> = Vec::new();
  for line in text.lines() {
    let numbered = line.starts_with(|c: char| c.is_ascii_digit()) && line.contains(". ");
    match items.last_mut() {
      Some(item) if !numbered && line.starts_with(' ') => {
        item.push(' ');
        item.push_str(line.trim());
      }
      _ if numbered => items.push(line.to_string()),
      _ => {}
    }
  }
  items
}

/// The design doc keeps up with the registry: every id has its row of the register and its entry under «Тесты каждой
/// строки D», and a row that leaves the rule to the owner («владелец») has a numbered item of «Решения, которые
/// подтверждает владелец» naming the id; the row of a closed entry says «закрыт», and its stage is a heading of the doc
fn design_doc_failures(registry: &[&Case]) -> Vec<String> {
  let doc = std::fs::read_to_string(format!("{ROOT}/docs/design/rust-domain-authority.md")).unwrap();
  let register = section(&doc, "### Реестр расхождений");
  let decisions = numbered_items(section(&doc, "## Решения, которые подтверждает владелец"));
  let mut failures = Vec::new();
  for id in REGISTRY {
    if !register.contains(&format!("\n- **{id}.**")) {
      failures.push(format!("{id}: the design doc has no entry `- **{id}.**` under «Тесты каждой строки D»"));
    }
    match register.lines().find(|line| line.starts_with(&format!("| {id} |"))) {
      None => failures.push(format!("{id}: the design doc has no row of it in the register")),
      Some(row) if row.contains("владелец") && !decisions.iter().any(|item| names_id(item, id)) => {
        failures.push(format!(
          "{id}: its row leaves the rule to the owner, and no item of «Решения, которые подтверждает владелец» names it"
        ));
      }
      Some(_) => {}
    }
  }
  for entry in registry {
    let (Some(id), Some(stage)) = (entry.discrepancy.as_deref(), entry.closed.as_deref()) else {
      continue;
    };
    if !doc.contains(&format!("\n### {stage}\n")) {
      failures.push(format!("{id}: closed in {stage}, which is no stage `### {stage}` of the design doc"));
    }
    if register.lines().any(|line| line.starts_with(&format!("| {id} |")) && !line.contains("закрыт")) {
      failures.push(format!("{id}: closed in {stage}, its row of the register doesn't say «закрыт»"));
    }
  }
  failures
}

#[test_log::test]
fn the_discrepancy_cases_name_every_id_as_the_register_does() {
  let failures = registry_failures(&cases());
  assert!(failures.is_empty(), "the registry:\n{}", failures.join("\n"));
  assert_group("discrepancies");
}

/// The rows of the table of `spec/branch/next-step.spec.md` (`  | <row> | ` under "The table"; a removed row is none)
fn next_step_rows() -> Vec<String> {
  let text = std::fs::read_to_string(format!("{ROOT}/spec/branch/next-step.spec.md")).unwrap();
  let table = &text[text.find("\n### The table\n").expect("next-step.spec.md has The table")..];
  table
    .lines()
    .filter_map(|line| line.strip_prefix("  | ")?.split_once(" | "))
    .filter(|(row, rest)| *row != "#" && !row.starts_with('-') && !rest.starts_with("removed:"))
    .map(|(row, _)| row.trim().to_string())
    .collect()
}

/// The ids are unique and of kebab case, each group has its test, and every row of the table of the next step is
/// named by a case (`spec/branch/next-step.spec.md#the-table:<row>`)
#[test_log::test]
fn the_cases_are_well_formed_and_name_every_row_of_the_next_step_table() {
  let cases = cases();
  let mut ids = BTreeSet::new();
  let duplicates: Vec<&str> = cases.iter().map(|case| case.id.as_str()).filter(|id| !ids.insert(*id)).collect();
  assert_eq!(duplicates, Vec::<&str>::new(), "ids of more than one case");
  let malformed: Vec<&str> = cases
    .iter()
    .map(|case| case.id.as_str())
    .filter(|id| {
      id.split('-')
        .any(|part| part.is_empty() || !part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()))
    })
    .collect();
  assert_eq!(malformed, Vec::<&str>::new(), "ids not in kebab case");
  let unknown: Vec<&str> = cases.iter().map(|case| case.group.as_str()).filter(|group| !GROUPS.contains(group)).collect();
  assert_eq!(unknown, Vec::<&str>::new(), "groups without their test");
  let rows = next_step_rows();
  assert!(rows.contains(&"1".to_string()) && !rows.contains(&"4".to_string()), "{rows:?}");
  let named: BTreeSet<&str> = cases
    .iter()
    .flat_map(|case| &case.spec)
    .filter_map(|spec| spec.strip_prefix("spec/branch/next-step.spec.md#the-table:"))
    .collect();
  let unnamed: Vec<&String> = rows.iter().filter(|row| !named.contains(row.as_str())).collect();
  assert_eq!(unnamed, Vec::<&String>::new(), "rows of the next step table no case names");
}

/// A commit of the placement of D8, as the commit list reads it
fn listed_commit(id: &str, subject: &str) -> Commit {
  Commit {
    id: id.to_string(),
    stripped_subject: subject.to_string(),
    message: subject.to_string(),
    author_name: "Test Author".to_string(),
    author_email: "test@example.com".to_string(),
    author_timestamp: 1_700_000_000,
    committer_timestamp: 1_700_000_000,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    subject: subject.to_string(),
    parent_id: None,
    tree_id: "tree".to_string(),
    note: None,
    mapped_commit_id: None,
    mapped_source: None,
    mapped_resolution: None,
  }
}

/// The subjects of D8 after the reword: `b1` lost its `(A/B)`, so B has no commits, and the stored parent B → A stays
const D8_SUBJECTS: [&str; 4] = ["(A) a1", "(A/A) a2", "b1", "(B/C) c1"];

/// The placement of C that D8 is about: declared on B, which has no commits, blocked by A (a stack problem of its own:
/// its own parent), no effective base
fn assert_d8_placement(stack: Option<&BranchStack>) {
  let stack = stack.expect("C is placed");
  assert_eq!(
    (
      stack.declared_parent.as_deref(),
      stack.effective_base.as_deref(),
      stack.blocked_by.as_ref().map(|block| block.parent.as_str())
    ),
    (Some("B"), None, Some("A"))
  );
}

/// D8, closed (the registry's entry `d8-whole-stack-parent`): the whole stack of C, whose declared parent B has no
/// commits and is stacked on A by `bd restack`, while A has a stack problem, is the stack as the user declared it (the
/// owner's decision 5): B, which no row has, makes C a root of its own, never the chain A → C its block names. The app
/// reads the same rule (`domain_policy::stack::whole_stack_of`, the whole stack of each root in the view)
#[test_log::test]
fn the_whole_stack_of_a_blocked_branch_with_an_inactive_declared_parent_is_d8() {
  // the placement alone, as stack_test.rs places branches
  let mut grouper = CommitGrouper::new_with(&GroupingInputs {
    stacks: true,
    stack_parents: StoreLines {
      entries: [("B".to_string(), "A".to_string())].into(),
      ..StoreLines::default()
    },
    ..GroupingInputs::default()
  });
  for (index, subject) in D8_SUBJECTS.iter().enumerate() {
    grouper.add_commit(listed_commit(&format!("c{index}"), subject));
  }
  let forest = grouper.finish_with_stacks().1.expect("stacks are on");
  let placed = place_branches(&forest, &HashMap::new());
  assert_d8_placement(placed.get("C"));

  // the same through the fixtures: B gets a commit for `bd restack B A`, which refuses a branch without commits, then
  // the reword drops its prefix
  let space = Space::new(&[]);
  let repo = &space.repo;
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let mut ids = Vec::new();
  for (index, subject) in ["(A) a1", "(A/A) a2", "(A/B) b1", "(B/C) c1"].iter().enumerate() {
    write(repo, &format!("f{index}.txt"), subject);
    ids.push(commit_all(repo, subject, 1_700_000_000 + 100 * i64::try_from(index).unwrap()));
  }
  let environment = space.environment();
  bd_ok(&environment, &["restack", "B", "A"]);
  bd_ok(&environment, &["reword", &ids[2], "--subject", "b1"]);
  let context = RepoContext::resolve(
    &environment,
    &RepoOptions {
      repo: None,
      prefix: Some(PREFIX),
      full_ids: false,
      no_cache: false,
    },
  )
  .unwrap();
  let synced = {
    let _lock = lock(&context).unwrap();
    sync_locked(&context).unwrap()
  };
  let branches = &synced.report.branches;
  assert_eq!(branches.iter().map(|branch| branch.name.as_str()).collect::<Vec<_>>(), ["C", "A"]);
  assert_d8_placement(branches.iter().find(|branch| branch.name == "C").and_then(|branch| branch.stack.as_ref()));

  // by declared parent: C heads a whole stack of its own, and A one without C
  let parked = &synced.report.grouping.parked.entries;
  let stacks = &synced.report.stack_branches;
  assert_eq!(
    (whole_stack(branches, parked, stacks, "C").unwrap(), whole_stack(branches, parked, stacks, "A").unwrap()),
    (WholeStack::Chain { root: "C", leaf: "C" }, WholeStack::Chain { root: "A", leaf: "A" })
  );
  // so the Safe Push of the whole stack of A lands A alone, its own stack problem refusing it, never C's block
  let (exit_code, output) = safe_push(&environment, &["A", "--whole-stack", "--check"]);
  assert_eq!((exit_code, &output["error"]["code"]), (4, &json!("push_refused")), "{output}");
  let message = output["error"]["message"].as_str().unwrap();
  assert!(
    message.contains("A is not synced (A prefix makes the branch its own parent") && !message.contains("C is not synced"),
    "{message}"
  );
}
