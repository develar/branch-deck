//! The side of bd of the scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` as a
//! repository of the bd-cli fixtures with the fake Patronus and the fake GitHub, then `plan()` after the preamble of
//! `safe_push()`, and the start itself without `--check`; today's answer expected. `src/lib/scenario-cases.test.ts` is
//! the side of the app

use crate::safe_push_command::{SafePushArgs, SafePushOutput, check_output, github_service, patronus_service, plan, start_of, start_output, whole_stack_arg};
use bd_cli::fixture_test::{PREFIX, commit_all, git, run_json, write};
use bd_cli::remote_fixture_test::{
  LANDED_SUBJECTS, Space, evaluated_review, fake_github, land_auth_by_content, minutes_ago, pull_request_json, reviewed_on_github, robot, safe_push, stack_robot,
};
use bd_core::context::{Environment, RepoContext, RepoOptions};
use bd_core::error::CliError;
use bd_core::sync_command::{lock, sync_locked};
use domain_types::patronus::PatronusPushMode;
use github::fake_github::{FakeAnswer, FakeGitHub};
use serde::Deserialize;
use serde::de::IgnoredAny;
use serde_json::{Value, json};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use sync_core::target_conflict::fetch_target;

/// The groups of the cases and their test functions here; the Vitest has the same groups
const GROUPS: [&str; 5] = ["modes-and-routes", "profiles", "review", "next-step-rows", "stack-and-whole-stack"];

/// A case of the table; `app` and `asymmetry` are the Vitest's
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
  id: String,
  group: String,
  title: String,
  spec: Vec<String>,
  discrepancy: Option<String>,
  #[expect(dead_code, reason = "the Vitest compares the classes of the outcomes")]
  asymmetry: Option<IgnoredAny>,
  given: Given,
  #[expect(dead_code, reason = "the expectations of the app, the Vitest's")]
  app: IgnoredAny,
  bd: Expected,
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
  parked: Option<Parked>,
  stack_branch: Option<StackBranch>,
  landed: Option<Landed>,
  landing_needs: Option<bool>,
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
enum Parked {
  /// The branch
  #[serde(rename = "self")]
  Itself,
  /// Its parent
  BuiltOn,
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
    let not_mapped = |what: &str| panic!("given.{what} is not mapped yet");
    if matches!(self.remote, Remote::Behind | Remote::RebuiltOnNewerBaseline) {
      not_mapped(&format!("remote {:?}", self.remote));
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

  fn mode(&self) -> PatronusPushMode {
    match self.mode {
      Mode::SafePush => PatronusPushMode::RebaseAutosquash,
      Mode::DryRun => PatronusPushMode::DryRun,
    }
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

/// The pull request #7 of the case's branch as the GitHub search lists it
fn pull_request_of(given: &GivenPullRequest, branch: &str) -> Value {
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
      Remote::UpToDate => bd_ok(&environment, &["push", branch]),
      Remote::Unpushed => {
        bd_ok(&environment, &["push", branch]);
        write(&space.repo, "more.txt", "more\n");
        commit_all(&space.repo, &format!("({branch}) More {branch}"), 1_700_009_000);
      }
      _ => {}
    }
    if let Some(parked) = given.parked {
      let name = match parked {
        Parked::Itself => branch,
        Parked::BuiltOn => given.parent_of(branch).expect("given.parked builtOn needs a parent"),
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

  /// `plan()` after the preamble of `safe_push()` with `--check`, its output as `--check` prints it
  fn check(&self, given: &Given) -> Result<SafePushOutput, CliError> {
    let args = given.args(true);
    let context = RepoContext::resolve(
      &self.environment,
      &RepoOptions {
        repo: None,
        prefix: Some(PREFIX),
        full_ids: false,
        no_cache: false,
      },
    )?;
    let service = patronus_service(&self.environment)?;
    let github = github_service(&context, &self.environment);
    let runtime = context.runtime()?;
    let target_fetch = (!args.create_pull_request).then(|| fetch_target(&context.git, &context.repository));
    // a check holds the lock for its sync only
    let synced = {
      let _lock = lock(&context)?;
      sync_locked(&context)?
    };
    let whole_stack = whole_stack_arg(&args, &synced)?;
    let start = start_of(&args, whole_stack.as_ref());
    let mode = given.mode();
    let plan = plan(&context, &args, &service, &github, runtime, &synced, mode, &start, target_fetch)?;
    Ok(check_output(start_output(&context, &args, mode, whole_stack.as_ref(), &plan), plan))
  }

  /// `bd safe-push` without `--check`: `{started_via}` or `{error}`
  fn start(&self, given: &Given) -> Value {
    let line = given.command_line();
    let args: Vec<&str> = line.iter().map(String::as_str).collect();
    let (exit_code, output) = safe_push(&self.environment, &args);
    if exit_code == 0 {
      json!({"started_via": output["started_via"]})
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

/// What bd answers today for the case, against its `bd` column: the failures
fn answer(case: &Case) -> Vec<String> {
  assert!(!case.spec.is_empty(), "{}: a case names the requirements it pins", case.id);
  let situation = Situation::of(&case.given);
  let check = match situation.check(&case.given) {
    Ok(output) => projection(&serde_json::to_value(output).unwrap()),
    Err(error) => json!({"error": error.code}),
  };
  let mut failures = mismatches(&case.id, "check", &case.bd.check, &check);
  if let Some(expected) = &case.bd.start {
    failures.extend(mismatches(&case.id, "start", expected, &situation.start(&case.given)));
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
