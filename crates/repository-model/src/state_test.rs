use crate::facts::FactStore;
use crate::patch::{PatchRejected, apply_patch};
use crate::state::{View, new_epoch};
use crate::table::TableState;
use crate::table::table_test::{grouped, grouping, remote_status, run_sync, sync_events};
use domain_types::session::{BranchChange, BranchRemote, RepositoryView, SyncState, SyncStep, TableChanges, Version, ViewPatch};
use domain_types::{BranchSyncStatus, CommitSyncStatus, ProgressReporter, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use sync_core::sync::{SyncOptions, SyncReport, sync_branches};
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

/// The view a window holds after `patches`, from an empty one of `epoch`
fn fold(epoch: u32, patches: Vec<ViewPatch>) -> RepositoryView {
  let mut view = View::new(epoch).snapshot();
  for patch in patches {
    apply_patch(&mut view, patch).unwrap();
  }
  view
}

/// A patch carries only the parts that differ from the ones sent last; a window that applies the patches holds the
/// snapshot, and a state that did not change sends nothing
#[test]
fn a_patch_carries_only_the_parts_that_changed() {
  let mut state = TableState::new();
  let mut view = View::new(7);
  run_sync(&mut state, sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created));
  let first = view.diff(&state, &FactStore::new()).unwrap();
  assert_eq!((first.base, first.next), (Version { epoch: 7, seq: 0 }, Version { epoch: 7, seq: 1 }));
  assert_eq!(first.changes.branches.keys().collect::<Vec<_>>(), ["auth", "ui"]);
  assert_eq!(view.diff(&state, &FactStore::new()), None);

  state.apply(SyncEvent::RemoteStatusUpdate(remote_status("ui", &["copy-u1"])));
  let second = view.diff(&state, &FactStore::new()).unwrap();
  let remote = BranchChange {
    remote: Some(BranchRemote {
      status: Some(remote_status("ui", &["copy-u1"])),
    }),
    ..BranchChange::default()
  };
  let expected = TableChanges {
    branches: BTreeMap::from([("ui".to_string(), remote)]),
    ..TableChanges::default()
  };
  assert_eq!(second.changes, expected, "the untouched parts keep their identity");
  assert_eq!(fold(7, vec![first, second]), view.snapshot());
}

/// A window applies a patch only to the view at its base, in the same epoch, and all of it or nothing; any other view
/// asks for a snapshot
#[test]
fn a_patch_applies_only_to_the_view_at_its_base_in_its_epoch() {
  let mut state = TableState::new();
  let mut view = View::new(3);
  run_sync(&mut state, sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created));
  let first = view.diff(&state, &FactStore::new()).unwrap();
  state.apply(SyncEvent::RemoteStatusUpdate(remote_status("ui", &[])));
  let second = view.diff(&state, &FactStore::new()).unwrap();

  let mut window = View::new(3).snapshot();
  assert_eq!(
    apply_patch(&mut window, second.clone()),
    Err(PatchRejected::MissingBase { view: 0, base: 1 }),
    "a lost patch"
  );
  let mut foreign = View::new(4).snapshot();
  assert_eq!(apply_patch(&mut foreign, first.clone()), Err(PatchRejected::ForeignEpoch { view: 4, patch: 3 }));
  apply_patch(&mut window, first.clone()).unwrap();
  assert_eq!(apply_patch(&mut window, first), Err(PatchRejected::MissingBase { view: 1, base: 0 }), "a patch twice");

  let mut incomplete = second.clone();
  incomplete.changes.branches.insert("new".to_string(), BranchChange::default());
  let before = window.clone();
  assert_eq!(apply_patch(&mut window, incomplete), Err(PatchRejected::IncompleteBranch("new".to_string())));
  assert_eq!(window, before, "a rejected patch changes nothing");
  apply_patch(&mut window, second).unwrap();
  assert_eq!(window, view.snapshot());
  assert_ne!(new_epoch(), new_epoch(), "each session has its own epoch");
}

/// A table synced once with every commit and branch unchanged, and the view of a window that has it
fn synced_unchanged() -> (TableState, View) {
  let mut state = TableState::new();
  let mut view = View::new(1);
  run_sync(&mut state, sync_events(CommitSyncStatus::Unchanged, BranchSyncStatus::Unchanged));
  view.diff(&state, &FactStore::new()).unwrap();
  (state, view)
}

/// A sync that groups and builds the table as it was sends only the toggles of the sync state, also with a patch after
/// every event; a branch the sync drops leaves the view
#[test]
fn an_unchanged_resync_sends_only_the_sync_state() {
  let (mut state, mut view) = synced_unchanged();

  let mut patches = Vec::new();
  state.begin_sync();
  patches.extend(view.diff(&state, &FactStore::new()));
  for event in sync_events(CommitSyncStatus::Unchanged, BranchSyncStatus::Unchanged) {
    state.apply(event);
    patches.extend(view.diff(&state, &FactStore::new()));
  }
  state.finish_sync(Ok(()));
  patches.extend(view.diff(&state, &FactStore::new()));
  let syncs: Vec<_> = patches.iter().map(|patch| patch.changes.sync.clone()).collect();
  let only_sync = patches.iter().all(|patch| {
    TableChanges {
      sync: None,
      ..patch.changes.clone()
    }
    .is_empty()
  });
  assert!(only_sync, "{patches:#?}");
  let running = |step| Some(SyncState::Running { step });
  assert_eq!(syncs, [running(SyncStep::Grouping), running(SyncStep::Building), Some(SyncState::Idle)]);

  state.begin_sync();
  state.apply(grouping(vec![grouped("ui", &["u1"])]));
  let dropped = view.diff(&state, &FactStore::new()).unwrap();
  assert_eq!(
    (dropped.changes.removed_branches, dropped.changes.order),
    (vec!["auth".to_string()], Some(vec!["ui".to_string()]))
  );
}

/// The sync after the one that wrote the mapping notes groups the same commits with their notes: a window shows them as
/// before, so no patch carries them again (141 branches of commits were 53 KB of each second sync); another subject does
#[test]
fn a_grouping_that_reads_the_notes_of_the_copies_resends_no_commit() {
  let (mut state, mut view) = synced_unchanged();

  let with_notes = |mut branch: domain_types::GroupedBranchInfo| {
    for commit in &mut branch.commits {
      commit.note = Some(format!("v-commit-v1:copy-{}", commit.id));
      commit.mapped_commit_id = Some(format!("copy-{}", commit.id));
    }
    branch
  };
  state.begin_sync();
  state.apply(grouping(vec![with_notes(grouped("auth", &["a2", "a1"])), with_notes(grouped("ui", &["u1"]))]));
  let patch = view.diff(&state, &FactStore::new()).unwrap();
  assert!(patch.changes.branches.values().all(|change| change.commits.is_none()), "{patch:#?}");

  let mut reworded = grouped("ui", &["u1"]);
  reworded.commits[0].message = "Another subject".to_string();
  state.apply(grouping(vec![with_notes(grouped("auth", &["a2", "a1"])), reworded]));
  let patch = view.diff(&state, &FactStore::new()).unwrap();
  assert_eq!(patch.changes.branches.keys().collect::<Vec<_>>(), ["ui"], "{patch:#?}");
  assert!(patch.changes.branches["ui"].commits.is_some());
}

/// The bytes of the JSON a window gets of each item
fn json_bytes<T: serde::Serialize>(items: &[T]) -> usize {
  items.iter().map(|item| serde_json::to_string(item).unwrap().len()).sum()
}

/// The events of one sync of `repo` and its report
async fn sync_of(git: &GitCommandExecutor, path: &str, reporter: impl ProgressReporter + Clone + 'static) -> SyncReport {
  sync_branches(git, path, "user-name", reporter, SyncOptions::default()).await.unwrap()
}

/// The events of a sync of `branch_table_large` in 16 ms: its 432 events take about 300 ms, more than one per ms
const EVENTS_PER_PATCH: usize = 16;

/// What a window gets of one sync of the 141 branches of `branch_table_large`, with a patch after every `batch` events
/// (the 16 ms merge of the session): the patches and the events they replace
async fn patches_of_sync(git: &GitCommandExecutor, path: &str, state: &mut TableState, view: &mut View, batch: usize) -> (Vec<ViewPatch>, Vec<SyncEvent>) {
  let reporter = TestReporter::new();
  sync_of(git, path, reporter.clone()).await;
  let events = reporter.get_events();
  let mut patches = Vec::new();
  state.begin_sync();
  patches.extend(view.diff(state, &FactStore::new()));
  for events in events.chunks(batch) {
    for event in events {
      state.apply(event.clone());
    }
    patches.extend(view.diff(state, &FactStore::new()));
  }
  state.finish_sync(Ok(()));
  patches.extend(view.diff(state, &FactStore::new()));
  (patches, events)
}

/// B15 and B2 of docs/design/rust-domain-authority.md on the 141 rows of `branch_table_large`: the patches of a first
/// sync, merged as in 16 ms, and its snapshot cost no more than its events; a sync that changes nothing costs at most
/// 1 KiB, the toggles of the sync state, also with a patch after every event (the second sync turns the statuses of the
/// first, `Created`, into `Unchanged`, so the third measures it). A window folding the patches holds the snapshot
#[tokio::test]
async fn the_patches_of_a_large_table_cost_less_than_its_events() {
  let directory = tempfile::TempDir::new().unwrap();
  test_utils::repo_template::templates::branch_table_large().build(directory.path()).unwrap();
  let path = directory.path().to_str().unwrap();
  let git = GitCommandExecutor::new();
  let mut state = TableState::new();
  let mut view = View::new(new_epoch());
  let epoch = view.version().epoch;

  let (first, events) = patches_of_sync(&git, path, &mut state, &mut view, EVENTS_PER_PATCH).await;
  assert_eq!(state.order().len(), 141);
  let event_bytes = json_bytes(&events);
  assert!(json_bytes(&first) <= event_bytes, "patches {} > events {event_bytes}", json_bytes(&first));
  assert!(json_bytes(&[view.snapshot()]) <= event_bytes);
  let (second, events) = patches_of_sync(&git, path, &mut state, &mut view, EVENTS_PER_PATCH).await;
  assert!(json_bytes(&second) <= json_bytes(&events));
  let (third, _) = patches_of_sync(&git, path, &mut state, &mut view, 1).await;
  assert!(json_bytes(&third) <= 1024, "{} bytes: {third:#?}", json_bytes(&third));

  let patches = first.into_iter().chain(second).chain(third).collect();
  assert_eq!(fold(epoch, patches), view.snapshot());
}

/// Moves `test/virtual/auth` as another process would, once the sync grouped the commits and before it builds them
#[derive(Clone)]
struct MovingReporter {
  events: TestReporter,
  git: GitCommandExecutor,
  path: String,
  moved_to: String,
}

impl ProgressReporter for MovingReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    if matches!(event, SyncEvent::BranchesGrouped { .. }) {
      self.git.execute_command(&["branch", "--force", "test/virtual/auth", &self.moved_to], &self.path)?;
    }
    self.events.send(event)
  }
}

/// A virtual branch another process moved while the sync built it asks for one more sync (`SyncReport::retry`), which
/// builds it; its row says the error until then
#[tokio::test]
async fn a_branch_moved_by_another_process_asks_for_a_retry() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("(auth) Add login", "login.txt", "login");
  let git = GitCommandExecutor::new();
  let reporter = MovingReporter {
    events: TestReporter::new(),
    git: git.clone(),
    path: repo.path_str().to_string(),
    moved_to: base,
  };
  let report = sync_branches(&git, repo.path_str(), "test", reporter.clone(), SyncOptions::default()).await.unwrap();
  assert!(report.retry);
  let mut state = TableState::new();
  run_sync(&mut state, reporter.events.get_events());
  let auth = state.branch("auth").unwrap();
  assert_eq!((auth.sync.status, auth.sync.has_error), (domain_types::session::BranchStatus::Error, true));

  let events = TestReporter::new();
  let retried = sync_branches(&git, repo.path_str(), "test", events.clone(), SyncOptions::default()).await.unwrap();
  assert!(!retried.retry);
  run_sync(&mut state, events.get_events());
  assert_eq!(state.branch("auth").unwrap().sync.status, domain_types::session::BranchStatus::Updated);
}
