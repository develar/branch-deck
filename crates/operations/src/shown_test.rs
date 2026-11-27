use crate::shown::{ShownRun, ShownServices, after_run, close_plan, delete_closed_plan, extend_plan, open_plan, run_shown, shown_body, shown_request};
use domain_policy::shown::{fix_conflict_comment, fix_taken_comment, recorded_landed, recorded_reviewers, shown_review};
use domain_types::error::ErrorCode;
use domain_types::fixtures::{asked, pull_request, robot};
use domain_types::landing::{LandedFix, PlannedEffect};
use domain_types::operation::{EffectStatus, Intent, PullRequestField, Resource};
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{ClosedShownReview, PullRequest, PullRequestState, Reviewer, ReviewerState, ShownReview, ShownState};
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
fn shown(draft: bool, reviewers: Vec<Reviewer>) -> ShownReview {
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
  let error = open_plan(&review, "auth", None, false, &[]).unwrap_err();
  assert_eq!(error.message, "No merged Safe Push landed auth yet: its shown review #12 opens once one did");
  let plan = open_plan(&review, "auth", Some(&merged("abc")), false, &[]).unwrap();
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
    open_plan(&ready, "auth", None, false, &[]).unwrap().effects,
    [PlannedEffect::RequestReviewers {
      number: 12,
      reviewers: logins(&["bob"])
    }]
  );
}

#[test]
fn a_draft_the_baseline_landed_opens_on_its_head_and_a_reviewer_the_user_adds_is_recorded() {
  // landed by content, no robot of it listed: its head stays
  let draft = shown(true, Vec::new());
  assert_eq!(
    open_plan(&draft, "auth", None, true, &[]).unwrap().effects,
    [
      PlannedEffect::MarkReady { number: 12 },
      PlannedEffect::RequestReviewers {
        number: 12,
        reviewers: logins(&["alice", "bob"])
      },
    ]
  );
  // ready: carol is asked beside bob and recorded, the plan is the one its run runs
  let ready = shown(
    false,
    vec![reviewer("alice", ReviewerState::Approved, false), reviewer("bob", ReviewerState::Commented, false)],
  );
  let plan = open_plan(&ready, "auth", None, false, &logins(&["carol", "Alice"])).unwrap();
  assert_eq!(
    plan.effects,
    [
      PlannedEffect::RequestReviewers {
        number: 12,
        reviewers: logins(&["alice", "bob", "carol"])
      },
      PlannedEffect::RecordShown {
        number: 12,
        reviewers: logins(&["alice", "bob", "carol"]),
        landed: Vec::new()
      },
    ]
  );
  assert_eq!(plan, open_plan(&ready, "auth", None, false, &logins(&["carol", "Alice"])).unwrap());
  assert_eq!(
    plan.intent,
    Intent::OpenShownReview {
      branch: "auth".to_owned(),
      reviewers: logins(&["carol", "Alice"])
    },
    "the intent names whom the user adds: the engine plans it again with them"
  );
  // a draft before its Safe Push merged: carol is only recorded, its open asks her with the others; nobody new is no plan
  let record = open_plan(&draft, "auth", None, false, &logins(&["carol"])).unwrap();
  assert_eq!(
    record.effects,
    [PlannedEffect::RecordShown {
      number: 12,
      reviewers: logins(&["alice", "bob", "carol"]),
      landed: Vec::new()
    }]
  );
  assert_eq!(open_plan(&draft, "auth", None, false, &logins(&["bob"])).unwrap_err().code, ErrorCode::NoShownReview);
}

#[test]
fn an_extend_adds_the_fixes_asks_the_reviewers_again_and_records_them_last() {
  let ready = shown(false, vec![reviewer("alice", ReviewerState::ChangesRequested, false)]);
  let error = extend_plan(&ready, "auth", &[], &[]).unwrap_err();
  assert_eq!(
    error.message,
    "No fix of auth landed after its shown review #12 opened: a fix lands by a Safe Push of the branch"
  );
  let fix = PatronusRobot {
    id: "r2".to_owned(),
    url: "https://patronus/robot/r2".to_owned(),
    ..merged("s2")
  };
  let plan = extend_plan(&ready, "auth", &[&fix], &[]).unwrap();
  assert_eq!(
    plan.effects,
    [
      PlannedEffect::ExtendReview {
        branch: "auth".to_owned(),
        number: 12,
        fixes: vec![LandedFix {
          robot: "r2".to_owned(),
          source: "s2".to_owned(),
          url: "https://patronus/robot/r2".to_owned()
        }]
      },
      PlannedEffect::RequestReviewers {
        number: 12,
        reviewers: logins(&["alice", "bob"])
      },
      PlannedEffect::RecordShown {
        number: 12,
        reviewers: logins(&["alice", "bob"]),
        landed: logins(&["s2"])
      },
    ]
  );
  assert_eq!(plan.intent, Intent::ExtendShownReview { branch: "auth".to_owned() });
  assert_eq!(
    shown_request("user", &plan.intent, Some(&plan)).resources.last(),
    Some(&Resource::PullRequest {
      number: 12,
      field: PullRequestField::Reviewers
    })
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

/// The closed shown review #12 of auth, ended at `head` and `base`, the remote's branches of it at `head_tip`, `base_tip`
fn closed(head_tip: Option<&str>, base_tip: Option<&str>) -> ClosedShownReview {
  ClosedShownReview {
    branch: "auth".to_owned(),
    number: 12,
    url: String::new(),
    head_ref_oid: "head".to_owned(),
    base_ref_oid: "base".to_owned(),
    closed_at: 1,
    head_tip: head_tip.map(str::to_owned),
    base_tip: base_tip.map(str::to_owned),
  }
}

#[test]
fn a_review_closed_elsewhere_deletes_the_branches_still_at_its_tips_and_a_reopened_one_without_its_base_pushes_it_again() {
  let plan = delete_closed_plan(&closed(Some("head"), Some("newer")), None).unwrap();
  assert_eq!(
    plan.effects,
    [PlannedEffect::DeleteReviewAt {
      branch: "auth".to_owned(),
      head: Some("head".to_owned()),
      base: None
    }]
  );
  assert_eq!(
    shown_request("user", &plan.intent, Some(&plan)).resources,
    [
      Resource::Ref {
        name: "user/review/auth/head".to_owned()
      },
      Resource::Ref {
        name: "user/review/auth/base".to_owned()
      }
    ]
  );
  // nothing left at its tips: nothing to do
  assert_eq!(delete_closed_plan(&closed(None, Some("newer")), None).unwrap().effects, []);
  // an open review of auth: its branches are that one's
  let open = shown(false, vec![reviewer("alice", ReviewerState::Requested, true)]);
  assert_eq!(
    delete_closed_plan(&closed(Some("head"), Some("base")), Some(&open)).unwrap_err().code,
    ErrorCode::NoShownReview
  );

  // reopened, its base deleted: GitHub restored only the head, so the open pushes the branches first
  let reopened = ShownReview {
    base_missing: true,
    ..shown(false, vec![reviewer("alice", ReviewerState::Approved, false)])
  };
  let plan = open_plan(&reopened, "auth", None, false, &[]).unwrap();
  assert_eq!(
    plan.effects,
    [
      PlannedEffect::PushReview {
        branch: "auth".to_owned(),
        tip: reopened.pull_request.head_ref_oid
      },
      PlannedEffect::RequestReviewers {
        number: 12,
        reviewers: logins(&["bob"])
      }
    ]
  );
}

/// A review closed on GitHub, listed by the search of the closed ones with what the remote has of its branches: its
/// delete takes the branches still at the tips of its pull request; once a newer review of auth took the head, the head
/// stays
#[tokio::test(flavor = "multi_thread")]
async fn the_branches_of_a_review_closed_on_github_are_deleted_where_they_are_at_its_tips() {
  let (repo, fork) = repo_on_origin();
  let landed = repo.create_commit("(auth) Add auth", "auth.txt", "auth\n");
  let review = shown(false, Vec::new());
  let fake_repository = github_of(&repo, &review);
  let services = services_of(&repo, &fake_repository).await;
  sync_core::review_head::push_review(&services.git, &services.repository_path, "origin", "user", "auth", &landed, "refs/remotes/origin/master").unwrap();
  fake_repository.update(|state| {
    let pull = &mut state.pulls[0].pull_request;
    pull.state = PullRequestState::Closed;
    pull.head_ref_oid.clone_from(&landed);
    pull.author = Some("user".to_owned());
  });
  let listed = |services: &ShownServices| {
    let services = services.clone();
    async move {
      remote_facts::pull_requests::list_pull_requests(&services.github, &services.repository_path, &services.repository, "user", true)
        .await
        .closed_shown_reviews
        .unwrap()
    }
  };
  let closed = listed(&services).await;
  assert_eq!(
    closed,
    [ClosedShownReview {
      branch: "auth".to_owned(),
      number: 12,
      url: review.pull_request.url.clone(),
      head_ref_oid: landed.clone(),
      base_ref_oid: fork.clone(),
      closed_at: review.pull_request.updated_at,
      head_tip: Some(landed.clone()),
      base_tip: Some(fork.clone()),
    }]
  );

  // a newer review of auth took the head since: the base of the closed one goes, the head stays
  let newer = repo.create_commit("(auth) Rework auth", "auth.txt", "auth 2\n");
  sync_core::review_head::push_review(&services.git, &services.repository_path, "origin", "user", "auth", &newer, "refs/remotes/origin/master").unwrap();
  let moved = ClosedShownReview {
    head_tip: Some(newer.clone()),
    ..closed[0].clone()
  };
  let run = run_shown(&services, &delete_closed_plan(&moved, None).unwrap()).await;
  assert!(run.records.iter().all(|record| record.status == EffectStatus::Done), "{:?}", run.records);
  assert_eq!(
    (origin_tip(&repo, "user/review/auth/head"), origin_tip(&repo, "user/review/auth/base")),
    (Some(newer), None)
  );
  // both at its tips: both go
  sync_core::review_head::push_review(&services.git, &services.repository_path, "origin", "user", "auth", &landed, "refs/remotes/origin/master").unwrap();
  let run = run_shown(&services, &delete_closed_plan(&listed(&services).await[0], None).unwrap()).await;
  assert!(run.records.iter().all(|record| record.status == EffectStatus::Done), "{:?}", run.records);
  assert_eq!((origin_tip(&repo, "user/review/auth/head"), origin_tip(&repo, "user/review/auth/base")), (None, None));
}

/// The tip of `branch` on the bare origin of `repo`, None for one it lacks
fn origin_tip(repo: &TestRepo, branch: &str) -> Option<String> {
  tip_in(&repo.path().join(".git/origin.git"), branch)
}

/// The tip of `branch` in the bare repository at `bare`, None for one it lacks
fn tip_in(bare: &std::path::Path, branch: &str) -> Option<String> {
  let output = git_command()
    .args(["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")])
    .current_dir(bare)
    .output()
    .unwrap();
  output.status.success().then(|| String::from_utf8(output.stdout).unwrap().trim().to_owned())
}

/// The open and the close of a shown review against a bare origin and the stateful fake of GitHub, whose branches are
/// the origin's: the open pushes its branches, marks it ready and asks the reviewers; the close closes it with the comment
/// and deletes both branches
#[tokio::test(flavor = "multi_thread")]
async fn the_open_and_the_close_of_a_shown_review_against_origin_and_github() {
  let (repo, fork) = repo_on_origin();
  let landed = repo.create_commit("(auth) Add auth", "auth.txt", "auth\n");
  let draft = shown(true, Vec::new());
  let fake_repository = github_of(&repo, &draft);
  let services = services_of(&repo, &fake_repository).await;

  let plan = open_plan(&draft, "auth", Some(&merged(&landed)), false, &[]).unwrap();
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
  let opened = after_run(draft, &plan, &run);
  assert_eq!(
    (opened.state, opened.pull_request.is_draft, opened.pull_request.head_ref_oid),
    (
      ShownState::InReview {
        waiting: logins(&["alice", "bob"])
      },
      false,
      landed.clone()
    )
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

/// The extend of a shown review against a bare origin and the stateful fake of GitHub: a fix that landed goes onto its
/// head, its reviewers are asked again and its body records the fix; a repeat adds nothing; a fix whose commit conflicts
/// takes the files as master has them and is named with them in a comment once; a fix that doesn't apply leaves the head
/// and is named in a comment once
#[tokio::test(flavor = "multi_thread")]
async fn the_extend_of_a_shown_review_against_origin_and_github() {
  let (repo, _) = repo_on_origin();
  let first = repo.create_commit("(auth) Add auth", "auth.txt", "auth\n");
  sync_core::review_head::push_review(repo.git_executor(), repo.path_str(), "origin", "user", "auth", &first, "refs/remotes/origin/master").unwrap();
  repo.land_copy_on_origin(&first);
  repo.git(&["checkout", "-q", "-B", "fix", "origin/master"]);
  let fix = repo.create_commit("(auth) Fix auth", "auth.txt", "auth 2\n");
  repo.land_copy_on_origin(&fix);

  // opened on the first landing, alice requested changes
  let review = shown(false, vec![reviewer("alice", ReviewerState::ChangesRequested, false)]);
  let fake_repository = github_of(&repo, &review);
  let services = services_of(&repo, &fake_repository).await;
  let robot_of = |id: &str, source: &str| PatronusRobot {
    id: id.to_owned(),
    url: format!("https://patronus/robot/{id}"),
    ..merged(source)
  };
  let r2 = robot_of("r2", &fix);
  let plan = extend_plan(&review, "auth", &[&r2], &[]).unwrap();
  let run = run_shown(&services, &plan).await;
  assert!(run.records.iter().all(|record| record.status == EffectStatus::Done), "{:?}", run.records);
  let head = origin_tip(&repo, "user/review/auth/head").unwrap();
  assert_eq!(repo.git(&["rev-parse", &format!("{head}^")]), first);
  assert_eq!(run.extended.as_ref().map(|extended| extended.head.clone()), Some(head.clone()));
  let state = fake_repository.state();
  let pull = &state.pulls[0];
  assert_eq!(asked(&pull.pull_request), [("alice", true), ("bob", true)]);
  assert_eq!(
    (recorded_reviewers(&pull.body), recorded_landed(&pull.body)),
    (logins(&["alice", "bob"]), vec![fix.clone()])
  );
  assert!(pull.body.starts_with("Adds auth."), "{}", pull.body);
  assert_eq!(pull.comments.len(), 0);
  let extended = after_run(review.clone(), &plan, &run);
  assert_eq!(
    (extended.pull_request.head_ref_oid.as_str(), extended.landed.clone(), extended.state.clone()),
    (
      head.as_str(),
      vec![fix.clone()],
      ShownState::InReview {
        waiting: logins(&["alice", "bob"])
      }
    )
  );

  // a repeat (a crash before the record): the head has the fix, nothing more is pushed
  let run = run_shown(&services, &plan).await;
  assert!(run.records.iter().all(|record| record.status == EffectStatus::Done), "{:?}", run.records);
  assert_eq!(origin_tip(&repo, "user/review/auth/head"), Some(head.clone()));

  // the target reworked auth, and the next fix is of that rework: its commit takes auth.txt as master has it after the
  // fix, a comment names the file once
  repo.git(&["checkout", "-q", "-B", "landing", "origin/master"]);
  repo.create_commit("Rework auth", "auth.txt", "reworked\n");
  repo.git(&["push", "-q", "origin", "HEAD:master"]);
  repo.git(&["fetch", "-q", "origin", "master"]);
  repo.git(&["checkout", "-q", "-B", "second", "origin/master"]);
  let second = repo.create_commit("(auth) Fix the rework", "auth.txt", "reworked 2\n");
  repo.land_copy_on_origin(&second);
  let r3 = robot_of("r3", &second);
  let plan = extend_plan(&extended, "auth", &[&r3], &[]).unwrap();
  let mut taken = Vec::new();
  for _ in 0..2 {
    let run: ShownRun = run_shown(&services, &plan).await;
    assert!(run.records.iter().all(|record| record.status == EffectStatus::Done), "{:?}", run.records);
    assert_eq!(run.not_applied, []);
    taken.push(run.taken.iter().map(|taken| (taken.fix.robot.clone(), taken.files.clone())).collect::<Vec<_>>());
  }
  // a repeat finds the commit on the head
  assert_eq!(taken, [vec![("r3".to_owned(), logins(&["auth.txt"]))], vec![]]);
  let taken_head = origin_tip(&repo, "user/review/auth/head").unwrap();
  assert_eq!(repo.git(&["rev-parse", &format!("{taken_head}^")]), head);
  assert_eq!(repo.git(&["show", &format!("{taken_head}:auth.txt")]), "reworked 2");
  let extended = after_run(extended, &plan, &run_shown(&services, &plan).await);

  // the target reworked it again, and the next fix landed under another subject (an autosquash): it doesn't apply, the
  // head stays, a comment names it once
  repo.git(&["checkout", "-q", "-B", "landing", "origin/master"]);
  repo.create_commit("Rework auth again", "auth.txt", "reworked 3\n");
  repo.git(&["push", "-q", "origin", "HEAD:master"]);
  repo.git(&["fetch", "-q", "origin", "master"]);
  repo.git(&["checkout", "-q", "-B", "third", "origin/master"]);
  let third = repo.create_commit("(auth) Fix the second rework", "auth.txt", "reworked 4\n");
  repo.git(&["checkout", "-q", "-B", "landing", "origin/master"]);
  repo.create_commit("Fix the second rework of auth", "auth.txt", "reworked 4\n");
  repo.git(&["push", "-q", "origin", "HEAD:master"]);
  repo.git(&["fetch", "-q", "origin", "master"]);
  let r4 = robot_of("r4", &third);
  let plan = extend_plan(&extended, "auth", &[&r4], &[]).unwrap();
  for _ in 0..2 {
    let run: ShownRun = run_shown(&services, &plan).await;
    assert!(run.records.iter().all(|record| record.status == EffectStatus::Done), "{:?}", run.records);
    assert_eq!(run.not_applied.iter().map(|fix| fix.robot.as_str()).collect::<Vec<_>>(), ["r4"]);
  }
  assert_eq!(origin_tip(&repo, "user/review/auth/head"), Some(taken_head));
  let state = fake_repository.state();
  assert_eq!(
    state.pulls[0].comments.iter().map(|comment| comment.body.clone()).collect::<Vec<_>>(),
    [
      fix_taken_comment(&second, &logins(&["auth.txt"]), "https://patronus/robot/r3", "master"),
      fix_conflict_comment(&third, "https://patronus/robot/r4")
    ]
  );
  assert_eq!(recorded_landed(&state.pulls[0].body), vec![fix, second, third]);
}

/// A repository on a bare origin whose master it fetched: the commit of it
fn repo_on_origin() -> (TestRepo, String) {
  let repo = TestRepo::new();
  let fork = repo.create_commit("Initial commit", "README.md", "# Test\n");
  push_to_bare_origin(&repo);
  repo.git(&["fetch", "-q", "origin"]);
  (repo, fork)
}

/// The stateful fake of GitHub of `acme/monorepo` (its user `user`, alice and bob can be asked) with the pull request of
/// `review`, its body recording alice and bob, whose branches are the ones of the bare origin of `repo`
fn github_of(repo: &TestRepo, review: &ShownReview) -> Arc<FakeGitHubRepository> {
  let fake_repository = Arc::new(FakeGitHubRepository::default());
  let origin = repo.path().join(".git/origin.git");
  fake_repository.set_branch_tip(Box::new(move |branch| tip_in(&origin, branch)));
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
      pull_request: review.pull_request.clone(),
      body: shown_body("Adds auth.", &logins(&["alice", "bob"]), "master"),
      comments: Vec::new(),
      assignees: Vec::new(),
      reviews: Vec::new(),
    }],
    ..RepositoryState::default()
  });
  fake_repository
}

/// The services of the runs of a shown review of `repo` against the fake of GitHub `repository`
async fn services_of(repo: &TestRepo, repository: &Arc<FakeGitHubRepository>) -> ShownServices {
  let fake = Arc::new(FakeGitHub::with_repository(Arc::clone(repository)));
  let url = github::fake_github::serve(Arc::clone(&fake)).await;
  let tokens = github::token::TokenChain::new(github::token::TokenSources::fixed("t"), git_executor::git_command_executor::GitCommandExecutor::new());
  let github = Arc::new(github::service::GitHubService::new(
    github::client::GitHubClient::with_urls(&url, &url, "Branch Deck test"),
    tokens,
  ));
  ShownServices {
    git: repo.git_executor().clone(),
    repository_path: repo.path_str().to_owned(),
    prefix: "user".to_owned(),
    remote: "origin".to_owned(),
    baseline: "origin/master".to_owned(),
    github,
    repository: GitHubRepository {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    },
    text: None,
  }
}
