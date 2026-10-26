use crate::branch_table::{BranchTable, RemoteFact};
use crate::moved_remote::{FetchedRemote, MovedFetches, MovedRemoteAsk};
use crate::table::table_test::grouped;
use crate::table::{RemoteSubject, TableState};
use domain_types::fixtures::{pull_request, remote_status};
use domain_types::pull_request::{PullRequest, PullRequestState};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;

/// A table of `auth` and `login`, both on the remote at `<name>-seen`
fn table() -> BranchTable {
  let mut table = BranchTable {
    prefix: "user".to_string(),
    baseline: "origin/master".to_string(),
    branches: vec![grouped("auth", &["a1"]), grouped("login", &["l1"])],
    ..BranchTable::default()
  };
  for name in ["auth", "login"] {
    table.remote.branches.insert(
      name.to_string(),
      RemoteFact {
        remote_tip: Some(format!("{name}-seen")),
        known: true,
        ..RemoteFact::default()
      },
    );
  }
  table
}

/// The open pull requests of the table, `auth`'s showing `head`, `login`'s at its remote tip
fn listed(head: &str) -> BTreeMap<String, PullRequest> {
  let mut auth = pull_request("auth", 1);
  auth.head_ref_oid = head.to_string();
  let mut login = pull_request("login", 2);
  login.head_ref_oid = "login-seen".to_string();
  BTreeMap::from([("auth".to_string(), auth), ("login".to_string(), login)])
}

fn ask(head: &str) -> MovedRemoteAsk {
  MovedRemoteAsk {
    branch: "auth".to_string(),
    head: head.to_string(),
  }
}

fn answer(head: &str, tip: &str, settled: bool) -> FetchedRemote {
  let mut status = remote_status("auth");
  status.remote_tip = Some(tip.to_string());
  FetchedRemote {
    branch: "auth".to_string(),
    head: head.to_string(),
    status: Some(status),
    settled,
    others: Vec::new(),
  }
}

const fn at(minutes: u64) -> Duration {
  Duration::from_secs(minutes * 60)
}

/// A listing that shows the pull request of a branch at a head its remote branch lacks asks its fetch once; another
/// listing of the same head asks nothing while it runs or once it brought the head, and a new head asks again
#[test]
fn a_moved_head_is_asked_once_and_a_new_head_again() {
  let (table, state) = (table(), TableState::new());
  let mut fetches = MovedFetches::default();
  assert_eq!(
    fetches.asks(at(0), &table, &state, &listed("theirs")),
    [ask("theirs")],
    "login's pull request is at its tip"
  );
  assert_eq!(fetches.asks(at(1), &table, &state, &listed("theirs")), [], "in flight");

  fetches.answered(["auth"], &[answer("theirs", "theirs", true)]);
  let mut fetched = table.clone();
  fetched.remote.branches.get_mut("auth").unwrap().remote_tip = Some("theirs".to_string());
  assert_eq!(fetches.asks(at(2), &fetched, &state, &listed("theirs")), [], "the tip is the head now");
  assert_eq!(fetches.asks(at(2), &fetched, &state, &listed("newer")), [ask("newer")]);
}

/// A fetch that didn't bring the head (a mirror that lags) is asked again after a minute, five and fifteen, then not
/// until another head
#[test]
fn a_head_the_fetch_did_not_bring_is_asked_again_three_times() {
  let (table, state) = (table(), TableState::new());
  let mut fetches = MovedFetches::default();
  let mut asked = Vec::new();
  for minute in 0..40 {
    if !fetches.asks(at(minute), &table, &state, &listed("theirs")).is_empty() {
      asked.push(minute);
      fetches.answered(["auth"], &[answer("theirs", "auth-seen", false)]);
    }
  }
  assert_eq!(asked, [0, 1, 6, 21]);
}

/// Neither a branch the app pushes nor one without an open pull request is fetched
#[test]
fn a_pushed_branch_and_a_closed_pull_request_are_not_fetched() {
  let table = table();
  let mut state = TableState::new();
  state.begin_push(&[RemoteSubject::Branch("auth".to_string())]);
  let mut fetches = MovedFetches::default();
  assert_eq!(fetches.asks(at(0), &table, &state, &listed("theirs")), []);

  let mut closed = listed("theirs");
  closed.get_mut("auth").unwrap().state = PullRequestState::Closed;
  assert_eq!(fetches.asks(at(0), &table, &TableState::new(), &closed), []);
}

/// A fetch the host didn't run (no table, a repository without a remote of its own) ends with no answer: the ask is out
/// of flight and asked again after the wait, never stuck
#[test]
fn an_ask_the_host_did_not_fetch_is_asked_again() {
  let (table, state) = (table(), TableState::new());
  let mut fetches = MovedFetches::default();
  assert_eq!(fetches.asks(at(0), &table, &state, &listed("theirs")), [ask("theirs")]);
  fetches.answered(["auth"], &[]);
  assert_eq!(fetches.asks(at(0), &table, &state, &listed("theirs")), [], "the wait of a retry");
  assert_eq!(fetches.asks(at(1), &table, &state, &listed("theirs")), [ask("theirs")]);
}
