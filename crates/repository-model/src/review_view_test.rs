use crate::facts::{FactResult, FactStore, RobotsRead};
use crate::readiness_facts::ReadinessAnswer;
use crate::review_request_facts::review_request_facts_test::{at, available};
use crate::review_view::{approval_landing_of, pull_key, review_actions_of, review_groups_of, review_stack_key, review_stacks_of, reviews_view};
use crate::table::TableState;
use crate::table::table_test::grouping;
use domain_types::SyncEvent;
use domain_types::actions::{ActionBlocker, ActionId, Availability};
use domain_types::fixtures::{missing_approvals, review_request};
use domain_types::operation::ReviewedLayer;
use domain_types::patronus::{BranchReviewReadiness, PatronusAvailability, PatronusReviewRequirement, PatronusRobotsResult, ReviewReadiness};
use domain_types::pull_request::{GitHubAuthStatus, PullRequestAvailability, PullRequestReviewEvent, ReviewerState};
use domain_types::review_request::{LinkedAnswer, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewRequestsResult, ReviewSearch, ReviewsAnswer};
use domain_types::session::{ApprovalLanding, Demand, PullRequestsView, ReviewSectionKind};
use domain_types::sign_in::AuthStatus;
use domain_types::youtrack::YouTrackAuthStatus;
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashSet};

/// A table synced on `origin/master`, which Patronus pushes into `master`
fn synced() -> TableState {
  let mut state = TableState::new();
  let mut grouped = grouping(Vec::new());
  if let SyncEvent::BranchesGrouped { target_branch, .. } = &mut grouped {
    *target_branch = Some("master".to_owned());
  }
  state.apply(grouped);
  state
}

fn patronus_answers(facts: &mut FactStore) {
  let listed = PatronusRobotsResult {
    availability: PatronusAvailability::Available,
    robots: Vec::new(),
    github_repository: None,
  };
  facts.apply(
    at(1),
    FactResult::Robots {
      read: RobotsRead::List { full: true },
      result: listed,
    },
  );
}

fn entry(request: ReviewRequest, involvement: ReviewInvolvement) -> ReviewEntry {
  ReviewEntry { request, involvement }
}

fn with_head(number: u32, base: &str) -> ReviewRequest {
  ReviewRequest {
    head_ref_oid: format!("head-{number}"),
    base_ref_name: base.to_owned(),
    ..review_request(number, 100)
  }
}

#[test]
fn the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review_and_what_its_approval_counts_for() {
  let state = synced();
  let mut facts = FactStore::new();
  patronus_answers(&mut facts);
  facts.review_requests.demand(&Demand {
    reviews: true,
    ..Demand::default()
  });
  let others = ReviewsAnswer {
    team: ReviewSearch {
      total_count: 1,
      entries: vec![entry(with_head(2, "release/253"), ReviewInvolvement::Team { teams: vec!["ide".to_owned()] })],
    },
    authored: ReviewSearch {
      total_count: 1,
      entries: vec![entry(
        with_head(3, "master"),
        ReviewInvolvement::Authored {
          reviewers: Vec::new(),
          is_draft: false,
          branch: None,
        },
      )],
    },
    linked: vec![LinkedAnswer::Open {
      entry: Box::new(entry(with_head(4, "master"), ReviewInvolvement::Linked { linked_at: 9 })),
    }],
    ..ReviewsAnswer::default()
  };
  let result = ReviewRequestsResult {
    availability: PullRequestAvailability::Available {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    },
    total_count: 1,
    requests: vec![with_head(1, "master")],
    answered: Vec::new(),
    others: Some(others),
  };
  facts.apply(at(2), FactResult::ReviewRequests(result));

  let reviews = reviews_view(&state, &facts, &BTreeMap::new());
  let kinds: Vec<(ReviewSectionKind, Vec<u32>)> = reviews
    .sections
    .iter()
    .map(|section| (section.kind, section.entries.iter().map(|entry| entry.request.number).collect()))
    .collect();
  assert_eq!(
    kinds,
    vec![
      (ReviewSectionKind::Linked, vec![4]),
      (ReviewSectionKind::Team, vec![2]),
      (ReviewSectionKind::Authored, vec![3])
    ]
  );
  // the link the user hasn't reviewed is the user's move
  let moves: Vec<Vec<u32>> = reviews.sections.iter().map(|section| section.moves.clone()).collect();
  assert_eq!(moves, vec![vec![4], vec![], vec![]]);

  let actions = review_actions_of(&state, &facts, &reviews);
  assert_eq!(actions.keys().cloned().collect::<Vec<_>>(), [1, 2, 3, 4].map(pull_key).to_vec());
  let reason = |number: u32, id: ActionId| {
    actions[&pull_key(number)].iter().find(|action| action.id == id).map(|action| match &action.availability {
      Availability::Enabled => None,
      Availability::Disabled { reason } => Some(reason.clone()),
      operation => panic!("no operation runs, yet {id:?} is {operation:?}"),
    })
  };
  // a request by name into the target: everything runs
  assert_eq!(reason(1, ActionId::SafePushPullRequest), Some(None));
  // into another base Patronus would merge it there
  assert_eq!(
    reason(2, ActionId::SafePushPullRequest),
    Some(Some(ActionBlocker::OtherBase {
      number: 2,
      base: "release/253".to_owned(),
      target: "master".to_owned(),
    }))
  );
  // the own pull request takes no review of its author
  assert_eq!(reason(3, ActionId::Approve), Some(Some(ActionBlocker::OwnPullRequest)));
  // a link can be removed
  assert_eq!(reason(4, ActionId::RemoveFromReviews), Some(None));

  // what an approval counts for where Patronus merges: the Safe Push of its own pull request into the target, one into
  // another base once it goes there, nothing of the own one
  assert_eq!(
    approval_landing_of(&state, &facts, &reviews),
    BTreeMap::from([
      (pull_key(1), ApprovalLanding::OwnSafePush),
      (
        pull_key(2),
        ApprovalLanding::OnceIntoTarget {
          base: "release/253".to_owned(),
          target: "master".to_owned(),
        }
      ),
      (pull_key(4), ApprovalLanding::OwnSafePush),
    ])
  );
  // nothing where Patronus doesn't merge
  assert_eq!(approval_landing_of(&state, &FactStore::new(), &reviews), BTreeMap::new());
}

/// The reviewer groups of a pull request a review window shows come of the readiness of `request:<number>` by
/// `pull:<number>`, each approved or not, the user's by the GitHub login of the app's sign-in; none of a pull request
/// whose changes need no approval, and none once the window closed
#[test]
fn the_view_has_the_reviewer_groups_of_a_pull_request_a_review_window_shows() {
  let state = synced();
  let mut facts = FactStore::new();
  patronus_answers(&mut facts);
  facts.review_requests.answer(at(2), available(vec![with_head(7, "master"), with_head(8, "master")]));
  facts.review_requests.fetched(7, "head-7".to_owned());
  facts.review_requests.fetched(8, "head-8".to_owned());
  let windows = Demand {
    review_windows: vec![7, 8],
    ..Demand::default()
  };
  let ask = |facts: &mut FactStore, demand: &Demand| {
    let requests = facts.review_requests.readiness_asks(&demand.review_windows);
    facts.readiness.set_requests(requests);
    let robots = facts.robots.view().head.value.clone();
    facts
      .readiness
      .update(&state, &PullRequestsView::default(), robots.as_ref(), demand, "user", &HashSet::new());
    facts.review_requests.demand(demand);
  };
  ask(&mut facts, &windows);
  let member =
    |login: &str, state: &str| serde_json::json!({"fullName": login, "username": login, "githubLogin": login, "available": true, "suggested": false, "approvalState": state});
  let group = |name: &str, approved: bool, reviewers: serde_json::Value| serde_json::json!({"name": name, "paths": [format!("{name}.kt")], "branchWide": false, "pathsSource": "patronus", "approved": approved, "reviewers": reviewers});
  let groups: PatronusReviewRequirement = serde_json::from_value(serde_json::json!({"required": true, "mergeRequest": null, "skipped": false, "groups": [
    group("Design", false, serde_json::json!([member("user-name", "notAdded")])),
    group("Air Plugin", true, serde_json::json!([member("carol", "approved")])),
  ]}))
  .unwrap();
  let answers = facts
    .readiness
    .digest()
    .subjects
    .iter()
    .map(|subject| {
      let needed = subject.key == "request:7";
      ReadinessAnswer {
        subject: subject.clone(),
        result: Ok(BranchReviewReadiness {
          branch_name: subject.key.clone(),
          tip: subject.ask.tip.clone(),
          requirement: Some(if needed { groups.clone() } else { NOT_REQUIRED }),
          readiness: if needed { missing_approvals("Design", Some(1)) } else { ReviewReadiness::NotRequired },
        }),
      }
    })
    .collect();
  facts.apply(at(3), FactResult::Readiness(answers));
  // (name, approved, yours) of each group, by row
  type Groups = Vec<(String, bool, Option<bool>)>;
  let summary = |facts: &FactStore| -> Vec<(String, Groups)> {
    review_groups_of(facts)
      .into_iter()
      .map(|(key, groups)| (key, groups.into_iter().map(|group| (group.name, group.approved, group.yours)).collect()))
      .collect()
  };
  let pull_7 = |design: Option<bool>, air: Option<bool>| vec![(pull_key(7), vec![("Design".to_owned(), false, design), ("Air Plugin".to_owned(), true, air)])];
  assert_eq!(summary(&facts), pull_7(None, None), "nobody knows whose a group is without the login of a sign-in");

  facts.apply(at(4), FactResult::Auth(Ok(signed_in("user-name"))));
  assert_eq!(summary(&facts), pull_7(Some(true), Some(false)));
  assert_eq!(review_groups_of(&facts)[&pull_key(7)][0].paths, ["Design.kt"]);

  ask(&mut facts, &Demand::default());
  assert_eq!(summary(&facts), [], "the window closed");
}

/// Approve N Pull Requests of a stack a review window shows is decided by `review-stack:<top>` on the heads its fetch
/// brought (openspec/specs/repository/review-requests/spec.md, "Review window: Approve N Pull Requests approves every layer
/// the window shows"): a layer GitHub says the user approved on that head is skipped, one approved from the app since
/// too, a layer shown on an older head than the listing names is approved on the one shown; the window closed, it goes
#[test]
fn the_view_decides_approve_n_pull_requests_of_a_stack_on_the_heads_its_fetch_brought() {
  let state = synced();
  let mut facts = FactStore::new();
  let approved_7 = ReviewRequest {
    reviewed_commit: Some("head-7".to_owned()),
    reviewed_state: Some(ReviewerState::Approved),
    ..with_head(7, "master")
  };
  facts
    .review_requests
    .answer(at(1), available(vec![approved_7, with_head(8, "branch-7"), with_head(9, "branch-8")]));
  facts.review_requests.demand(&Demand {
    review_windows: vec![7, 8, 9],
    ..Demand::default()
  });
  assert_eq!(review_stacks_of(&state, &facts), BTreeMap::new(), "no fetch of the stack yet");

  let layers = vec![(7, "head-7".to_owned()), (8, "head-8".to_owned()), (9, "old-9".to_owned())];
  facts.review_requests.stack_fetched(9, layers);
  let layer = |number: u32, commit: &str| ReviewedLayer {
    number,
    commit: commit.to_owned(),
  };
  let approval = &review_stacks_of(&state, &facts)[&review_stack_key(9)];
  assert_eq!(
    (approval.approves.clone(), approval.approved.clone()),
    (vec![layer(8, "head-8"), layer(9, "old-9")], vec![7])
  );
  assert_eq!((approval.action.id, &approval.action.availability), (ActionId::ApproveStack, &Availability::Enabled));

  facts.review_requests.submitted(8, PullRequestReviewEvent::Approve, "head-8".to_owned());
  facts.review_requests.submitted(9, PullRequestReviewEvent::Approve, "old-9".to_owned());
  let done = &review_stacks_of(&state, &facts)[&review_stack_key(9)];
  assert_eq!((done.approves.clone(), done.approved.clone()), (Vec::new(), vec![7, 8, 9]));
  assert_eq!(
    done.action.availability,
    Availability::Disabled {
      reason: ActionBlocker::StackApproved
    }
  );

  facts.review_requests.demand(&Demand::default());
  assert_eq!(review_stacks_of(&state, &facts), BTreeMap::new(), "the window closed");
}

const NOT_REQUIRED: PatronusReviewRequirement = PatronusReviewRequirement {
  required: false,
  merge_request: None,
  groups: Vec::new(),
  skipped: false,
};

/// The sign-ins with the app's own GitHub sign-in of `login`
fn signed_in(login: &str) -> AuthStatus {
  AuthStatus {
    github: GitHubAuthStatus {
      sign_in_available: true,
      signed_in_as: Some(login.to_owned()),
      token_source: None,
      manage_url: None,
    },
    youtrack: YouTrackAuthStatus {
      url: None,
      signed_in_as: None,
      source: None,
      problem: None,
      signed_in_at: None,
      manage_url: None,
    },
  }
}
