//! The side of bd of the scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` as a
//! repository of the bd-cli fixtures with the fake Patronus and the fake GitHub, then `safe_push()` itself with
//! `--check` (the production path: its preamble, `plan()`, the output of a check), and the start without `--check`;
//! today's answer expected. `src/lib/scenario-cases.test.ts` is the side of the app

use crate::safe_push_command::{SafePushArgs, SafePushOutput, safe_push as safe_push_command};
use bd_cli::fixture_test::{PREFIX, commit_all, commit_changed_declaration, commit_dangling_reference, git, run_json, write};
use bd_cli::remote_fixture_test::{
  LANDED_SUBJECTS, Space, evaluated_review, fake_github, land_auth_by_content, minutes_ago, pull_request_json, push_from_elsewhere_to, rebuild_on_a_newer_baseline,
  reviewed_on_github, robot, safe_push, stack_robot,
};
use bd_core::context::{Environment, RepoContext, RepoOptions};
use bd_core::error::CliError;
use bd_core::stack_summary::{WholeStack, whole_stack_of};
use bd_core::sync_command::{lock, sync_locked};
use domain_types::stack::BranchStack;
use domain_types::commit::Commit;
use github::fake_github::{FakeAnswer, FakeGitHub};
use history_rewrite::commit_grouper::{CommitGrouper, GroupingInputs};
use history_rewrite::stack::place_branches;
use history_rewrite::store_lines::StoreLines;
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::Ordering;

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
  /// Where each side does it today: `path#text` or `path:line#text` (a test, a function, a line), from the repository root
  today: Option<Today>,
  #[expect(dead_code, reason = "the Vitest compares the classes of the outcomes")]
  asymmetry: Option<IgnoredAny>,
  given: Option<Given>,
  #[expect(dead_code, reason = "the expectations of the app, the Vitest's")]
  app: Option<IgnoredAny>,
  bd: Option<Expected>,
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
  check: serde_json::Map<String, Value>,
  /// `{started_via}` or `{error}` of the start without `--check`
  start: Option<serde_json::Map<String, Value>>,
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
  /// `bd` reads no branch rules (D1, D9)
  rules: Option<IgnoredAny>,
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
      rules: None,
      parked: None,
      stack_branch: None,
      landed: None,
      landing_needs: None,
      dangling: None,
    }
  }
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
  /// Serves the fakes while the case runs
  _space: Space,
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
    environment.patronus_new_api = true;
    space.fake.list_profiles.store(true, Ordering::SeqCst);
  }
  space.fake.details.lock().unwrap().insert("older".to_string(), details);
}

/// The robots of the case as the list answers them, after the list `reviewed_on_github` left: `older` (`sourceRevision`
/// `0000`), the others on the tips the sync built of their branch or of the stack branch
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
    if robot_of.state != RobotState::Running {
      // in the order of the list: the first merged first
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
      _space: space,
      environment,
      _github: github,
    }
  }

  /// `safe_push()` with `--check`, the function the command runs: its output as `--check` prints it, or the error of
  /// the plan
  fn check(&self, given: &Given) -> Result<SafePushOutput, CliError> {
    let context = RepoContext::resolve(
      &self.environment,
      &RepoOptions {
        repo: None,
        prefix: Some(PREFIX),
        full_ids: false,
        no_cache: false,
      },
    )?;
    safe_push_command(&context, &given.args(true), &self.environment)
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
    "lands": output["lands"],
    "retargeted": codes("retargeted", "number"),
  })
}

/// The keys of `expected` that `actual` answers otherwise, each named with the case
fn mismatches(id: &str, column: &str, expected: &serde_json::Map<String, Value>, actual: &Value) -> Vec<String> {
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

/// What bd answers today for the case, against its `bd` column: the failures; a recorded case has none to answer
fn answer(case: &Case) -> Vec<String> {
  assert!(!case.spec.is_empty(), "{}: a case names the requirements it pins", case.id);
  let broken: Vec<String> = case
    .spec
    .iter()
    .filter_map(|spec| broken_spec(spec))
    .map(|broken| format!("{}: {broken}", case.id))
    .collect();
  if !broken.is_empty() {
    return broken;
  }
  let (given, expected) = match (case.kind, &case.given, &case.bd) {
    (CaseKind::Exercised, Some(given), Some(expected)) => (given, expected),
    (CaseKind::Recorded, None, None) => return Vec::new(),
    _ => return vec![format!("{}: an exercised case has given and bd, a recorded one neither", case.id)],
  };
  let situation = Situation::of(given);
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
  failures.into_iter().map(|failure| format!("{failure} — {}{context}", case.title)).collect()
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

/// Every case of `group`, each in its own repository on its own thread; one message names every case that answers
/// otherwise
fn assert_group(group: &str) {
  let cases = cases();
  let unknown: Vec<&str> = cases.iter().map(|case| case.group.as_str()).filter(|known| !GROUPS.contains(known)).collect();
  assert!(unknown.is_empty(), "groups without their test: {unknown:?}");
  let of_group: Vec<&Case> = cases.iter().filter(|case| case.group == group).collect();
  assert!(!of_group.is_empty(), "no case of {group}");
  let answers: Vec<Vec<String>> = std::thread::scope(|scope| {
    #[expect(clippy::needless_collect, reason = "every case starts before the first is joined")]
    let running: Vec<_> = of_group.iter().map(|case| (case.id.as_str(), scope.spawn(|| answer(case)))).collect();
    running
      .into_iter()
      .map(|(id, thread)| {
        thread.join().unwrap_or_else(|panic| {
          let message = panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(ToString::to_string));
          vec![format!("{id}: panicked: {}", message.unwrap_or_default())]
        })
      })
      .collect()
  });
  let failing = answers.iter().filter(|failures| !failures.is_empty()).count();
  assert!(
    failing == 0,
    "{failing} of {} cases of {group} answer otherwise:\n{}",
    of_group.len(),
    answers.concat().join("\n")
  );
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

/// D8 (the registry's entry `d8-whole-stack-parent` says what the app answers): the whole stack of C, whose declared
/// parent B has no commits and is stacked on A by `bd restack`, while A has a stack problem. bd follows `blocked_by`: C's
/// whole stack is the chain A → C; the app follows `declaredParent`, which no row has: C is a root of its own
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

  // bd: `effective_base ?? blocked_by.parent`, the chain from A down to C
  let whole_stack = whole_stack_of(branches, &synced.report.stack_branches, "C").unwrap();
  assert!(
    matches!(&whole_stack, WholeStack::Chain { root, leaf } if root == "A" && leaf == "C"),
    "{:?}",
    whole_stack.summary(PREFIX)
  );
  // and the Safe Push of the whole stack of A lands C, its leaf, which the block refuses
  let (exit_code, output) = safe_push(&environment, &["A", "--whole-stack", "--check"]);
  assert_eq!((exit_code, &output["error"]["code"]), (4, &json!("push_refused")), "{output}");
  let message = output["error"]["message"].as_str().unwrap();
  assert!(message.contains("C is not synced (Blocked: parent A has a stack problem"), "{message}");
}
