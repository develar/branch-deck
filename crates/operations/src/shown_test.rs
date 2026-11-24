use crate::shown::{ShownServices, close_plan, open_plan, opened, run_shown, shown_body, shown_request};
use domain_policy::shown::shown_review;
use domain_types::fixtures::{pull_request, robot};
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, Intent, PullRequestField, Resource};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, PullRequestState, Reviewer, ReviewerState, ShownState};
use domain_types::reason::Reason;
use github::fake_github::FakeGitHub;
use github::fake_repository::{FakeGitHubRepository, FakePull, RepositoryState};
use pretty_assertions::assert_eq;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use test_utils::git_test_utils::{TestRepo, git_command, push_to_bare_origin};

fn logins(logins: &[&str]) -> Vec<String> {
  logins.iter().map(|login| (*login).to_owned()).collect()
}

/// The shown review #12 of auth, its body recording alice and bob
fn shown(draft: bool, reviewers: Vec<Reviewer>) -> domain_types::pull_request::ShownReview {
  let pull_request = PullRequest {
    head_ref_name: "user/review/auth/head".to_owned(),
    base_ref_name: "user/review/auth/base".to_owned(),
    is_draft: draft,
    reviewers,
    ..pull_request("auth", 12)
  };
  shown_review(pull_request, &shown_body("Adds auth.", &logins(&["alice", "bob"]), "master"))
}

fn merged(revision: &str) -> PatronusRobot {
  PatronusRobot {
    source_revision: Some(revision.to_owned()),
    ..robot("r1", Some("auth"), PatronusRobotStatus::Success, 1)
  }
}

fn reviewer(login: &str, state: ReviewerState, requested: bool) -> Reviewer {
  Reviewer {
    login: login.to_owned(),
    state,
    requested,
  }
}

#[test]
fn a_draft_opens_on_what_the_robot_landed_and_asks_the_reviewers_its_body_records() {
  let review = shown(true, Vec::new());
  assert_eq!(review.reviewers, logins(&["alice", "bob"]));
  let error = open_plan(&review, "auth", None).unwrap_err();
  assert_eq!(error.message, "No merged Safe Push landed auth yet: its shown review #12 opens once one did");
  let plan = open_plan(&review, "auth", Some(&merged("abc"))).unwrap();
  assert_eq!(
    plan.effects,
    [
      PlannedEffect::PushReview {
        branch: "auth".to_owned(),
        tip: "abc".to_owned()
      },
      PlannedEffect::MarkReady { number: 12 },
      PlannedEffect::RequestReviewers {
        number: 12,
        reviewers: logins(&["alice", "bob"])
      },
    ]
  );
  let request = shown_request("user", &plan.intent, Some(&plan));
  assert_eq!(
    request.resources,
    [
      Resource::Ref {
        name: "user/review/auth/head".to_owned()
      },
      Resource::Ref {
        name: "user/review/auth/base".to_owned()
      },
      Resource::PullRequest {
        number: 12,
        field: PullRequestField::Reviewers
      },
    ]
  );
  // ready already: only those it asked who neither approved nor are asked now
  let ready = shown(
    false,
    vec![reviewer("alice", ReviewerState::Approved, false), reviewer("bob", ReviewerState::Commented, false)],
  );
  assert_eq!(
    open_plan(&ready, "auth", None).unwrap().effects,
    [PlannedEffect::RequestReviewers {
      number: 12,
      reviewers: logins(&["bob"])
    }]
  );
}

#[test]
fn a_close_waits_for_every_approval_and_deletes_the_branches_of_the_review() {
  let waiting = shown(
    false,
    vec![reviewer("alice", ReviewerState::Approved, false), reviewer("bob", ReviewerState::Requested, true)],
  );
  let plan = close_plan(&waiting, "auth", false, "master");
  assert_eq!(
    plan.blockers,
    [Reason::ReviewNotApproved {
      number: 12,
      waiting: logins(&["bob"])
    }]
  );
  assert_eq!(close_plan(&waiting, "auth", true, "master").blockers, []);
  let approved = shown(
    false,
    vec![reviewer("alice", ReviewerState::Approved, false), reviewer("bob", ReviewerState::Approved, false)],
  );
  let plan = close_plan(&approved, "auth", false, "master");
  assert_eq!(
    plan.effects,
    [
      PlannedEffect::ClosePullRequest {
        branch: "auth".to_owned(),
        number: 12,
        comment: "Reviewed after landing: approved by alice, bob. The change is in master.".to_owned()
      },
      PlannedEffect::DeleteReview { branch: "auth".to_owned() },
    ]
  );
  assert_eq!(
    plan.intent,
    Intent::CloseShownReview {
      branch: "auth".to_owned(),
      allow_unapproved: false
    }
  );
}

/// The tip of `branch` on the bare origin of `repo`, None for one it lacks
fn origin_tip(repo: &TestRepo, branch: &str) -> Option<String> {
  let output = git_command()
    .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
    .current_dir(repo.path().join(".git/origin.git"))
    .output()
    .unwrap();
  output.status.success().then(|| String::from_utf8(output.stdout).unwrap().trim().to_owned())
}

/// The open and the close of a shown review against a bare origin and the stateful fake of GitHub, whose branches are
/// the origin's: the open pushes its branches, marks it ready and asks the reviewers; the close closes it with the comment
/// and deletes both branches
#[tokio::test(flavor = "multi_thread")]
async fn the_open_and_the_close_of_a_shown_review_against_origin_and_github() {
  let repo = TestRepo::new();
  let fork = repo.create_commit("Initial commit", "README.md", "# Test\n");
  push_to_bare_origin(&repo);
  repo.git(&["fetch", "-q", "origin"]);
  let landed = repo.create_commit("(auth) Add auth", "auth.txt", "auth\n");
  let path = repo.path_str().to_owned();

  let fake_repository = Arc::new(FakeGitHubRepository::default());
  let draft = shown(true, Vec::new());
  fake_repository.set(RepositoryState {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
    token: "t".to_owned(),
    viewer: "user".to_owned(),
    users: ["alice", "bob"]
      .iter()
      .map(|login| domain_types::pull_request::GitHubUser {
        login: (*login).to_owned(),
        name: None,
      })
      .collect(),
    pulls: vec![FakePull {
      pull_request: draft.pull_request.clone(),
      body: shown_body("Adds auth.", &logins(&["alice", "bob"]), "master"),
      comments: Vec::new(),
      assignees: Vec::new(),
      reviews: Vec::new(),
    }],
    ..RepositoryState::default()
  });
  let fake = Arc::new(FakeGitHub::with_repository(Arc::clone(&fake_repository)));
  let url = github::fake_github::serve(Arc::clone(&fake)).await;
  let tokens = github::token::TokenChain::new(github::token::TokenSources::fixed("t"), git_executor::git_command_executor::GitCommandExecutor::new());
  let github = Arc::new(github::service::GitHubService::new(
    github::client::GitHubClient::with_urls(&url, &url, "Branch Deck test"),
    tokens,
  ));
  let services = ShownServices {
    git: repo.git_executor().clone(),
    repository_path: path,
    prefix: "user".to_owned(),
    remote: "origin".to_owned(),
    baseline: "origin/master".to_owned(),
    github,
    repository: GitHubRepository {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    },
    text: None,
  };

  let plan = open_plan(&draft, "auth", Some(&merged(&landed))).unwrap();
  let run = run_shown(&services, &plan).await;
  assert!(run.records.iter().all(|record| record.status == EffectStatus::Done), "{:?}", run.records);
  assert_eq!(
    (origin_tip(&repo, "user/review/auth/head"), origin_tip(&repo, "user/review/auth/base")),
    (Some(landed.clone()), Some(fork))
  );
  let state = fake_repository.state();
  let pull = &state.pulls[0].pull_request;
  assert!(!pull.is_draft);
  assert_eq!(
    pull
      .reviewers
      .iter()
      .filter(|reviewer| reviewer.requested)
      .map(|reviewer| reviewer.login.as_str())
      .collect::<Vec<_>>(),
    ["alice", "bob"]
  );
  assert_eq!(
    opened(draft).state,
    ShownState::InReview {
      waiting: logins(&["alice", "bob"])
    }
  );

  let approved = shown(
    false,
    vec![reviewer("alice", ReviewerState::Approved, false), reviewer("bob", ReviewerState::Approved, false)],
  );
  let run = run_shown(&services, &close_plan(&approved, "auth", false, "master")).await;
  assert!(run.records.iter().all(|record| record.status == EffectStatus::Done), "{:?}", run.records);
  let state = fake_repository.state();
  assert_eq!(state.pulls[0].pull_request.state, PullRequestState::Closed);
  assert_eq!(
    state.pulls[0].comments.last().unwrap().body,
    "Reviewed after landing: approved by alice, bob. The change is in master."
  );
  assert_eq!((origin_tip(&repo, "user/review/auth/head"), origin_tip(&repo, "user/review/auth/base")), (None, None));
}
