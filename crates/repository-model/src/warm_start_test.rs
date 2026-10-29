use crate::facts::{FactResult, FactStore, RobotsRead};
use crate::review_request_facts::review_request_facts_test::at;
use crate::session::SessionKey;
use crate::table::TableState;
use crate::table::table_test::{grouped, grouping, remote_status, run_sync, sync_events, unapplied};
use crate::warm_start::{CachedSession, FRAME_SCHEMA, frame_of, read_frame, write_frame};
use domain_types::commit::Commit;
use domain_types::fixtures::{pull_request, review_request, robot};
use domain_types::patronus::{PatronusAvailability, PatronusRobotStatus, PatronusRobotsResult};
use domain_types::pull_request::{PullRequestAvailability, PullRequestsResult};
use domain_types::review_request::ReviewRequestsResult;
use domain_types::{BranchSyncStatus, CommitSyncStatus, SyncEvent};
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;

pub(crate) fn key() -> SessionKey {
  SessionKey {
    worktree: "/repo".to_owned(),
    prefix: "user".to_owned(),
  }
}

pub(crate) fn github() -> PullRequestAvailability {
  PullRequestAvailability::Available {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
  }
}

/// The table of a sync of `auth` and `ui`, with an unapplied branch
pub(crate) fn synced_state() -> TableState {
  let mut state = TableState::with_prefix("user");
  let mut events = sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created);
  events.push(SyncEvent::UnappliedBranchesFound {
    branches: vec![unapplied("old", Some(remote_status("old", &[])))],
  });
  run_sync(&mut state, events);
  state
}

/// The answer of a list of the robots of Patronus: `robots` of the branches named so
pub(crate) fn robots_listed(robots: &[(&str, &str)]) -> FactResult {
  let result = PatronusRobotsResult {
    availability: PatronusAvailability::Available,
    robots: robots.iter().map(|(id, branch)| robot(id, Some(branch), PatronusRobotStatus::Running, 10)).collect(),
    github_repository: Some("acme/monorepo".to_owned()),
  };
  FactResult::Robots {
    read: RobotsRead::List { full: true },
    result,
  }
}

/// The facts of a session that Patronus and GitHub answered: a robot of `auth`, the pull request of `ui`, a review
/// request
pub(crate) fn answered_facts() -> FactStore {
  let mut facts = FactStore::new();
  facts.apply(at(1), robots_listed(&[("r1", "auth")]));
  let listing = PullRequestsResult {
    pull_requests: [("ui".to_owned(), pull_request("ui", 7))].into(),
    ..PullRequestsResult::unavailable(github())
  };
  facts.apply(at(2), FactResult::PullRequests(listing));
  let requests = ReviewRequestsResult {
    requests: vec![review_request(9, 1_700_000_000)],
    ..ReviewRequestsResult::unavailable(github())
  };
  facts.apply(at(3), FactResult::ReviewRequests(requests));
  facts
}

/// A frame reads back from the object cache as it was written: the parts of the table and the facts through JSON (the
/// view's parts are tagged by a field), one frame per worktree, which the next save replaces. The table it seeds has
/// each commit with the author's email and the subject its view leaves out, which find the commit again on a moved
/// HEAD (`CommitKey`)
#[test]
fn a_frame_reads_back_from_the_object_cache_as_it_was_written() {
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());
  let state = synced_state();
  let frame = frame_of(&key(), &state, &answered_facts());
  write_frame(&cache, &frame);
  let read = read_frame(&cache, "/repo").unwrap();
  // the notes a sync reads with the commits are no part of a view, nor of a frame
  assert_eq!(serde_json::to_value(&read).unwrap(), serde_json::to_value(&frame).unwrap());
  assert!(read.seeds(&key()));
  assert_eq!(read_frame(&cache, "/other"), None);
  let identity = |commit: &Commit| (commit.id.clone(), commit.author_email.clone(), commit.author_timestamp, commit.subject.clone());
  let seeded = TableState::from_cached("user", read.table);
  assert_eq!(seeded.commits().map(identity).collect::<Vec<_>>(), state.commits().map(identity).collect::<Vec<_>>());
  assert!(seeded.commits().all(|commit| !commit.author_email.is_empty() && !commit.subject.is_empty()));

  let mut later = frame;
  later.table.order.reverse();
  write_frame(&cache, &later);
  assert_eq!(read_frame(&cache, "/repo").unwrap().table.order, ["ui", "auth"]);
}

/// A frame seeds only the session of its key and schema: another prefix of the worktree, or a frame of another schema,
/// seeds nothing
#[test]
fn a_frame_seeds_only_the_session_of_its_key_and_schema() {
  let frame = frame_of(&key(), &synced_state(), &FactStore::new());
  let other_prefix = SessionKey {
    prefix: "other".to_owned(),
    ..key()
  };
  assert!(frame.seeds(&key()));
  assert!(!frame.seeds(&other_prefix));
  let older = CachedSession {
    schema: FRAME_SCHEMA - 1,
    ..frame
  };
  assert!(!older.seeds(&key()));
}

/// While a sync runs the frame keeps the table the last sync committed, not the rows the running one builds
#[test]
fn a_frame_while_a_sync_runs_keeps_the_table_it_started_from() {
  let mut state = synced_state();
  let committed = frame_of(&key(), &state, &FactStore::new()).table;
  state.begin_sync();
  state.apply(grouping(vec![grouped("auth", &["a3", "a2", "a1"])]));
  assert_eq!(frame_of(&key(), &state, &FactStore::new()).table, committed);
}
