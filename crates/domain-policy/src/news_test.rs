use crate::news::{ApprovalNews, Finished, approval_news, request_key, review_requested, robot_finished};
use crate::readiness::Answered;
use domain_types::fixtures;
use domain_types::patronus::{
  ApprovedReviewGroup, MissingApprovalsReason, MissingReviewGroup, PatronusPushMode, PatronusRobot, PatronusRobotStatus, Person, ReviewReadiness, ReviewReadinessBranch,
  ReviewStart, UnavailableReadinessReason, UnknownReadinessReason,
};
use domain_types::pull_request::{Reviewer, ReviewerState};
use domain_types::review_request::{BackToYou, ReviewRequest};
use domain_types::session::NewsEvent;
use pretty_assertions::assert_eq;
use std::collections::BTreeSet;

pub(crate) fn person(login: &str, full_name: &str) -> Person {
  Person {
    login: Some(login.to_owned()),
    full_name: Some(full_name.to_owned()),
  }
}

pub(crate) fn ilya() -> Person {
  person("ikorennoy", "Ilya Korennoy")
}

pub(crate) fn waiting(groups: &[&str]) -> ReviewReadiness {
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

pub(crate) fn approved(groups: &[(&str, Vec<Person>)]) -> ReviewReadiness {
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

pub(crate) fn reviewer(login: &str, state: ReviewerState, requested: bool) -> Reviewer {
  Reviewer {
    login: login.to_owned(),
    state,
    requested,
  }
}

/// The ask of `wsm-skip-wa-2` at `tip` from pull request #1078 with `reviewers`
pub(crate) fn ask(tip: &str, reviewers: Vec<Reviewer>) -> ReviewReadinessBranch {
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

/// A robot known running or canceling that finished: merged when it pushed (a dry run pushes nothing), failed; nothing
/// for a cancel, while it runs, nor of a robot known finished already
#[test]
fn a_robot_known_running_that_finished_merged_or_failed() {
  let robot = |status, mode| PatronusRobot {
    push_mode: mode,
    ..fixtures::robot("r", Some("auth"), status, 1)
  };
  let (safe_push, dry_run) = (PatronusPushMode::RebaseAutosquash, PatronusPushMode::DryRun);
  let running = robot(PatronusRobotStatus::Running, safe_push);
  let canceling = robot(PatronusRobotStatus::Canceling, safe_push);
  let success = robot(PatronusRobotStatus::Success, safe_push);
  let failure = robot(PatronusRobotStatus::Failure, safe_push);
  assert_eq!(robot_finished(&running, &success), Some(Finished::Merged));
  assert_eq!(robot_finished(&canceling, &failure), Some(Finished::Failed));
  assert_eq!(robot_finished(&running, &robot(PatronusRobotStatus::Success, dry_run)), None, "a dry run merged nothing");
  assert_eq!(robot_finished(&running, &robot(PatronusRobotStatus::Canceled, safe_push)), None);
  assert_eq!(robot_finished(&running, &running), None);
  assert_eq!(robot_finished(&success, &success), None, "known finished");
}

/// The requests a listing adds against the one before: a new request and a re-request (a new request time), never one
/// back to the user after their answer, nor one the listing before had
#[test]
fn a_listing_tells_the_requests_it_adds_and_re_requests_never_one_back_to_the_user() {
  let known: BTreeSet<(u32, u32)> = [request_key(&fixtures::review_request(1, 100)), request_key(&fixtures::review_request(2, 100))].into();
  let back = ReviewRequest {
    back_to_you: Some(BackToYou { at: 300, replied_by: None }),
    ..fixtures::review_request(4, 300)
  };
  let listing = [fixtures::review_request(1, 100), fixtures::review_request(2, 200), fixtures::review_request(3, 300), back];
  assert_eq!(review_requested(&known, &listing), Some(NewsEvent::ReviewRequested { numbers: vec![2, 3] }));
  assert_eq!(review_requested(&known, &listing[..1]), None);
}

/// A pull request the listing before knew open with its auto-merge enabled, which a listing shows merged, is news, of a
/// branch and of a whole stack; one merged by the click (the session marked it merged), one without an auto-merge, and
/// another pull request of the branch are not
#[test]
fn a_pull_request_github_merged_by_its_auto_merge_is_news_once() {
  use crate::news::auto_merged;
  use domain_types::fixtures::pull_request;
  use domain_types::pull_request::{MergeMethod, PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult};
  use domain_types::session::{NewsEvent, PullRequestsView};
  let known = |auto: Option<MergeMethod>, state: PullRequestState| {
    let mut auth = pull_request("auth", 7);
    auth.auto_merge = auto;
    auth.state = state;
    let mut stack = pull_request("air", 12);
    stack.auto_merge = Some(MergeMethod::Merge);
    PullRequestsView {
      pull_requests: [("auth".to_string(), auth)].into(),
      stack_pull_requests: [("air".to_string(), stack)].into(),
      ..PullRequestsView::default()
    }
  };
  let merged = |pull_request: &mut PullRequest| pull_request.state = PullRequestState::Merged;
  let mut listed = PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub);
  let mut auth = pull_request("auth", 7);
  merged(&mut auth);
  listed.pull_requests.insert("auth".to_string(), auth);
  let mut stack = pull_request("air", 12);
  merged(&mut stack);
  listed.stack_pull_requests.insert("air".to_string(), stack);
  let news = auto_merged(&known(Some(MergeMethod::Rebase), PullRequestState::Open), &listed);
  assert_eq!(
    news,
    [
      NewsEvent::AutoMerged {
        number: 7,
        subject: "auth".to_string(),
        into: "master".to_string()
      },
      NewsEvent::AutoMerged {
        number: 12,
        subject: "stack:air".to_string(),
        into: "master".to_string()
      },
    ]
  );
  let told = |auto, state| auto_merged(&known(auto, state), &listed).len();
  assert_eq!(told(None, PullRequestState::Open), 1, "the stack's only");
  assert_eq!(told(Some(MergeMethod::Rebase), PullRequestState::Merged), 1, "the stack's only");
}
