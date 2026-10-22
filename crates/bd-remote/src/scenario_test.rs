//! The side of bd of the scenario table (`crates/domain-policy/cases/*.json`, its README): each case's `given` as a
//! repository of the bd-cli fixtures with the fake Patronus and the fake GitHub, then `plan()` after the preamble of
//! `safe_push()`, and the start itself without `--check`; today's answer expected. `src/lib/scenario-cases.test.ts` is
//! the side of the app

use crate::safe_push_command::{SafePushArgs, SafePushOutput, check_output, github_service, patronus_service, plan, start_of, start_output, whole_stack_arg};
use bd_cli::fixture_test::{PREFIX, run_json};
use bd_cli::remote_fixture_test::{Space, evaluated_review, fake_github, minutes_ago, pull_request_json, reviewed_on_github, robot, safe_push};
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
const GROUPS: [&str; 3] = ["modes-and-routes", "profiles", "review"];

const BRANCH: &str = "auth";

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
  /// A command is a confirmed start: only the app tells a click from a key
  via: Via,
  subject: GivenSubject,
  commits: Vec<String>,
  target: Target,
  remote: Remote,
  robot: Option<GivenRobot>,
  pull_request: Option<GivenPullRequest>,
  readiness: Readiness,
  profile: Option<GivenProfile>,
  flags: Vec<Flag>,
  rules: Option<IgnoredAny>,
  parked: Option<bool>,
  stack_branch: Option<IgnoredAny>,
  landed: Option<bool>,
  landing_needs: Option<bool>,
}

impl Default for Given {
  fn default() -> Self {
    Self {
      mode: Mode::SafePush,
      via: Via::Click,
      subject: GivenSubject::Branch,
      commits: vec!["(auth) Add auth".to_string()],
      target: Target::Protected,
      remote: Remote::None,
      robot: None,
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GivenRobot {
  state: RobotState,
  /// `tip`, `older`, `stack`, `ancestor:<branch>`, `child:<branch>`
  on: String,
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
#[serde(deny_unknown_fields, default)]
struct GivenPullRequest {
  base: String,
  draft: bool,
  approvers: Vec<String>,
  reviewers: Vec<String>,
  state: PullRequestState,
}

impl Default for GivenPullRequest {
  fn default() -> Self {
    Self {
      base: "master".to_string(),
      draft: false,
      approvers: Vec::new(),
      reviewers: Vec::new(),
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

  /// The values the adapter maps (the README says which); anything else fails the case
  fn check_mapped(&self) {
    let not_mapped = |what: &str| panic!("given.{what} is not mapped yet");
    if self.subject != GivenSubject::Branch {
      not_mapped("subject");
    }
    if self.commits.len() != 1 {
      not_mapped("commits (more than one branch)");
    }
    if !matches!(self.remote, Remote::None | Remote::UpToDate) {
      not_mapped(&format!("remote {:?}", self.remote));
    }
    if let Some(robot) = &self.robot
      && (robot.state != RobotState::Failure || robot.on != "older")
    {
      not_mapped(&format!("robot {:?} on {}", robot.state, robot.on));
    }
    if self.pull_request.as_ref().is_some_and(|pull_request| !pull_request.reviewers.is_empty()) {
      not_mapped("pullRequest.reviewers");
    }
    for (present, what) in [
      (self.rules.is_some(), "rules"),
      (self.parked.is_some(), "parked"),
      (self.stack_branch.is_some(), "stackBranch"),
      (self.landed.is_some(), "landed"),
      (self.landing_needs.is_some(), "landingNeeds"),
    ] {
      if present {
        not_mapped(what);
      }
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

  fn mode(&self) -> PatronusPushMode {
    match self.mode {
      Mode::SafePush => PatronusPushMode::RebaseAutosquash,
      Mode::DryRun => PatronusPushMode::DryRun,
    }
  }

  fn args(&self, check: bool) -> SafePushArgs {
    SafePushArgs {
      branch: BRANCH.to_string(),
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
    let mut line = vec![BRANCH.to_string()];
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

/// The pull request #7 of auth as the GitHub search lists it
fn pull_request_of(given: &GivenPullRequest) -> Value {
  let approvers: Vec<&str> = given.approvers.iter().map(String::as_str).collect();
  let mut pull_request = pull_request_json(&approvers);
  pull_request["author"] = json!({"login": "me"});
  pull_request["isDraft"] = json!(given.draft);
  pull_request["baseRefName"] = json!(given.base);
  pull_request["state"] = json!(format!("{:?}", given.state).to_uppercase());
  pull_request
}

/// The robot `older` of auth as the list and `GET /robots/{id}` answer it: with the profile of `profile.lastRobot`,
/// whose options say whether it skips the review
fn add_robot(space: &Space, environment: &mut Environment, given: &GivenRobot, profile: Option<&GivenProfile>) {
  let state = format!("{:?}", given.state).to_uppercase();
  let listed = robot("older", &state, BRANCH, "0000");
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
  let mut robots = space.fake.robots.lock().unwrap().take().unwrap_or_default();
  robots.push(listed);
  *space.fake.robots.lock().unwrap() = Some(robots);
}

impl Situation {
  fn of(given: &Given) -> Self {
    given.check_mapped();
    let subjects: Vec<&str> = given.commits.iter().map(String::as_str).collect();
    let space = Space::new(&subjects);
    let pull_requests: Vec<Value> = given.pull_request.iter().map(pull_request_of).collect();
    // every repository but one only on Space is reviewed on GitHub
    let (mut environment, github) = if given.readiness == Readiness::Space {
      (space.environment(), None)
    } else {
      let (github, url) = fake_github(&space, &pull_requests);
      (reviewed_on_github(&space, &url), Some(github))
    };
    let review_required = !matches!(given.readiness, Readiness::NotRequired | Readiness::Unlisted);
    space.fake.review_required.store(review_required, Ordering::SeqCst);
    let has_pull_request = given.pull_request.is_some();
    match given.readiness {
      Readiness::NotRequired | Readiness::Space => {}
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
    if let Some(robot) = &given.robot {
      add_robot(&space, &mut environment, robot, given.profile.as_ref());
    }
    if given.remote == Remote::UpToDate {
      let (exit_code, output) = run_json(&environment, &["--prefix", PREFIX, "push", BRANCH]);
      assert_eq!(exit_code, 0, "bd push: {output}");
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
