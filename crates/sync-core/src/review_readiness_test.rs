use crate::github_repository::GitHubRepository;
use crate::patronus::SpaceRepository;
use crate::review_readiness::{ReadinessInputs, ReadinessRepository, branch_review_readiness, is_fresh, prepare_review_readiness};
use domain_types::patronus::{
  BranchReviewReadiness, GetBranchReviewReadinessParams, MissingApprovalsReason, PatronusReviewRequirement, ReviewReadiness, ReviewReadinessBranch, ReviewStart,
  ReviewerApprovalState,
};
use domain_types::pull_request::{Reviewer, ReviewerState};
use object_cache::cache::{CacheConfig, ObjectCache};
use object_cache::review_answer::ReviewAnswer;
use patronus::fake_patronus::{FakePatronus, start_fake};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A branch of one commit on `baseline`, a Patronus that evaluates pull request #7 (the code owner is asked, hasn't
/// approved), the cache of the test
struct Setup {
  repo: TestRepo,
  tip: String,
  fake: Arc<FakePatronus>,
  service: PatronusService,
  cache: ObjectCache,
  _cache_dir: tempfile::TempDir,
}

async fn setup() -> Setup {
  let repo = TestRepo::new();
  repo.create_commit("Base", "base.txt", "base");
  let path = repo.path().to_str().unwrap();
  repo.git_executor().execute_command(&["branch", "baseline"], path).unwrap();
  repo.create_commit("IJPL-1 fix", "platform/A.kt", "a");
  let tip = repo.git_executor().execute_command(&["rev-parse", "HEAD"], path).unwrap();

  let (fake, client) = start_fake().await;
  fake.review_required.store(true, Ordering::SeqCst);
  *fake.pull_request_review.lock().unwrap() = Some(json!({
    "approvalRequired": true,
    "reviewerGroups": [{"groupName": "IJPL Code Platform", "paths": ["platform/A.kt"], "requiredApprovals": 1, "collectedApprovals": 0, "approved": false, "selfApprovalAllowed": false,
      "reviewers": [{"spaceUsername": "ikorennoy", "fullName": "Ilya Korennoy", "githubUsername": "ikorennoy", "isAvailable": true, "isSuggested": true, "approvalState": "REVIEWING"}]}],
  }));
  let service = PatronusService::new(client, |_| {
    TokenLookup::Found(Token {
      value: "t".to_string(),
      stamp: TokenStamp::Env,
    })
  });
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(cache_dir.path(), &CacheConfig::default());
  Setup {
    repo,
    tip,
    fake,
    service,
    cache,
    _cache_dir: cache_dir,
  }
}

fn branch(tip: &str, start: ReviewStart, reviewers: Vec<Reviewer>) -> ReviewReadinessBranch {
  ReviewReadinessBranch {
    branch_name: "IJPL-1".to_string(),
    source: None,
    tip: tip.to_string(),
    start,
    reviewers,
    author: Some("me".to_string()),
  }
}

fn reviewer(login: &str, state: ReviewerState, requested: bool) -> Reviewer {
  Reviewer {
    login: login.to_string(),
    state,
    requested,
  }
}

impl Setup {
  async fn readiness(&self, branches: &[ReviewReadinessBranch], recheck: bool) -> Vec<BranchReviewReadiness> {
    self.readiness_in(branches, recheck, true).await
  }

  /// `readiness`, of a repository reviewed on GitHub (`acme/monorepo`) or, without `on_github`, only on Space
  async fn readiness_in(&self, branches: &[ReviewReadinessBranch], recheck: bool, on_github: bool) -> Vec<BranchReviewReadiness> {
    let space = SpaceRepository {
      project_key: "ij".to_string(),
      name: "ultimate".to_string(),
    };
    let github = GitHubRepository {
      owner: "acme".to_string(),
      name: "monorepo".to_string(),
    };
    let repository = ReadinessRepository {
      space: &space,
      github: on_github.then_some(&github),
      target: "master",
      baseline: "baseline",
      branch_prefix: "me",
    };
    let path = self.repo.path().to_str().unwrap();
    branch_review_readiness(self.repo.git_executor(), path, &self.cache, &self.service, &repository, branches, recheck)
      .await
      .unwrap()
  }

  fn checks(&self) -> u32 {
    self.fake.check_calls.load(Ordering::SeqCst)
  }
}

#[test(tokio::test(flavor = "multi_thread"))]
async fn the_answer_about_a_pull_request_is_kept_until_a_review_changes() {
  let setup = setup().await;
  let requested = vec![
    reviewer("ikorennoy", ReviewerState::Requested, true),
    reviewer("junie-jetbrains", ReviewerState::Approved, false),
  ];
  let from_pull_request = branch(&setup.tip, ReviewStart::PullRequest { number: 7 }, requested.clone());

  let [answer] = <[BranchReviewReadiness; 1]>::try_from(setup.readiness(std::slice::from_ref(&from_pull_request), false).await).unwrap();
  assert_eq!((answer.branch_name.as_str(), answer.tip.as_str()), ("IJPL-1", setup.tip.as_str()));
  let ReviewReadiness::Missing { groups, not_counted, reason } = &answer.readiness else {
    panic!("{answer:?}")
  };
  assert_eq!(reason, &MissingApprovalsReason::ApprovalsMissing);
  assert_eq!(groups[0].requested[0].full_name.as_deref(), Some("Ilya Korennoy"));
  assert_eq!(not_counted[0].login.as_deref(), Some("junie-jetbrains"));
  let requirement = answer.requirement.unwrap();
  assert_eq!(requirement.groups[0].reviewers[0].approval_state, Some(ReviewerApprovalState::Reviewing));
  assert_eq!(*setup.fake.checked_pull_requests.lock().unwrap(), [json!({"repository": "acme/monorepo", "number": 7})]);
  // the pre-check got the commit with its path, from the object cache's diff-tree
  assert_eq!(setup.checks(), 1);

  // the same paths and reviews: kept, also after a sync onto a new baseline
  setup.readiness(std::slice::from_ref(&from_pull_request), false).await;
  assert_eq!(setup.checks(), 1);

  // a review: asked once
  let approved = branch(
    &setup.tip,
    ReviewStart::PullRequest { number: 7 },
    vec![reviewer("ikorennoy", ReviewerState::Approved, false), requested[1].clone()],
  );
  setup.readiness(std::slice::from_ref(&approved), false).await;
  setup.readiness(std::slice::from_ref(&approved), false).await;
  assert_eq!(setup.checks(), 2);

  // a recheck always asks, and keeps the answer
  setup.readiness(std::slice::from_ref(&approved), true).await;
  assert_eq!(setup.checks(), 3);
}

/// A start through the API sends no pull request: the answer without one is the verdict
#[test(tokio::test(flavor = "multi_thread"))]
async fn a_rest_start_is_checked_without_the_pull_request() {
  let setup = setup().await;
  let answers = setup.readiness(&[branch(&setup.tip, ReviewStart::Rest, Vec::new())], false).await;
  assert!(
    matches!(
      answers[0].readiness,
      ReviewReadiness::Missing {
        reason: MissingApprovalsReason::NoPullRequestContext,
        ..
      }
    ),
    "{answers:?}"
  );
  assert_eq!(*setup.fake.checked_pull_requests.lock().unwrap(), [serde_json::Value::Null]);
}

#[test(tokio::test(flavor = "multi_thread"))]
async fn a_failed_check_is_unavailable_and_not_kept() {
  let setup = setup().await;
  setup.fake.set_status(503);
  let from_pull_request = branch(&setup.tip, ReviewStart::PullRequest { number: 7 }, Vec::new());
  let answers = setup.readiness(&[from_pull_request.clone(), from_pull_request.clone()], false).await;
  assert!(matches!(answers[0].readiness, ReviewReadiness::Unavailable { .. }), "{answers:?}");
  assert_eq!(answers[1].readiness, answers[0].readiness);
  // one shared backoff: the second branch of the batch is not asked
  assert_eq!(setup.checks(), 1);

  setup.fake.set_status(200);
  let answers = setup.readiness(std::slice::from_ref(&from_pull_request), true).await;
  assert!(matches!(answers[0].readiness, ReviewReadiness::Missing { .. }), "{answers:?}");
  assert_eq!(setup.checks(), 2);
}

#[test]
fn an_answer_with_the_evaluation_holds_a_day_one_without_it_minutes() {
  let evaluated = ReviewAnswer {
    fetched_at: 1_000_000,
    answer: PatronusReviewRequirement {
      required: true,
      merge_request: None,
      groups: vec![domain_types::patronus::PatronusReviewerGroup {
        name: "Air".to_string(),
        reviewers: Vec::new(),
        paths: Vec::new(),
        branch_wide: false,
        paths_source: domain_types::patronus::ReviewPathsSource::Patronus,
        required_approvals: Some(1),
        collected_approvals: Some(0),
        approved: Some(false),
        self_approval_allowed: Some(false),
      }],
      skipped: false,
    },
  };
  assert!(is_fresh(&evaluated, 1_000_000 + 23 * 3600));
  assert!(!is_fresh(&evaluated, 1_000_000 + 25 * 3600));

  let mut required = evaluated;
  required.answer.groups[0].approved = None;
  assert!(is_fresh(&required, 1_000_000 + 60));
  assert!(!is_fresh(&required, 1_000_000 + 180));
  let not_required = ReviewAnswer {
    answer: PatronusReviewRequirement {
      required: false,
      groups: Vec::new(),
      ..required.answer.clone()
    },
    ..required
  };
  assert!(is_fresh(&not_required, 1_000_000 + 300));
  assert!(!is_fresh(&not_required, 1_000_000 + 900));
}

#[tokio::test]
async fn a_tip_gone_since_the_page_sent_it_makes_only_its_branch_unavailable() {
  let setup = setup().await;
  let gone = branch("0123456789abcdef0123456789abcdef01234567", ReviewStart::Rest, Vec::new());
  let answers = setup.readiness(&[branch(&setup.tip, ReviewStart::Rest, Vec::new()), gone], false).await;
  assert!(matches!(answers[0].readiness, ReviewReadiness::Missing { .. }), "{answers:?}");
  assert!(matches!(answers[1].readiness, ReviewReadiness::Unavailable { .. }), "{answers:?}");
}

/// The whole stack is asked as its stack branch (`source`), under the key the caller gives (the app's `stack:<root>`):
/// a repository only on Space looks up the merge request of the stack branch. A source that is not a stack branch of
/// the prefix is not asked
#[test(tokio::test(flavor = "multi_thread"))]
async fn the_whole_stack_is_asked_as_its_stack_branch_under_the_key_of_the_caller() {
  let setup = setup().await;
  let stack = |source: &str| ReviewReadinessBranch {
    branch_name: "stack:IJPL-1".to_string(),
    source: Some(source.to_string()),
    ..branch(&setup.tip, ReviewStart::Rest, Vec::new())
  };
  let answers = setup.readiness_in(&[stack("me/stack/IJPL-1"), stack("me/virtual/IJPL-1")], false, false).await;
  assert_eq!(answers[0].branch_name, "stack:IJPL-1");
  assert!(matches!(answers[0].readiness, ReviewReadiness::Missing { .. }), "{answers:?}");
  assert_eq!(*setup.fake.merge_request_sources.lock().unwrap(), ["me/stack/IJPL-1"]);
  assert_eq!(
    answers[1].readiness,
    ReviewReadiness::Unavailable {
      reason: domain_types::patronus::UnavailableReadinessReason::Error,
      message: "me/virtual/IJPL-1 is not a stack branch of me".to_string(),
    }
  );
}

/// The one composition of the inputs the app and the test server read the readiness with: the tips are checked, the
/// target is the branch of origin the baseline tracks, a local baseline is refused
#[test(tokio::test)]
async fn the_inputs_of_the_readiness_are_composed_once_for_the_app_and_the_test_server() {
  let setup = setup().await;
  let path = setup.repo.path().to_str().unwrap();
  let git = setup.repo.git_executor();
  git.execute_command(&["remote", "add", "origin", "."], path).unwrap();
  git.execute_command(&["update-ref", "refs/remotes/origin/master", "HEAD"], path).unwrap();
  let space = SpaceRepository {
    project_key: "ij".to_string(),
    name: "ultimate".to_string(),
  };
  let github = GitHubRepository {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
  };
  let params = |baseline: &str, tip: &str| GetBranchReviewReadinessParams {
    repository_path: path.to_string(),
    branch_prefix: "me".to_string(),
    baseline_branch: baseline.to_string(),
    branches: vec![branch(tip, ReviewStart::Rest, Vec::new())],
    recheck: false,
  };

  let inputs = prepare_review_readiness(git, space.clone(), Some(github.clone()), &params("origin/master", &setup.tip)).unwrap();
  assert_eq!(
    inputs,
    ReadinessInputs {
      space: space.clone(),
      github: Some(github),
      target: "master".to_string(),
      baseline: "origin/master".to_string(),
      branch_prefix: "me".to_string(),
    }
  );
  assert_eq!(
    (inputs.repository().target, inputs.repository().github.map(|it| it.owner.as_str())),
    ("master", Some("acme"))
  );

  let refused = prepare_review_readiness(git, space.clone(), None, &params("origin/master", "HEAD")).unwrap_err();
  assert_eq!(refused.to_string(), "Not a commit id: HEAD");
  let local = prepare_review_readiness(git, space, None, &params("baseline", &setup.tip)).unwrap_err();
  assert!(local.to_string().contains("not a branch of origin"), "{local}");
}
