use crate::branch_table::{BranchTable, RemoteFact};
use crate::facts::{FactResult, FactStore};
use crate::patronus_view::patronus_view_test::chain;
use crate::review_request_facts::review_request_facts_test::at;
use crate::table::TableState;
use domain_types::fixtures::pull_request;
use domain_types::landed::LandedBranch;
use domain_types::landing::{BaselinePush, LandedPullRequest, LandedWith};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult};
use domain_types::session::{BaselineStatus, NewsEvent};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::sync::Arc;

fn at_head(branch: &str, number: u32, head: &str, state: PullRequestState) -> PullRequest {
  PullRequest {
    head_ref_oid: head.to_owned(),
    state,
    ..pull_request(branch, number)
  }
}

/// The facts of a session over `table` (`chain`: auth, login on it, oauth on login, each at `v<initial>1`) with the
/// listing `pull_requests`
fn facts_of(table: BranchTable, pull_requests: Vec<PullRequest>) -> FactStore {
  let mut facts = FactStore::new();
  let _ = facts.table.answer(at(1), Ok(Arc::new(table)));
  facts.apply(
    at(2),
    FactResult::PullRequests(PullRequestsResult {
      viewer_permission: None,
      merge_methods: None,
      availability: PullRequestAvailability::Available {
        owner: "acme".to_owned(),
        name: "monorepo".to_owned(),
      },
      pull_requests: pull_requests
        .into_iter()
        .map(|pull_request| (pull_request.head_ref_name.trim_start_matches("user/virtual/").to_owned(), pull_request))
        .collect(),
      stack_pull_requests: BTreeMap::new(),
      github_stacks: false,
    }),
  );
  facts
}

/// A remote branch at `tip` that has no commit the branch lacks (an older version of it)
fn lagging(tip: &str) -> RemoteFact {
  RemoteFact {
    remote_tip: Some(tip.to_owned()),
    up_to_date: false,
    known: true,
    drops: 0,
    others: 0,
  }
}

fn told(facts: &FactStore) -> Vec<NewsEvent> {
  facts.news().iter().map(|news| news.event.clone()).collect()
}

fn closes(branch: &str, number: u32) -> LandedPullRequest {
  LandedPullRequest {
    branch: branch.to_owned(),
    number,
  }
}

/// The landing report of a fetched `origin/master` that has `whole` (none of their commits stays local)
fn fetched(state: &mut TableState, whole: &[&str]) {
  let section = &mut state.parts.sections.landed;
  section.baseline = Some(BaselineStatus {
    baseline_branch: "origin/master".to_owned(),
    update: None,
    check_error: None,
  });
  section.landing = whole
    .iter()
    .map(|name| LandedBranch {
      name: Some((*name).to_owned()),
      commits: Vec::new(),
      remaining: 0,
      landed_at: 1,
    })
    .collect();
}

/// A push of oauth into the target is told as it ends, over the table before its update: the pull requests of what it
/// landed that show an older version of a branch close, its own too (its remote lags the copy); auth's, whose head is
/// its tip in the history of the copy, is GitHub's to merge. The fetch after it shows the same branches in the baseline
/// and tells nothing again; the session keeps what landed them for the plan of the close
#[test]
fn a_push_tells_the_pull_requests_it_landed_that_show_an_older_version_and_the_fetch_after_it_tells_nothing_again() {
  let mut table = chain();
  table.remote.branches.insert("login".to_owned(), lagging("vl0"));
  table.remote.branches.insert("oauth".to_owned(), lagging("vo0"));
  let mut facts = facts_of(
    table,
    vec![
      at_head("auth", 5, "va1", PullRequestState::Open),
      at_head("login", 6, "vl0", PullRequestState::Open),
      at_head("oauth", 7, "vo0", PullRequestState::Open),
    ],
  );
  let mut state = TableState::new();
  let pushed = |tip: &str| BaselinePush {
    tip: tip.to_owned(),
    upstream: "origin/master".to_owned(),
  };
  facts.tell_push(
    &state,
    &[("auth".to_owned(), pushed("va1")), ("login".to_owned(), pushed("vl1")), ("oauth".to_owned(), pushed("vo1"))],
  );
  assert_eq!(
    told(&facts),
    vec![NewsEvent::Landed {
      landing: "landing-1".to_owned(),
      closes: vec![closes("login", 6), closes("oauth", 7)],
    }]
  );
  assert_eq!(
    facts.landed_closes.get("landing-1").map(|landing| &landing.with),
    Some(&LandedWith::Push {
      subject: "oauth".to_owned(),
      tip: "vo1".to_owned(),
      upstream: "origin/master".to_owned(),
    })
  );
  fetched(&mut state, &["auth", "login", "oauth"]);
  facts.tell_landings(&state);
  assert_eq!(told(&facts).len(), 1, "the push judged what it landed");
}

/// The fetched baseline that has branches of the table tells the open pull requests of them that hold nothing more, once
/// per tip; a pull request GitHub merged with the commits of its branch tells those of the branches it is built on, none
/// told before, a robot's merge neither
#[test]
fn the_fetched_baseline_and_a_merged_pull_request_tell_the_open_pull_requests_of_what_they_have_once() {
  let mut facts = facts_of(
    chain(),
    vec![
      at_head("auth", 5, "va1", PullRequestState::Open),
      at_head("login", 6, "vl1", PullRequestState::Open),
      at_head("oauth", 7, "vo1", PullRequestState::Merged),
    ],
  );
  let mut state = TableState::new();
  fetched(&mut state, &["auth"]);
  facts.tell_landings(&state);
  facts.tell_landings(&state);
  assert_eq!(
    told(&facts),
    vec![
      NewsEvent::Landed {
        landing: "landing-1".to_owned(),
        closes: vec![closes("auth", 5)],
      },
      NewsEvent::Landed {
        landing: "landing-2".to_owned(),
        closes: vec![closes("login", 6)],
      },
    ]
  );
  assert_eq!(
    facts.landed_closes.get("landing-1").map(|landing| &landing.with),
    Some(&LandedWith::Baseline {
      upstream: "origin/master".to_owned()
    })
  );
  assert_eq!(
    facts.landed_closes.get("landing-2").map(|landing| &landing.with),
    Some(&LandedWith::PullRequest {
      number: 7,
      subject: "oauth".to_owned(),
      target: "master".to_owned(),
    })
  );

  // what a robot's merge told is told by no landing
  let mut facts = facts_of(chain(), vec![at_head("auth", 5, "va1", PullRequestState::Open)]);
  facts.landed_closes.told_by_robot(&[closes("auth", 5)]);
  facts.tell_landings(&state);
  assert_eq!(told(&facts), Vec::new());
}
