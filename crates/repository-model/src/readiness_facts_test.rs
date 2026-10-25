use crate::facts::{FactResult, FactStore, Observation, Seq};
use crate::readiness_facts::{ReadinessAnswer, ReadinessFact, ReadinessSubject};
use crate::table::TableState;
use crate::table::table_test::{run_sync, sync_events};
use domain_policy::refresh::ReadinessPriority;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{missing_approvals, pull_request};
use domain_types::patronus::{ApprovedReviewGroup, BranchReviewReadiness, PatronusAvailability, Person, ReviewReadiness, ReviewStart};
use domain_types::pull_request::{PullRequest, Reviewer, ReviewerState};
use domain_types::session::{Demand, FactState, NewsEvent, PullRequestsView, RobotsHead};
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use domain_types::{BranchSyncStatus, CommitSyncStatus, SyncEvent};
use pretty_assertions::assert_eq;
use std::time::Duration;

/// The table of `auth` (copies `copy-a2`, `copy-a1`) and `ui` (`copy-u1`), with the whole stack of `auth` built
fn table() -> TableState {
  let mut state = TableState::new();
  let mut events = sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created);
  events.push(SyncEvent::StackBranchUpdate(Box::new(StackBranchUpdate {
    root: "auth".to_owned(),
    branch_name: "user/stack/auth".to_owned(),
    members: vec!["auth".to_owned(), "ui".to_owned()],
    commit_count: 3,
    state: StackBranchState::Built,
    tip: Some("stack-tip".to_owned()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  })));
  run_sync(&mut state, events);
  state
}

fn available() -> RobotsHead {
  RobotsHead {
    availability: PatronusAvailability::Available,
    github_repository: Some("acme/monorepo".to_owned()),
  }
}

/// The open pull request #7 of `auth` into master, with `reviewers`
fn pull_requests(reviewers: Vec<Reviewer>) -> PullRequestsView {
  let pull_request = PullRequest {
    reviewers,
    ..pull_request("auth", 7)
  };
  PullRequestsView {
    pull_requests: [("auth".to_owned(), pull_request)].into(),
    ..PullRequestsView::default()
  }
}

fn shows(branches: &[&str]) -> Demand {
  Demand {
    visible_branches: branches.iter().map(ToString::to_string).collect(),
    ..Demand::default()
  }
}

fn at(seq: u32) -> Observation {
  Observation {
    seq: Seq(seq),
    inputs: 0,
    at: Duration::from_secs(u64::from(seq)),
    observed_at: 1_700_000_000,
  }
}

fn missing() -> ReviewReadiness {
  missing_approvals("IJPL Code Platform", Some(1))
}

fn answer(subject: &ReadinessSubject, readiness: ReviewReadiness) -> ReadinessAnswer {
  ReadinessAnswer {
    subject: subject.clone(),
    result: Ok(BranchReviewReadiness {
      branch_name: subject.key.clone(),
      tip: subject.ask.tip.clone(),
      requirement: None,
      readiness,
    }),
  }
}

fn subject<'a>(fact: &'a ReadinessFact, key: &str) -> &'a ReadinessSubject {
  fact.digest().subjects.iter().find(|subject| subject.key == key).unwrap()
}

fn state_of(fact: &ReadinessFact, key: &str) -> FactState {
  fact.fact(key).unwrap().state.clone()
}

/// The ask of a shown subject is the tip its row shows and the pull request its robot would start from, with the
/// reviews of it; the whole stack is asked as its stack branch; a subject nothing shows is asked for nothing
#[test]
fn the_ask_is_the_tip_a_window_shows_and_the_pull_request_its_robot_starts_from() {
  let mut fact = ReadinessFact::default();
  let state = table();
  let demand = Demand {
    visible_stacks: vec!["auth".to_owned()],
    needed: vec!["ui".to_owned()],
    ..shows(&["auth"])
  };
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &demand);
  let digest = fact.digest();
  assert_eq!(digest.baseline, "origin/master");
  let asked: Vec<(&str, &str, ReviewStart, Option<&str>, ReadinessPriority)> = digest
    .subjects
    .iter()
    .map(|subject| {
      (
        subject.key.as_str(),
        subject.ask.tip.as_str(),
        subject.ask.start,
        subject.ask.source.as_deref(),
        subject.priority,
      )
    })
    .collect();
  assert_eq!(
    asked,
    [
      ("stack:auth", "stack-tip", ReviewStart::Rest, Some("user/stack/auth"), ReadinessPriority::Shown),
      ("auth", "copy-a2", ReviewStart::PullRequest { number: 7 }, None, ReadinessPriority::Shown),
      ("ui", "copy-u1", ReviewStart::Rest, None, ReadinessPriority::Demanded),
    ]
  );
  assert_eq!(state_of(&fact, "auth"), FactState::Loading);
  // nothing asks for it: no subject
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &Demand::default());
  assert_eq!(fact.digest().subjects, []);
  // without the GitHub repository Patronus names, the robot would start through the API
  let head = RobotsHead {
    github_repository: None,
    ..available()
  };
  fact.update(&state, &pull_requests(vec![]), Some(&head), &shows(&["auth"]));
  assert_eq!(subject(&fact, "auth").ask.start, ReviewStart::Rest);
}

/// Before Patronus answered the list of the robots a needed subject is `Loading`; while Patronus doesn't answer for the
/// repository it is `Unavailable` with why and nothing is asked; once Patronus answers it is asked
#[test]
fn a_needed_subject_is_unavailable_while_patronus_does_not_answer_for_the_repository() {
  let mut fact = ReadinessFact::default();
  let (state, demand) = (
    table(),
    Demand {
      needed: vec!["auth".to_owned()],
      ..Demand::default()
    },
  );
  fact.update(&state, &pull_requests(vec![]), None, &demand);
  assert_eq!((state_of(&fact, "auth"), fact.digest().subjects.len()), (FactState::Loading, 0));
  let logged_out = RobotsHead {
    availability: PatronusAvailability::NotLoggedIn,
    github_repository: None,
  };
  fact.update(&state, &pull_requests(vec![]), Some(&logged_out), &demand);
  let reason = ApiError::new(ErrorCode::NotLoggedIn, "Run ./safePush.cmd once to log in to Patronus");
  assert_eq!((state_of(&fact, "auth"), fact.digest().subjects.len()), (FactState::Unavailable { reason }, 0));
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &demand);
  assert_eq!((state_of(&fact, "auth"), fact.digest().subjects.len()), (FactState::Loading, 1));
}

/// A new review is new inputs: the answer is `Stale` in the update that saw it, and asked again while shown; a hidden
/// subject is asked again only when its last answer waits for approvals at the same tip and start (the hidden ones
/// last), never for another tip
#[test]
fn new_inputs_make_the_readiness_stale_and_a_hidden_subject_is_asked_only_for_new_reviews() {
  let mut fact = ReadinessFact::default();
  let state = table();
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &shows(&["auth"]));
  let asked = subject(&fact, "auth").clone();
  assert!(fact.answer(at(2), answer(&asked, missing())).0);
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &shows(&["auth"]));
  assert_eq!(state_of(&fact, "auth"), FactState::Fresh);

  let reviewed = pull_requests(vec![Reviewer {
    login: "ikorennoy".to_owned(),
    state: ReviewerState::Approved,
    requested: false,
  }]);
  // hidden now: the new review asks it, after the shown ones
  fact.update(&state, &reviewed, Some(&available()), &Demand::default());
  assert_eq!(state_of(&fact, "auth"), FactState::Stale { reason: None });
  assert_eq!(subject(&fact, "auth").priority, ReadinessPriority::HiddenAwaiting);
  assert_ne!(subject(&fact, "auth").inputs, asked.inputs);
  assert!(fact.digest().awaits_approval, "each poll of the review requests lists the pull requests for it");

  // another tip: stale, and not asked until a window shows it
  let mut moved = state.clone();
  moved.apply(SyncEvent::CommitSynced {
    branch_name: "auth".to_owned(),
    commit_hash: "a2".to_owned(),
    new_hash: "copy-a2-new".to_owned(),
    status: CommitSyncStatus::Created,
    resolved: None,
  });
  fact.update(&moved, &reviewed, Some(&available()), &Demand::default());
  assert_eq!((state_of(&fact, "auth"), fact.digest().subjects.len()), (FactState::Stale { reason: None }, 0));
  assert!(!fact.digest().awaits_approval);
}

/// Check Approvals Again makes the readiness `Stale` at once and asks it at once with `recheck`; an answer of an ask that
/// started before it never replaces the recheck's: an older one is dropped, one that comes first is `Stale`
#[test]
fn the_answer_of_an_ask_started_before_a_recheck_never_replaces_the_rechecks() {
  let mut fact = ReadinessFact::default();
  let state = table();
  let update = |fact: &mut ReadinessFact| fact.update(&state, &pull_requests(vec![]), Some(&available()), &shows(&["auth"]));
  update(&mut fact);
  let batch = subject(&fact, "auth").clone();
  fact.recheck("auth");
  update(&mut fact);
  let recheck = subject(&fact, "auth").clone();
  assert_eq!((recheck.priority, recheck.recheck, recheck.rechecks), (ReadinessPriority::Demanded, true, 1));
  // the batch began first (seq 2), the recheck after (seq 3): the batch answering first is of other inputs
  fact.answer(at(2), answer(&batch, missing()));
  assert_eq!(state_of(&fact, "auth"), FactState::Stale { reason: None });
  fact.answer(at(3), answer(&recheck, ReviewReadiness::NotRequired));
  update(&mut fact);
  assert_eq!((state_of(&fact, "auth"), subject(&fact, "auth").recheck), (FactState::Fresh, false));
  assert_eq!(subject(&fact, "auth").priority, ReadinessPriority::Shown);

  // a second recheck whose batch answers after it: dropped
  fact.recheck("auth");
  update(&mut fact);
  let again = subject(&fact, "auth").clone();
  fact.answer(at(5), answer(&again, ReviewReadiness::NotRequired));
  assert!(!fact.answer(at(4), answer(&recheck, missing())).0);
  assert_eq!(fact.fact("auth").unwrap().value.as_ref().unwrap().readiness, ReviewReadiness::NotRequired);
}

/// An answer that made a subject ready that waited for approvals, for the same tip and pull request with a new review,
/// is news once; a recheck of the same reviews is no news
#[test]
fn the_approvals_that_arrived_are_news_once() {
  let mut store = FactStore::new();
  let state = table();
  let ilya = Person {
    login: Some("ikorennoy".to_owned()),
    full_name: Some("Ilya Korennoy".to_owned()),
  };
  let ready = ReviewReadiness::Approved {
    groups: vec![ApprovedReviewGroup {
      name: "IJPL Code Platform".to_owned(),
      approved_by: vec![ilya.clone()],
    }],
  };
  store.readiness.update(&state, &pull_requests(vec![]), Some(&available()), &shows(&["auth"]));
  let waiting = subject(&store.readiness, "auth").clone();
  store.apply(at(1), FactResult::Readiness(vec![answer(&waiting, missing())]));
  let approved = pull_requests(vec![Reviewer {
    login: "ikorennoy".to_owned(),
    state: ReviewerState::Approved,
    requested: false,
  }]);
  store.readiness.update(&state, &approved, Some(&available()), &shows(&["auth"]));
  let reviewed = subject(&store.readiness, "auth").clone();
  store.apply(at(2), FactResult::Readiness(vec![answer(&reviewed, ready.clone())]));
  let news: Vec<NewsEvent> = store.news().iter().map(|news| news.event.clone()).collect();
  assert_eq!(
    news,
    [NewsEvent::ApprovalsArrived {
      subject: "auth".to_owned(),
      pull_request: 7,
      approvers: vec![ilya],
      groups: vec!["IJPL Code Platform".to_owned()],
    }]
  );
  // a recheck of the same reviews answers the same: no news
  store.readiness.recheck("auth");
  store.readiness.update(&state, &approved, Some(&available()), &shows(&["auth"]));
  let recheck = subject(&store.readiness, "auth").clone();
  store.apply(at(3), FactResult::Readiness(vec![answer(&recheck, ready)]));
  assert_eq!(store.news().len(), 1);
}
