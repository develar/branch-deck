//! The side of bd of the scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` as a
//! repository of the bd-cli fixtures with the fake Patronus and the fake GitHub, then `safe_push()` itself with
//! `--check` (the production path: its preamble, `plan()`, the output of a check), and the start without `--check`;
//! today's answer expected. `src/lib/scenario-cases.test.ts` is the side of the app

use crate::patronus_facts::{PatronusFacts, table_of};
use crate::push_command::github_repository;
use crate::robot_list::list_robots;
use crate::safe_push_command::{SafePushArgs, SafePushOutput, github_service, open_pull_requests, patronus_service, safe_push as safe_push_command};
use bd_cli::fixture_test::{PREFIX, commit_all, commit_changed_declaration, commit_dangling_reference, git, run_json, write};
use bd_cli::remote_fixture_test::{
  LANDED_SUBJECTS, MASTER_RULES, Space, dismissing_rules, evaluated_review, fake_github, land_auth_by_content, minutes_ago, pull_request_json, push_from_elsewhere_to,
  rebuild_on_a_newer_baseline, reviewed_on_github, robot, safe_push, stack_robot,
};
use bd_core::context::{Environment, RepoContext, RepoOptions};
use bd_core::error::CliError;
use bd_core::stack_summary::whole_stack;
use bd_core::sync_command::{SyncOutput, lock, sync_locked};
use domain_policy::stack::WholeStack;
use domain_types::commit::Commit;
use domain_types::patronus::{ReviewReadinessBranch, ReviewStart};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestState as ListedPullRequestState, PullRequestsResult};
use domain_types::stack::{BranchStack, StackBranchState};
use github::fake_github::{FakeAnswer, FakeGitHub};
use history_rewrite::commit_grouper::{CommitGrouper, GroupingInputs};
use history_rewrite::stack::place_branches;
use history_rewrite::store_lines::StoreLines;
use remote_facts::review_readiness::{ReadinessRepository, branch_review_readiness};
use repository_model::patronus_view::{landed_commits, with_landing_needs};
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::patronus::target_branch_of;
use trial_check::landing_gate::landing_needs_of;

/// The groups of the cases and their test functions here; the Vitest has the same groups
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

/// A case of the table; `app` and `asymmetry` are the Vitest's
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
  discrepancy: Option<String>,
  /// Of an entry of the registry: the exercised cases of other groups that record the same discrepancy
  #[serde(default)]
  twins: Vec<String>,
  /// Of an entry of the registry: the stage of the design doc that closed the discrepancy; the entry stays the key of
  /// its id, `today` says where each side does it now, and no case is its twin any longer
  closed: Option<String>,
  /// Where each side does it today: `path#text` or `path:line#text` (a test, a function, a line), from the repository root
  today: Option<Today>,
  #[expect(dead_code, reason = "the Vitest compares the classes of the outcomes")]
  asymmetry: Option<IgnoredAny>,
  given: Option<Given>,
  #[expect(dead_code, reason = "the expectations of the app, the Vitest's")]
  app: Option<IgnoredAny>,
  bd: Option<Expected>,
  /// `given` as the file has it, which the facts of the case record, so the Vitest tells facts of another `given`
  #[serde(skip)]
  raw_given: Value,
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
  /// Why the sides differ although their classes agree (D9), or what the pointers show
  #[expect(dead_code, reason = "the Vitest asks for it where the classes of a discrepancy agree")]
  why: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
  /// A subset of the projection of the `--check` output (`projection`), or `{error}`
  check: Map<String, Value>,
  /// `{started_via}` or `{error}` of the start without `--check`
  start: Option<Map<String, Value>>,
}

/// The closed vocabulary of a situation, the schema of the README (`src/test-support/scenario-case.ts` is its twin)
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase", default)]
struct Given {
  mode: Mode,
  /// The branch the case starts
  branch: String,
  /// A command is a confirmed start: only the app tells a click from a key
  via: Via,
  subject: GivenSubject,
  commits: Vec<String>,
  target: Target,
  remote: Remote,
  robot: Option<GivenRobots>,
  /// The app's sync runs: `bd` syncs itself before it plans, so nothing of it is a state of bd
  sync: Option<IgnoredAny>,
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
      via: Via::Click,
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
  /// The app has no answer yet; bd waits for its read, which answers `keep`
  Loading,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Mode {
  SafePush,
  DryRun,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
enum Via {
  Click,
  Key,
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
  /// `approvals`: it failed for the approvals; bd decides nothing by it
  #[expect(dead_code, reason = "the chip of the app; bd starts a new robot either way")]
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
  /// The app didn't ask yet; bd always asks: the pre-check needs no approval
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
      (Rules::Keep | Rules::Loading, _) | (_, None) => {}
      (Rules::Dismiss, Some(github)) => github.answer("GET", MASTER_RULES, FakeAnswer::ok(dismissing_rules())),
      (Rules::Unknown, Some(github)) => github.answer("GET", MASTER_RULES, FakeAnswer::status(500, json!({"message": "Server Error"}))),
    }
    if given.target == Target::Unprotected {
      *space.fake.start_options.lock().unwrap() = Some(json!({"supported": false, "emergencySupported": false, "profiles": []}));
    }
    match given.landed {
      Some(Landed::InBaseline) => {
        git(&space.repo, &["update-ref", "refs/remotes/origin/master", "HEAD"]);
      }
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

// ---- the facts of the app's side ----

/// The facts of the app's side of each exercised case, one file per case id: what the sync, the repository view, the
/// robot list and the review scan leave of the case's repository, which `src/test-support/scenario-facts.ts` seeds into
/// the stores instead of computing any of it (`assert_facts_kept`, by the group test of the case)
const FACTS_DIRECTORY: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../domain-policy/cases/facts");

/// The environment variable that lets the facts test rewrite the files
const UPDATE_FACTS: &str = "UPDATE_SCENARIO_FACTS";

/// The branch of a virtual branch's full name
fn short_name(full_name: &str) -> Option<&str> {
  full_name.strip_prefix(&format!("{PREFIX}/virtual/"))
}

/// The ids the sync left, each under the name the TS adapter uses: `<branch>-tip` (the tip of the virtual branch),
/// `<branch>-remote` (its remote branch, unless that is the tip), `stack-<root>-tip`, `stack-<root>-remote`,
/// `<branch>-c<i>` (the i-th original commit of the branch, the oldest first) and `baseline`; the first name of an id wins
fn names_of(synced: &SyncOutput) -> HashMap<String, String> {
  let report = &synced.report;
  let mut names: HashMap<String, String> = HashMap::new();
  for (full_name, tip) in &report.tips {
    if let Some(short) = short_name(full_name) {
      names.entry(tip.clone()).or_insert_with(|| format!("{short}-tip"));
    }
  }
  for stack in &report.stack_branches {
    if let Some(tip) = &stack.tip {
      names.entry(tip.clone()).or_insert_with(|| format!("stack-{}-tip", stack.root));
    }
  }
  for (full_name, tip) in &report.remote_tips {
    if let Some(short) = short_name(full_name) {
      names.entry(tip.clone()).or_insert_with(|| format!("{short}-remote"));
    }
  }
  for stack in &report.stack_branches {
    if let Some(tip) = stack.remote.as_ref().and_then(|remote| remote.remote_tip.clone()) {
      names.entry(tip).or_insert_with(|| format!("stack-{}-remote", stack.root));
    }
  }
  for branch in &report.branches {
    for (index, commit) in branch.commits.iter().rev().enumerate() {
      names.entry(commit.id.clone()).or_insert_with(|| format!("{}-c{index}", branch.name));
    }
  }
  names.entry(report.baseline_commit.clone()).or_insert_with(|| "baseline".to_string());
  names
}

/// Every string of `value` that is an id of `names` becomes its name
fn name_ids(value: &mut Value, names: &HashMap<String, String>) {
  match value {
    Value::String(text) => {
      if let Some(name) = names.get(text.as_str()) {
        *text = name.clone();
      }
    }
    Value::Array(items) => items.iter_mut().for_each(|item| name_ids(item, names)),
    Value::Object(fields) => fields.values_mut().for_each(|field| name_ids(field, names)),
    _ => {}
  }
}

/// The rank of every time of `times`: the same for times less than 30 seconds apart (the fixtures take a time of their
/// own per robot, a second apart at most), one more for each later instant
fn ranks_of(times: impl Iterator<Item = u64>) -> HashMap<u64, u64> {
  let mut times: Vec<u64> = times.collect();
  times.sort_unstable();
  let mut ranks = HashMap::new();
  let mut rank = 0;
  for (index, time) in times.iter().enumerate() {
    if index > 0 && time - times[index - 1] >= 30 {
      rank += 1;
    }
    ranks.insert(*time, rank);
  }
  ranks
}

/// The robots in the order of the list (by start, the newest first; a tie by the order the fake keeps, which the app
/// breaks the same way, `isPreferredRanked`), with the times the fakes give relative to now made the same on every
/// run: the starts and the finishes each by their rank (`add_robots` starts and finishes them minutes apart), so the
/// order of the starts and of the finishes, what the app reads of the times, stays
fn settle_robots(robots: &mut Value) {
  let Some(robots) = robots.as_array_mut() else {
    return;
  };
  for (field, from) in [("startTime", 1_704_189_000), ("finishTime", 1_704_189_600)] {
    let ranks = ranks_of(robots.iter().filter_map(|robot| robot[field].as_u64()));
    for robot in robots.iter_mut() {
      if let Some(time) = robot[field].as_u64() {
        robot[field] = json!(from + 600 * ranks[&time]);
      }
    }
  }
}

/// `value` with the keys of every object sorted
fn sorted(value: Value) -> Value {
  match value {
    Value::Array(items) => Value::Array(items.into_iter().map(sorted).collect()),
    Value::Object(fields) => {
      let by_key: BTreeMap<String, Value> = fields.into_iter().map(|(key, field)| (key, sorted(field))).collect();
      Value::Object(by_key.into_iter().collect::<Map<String, Value>>())
    }
    other => other,
  }
}

/// `value` as the facts files hold it: an object or a list of up to 100 characters on one line, the rest one field or
/// item per line, indented by two spaces
fn pretty(value: &Value, indent: usize) -> String {
  let compact = serde_json::to_string(value).unwrap();
  let inner = " ".repeat(indent + 2);
  let outer = " ".repeat(indent);
  match value {
    Value::Object(fields) if !fields.is_empty() && compact.len() > 100 => {
      let lines: Vec<String> = fields.iter().map(|(key, field)| format!("{inner}{}: {}", json!(key), pretty(field, indent + 2))).collect();
      format!("{{\n{}\n{outer}}}", lines.join(",\n"))
    }
    Value::Array(items) if !items.is_empty() && compact.len() > 100 => {
      let lines: Vec<String> = items.iter().map(|item| format!("{inner}{}", pretty(item, indent + 2))).collect();
      format!("[\n{}\n{outer}]", lines.join(",\n"))
    }
    _ => inline(value),
  }
}

/// `value` on one line, a space after every colon and comma
fn inline(value: &Value) -> String {
  match value {
    Value::Object(fields) if !fields.is_empty() => {
      let fields: Vec<String> = fields.iter().map(|(key, field)| format!("{}: {}", json!(key), inline(field))).collect();
      format!("{{{}}}", fields.join(", "))
    }
    Value::Array(items) if !items.is_empty() => {
      let items: Vec<String> = items.iter().map(inline).collect();
      format!("[{}]", items.join(", "))
    }
    other => serde_json::to_string(other).unwrap(),
  }
}

impl Situation {
  /// The facts of the app's side (`FACTS_DIRECTORY`): the tips of the sync (`tips`, `remoteTips`, `stackTips`, by the
  /// short names), the robots as the list answers them (`list_robots`, by id, `settle_robots`), the repository view (`patronus_view` with the landing needs, as
  /// `bd status` reads it) and the review readiness of the branch and of its whole stack (`readiness`, by the app's keys),
  /// with the ids named and the times settled
  fn facts(&self, given: &Given, raw_given: &Value) -> Value {
    let context = self.context().unwrap();
    let synced = {
      let _lock = lock(&context).unwrap();
      sync_locked(&context).unwrap()
    };
    let table = table_of(&context, &synced);
    // the list as the app's poll and `plan()` read it, never the one a `bd push` of the fixture kept in the cache
    let space = context.space_repository().unwrap();
    let robots = match &space {
      Some(space) => list_robots(&context, &patronus_service(&self.environment).unwrap(), space).unwrap(),
      None => Vec::new(),
    };
    let mut facts = PatronusFacts::read(&context, &table, robots);
    let on_space = space.is_some();
    if on_space {
      let needs = landing_needs_of(&context.git, &context.cache(), &context.repository, &context.prefix, &landed_commits(&facts.view)).unwrap();
      with_landing_needs(&mut facts.view, &table, &needs);
    }
    let readiness = if on_space { self.readiness(&context, &synced, given) } else { BTreeMap::new() };
    let report = &synced.report;
    let tips = |of: &HashMap<String, String>| -> BTreeMap<String, String> {
      of.iter()
        .filter_map(|(full_name, tip)| short_name(full_name).map(|short| (short.to_string(), tip.clone())))
        .collect()
    };
    let stack_tips: BTreeMap<String, String> = report
      .stack_branches
      .iter()
      .filter_map(|stack| stack.tip.clone().map(|tip| (stack.root.clone(), tip)))
      .collect();
    let mut value = json!({
      "given": raw_given,
      "tips": tips(&report.tips),
      "remoteTips": tips(&report.remote_tips),
      "stackTips": stack_tips,
      "robots": facts.robots,
      "view": facts.view,
      "readiness": readiness,
    });
    name_ids(&mut value, &names_of(&synced));
    settle_robots(&mut value["robots"]);
    // the fake listens on a port of its own: the robot pages under one host
    let value: Value = serde_json::from_str(&serde_json::to_string(&value).unwrap().replace(&self.space.patronus_url, "https://patronus")).unwrap();
    sorted(value)
  }

  /// The review readiness as the app's scan asks it (`readinessInputOf`): the branch from its open pull request into
  /// the target, else through the API; the whole stack of its root as its built stack branch with the pull request of
  /// the stack, or of a straight chain its last branch, whose step the whole stack takes (its row asks it); nothing
  /// while the app hasn't asked (`notAsked`)
  fn readiness(&self, context: &RepoContext, synced: &SyncOutput, given: &Given) -> BTreeMap<String, Value> {
    if given.readiness == Readiness::NotAsked {
      return BTreeMap::new();
    }
    let space = context.space_repository().unwrap().expect("on Space");
    let service = patronus_service(&self.environment).unwrap();
    let github_service = github_service(context, &self.environment);
    let runtime = context.runtime().unwrap();
    let target = target_branch_of(synced.report.upstream.as_ref()).expect("the baseline is a branch of origin");
    let github = github_repository(context, &synced.report).unwrap();
    let listed = match &github {
      Some(repository) => open_pull_requests(context, &github_service, runtime, repository),
      None => PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
    };
    let repository = ReadinessRepository {
      space: &space,
      github: github.as_ref(),
      target: &target,
      baseline: &synced.baseline,
      branch_prefix: &context.prefix,
    };
    // the start as the app chooses it (`getPullRequestStart`: the open pull request into the target), built as every
    // start of bd is; the facts record the pull request and the tip asked, and `seedReadiness` of the Vitest refuses
    // an answer whose input the app computes otherwise, so this choice can't drift from the app's unnoticed
    let ask = |key: &str, source: Option<String>, tip: &str, pull_request: Option<&PullRequest>| {
      let start =
        pull_request.filter(|pull_request| pull_request.state == ListedPullRequestState::Open && (pull_request.base_ref_name.is_empty() || pull_request.base_ref_name == target));
      ReviewReadinessBranch::starting_from(key.to_string(), source, tip.to_string(), start)
    };
    let name = given.branch.as_str();
    let mut asks = Vec::new();
    if let Some(tip) = synced.report.tips.get(&virtual_branch(name)) {
      asks.push(ask(name, None, tip, listed.pull_requests.get(name)));
    }
    let root = given.root_of(name);
    if let Some(stack) = synced
      .report
      .stack_branches
      .iter()
      .find(|stack| stack.root == root && stack.state == StackBranchState::Built && !stack.members.is_empty())
      && let Some(tip) = &stack.tip
    {
      asks.push(ask(&format!("stack:{root}"), Some(stack.branch_name.clone()), tip, listed.stack_pull_requests.get(root)));
    }
    if given.subject == GivenSubject::WholeStack
      && let Ok(WholeStack::Chain { leaf, .. }) = whole_stack(&synced.report.branches, &synced.report.grouping.parked.entries, &synced.report.stack_branches, name)
      && leaf != name
      && let Some(tip) = synced.report.tips.get(&virtual_branch(leaf))
    {
      asks.push(ask(leaf, None, tip, listed.pull_requests.get(leaf)));
    }
    let answers = runtime
      .block_on(branch_review_readiness(
        &context.git,
        &context.repository,
        &context.cache(),
        &service,
        &repository,
        &asks,
        true,
      ))
      .unwrap();
    answers
      .into_iter()
      .zip(&asks)
      .map(|(answer, ask)| {
        let pull_request = match ask.start {
          ReviewStart::PullRequest { number } => Some(number),
          ReviewStart::Rest => None,
        };
        (
          answer.branch_name,
          json!({"readiness": answer.readiness, "requirement": answer.requirement, "tip": answer.tip, "pullRequest": pull_request}),
        )
      })
      .collect()
  }
}

/// The facts files of `FACTS_DIRECTORY` as they are, by case id
fn filed_facts() -> BTreeMap<String, Value> {
  std::fs::read_dir(FACTS_DIRECTORY).map_or_default(|entries| {
    entries
      .map(|entry| entry.unwrap().path())
      .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
      .map(|path| {
        let id = path.file_stem().unwrap().to_str().unwrap().to_string();
        (id, serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap())
      })
      .collect()
  })
}

/// The facts the app's side reads of the cases of `group` are what their fixtures leave today: `facts/<id>.json` of
/// every exercised case equals what the Rust core computed of its repository (`computed`, by case id). A change of the
/// sync, of `patronus_view`, of the robot list or of the review scan changes the files: `UPDATE_SCENARIO_FACTS=1`
/// rewrites them, and the diff is reviewed like code, as the Vitest answers from them
fn assert_facts_kept(group: &str, computed: &BTreeMap<String, Value>) {
  assert!(!computed.is_empty(), "no exercised case of {group}");
  if std::env::var_os(UPDATE_FACTS).is_some() {
    let directory = PathBuf::from(FACTS_DIRECTORY);
    std::fs::create_dir_all(&directory).unwrap();
    for (id, facts) in computed {
      std::fs::write(directory.join(format!("{id}.json")), format!("{}\n", pretty(facts, 0))).unwrap();
    }
    return;
  }
  let filed = filed_facts();
  let differing: Vec<&String> = computed.iter().filter(|(id, facts)| filed.get(*id) != Some(facts)).map(|(id, _)| id).collect();
  if let Some(id) = differing.first() {
    pretty_assertions::assert_eq!(
      pretty(filed.get(*id).unwrap_or(&Value::Null), 0),
      pretty(&computed[*id], 0),
      "the facts of {} cases of {group} differ from their files, {id} first; {UPDATE_FACTS}=1 cargo nextest run -p bd-remote -E 'test(/scenario/)' rewrites them",
      differing.len()
    );
  }
}

/// A facts file of a case that is gone, or that no fixture builds any more (recorded, or without `given`), misleads
/// nobody: it goes with the case (`UPDATE_SCENARIO_FACTS=1` removes it)
#[test_log::test]
fn no_facts_file_is_of_a_case_that_is_gone() {
  let cases = cases();
  let stale: Vec<String> = filed_facts()
    .into_keys()
    .filter(|id| !cases.iter().any(|case| case.id == *id && case.kind == CaseKind::Exercised && case.given.is_some()))
    .collect();
  if std::env::var_os(UPDATE_FACTS).is_some() {
    for id in &stale {
      std::fs::remove_file(PathBuf::from(FACTS_DIRECTORY).join(format!("{id}.json"))).unwrap();
    }
    return;
  }
  assert!(stale.is_empty(), "facts of no exercised case: {stale:?}; {UPDATE_FACTS}=1 removes them");
}

/// What a case compares of the `--check` output: the codes of the refusals, the profile and the pull request by id
fn projection(output: &Value) -> Value {
  let codes = |key: &str, field: &str| -> Value {
    output[key]
      .as_array()
      .map_or_else(Vec::new, |items| items.iter().map(|item| item[field].clone()).collect())
      .into()
  };
  json!({
    "refusals": codes("refusals", "code"),
    "push_needed": output["push_needed"],
    "profile": output["profile"]["id"],
    "pull_request": output["pull_request"]["number"],
    "readiness": output["review"]["readiness"],
    "rules": output["rules"]["dismiss_stale_reviews_on_push"],
    "lands": output["lands"],
    "retargeted": codes("retargeted", "number"),
  })
}

/// The keys of `expected` that `actual` answers otherwise, each named with the case
fn mismatches(id: &str, column: &str, expected: &Map<String, Value>, actual: &Value) -> Vec<String> {
  expected
    .iter()
    .filter(|(key, value)| actual.get(key.as_str()).unwrap_or(&Value::Null) != *value)
    .map(|(key, value)| {
      format!(
        "{id}: bd.{column}.{key}: expected {value}, bd answers {} (all: {actual})",
        actual.get(key.as_str()).unwrap_or(&Value::Null)
      )
    })
    .collect()
}

/// What one case's repository answers: the failures of the `bd` column, and the facts of the app's side of an
/// exercised case
struct Answer {
  failures: Vec<String>,
  facts: Option<Value>,
}

impl Answer {
  fn failed(failures: Vec<String>) -> Self {
    Self { failures, facts: None }
  }
}

/// What bd answers today for the case, against its `bd` column, and the facts of the app's side of the same
/// repository (`Situation::facts`, before the check and the start, which push and start robots); a recorded case has
/// none to answer
fn answer(case: &Case) -> Answer {
  assert!(!case.spec.is_empty(), "{}: a case names the requirements it pins", case.id);
  let broken: Vec<String> = case
    .spec
    .iter()
    .filter_map(|spec| broken_spec(spec))
    .map(|broken| format!("{}: {broken}", case.id))
    .collect();
  if !broken.is_empty() {
    return Answer::failed(broken);
  }
  let (given, expected) = match (case.kind, &case.given, &case.bd) {
    (CaseKind::Exercised, Some(given), Some(expected)) => (given, expected),
    (CaseKind::Recorded, None, None) => return Answer::failed(Vec::new()),
    _ => return Answer::failed(vec![format!("{}: an exercised case has given and bd, a recorded one neither", case.id)]),
  };
  let situation = Situation::of(given);
  let facts = situation.facts(given, &case.raw_given);
  let check = match situation.check(given) {
    Ok(output) => projection(&serde_json::to_value(output).unwrap()),
    Err(error) => json!({"error": error.code}),
  };
  let mut failures = mismatches(&case.id, "check", &expected.check, &check);
  if let Some(start) = &expected.start {
    failures.extend(mismatches(&case.id, "start", start, &situation.start(given)));
  }
  let context = match &case.discrepancy {
    Some(id) => format!(" ({id})"),
    None => String::new(),
  };
  Answer {
    failures: failures.into_iter().map(|failure| format!("{failure} — {}{context}", case.title)).collect(),
    facts: Some(facts),
  }
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
      let cases = serde_json::from_str::<Vec<Case>>(&text).unwrap_or_else(|e| panic!("{}: {e}", file.display()));
      let raw = serde_json::from_str::<Vec<Value>>(&text).unwrap();
      cases.into_iter().zip(raw).map(|(mut case, raw)| {
        case.raw_given = raw.get("given").cloned().unwrap_or(Value::Null);
        case
      })
    })
    .collect()
}

/// Every case of `group`, each in its own repository on its own thread, built once for both sides: one message names
/// every case that answers otherwise, then the facts of the app's side are compared with their files (or written,
/// `UPDATE_SCENARIO_FACTS=1`, before the answers are judged)
fn assert_group(group: &str) {
  let cases = cases();
  let unknown: Vec<&str> = cases.iter().map(|case| case.group.as_str()).filter(|known| !GROUPS.contains(known)).collect();
  assert!(unknown.is_empty(), "groups without their test: {unknown:?}");
  let of_group: Vec<&Case> = cases.iter().filter(|case| case.group == group).collect();
  assert!(!of_group.is_empty(), "no case of {group}");
  let answers: Vec<(&str, Answer)> = std::thread::scope(|scope| {
    #[expect(clippy::needless_collect, reason = "every case starts before the first is joined")]
    let running: Vec<_> = of_group.iter().map(|case| (case.id.as_str(), scope.spawn(|| answer(case)))).collect();
    running
      .into_iter()
      .map(|(id, thread)| {
        let answer = thread.join().unwrap_or_else(|panic| {
          let message = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(ToString::to_string));
          Answer::failed(vec![format!("{id}: panicked: {}", message.unwrap_or_default())])
        });
        (id, answer)
      })
      .collect()
  });
  let facts: BTreeMap<String, Value> = answers
    .iter()
    .filter_map(|(id, answer)| answer.facts.clone().map(|facts| ((*id).to_string(), facts)))
    .collect();
  let failing = answers.iter().filter(|(_, answer)| !answer.failures.is_empty()).count();
  let exercised = of_group.iter().any(|case| case.kind == CaseKind::Exercised);
  if exercised && std::env::var_os(UPDATE_FACTS).is_some() {
    assert_facts_kept(group, &facts);
  }
  assert!(
    failing == 0,
    "{failing} of {} cases of {group} answer otherwise:\n{}",
    of_group.len(),
    answers
      .iter()
      .flat_map(|(_, answer)| answer.failures.iter().map(String::as_str))
      .collect::<Vec<_>>()
      .join("\n")
  );
  if exercised && std::env::var_os(UPDATE_FACTS).is_none() {
    assert_facts_kept(group, &facts);
  }
}

#[test_log::test]
fn the_safe_push_mode_and_route_cases_answer_as_bd_answers_today() {
  assert_group("modes-and-routes");
}

#[test_log::test]
fn the_safe_push_profile_cases_answer_as_bd_answers_today() {
  assert_group("profiles");
}

#[test_log::test]
fn the_safe_push_review_cases_answer_as_bd_answers_today() {
  assert_group("review");
}

#[test_log::test]
fn the_next_step_row_cases_answer_as_bd_answers_today() {
  assert_group("next-step-rows");
}

#[test_log::test]
fn the_stack_and_whole_stack_cases_answer_as_bd_answers_today() {
  assert_group("stack-and-whole-stack");
}

#[test_log::test]
fn the_landing_cases_answer_as_bd_answers_today() {
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

/// The group `discrepancies` is the registry: one entry of each id, a recorded one with its pointers; a case of another
/// group with a discrepancy is the `twin` of its entry
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
    if let Some(stage) = &entry.closed
      && !entry.twins.is_empty()
    {
      failures.push(format!("{}: closed in {stage}, it has no twins", entry.id));
    }
    for twin in &entry.twins {
      match cases.iter().find(|case| case.id == *twin) {
        Some(case) if case.group != "discrepancies" && case.discrepancy.as_deref() == Some(id) => {}
        _ => failures.push(format!("{}: its twin {twin} is an exercised case of another group with {id}", entry.id)),
      }
    }
  }
  for case in cases.iter().filter(|case| case.group != "discrepancies") {
    if let Some(id) = &case.discrepancy
      && !registry.iter().any(|entry| entry.discrepancy.as_ref() == Some(id) && entry.twins.contains(&case.id))
    {
      failures.push(format!("{}: {id} outside the registry, whose entry doesn't name it as its twin", case.id));
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
