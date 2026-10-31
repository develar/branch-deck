use crate::derive::wanted_readiness;
use crate::facts::{FactResult, FactStore, Observation, Seq};
use crate::pull_request_facts::PullRequestsFact;
use crate::readiness_facts::{ReadinessAnswer, ReadinessFact, ReadinessSubject};
use crate::review_request_facts::ReviewRequestsFact;
use crate::review_request_facts::review_request_facts_test::{at as review_at, available as review_available};
use crate::table::TableState;
use crate::table::table_test::{run_sync, sync_events};
use domain_policy::refresh::ReadinessPriority;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{missing_approvals, pull_request, review_request};
use domain_types::patronus::{ApprovedReviewGroup, BranchReviewReadiness, PatronusAvailability, Person, ReviewReadiness, ReviewReadinessBranch, ReviewStart};
use domain_types::pull_request::{PullRequest, Reviewer, ReviewerState};
use domain_types::review_request::ReviewRequest;
use domain_types::session::{Demand, FactState, NewsEvent, PullRequestsView, RobotsHead, stack_readiness_key};
use domain_types::stack::{StackBranchState, StackBranchUpdate};
use domain_types::{BranchSyncStatus, CommitSyncStatus, SyncEvent};
use pretty_assertions::assert_eq;
use std::collections::HashSet;
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
    my_email: None,
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
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &demand, "user", &shown(&demand));
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
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &Demand::default(), "user", &shown(&Demand::default()));
  assert_eq!(fact.digest().subjects, []);
  // without the GitHub repository Patronus names, the robot would start through the API
  let head = RobotsHead {
    github_repository: None,
    ..available()
  };
  fact.update(&state, &pull_requests(vec![]), Some(&head), &shows(&["auth"]), "user", &shown(&shows(&["auth"])));
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
  fact.update(&state, &pull_requests(vec![]), None, &demand, "user", &shown(&demand));
  assert_eq!((state_of(&fact, "auth"), fact.digest().subjects.len()), (FactState::Loading, 0));
  let logged_out = RobotsHead {
    availability: PatronusAvailability::NotLoggedIn,
    github_repository: None,
  };
  fact.update(&state, &pull_requests(vec![]), Some(&logged_out), &demand, "user", &shown(&demand));
  let reason = ApiError::new(ErrorCode::NotLoggedIn, "Run ./safePush.cmd once to log in to Patronus");
  assert_eq!((state_of(&fact, "auth"), fact.digest().subjects.len()), (FactState::Unavailable { reason }, 0));
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &demand, "user", &shown(&demand));
  assert_eq!((state_of(&fact, "auth"), fact.digest().subjects.len()), (FactState::Loading, 1));
}

/// A new review is new inputs: the answer is `Stale` in the update that saw it, and asked again while shown; a hidden
/// subject is asked again only when its last answer waits for approvals at the same tip and start (the hidden ones
/// last), never for another tip
#[test]
fn new_inputs_make_the_readiness_stale_and_a_hidden_subject_is_asked_only_for_new_reviews() {
  let mut fact = ReadinessFact::default();
  let state = table();
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &shows(&["auth"]), "user", &shown(&shows(&["auth"])));
  let asked = subject(&fact, "auth").clone();
  assert!(fact.answer(at(2), answer(&asked, missing())).0);
  fact.update(&state, &pull_requests(vec![]), Some(&available()), &shows(&["auth"]), "user", &shown(&shows(&["auth"])));
  assert_eq!(state_of(&fact, "auth"), FactState::Fresh);

  let reviewed = pull_requests(vec![Reviewer {
    login: "ikorennoy".to_owned(),
    state: ReviewerState::Approved,
    requested: false,
  }]);
  // hidden now: the new review asks it, after the shown ones
  fact.update(&state, &reviewed, Some(&available()), &Demand::default(), "user", &shown(&Demand::default()));
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
  fact.update(&moved, &reviewed, Some(&available()), &Demand::default(), "user", &HashSet::new());
  assert_eq!((state_of(&fact, "auth"), fact.digest().subjects.len()), (FactState::Stale { reason: None }, 0));
  assert!(!fact.digest().awaits_approval);
}

/// Check Approvals Again makes the readiness `Stale` at once and asks it at once with `recheck`; an answer of an ask that
/// started before it never replaces the recheck's: an older one is dropped, one that comes first is `Stale`
#[test]
fn the_answer_of_an_ask_started_before_a_recheck_never_replaces_the_rechecks() {
  let mut fact = ReadinessFact::default();
  let state = table();
  let update = |fact: &mut ReadinessFact| fact.update(&state, &pull_requests(vec![]), Some(&available()), &shows(&["auth"]), "user", &shown(&shows(&["auth"])));
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
  store
    .readiness
    .update(&state, &pull_requests(vec![]), Some(&available()), &shows(&["auth"]), "user", &shown(&shows(&["auth"])));
  let waiting = subject(&store.readiness, "auth").clone();
  store.apply(at(1), FactResult::Readiness(vec![answer(&waiting, missing())]));
  let approved = pull_requests(vec![Reviewer {
    login: "ikorennoy".to_owned(),
    state: ReviewerState::Approved,
    requested: false,
  }]);
  store
    .readiness
    .update(&state, &approved, Some(&available()), &shows(&["auth"]), "user", &shown(&shows(&["auth"])));
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
  store
    .readiness
    .update(&state, &approved, Some(&available()), &shows(&["auth"]), "user", &shown(&shows(&["auth"])));
  let recheck = subject(&store.readiness, "auth").clone();
  store.apply(at(3), FactResult::Readiness(vec![answer(&recheck, ready)]));
  assert_eq!(store.news().len(), 1);
}

/// The subjects `demand` shows, by key, as wanted: as if the step of each waited for the readiness
fn shown(demand: &Demand) -> HashSet<String> {
  demand
    .visible_branches
    .iter()
    .cloned()
    .chain(demand.visible_stacks.iter().map(|root| stack_readiness_key(root)))
    .collect()
}

/// Of the subjects a window shows, the session asks the readiness only of those whose next step waits for it
/// (`derive::wanted_readiness`): none before GitHub lists the pull requests (their steps wait for the listing), then
/// the shown branches past rows 1 to 6 and the whole stacks; a branch its robot runs on (row 1) is not asked
#[test]
fn a_shown_subject_is_asked_only_while_its_next_step_waits_for_the_readiness() {
  let (state, mut facts) = crate::derive::derive_budget_test::session();
  let demand = Demand {
    visible_branches: state.order().to_vec(),
    visible_stacks: vec!["stack0".to_owned()],
    ..Demand::default()
  };
  let wanted = wanted_readiness(&state, &facts, "user", &demand);
  assert!(wanted.contains("lone001") && wanted.contains("stack:stack0"), "past rows 1 to 6, pushed or not");
  let running = facts.robots.view().robots["r0"].branch_name.clone().unwrap();
  assert!(!wanted.contains(&running), "row 1: its robot runs");
  facts.pull_requests = PullRequestsFact::default();
  assert_eq!(wanted_readiness(&state, &facts, "user", &demand), HashSet::new(), "the steps wait for the listing first");
}

/// A pull request of someone else a review window shows into the default branch is asked once the head the listing
/// names is the one its review fetch brought: as its head branch, from the pull request with its reviewers and its
/// author, needed now; a new head waits for its fetch, a new review asks again, its approvals are no news, and its record
/// goes with the window. One into another branch has none
#[test]
fn a_pull_request_a_review_window_shows_is_asked_once_its_head_is_fetched_and_goes_with_the_window() {
  let state = table();
  let carol = |state| {
    vec![Reviewer {
      login: "carol".to_owned(),
      state,
      requested: false,
    }]
  };
  let request = |number: u32, head: &str, into_default: bool, reviewers: Vec<Reviewer>| ReviewRequest {
    head_ref_oid: head.to_owned(),
    base_is_default: into_default,
    reviewers,
    ..review_request(number, 1)
  };
  let mut requests = ReviewRequestsFact::default();
  requests.answer(
    review_at(1),
    review_available(vec![request(7, "head-1", true, carol(ReviewerState::Commented)), request(8, "head-8", false, Vec::new())]),
  );
  let windows = Demand {
    review_windows: vec![7, 8],
    ..Demand::default()
  };
  let update = |fact: &mut ReadinessFact, requests: &mut ReviewRequestsFact, demand: &Demand| {
    fact.set_requests(requests.readiness_asks(&demand.review_windows));
    fact.update(&state, &pull_requests(vec![]), Some(&available()), demand, "user", &HashSet::new());
    requests.demand(demand);
  };

  let mut fact = ReadinessFact::default();
  update(&mut fact, &mut requests, &windows);
  assert_eq!(fact.digest().subjects, [], "nothing is asked before the fetch");
  assert_eq!(state_of(&fact, "request:7"), FactState::Loading);
  assert!(fact.fact("request:8").is_none(), "a pull request into another branch has no record");

  requests.fetched(7, "head-1".to_owned());
  requests.fetched(8, "head-8".to_owned());
  update(&mut fact, &mut requests, &windows);
  let asked = subject(&fact, "request:7").clone();
  assert_eq!(
    asked.ask,
    ReviewReadinessBranch {
      branch_name: "request:7".to_owned(),
      source: None,
      pull_request_head: Some("alice/change-7".to_owned()),
      tip: "head-1".to_owned(),
      start: ReviewStart::PullRequest { number: 7 },
      reviewers: carol(ReviewerState::Commented),
      author: Some("alice".to_owned()),
    }
  );
  assert_eq!(asked.priority, ReadinessPriority::Demanded);
  assert_eq!(fact.digest().subjects.len(), 1);
  assert!(!fact.digest().awaits_approval);
  fact.answer(at(2), answer(&asked, missing()));
  assert_eq!(state_of(&fact, "request:7"), FactState::Fresh);

  // the listing names a new head: the readiness waits for the fetch of it
  requests.answer(review_at(3), review_available(vec![request(7, "head-2", true, carol(ReviewerState::Commented))]));
  update(&mut fact, &mut requests, &windows);
  assert_eq!(fact.digest().subjects, [], "the new head is not fetched yet");
  let waiting = fact.fact("request:7").unwrap();
  assert_eq!(
    (waiting.value.is_none(), waiting.state.clone()),
    (true, FactState::Loading),
    "the answer of the earlier head is no answer for the new one, so no groups while its fetch runs or after it failed"
  );
  requests.fetched(7, "head-2".to_owned());
  update(&mut fact, &mut requests, &windows);
  let moved = subject(&fact, "request:7").clone();
  assert_eq!((moved.ask.tip.as_str(), fact.fact("request:7").unwrap().value.is_none()), ("head-2", true));
  fact.answer(at(4), answer(&moved, missing()));
  let answered = fact.fact("request:7").unwrap();
  assert_eq!(
    (answered.state.clone(), answered.value.as_ref().map(|value| value.ask.tip.as_str())),
    (FactState::Fresh, Some("head-2"))
  );

  // a new review asks again; its approval is no news
  requests.answer(review_at(5), review_available(vec![request(7, "head-2", true, carol(ReviewerState::Approved))]));
  update(&mut fact, &mut requests, &windows);
  let reviewed = subject(&fact, "request:7").clone();
  assert_eq!(reviewed.ask.reviewers, carol(ReviewerState::Approved));
  let approved = ReviewReadiness::Approved {
    groups: vec![ApprovedReviewGroup {
      name: "IJPL Code Platform".to_owned(),
      approved_by: vec![Person {
        login: Some("carol".to_owned()),
        full_name: None,
      }],
    }],
  };
  assert_eq!(fact.answer(at(6), answer(&reviewed, approved)), (true, None));

  // the window closes: nothing is asked, the record goes, and the fetched head with it
  update(&mut fact, &mut requests, &Demand::default());
  assert_eq!((fact.digest().subjects.len(), fact.fact("request:7").is_some()), (0, false));
  // a fetch that ends after its window closed asks nothing
  requests.fetched(7, "head-2".to_owned());
  update(&mut fact, &mut requests, &Demand::default());
  assert_eq!(
    (fact.digest().subjects.len(), fact.fact("request:7").is_some()),
    (0, false),
    "the window of the late fetch is closed"
  );
  requests.fetched(8, "head-8".to_owned());
  let windows_8 = Demand {
    review_windows: vec![8],
    ..Demand::default()
  };
  update(&mut fact, &mut requests, &windows_8);
  assert_eq!(fact.digest().subjects, [], "a window into another branch asks nothing");
}

/// The answer of an earlier head of a pull request, come after the new head was fetched, is dropped: before or after
/// the ask of the new head fails, the record holds no value until the answer of its ask now. So does one of earlier
/// reviews of the same head
#[test]
fn a_late_answer_of_an_earlier_head_of_a_pull_request_never_becomes_its_readiness() {
  let state = table();
  let request = |head: &str, reviewers: Vec<Reviewer>| ReviewRequest {
    head_ref_oid: head.to_owned(),
    base_is_default: true,
    reviewers,
    ..review_request(7, 1)
  };
  let mut requests = ReviewRequestsFact::default();
  requests.answer(review_at(1), review_available(vec![request("head-1", Vec::new())]));
  let windows = Demand {
    review_windows: vec![7],
    ..Demand::default()
  };
  let update = |fact: &mut ReadinessFact, requests: &mut ReviewRequestsFact| {
    fact.set_requests(requests.readiness_asks(&windows.review_windows));
    fact.update(&state, &pull_requests(vec![]), Some(&available()), &windows, "user", &HashSet::new());
    requests.demand(&windows);
  };
  let value_tip = |fact: &ReadinessFact| fact.fact("request:7").unwrap().value.as_ref().map(|value| value.ask.tip.clone());

  let mut fact = ReadinessFact::default();
  requests.fetched(7, "head-1".to_owned());
  update(&mut fact, &mut requests);
  // the read of head-1 is in flight as the listing names head-2, whose fetch ends before that read
  let earlier = subject(&fact, "request:7").clone();
  requests.answer(review_at(2), review_available(vec![request("head-2", Vec::new())]));
  update(&mut fact, &mut requests);
  requests.fetched(7, "head-2".to_owned());
  update(&mut fact, &mut requests);
  let now = subject(&fact, "request:7").clone();
  assert_eq!(now.ask.tip, "head-2");

  assert_eq!(fact.answer(at(3), answer(&earlier, missing())), (false, None), "the answer of head-1 comes late");
  assert_eq!(value_tip(&fact), None);
  let failed = ReadinessAnswer {
    subject: now.clone(),
    result: Err(ApiError::new(ErrorCode::NetworkError, "Patronus is unreachable")),
  };
  assert_eq!(fact.answer(at(4), failed).0, true);
  assert_eq!(value_tip(&fact), None, "the failure of head-2 keeps no answer of head-1");
  assert_eq!(fact.answer(at(5), answer(&earlier, missing())), (false, None), "nor does one come after it");
  assert_eq!(value_tip(&fact), None);

  // the ask of head-2 again: its answer is the readiness
  update(&mut fact, &mut requests);
  let retried = subject(&fact, "request:7").clone();
  assert_eq!(retried.ask, now.ask);
  fact.answer(at(6), answer(&retried, missing()));
  assert_eq!((state_of(&fact, "request:7"), value_tip(&fact)), (FactState::Fresh, Some("head-2".to_owned())));

  // a new review of the same head: the late answer of the earlier reviews is dropped, the kept one is stale
  let carol = vec![Reviewer {
    login: "carol".to_owned(),
    state: ReviewerState::Approved,
    requested: false,
  }];
  requests.answer(review_at(7), review_available(vec![request("head-2", carol.clone())]));
  update(&mut fact, &mut requests);
  assert_eq!(subject(&fact, "request:7").ask.reviewers, carol);
  assert_eq!(fact.answer(at(8), answer(&retried, missing())), (false, None));
  assert!(matches!(state_of(&fact, "request:7"), FactState::Stale { .. }));
  assert_eq!(value_tip(&fact), Some("head-2".to_owned()));
}
