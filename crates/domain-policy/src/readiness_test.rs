use crate::readiness::{Answered, ApprovalNews, approval_news, start_pull_request};
use domain_types::fixtures::pull_request;
use domain_types::patronus::{
  ApprovedReviewGroup, MissingApprovalsReason, MissingReviewGroup, Person, ReviewReadiness, ReviewReadinessBranch, ReviewStart, UnavailableReadinessReason, UnknownReadinessReason,
};
use domain_types::pull_request::{PullRequestState, Reviewer, ReviewerState};
use pretty_assertions::assert_eq;

fn person(login: &str, full_name: &str) -> Person {
  Person {
    login: Some(login.to_owned()),
    full_name: Some(full_name.to_owned()),
  }
}

fn ilya() -> Person {
  person("ikorennoy", "Ilya Korennoy")
}

fn waiting(groups: &[&str]) -> ReviewReadiness {
  ReviewReadiness::Missing {
    groups: groups
      .iter()
      .map(|name| MissingReviewGroup {
        name: (*name).to_owned(),
        needed: Some(1),
        approved_by: Vec::new(),
        requested: vec![ilya()],
        suggested: vec![ilya()],
        changes_requested_by: Vec::new(),
      })
      .collect(),
    not_counted: Vec::new(),
    reason: MissingApprovalsReason::ApprovalsMissing,
  }
}

fn approved(groups: &[(&str, Vec<Person>)]) -> ReviewReadiness {
  ReviewReadiness::Approved {
    groups: groups
      .iter()
      .map(|(name, approved_by)| ApprovedReviewGroup {
        name: (*name).to_owned(),
        approved_by: approved_by.clone(),
      })
      .collect(),
  }
}

fn reviewer(login: &str, state: ReviewerState, requested: bool) -> Reviewer {
  Reviewer {
    login: login.to_owned(),
    state,
    requested,
  }
}

/// The ask of `wsm-skip-wa-2` at `tip` from pull request #1078 with `reviewers`
fn ask(tip: &str, reviewers: Vec<Reviewer>) -> ReviewReadinessBranch {
  ReviewReadinessBranch {
    branch_name: "wsm-skip-wa-2".to_owned(),
    source: None,
    tip: tip.to_owned(),
    start: ReviewStart::PullRequest { number: 1078 },
    reviewers,
    author: Some("user".to_owned()),
  }
}

fn news(before: (&ReviewReadinessBranch, &ReviewReadiness), after: (&ReviewReadinessBranch, &ReviewReadiness)) -> Option<ApprovalNews> {
  approval_news(
    Answered {
      ask: before.0,
      readiness: before.1,
    },
    Answered { ask: after.0, readiness: after.1 },
  )
}

#[test]
fn tells_the_approvals_that_made_a_waiting_pull_request_ready_for_the_same_tip() {
  let (before, after) = (
    ask("t1", vec![reviewer("ikorennoy", ReviewerState::Requested, true)]),
    ask("t1", vec![reviewer("ikorennoy", ReviewerState::Approved, false)]),
  );
  assert_eq!(
    news((&before, &waiting(&["IJPL Code Platform"])), (&after, &approved(&[("IJPL Code Platform", vec![ilya()])]))),
    Some(ApprovalNews {
      pull_request: 1078,
      approvers: vec![ilya()],
      groups: vec!["IJPL Code Platform".to_owned()],
    })
  );
  // no approval named once Patronus needs none: the code owners approved
  assert_eq!(
    news((&before, &waiting(&["IJPL Code Platform"])), (&after, &ReviewReadiness::NotRequired)).map(|news| news.approvers),
    Some(Vec::new())
  );
}

#[test]
fn names_every_approver_of_the_groups_that_waited_each_once_a_group_approved_before_is_not_news() {
  let anna = person("aivanova", "Anna Ivanova");
  let (before, after) = (ask("t1", Vec::new()), ask("t1", vec![reviewer("aivanova", ReviewerState::Approved, false)]));
  let after_readiness = approved(&[
    ("IJPL Code Platform", vec![ilya()]),
    ("Air Platform", vec![anna.clone(), person("IKorennoy", "Ilya K.")]),
    ("Docs", vec![person("bob", "Bob")]),
  ]);
  let told = news((&before, &waiting(&["IJPL Code Platform", "Air Platform"])), (&after, &after_readiness)).unwrap();
  assert_eq!(told.approvers, vec![ilya(), anna]);
  assert_eq!(told.groups, vec!["IJPL Code Platform".to_owned(), "Air Platform".to_owned()]);
}

#[test]
fn tells_the_approval_of_a_reviewer_github_waited_for_while_patronus_cannot_judge_the_approvals() {
  let unknown = ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::ApprovalsNotEvaluated,
  };
  let unavailable = ReviewReadiness::Unavailable {
    reason: UnavailableReadinessReason::Error,
    message: "Patronus answered 500".to_owned(),
  };
  let junie = || reviewer("junie-jetbrains", ReviewerState::Approved, false);
  let before = ask("t1", vec![junie(), reviewer("ikorennoy", ReviewerState::Requested, true)]);
  let after = ask("t1", vec![junie(), reviewer("ikorennoy", ReviewerState::Approved, false)]);
  let expected = Some(ApprovalNews {
    pull_request: 1078,
    approvers: vec![Person {
      login: Some("ikorennoy".to_owned()),
      full_name: None,
    }],
    groups: Vec::new(),
  });
  assert_eq!(news((&before, &unknown), (&after, &unknown)), expected);
  assert_eq!(news((&before, &unavailable), (&after, &unknown)), expected);
  // still waiting for someone else, or nobody approved (the request was withdrawn): no news
  let alice = || reviewer("alice", ReviewerState::Requested, true);
  let both = ask("t1", vec![reviewer("ikorennoy", ReviewerState::Requested, true), alice()]);
  assert_eq!(
    news(
      (&both, &unknown),
      (&ask("t1", vec![reviewer("ikorennoy", ReviewerState::Approved, false), alice()]), &unknown)
    ),
    None
  );
  assert_eq!(news((&before, &unknown), (&ask("t1", vec![junie()]), &unknown)), None);
  // a bot GitHub waits for is no reviewer of the news
  let bot = ask("t1", vec![reviewer("copilot-pull-request-reviewer[bot]", ReviewerState::Requested, true)]);
  assert_eq!(news((&bot, &unknown), (&ask("t1", Vec::new()), &unknown)), None);
}

#[test]
fn is_no_news_for_another_tip_a_start_through_the_api_the_same_reviews_or_a_readiness_still_missing() {
  let missing = waiting(&["IJPL Code Platform"]);
  let ready = approved(&[("IJPL Code Platform", vec![ilya()])]);
  let before = ask("t1", Vec::new());
  let reviewed = ask("t1", vec![reviewer("ikorennoy", ReviewerState::Approved, false)]);
  // a new commit of the user, not a review
  assert_eq!(news((&before, &missing), (&ask("t2", reviewed.reviewers.clone()), &ready)), None);
  let rest = |reviewers| ReviewReadinessBranch {
    start: ReviewStart::Rest,
    ..ask("t1", reviewers)
  };
  assert_eq!(news((&rest(Vec::new()), &missing), (&rest(reviewed.reviewers.clone()), &ready)), None);
  let other_pull_request = ReviewReadinessBranch {
    start: ReviewStart::PullRequest { number: 7 },
    ..before.clone()
  };
  assert_eq!(news((&other_pull_request, &missing), (&reviewed, &ready)), None);
  assert_eq!(news((&before, &missing), (&reviewed, &missing)), None);
  // a recheck of the same reviews: the kept answer was older, no review is new
  assert_eq!(news((&reviewed, &missing), (&reviewed, &ready)), None);
  let unknown = ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::ApprovalsNotEvaluated,
  };
  assert_eq!(news((&before, &unknown), (&reviewed, &ready)), None);
}

#[test]
fn a_safe_push_starts_from_the_open_pull_request_into_the_target_while_patronus_names_the_github_repository() {
  let open = pull_request("auth", 7);
  assert_eq!(start_pull_request(Some(&open), "master", Some("acme/monorepo")).map(|it| it.number), Some(7));
  // a base not read yet counts as the target
  let unread = domain_types::pull_request::PullRequest {
    base_ref_name: String::new(),
    ..open.clone()
  };
  assert_eq!(start_pull_request(Some(&unread), "master", Some("acme/monorepo")).map(|it| it.number), Some(7));
  // through the API: another base, a closed one, none, or no GitHub repository Patronus knows
  let other_base = domain_types::pull_request::PullRequest {
    base_ref_name: "user/virtual/login".to_owned(),
    ..open.clone()
  };
  let closed = domain_types::pull_request::PullRequest {
    state: PullRequestState::Closed,
    ..open.clone()
  };
  assert_eq!(start_pull_request(Some(&other_base), "master", Some("acme/monorepo")), None);
  assert_eq!(start_pull_request(Some(&closed), "master", Some("acme/monorepo")), None);
  assert_eq!(start_pull_request(None, "master", Some("acme/monorepo")), None);
  assert_eq!(start_pull_request(Some(&open), "master", None), None);
}
