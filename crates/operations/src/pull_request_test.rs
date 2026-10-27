use crate::http_budget_test::TIP;
use crate::land_execute::land_execute_test::{ALPHA, ALPHA_REMOTE, Fixture, fixture};
use crate::land_execute::{Accepted, Planned, PushLease, PushedRef, Steps, run_planned};
use crate::pull_request::{PullRequestFacts, create_planned, pull_request_request, retarget_request, review_planned};
use crate::robot::robot_test::{Sources, running};
use crate::witness::{Witness, read_refs};
use domain_policy::admission::admit;
use domain_types::fixtures::{missing_approvals, pull_request};
use domain_types::landing::{PlannedEffect, Subject};
use domain_types::operation::{Admission, CreatePullRequest, EffectStatus, Fingerprint, Intent, OperationId, OperationResult, Plan, PullRequestField, Resource, RunningMove};
use domain_types::patronus::{PatronusPushMode, Person, ReviewReadiness, ReviewReadinessBranch};
use domain_types::pull_request::PullRequest;
use domain_types::reason::Reason;
use domain_types::repository_view::RestartAction;
use domain_types::session::{Fact, FactState, SubjectReadiness};
use github::fake_github::{FakeAnswer, FakeGitHub, start_fake_service};
use pretty_assertions::assert_eq;
use repository_model::derive::ActionPushView;
use serde_json::{Value, json};
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;

const AUTH: &str = "refs/heads/user/virtual/auth";
const AUTH_REMOTE: &str = "refs/remotes/origin/user/virtual/auth";

/// The sources of `auth` at `TIP` whose remote branch is at "older" (`up_to_date`: it has the branch), without robots
fn sources(up_to_date: bool) -> Sources {
  Sources::new(Some("older"), up_to_date, Vec::new(), RestartAction::Restart)
}

/// The push of `branches` the button of auth makes (`derive::action_push_of`)
fn pushes(branches: &[&str], blocker: Option<Reason>) -> ActionPushView {
  ActionPushView {
    branches: branches.iter().map(|branch| (*branch).to_owned()).collect(),
    blocker,
  }
}

fn create(on_github: bool) -> CreatePullRequest {
  CreatePullRequest {
    subject: Subject::branch("auth"),
    title: "Add auth".to_owned(),
    body: "b".to_owned(),
    assign_self: true,
    reviewers: vec!["bob".to_owned()],
    on_github,
  }
}

fn created(sources: &Sources, push: ActionPushView, create: &CreatePullRequest) -> Planned {
  sources.with(|sources| create_planned(&PullRequestFacts { sources, push }, create)).unwrap()
}

/// A creation of a branch its remote lags pushes it first, on the tips it was planned on, then opens the pull request;
/// with "Continue on GitHub" it waits for GitHub to have the branch instead, and without a push it has nothing to wait
/// for. A push that can't be made for the user refuses it; a pull request of what it pushes into a virtual branch goes
/// into the target first (D3a)
#[test]
fn a_creation_pushes_first_what_the_remote_lacks_then_opens_the_pull_request() {
  let lagging = sources(false);
  let planned = created(&lagging, pushes(&["auth"], None), &create(false));
  let push = PlannedEffect::Push {
    branches: vec!["auth".to_owned()],
    force: false,
  };
  assert_eq!(planned.plan.effects, [push.clone(), PlannedEffect::CreatePullRequest]);
  assert_eq!(planned.plan.blockers, [] as [Reason; 0]);
  let refs = [(AUTH.to_owned(), Some(TIP.to_owned())), (AUTH_REMOTE.to_owned(), Some("older".to_owned()))];
  assert_eq!(planned.witness.refs, refs.into_iter().collect());
  assert_eq!((planned.source_ref.as_str(), planned.plan.lands.len()), (AUTH, 0));
  let on_github = created(&lagging, pushes(&["auth"], None), &create(true));
  let wait = PlannedEffect::WaitForMirror {
    branch: "user/virtual/auth".to_owned(),
  };
  assert_eq!(on_github.plan.effects, [push, wait]);

  let pushed = sources(true);
  assert_eq!(created(&pushed, pushes(&[], None), &create(false)).plan.effects, [PlannedEffect::CreatePullRequest]);
  assert_eq!(created(&pushed, pushes(&[], None), &create(true)).plan.effects, [] as [PlannedEffect; 0]);

  let drops = Reason::DropsRemote {
    branch: "auth".to_owned(),
    behind: 2,
  };
  assert_eq!(
    created(&lagging, pushes(&["auth"], Some(drops.clone())), &create(false)).plan.blockers,
    std::slice::from_ref(&drops)
  );
  // a branch whose push asks first sends nothing, and refuses the creation: its pull request would show the remote's
  assert_eq!(created(&lagging, pushes(&[], Some(drops.clone())), &create(false)).plan.blockers, [drops]);

  let mut into_parent = lagging;
  let parent = PullRequest {
    base_ref_name: "user/virtual/base".to_owned(),
    ..pull_request("auth", 7)
  };
  into_parent.pull_requests.pull_requests.insert("auth".to_owned(), parent);
  let retargeted = created(&into_parent, pushes(&["auth"], None), &create(false));
  assert_eq!(
    retargeted.plan.effects.first(),
    Some(&PlannedEffect::Retarget {
      branch: "auth".to_owned(),
      number: 7,
      from: "user/virtual/base".to_owned(),
      to: "master".to_owned(),
    })
  );
  assert!(retargeted.witness.pull_requests.contains_key(&7), "the pull request it retargets is of its witness");
}

/// The readiness of auth: IJPL Code Platform hasn't approved, Patronus suggests `login` (none: no GitHub login)
fn missing_code_owner(login: Option<&str>) -> Fact<SubjectReadiness> {
  let ReviewReadiness::Missing { mut groups, not_counted, reason } = missing_approvals("IJPL Code Platform", Some(1)) else {
    unreachable!()
  };
  groups[0].suggested = vec![Person {
    login: login.map(str::to_owned),
    full_name: Some("Ilya Korennoy".to_owned()),
  }];
  Fact {
    value: Some(SubjectReadiness {
      ask: ReviewReadinessBranch::starting_from("auth".to_owned(), None, TIP.to_owned(), None),
      requirement: None,
      readiness: ReviewReadiness::Missing { groups, not_counted, reason },
    }),
    state: FactState::Fresh,
    seq: 1,
    observed_at: 0,
  }
}

/// The sources of auth with its open pull request #7 into the target and the readiness `readiness`
fn reviewed(up_to_date: bool, readiness: Option<Fact<SubjectReadiness>>) -> Sources {
  let mut sources = sources(up_to_date);
  sources.pull_requests.pull_requests.insert("auth".to_owned(), pull_request("auth", 7));
  sources.readiness.extend(readiness.map(|fact| ("auth".to_owned(), fact)));
  sources
}

fn requested(sources: &Sources, push: ActionPushView, reviewers: &[&str], push_first: bool) -> Option<Planned> {
  let reviewers: Vec<String> = reviewers.iter().map(|login| (*login).to_owned()).collect();
  sources.with(|sources| review_planned(&PullRequestFacts { sources, push }, &Subject::branch("auth"), &reviewers, push_first))
}

fn ask(reviewers: &[&str]) -> PlannedEffect {
  PlannedEffect::RequestReviewers {
    number: 7,
    reviewers: reviewers.iter().map(|login| (*login).to_owned()).collect(),
  }
}

/// A review request asks whom it is told, else the code owner Request Review… preselects; one of a branch its remote
/// lags pushes first only when told to (D5), else `NeedsPush` refuses it; a draft, a choice of nobody, a branch without
/// an open pull request have no request to make
#[test]
fn a_review_request_pushes_first_only_when_told_to_and_asks_the_preselected_code_owner() {
  let pushed = reviewed(true, Some(missing_code_owner(Some("ikorennoy"))));
  let planned = requested(&pushed, pushes(&[], None), &[], false).unwrap();
  assert_eq!((planned.plan.effects.as_slice(), planned.plan.pull_request), (&[ask(&["ikorennoy"])][..], Some(7)));
  assert_eq!(planned.plan.blockers, [] as [Reason; 0]);
  let told = requested(&pushed, pushes(&[], None), &["alice", " Alice ", "bob"], false).unwrap();
  assert_eq!(told.plan.effects, [ask(&["alice", "bob"])]);

  let lagging = reviewed(false, Some(missing_code_owner(Some("ikorennoy"))));
  let refused = requested(&lagging, pushes(&["auth"], None), &[], false).unwrap();
  assert_eq!(refused.plan.blockers, [Reason::NeedsPush { branch: "auth".to_owned() }]);
  let first = requested(&lagging, pushes(&["auth"], None), &[], true).unwrap();
  let push = PlannedEffect::Push {
    branches: vec!["auth".to_owned()],
    force: false,
  };
  assert_eq!((first.plan.effects, first.plan.blockers), (vec![push, ask(&["ikorennoy"])], Vec::new()));

  let no_login = reviewed(true, Some(missing_code_owner(None)));
  let groups = vec!["IJPL Code Platform".to_owned()];
  assert_eq!(requested(&no_login, pushes(&[], None), &[], false).unwrap().plan.blockers, [Reason::NoReviewer { groups }]);
  let mut draft = reviewed(true, None);
  draft.pull_requests.pull_requests.get_mut("auth").unwrap().is_draft = true;
  assert_eq!(
    requested(&draft, pushes(&[], None), &["alice"], false).unwrap().plan.blockers,
    [Reason::Draft { number: 7 }]
  );
  assert!(requested(&sources(true), pushes(&[], None), &["alice"], false).is_none(), "no open pull request");
}

/// A review request holds the reviewers of its pull request: a retarget of it runs beside, unless its push retargets the
/// pull request too, which holds its base and waits for the retarget to be decided again over what it left. A creation
/// holds the head of its subject: the same click again joins it, another text waits for it
#[test]
fn a_review_request_waits_for_a_retarget_of_its_pull_request_only_when_its_push_retargets_it() {
  let retarget = retarget_request(7);
  let runs = [running("retarget", &retarget)];
  let plain = requested(&reviewed(true, None), pushes(&[], None), &["alice"], false).unwrap();
  let request = pull_request_request(&plain.plan.intent, Some(&plain));
  let reviewers = Resource::PullRequest {
    number: 7,
    field: PullRequestField::Reviewers,
  };
  assert_eq!(request.resources, [reviewers]);
  assert_eq!(
    request.moves,
    [RunningMove::RequestingReview {
      subject: "auth".to_owned(),
      number: 7
    }]
  );
  assert_eq!(admit(&runs, &request.intent, &request.resources), Admission::Run);

  let mut into_parent = reviewed(false, None);
  into_parent.pull_requests.pull_requests.get_mut("auth").unwrap().base_ref_name = "user/virtual/base".to_owned();
  let retargets = requested(&into_parent, pushes(&["auth"], None), &["alice"], true).unwrap();
  assert!(
    matches!(retargets.plan.effects[0], PlannedEffect::Retarget { number: 7, .. }),
    "{:?}",
    retargets.plan.effects
  );
  let request = pull_request_request(&retargets.plan.intent, Some(&retargets));
  let base = Resource::PullRequest {
    number: 7,
    field: PullRequestField::Base,
  };
  assert!(request.resources.contains(&base), "{:?}", request.resources);
  let queued = Admission::Queue {
    op: OperationId("retarget".to_owned()),
  };
  assert_eq!(admit(&runs, &request.intent, &request.resources), queued);

  let creation = created(&sources(false), pushes(&["auth"], None), &create(false));
  let request = pull_request_request(&creation.plan.intent, Some(&creation));
  assert_eq!(
    request.resources,
    [Resource::Ref {
      name: "user/virtual/auth".to_owned()
    }]
  );
  assert_eq!(request.moves, [RunningMove::CreatingPullRequest { subject: "auth".to_owned() }]);
  let runs = [running("create", &request)];
  let joined = Admission::Join {
    op: OperationId("create".to_owned()),
  };
  assert_eq!(admit(&runs, &request.intent, &request.resources), joined);
  let other = Intent::CreatePullRequest(CreatePullRequest {
    title: "Add the auth".to_owned(),
    ..create(false)
  });
  let queued = Admission::Queue {
    op: OperationId("create".to_owned()),
  };
  assert_eq!(admit(&runs, &other, &request.resources), queued);
}

const PULLS: &str = "/repos/acme/monorepo/pulls";

/// The fixture of a start (`alpha` not on the bare origin yet) with the fake GitHub of `acme/monorepo`, which knows the
/// user `me`
async fn on_github() -> (Fixture, Arc<FakeGitHub>) {
  let mut fixture = fixture().await;
  let (fake, github) = start_fake_service("t").await;
  fake.answer("GET", "/user", FakeAnswer::ok(json!({"login": "me"})));
  let repository = GitHubRepository {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
  };
  fixture.services.github = Some((Arc::new(github), repository));
  (fixture, fake)
}

/// The plan of `intent` of alpha with `effects`, planned on the tips of alpha and its remote branch read now; its push
/// sends alpha
fn alpha_planned(fixture: &Fixture, intent: Intent, effects: Vec<PlannedEffect>) -> Planned {
  let pushes = effects.iter().any(|effect| matches!(effect, PlannedEffect::Push { .. }));
  let names = [ALPHA.to_owned(), ALPHA_REMOTE.to_owned()];
  let refs = if pushes {
    read_refs(&fixture.services.git, &fixture.services.repository_path, &names).unwrap()
  } else {
    std::collections::BTreeMap::new()
  };
  let number = matches!(intent, Intent::RequestReview { .. }).then_some(7);
  Planned {
    plan: Plan {
      intent,
      fingerprint: Fingerprint("planned".to_owned()),
      lands: Vec::new(),
      effects,
      confirmations: Vec::new(),
      blockers: Vec::new(),
      waits: Vec::new(),
      alternatives: None,
      pull_request: number,
      approvals_skipped_by: None,
    },
    witness: Witness::of(refs, &[], &[]),
    pushes: pushes
      .then(|| PushedRef {
        name: "alpha".to_owned(),
        full_name: "test/virtual/alpha".to_owned(),
        parent_ref: None,
        whole_stack: false,
        unapplied: false,
        up_to: None,
        lease: PushLease::Frozen,
        my_email: None,
      })
      .into_iter()
      .collect(),
    source_ref: ALPHA.to_owned(),
    merge_request: None,
    notes: Vec::new(),
    subject: Subject::branch("alpha"),
    mode: PatronusPushMode::RebaseAutosquash,
  }
}

/// Runs `planned` on the host of the fixture, accepted as planned: how it went, and what it told the host
async fn run(fixture: &Fixture, planned: Planned) -> (crate::land_execute::Started, Vec<String>) {
  let host = Fixture::host(planned.clone());
  let accepted = Accepted {
    fingerprint: Fingerprint("planned".to_owned()),
    confirmations: Vec::new(),
  };
  let started = run_planned(&host, &fixture.services, &Steps::none(), OperationId("op".to_owned()), planned, &accepted)
    .await
    .unwrap();
  (started, host.told())
}

/// The writes of the fake GitHub, in their order: `METHOD path`
fn writes(fake: &FakeGitHub) -> Vec<String> {
  fake
    .requests()
    .iter()
    .filter(|request| request.method != "GET")
    .map(|request| format!("{} {}", request.method, request.path))
    .collect()
}

fn alpha_creation(on_github: bool) -> Intent {
  Intent::CreatePullRequest(CreatePullRequest {
    subject: Subject::branch("alpha"),
    on_github,
    ..create(false)
  })
}

/// The pull request #12 of alpha, as GitHub's REST answers it
fn alpha_pull_request() -> Value {
  json!({"number": 12, "title": "Add auth", "html_url": "https://github.com/acme/monorepo/pull/12", "state": "open", "draft": false,
    "head": {"ref": "test/virtual/alpha", "sha": "1111"}, "base": {"ref": "master"}, "updated_at": "2026-10-14T09:30:00Z",
    "user": {"login": "me"}, "requested_reviewers": []})
}

/// A creation again while GitHub's search doesn't list the pull request yet (a retry after a timeout, the app and `bd`
/// at once): GitHub refuses a second one of the head, and the REST list of the head finds the first, `created: false`,
/// which goes into the session
#[tokio::test(flavor = "multi_thread")]
async fn a_creation_again_finds_the_pull_request_the_search_does_not_list_yet() {
  let (fixture, fake) = on_github().await;
  let exists = json!({"resource": "PullRequest", "code": "custom", "message": "A pull request already exists for acme:test/virtual/alpha."});
  fake.answer("POST", PULLS, FakeAnswer::status(422, json!({"message": "Validation Failed", "errors": [exists]})));
  fake.answer("GET", PULLS, FakeAnswer::ok(json!([alpha_pull_request()])));
  let planned = alpha_planned(&fixture, alpha_creation(false), vec![PlannedEffect::CreatePullRequest]);
  let (started, told) = run(&fixture, planned).await;
  assert_eq!(started.outcome.effects[0].status, EffectStatus::Done);
  let OperationResult::PullRequest(created) = &started.outcome.result else {
    panic!("{:?}", started.outcome.result)
  };
  assert_eq!((created.created, created.pull_request.number), (false, 12));
  assert_eq!(writes(&fake), [format!("POST {PULLS}")], "nothing is assigned or asked of the one it found");
  assert!(told.iter().any(|told| told.starts_with("pull request Created")), "{told:?}");
  assert_eq!(started.outcome.news_key, None);
}

/// A review request of a branch its remote lags retargets its pull request into the target first, then pushes, then
/// asks whom its plan names: the PATCH of the base before the request of the reviewers, whom GitHub asked into the
/// session
#[tokio::test(flavor = "multi_thread")]
async fn a_review_request_retargets_and_pushes_first_then_asks_whom_its_plan_names() {
  let (fixture, fake) = on_github().await;
  fake.answer("PATCH", &format!("{PULLS}/7"), FakeAnswer::ok(json!({})));
  fake.answer(
    "POST",
    &format!("{PULLS}/7/requested_reviewers"),
    FakeAnswer::status(201, json!({"number": 7, "requested_reviewers": [{"login": "bob"}]})),
  );
  let intent = Intent::RequestReview {
    subject: Subject::branch("alpha"),
    reviewers: vec!["bob".to_owned()],
    push_first: true,
  };
  let effects = vec![
    PlannedEffect::Retarget {
      branch: "alpha".to_owned(),
      number: 7,
      from: "test/virtual/base".to_owned(),
      to: "master".to_owned(),
    },
    PlannedEffect::Push {
      branches: vec!["alpha".to_owned()],
      force: false,
    },
    PlannedEffect::RequestReviewers {
      number: 7,
      reviewers: vec!["bob".to_owned()],
    },
  ];
  let (started, told) = run(&fixture, alpha_planned(&fixture, intent, effects)).await;
  let statuses: Vec<&EffectStatus> = started.outcome.effects.iter().map(|record| &record.status).collect();
  assert_eq!(statuses, [&EffectStatus::Done; 3]);
  assert_eq!(writes(&fake), [format!("PATCH {PULLS}/7"), format!("POST {PULLS}/7/requested_reviewers")]);
  assert!(fixture.remote_alpha().is_some(), "the push went before the request");
  let OperationResult::Reviewers(requested) = &started.outcome.result else {
    panic!("{:?}", started.outcome.result)
  };
  assert_eq!(requested.requested, ["bob"]);
  assert!(told.iter().any(|told| told.starts_with("pull request ReviewersRequested")), "{told:?}");
}

/// "Continue on GitHub" pushes what the pull request needs and waits until GitHub has the branch, whose form the page
/// opens then; nothing is written to GitHub
#[tokio::test(flavor = "multi_thread")]
async fn continue_on_github_pushes_and_waits_until_github_has_the_branch() {
  let (fixture, fake) = on_github().await;
  fake.answer(
    "GET",
    "/repos/acme/monorepo/branches/test/virtual/alpha",
    FakeAnswer::ok(json!({"name": "test/virtual/alpha"})),
  );
  let effects = vec![
    PlannedEffect::Push {
      branches: vec!["alpha".to_owned()],
      force: false,
    },
    PlannedEffect::WaitForMirror {
      branch: "test/virtual/alpha".to_owned(),
    },
  ];
  let (started, _) = run(&fixture, alpha_planned(&fixture, alpha_creation(true), effects)).await;
  let statuses: Vec<&EffectStatus> = started.outcome.effects.iter().map(|record| &record.status).collect();
  assert_eq!(statuses, [&EffectStatus::Done; 2]);
  assert!(fixture.remote_alpha().is_some());
  assert_eq!(writes(&fake), [] as [String; 0]);
  assert_eq!(started.outcome.result, OperationResult::None);
}
