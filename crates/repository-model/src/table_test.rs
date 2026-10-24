use crate::table::TableState;
use domain_types::commit::Commit;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::session::{BranchStatus, CommitSync, SyncState, SyncStep};
use domain_types::stack::{BranchStack, StackProblem, StackProblemView};
use domain_types::{BranchError, BranchOrigin, BranchSyncStatus, CommitSyncStatus, GroupedBranchInfo, RemoteStatusUpdate, SyncEvent};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use test_utils::sync_fixtures::integrated_branch_info;

pub(crate) fn commit(id: &str) -> Commit {
  Commit {
    id: id.to_string(),
    stripped_subject: format!("Subject of {id}"),
    message: format!("Subject of {id}"),
    author_name: "Test User".to_string(),
    author_email: "test@example.com".to_string(),
    author_timestamp: 1_704_067_200,
    committer_timestamp: 1_704_067_200,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    subject: format!("Subject of {id}"),
    parent_id: None,
    tree_id: String::new(),
    note: None,
    mapped_commit_id: None,
    mapped_source: None,
    mapped_resolution: None,
  }
}

pub(crate) fn grouped(name: &str, commits: &[&str]) -> GroupedBranchInfo {
  GroupedBranchInfo {
    name: name.to_string(),
    commits: commits.iter().map(|id| commit(id)).collect(),
    latest_commit_time: 1_704_067_200,
    summary: format!("Summary of {name}"),
    description_title: None,
    all_commits_have_issue_references: false,
    my_email: None,
    origin: BranchOrigin::Prefix,
    issue_key: None,
    stack: None,
  }
}

pub(crate) fn grouping(branches: Vec<GroupedBranchInfo>) -> SyncEvent {
  SyncEvent::BranchesGrouped {
    branches,
    baseline_branch: "origin/master".to_string(),
    stacks: false,
    issue_groups: BTreeMap::new(),
    stack_parents: BTreeMap::new(),
    parked: BTreeMap::new(),
    stack_branches: Vec::new(),
  }
}

pub(crate) fn synced(branch: &str, commit: &str, status: CommitSyncStatus) -> SyncEvent {
  SyncEvent::CommitSynced {
    branch_name: branch.to_string(),
    commit_hash: commit.to_string(),
    new_hash: format!("copy-{commit}"),
    status,
    resolved: None,
  }
}

pub(crate) fn branch_status(branch: &str, status: BranchSyncStatus) -> SyncEvent {
  SyncEvent::BranchStatusUpdate {
    branch_name: branch.to_string(),
    status,
    error: None,
  }
}

pub(crate) fn remote_status(branch: &str, unpushed: &[&str]) -> RemoteStatusUpdate {
  RemoteStatusUpdate {
    branch_name: branch.to_string(),
    remote_exists: true,
    remote_tip: Some("remote-tip".to_string()),
    unpushed_commits: unpushed.iter().map(ToString::to_string).collect(),
    commits_behind: 0,
    dropped_commits: Vec::new(),
    commits_amended: 0,
    commits_restacked: 0,
    commits_landed: 0,
    landed_commits: Vec::new(),
    commits_reworded: 0,
    ancestor_commits_ahead: 0,
    my_unpushed_count: 0,
    fast_forward: true,
    last_push_time: 0,
  }
}

/// The events of one sync of `auth` (two commits) and `ui` (one), each commit with `status`, `auth` with a remote status
pub(crate) fn sync_events(status: CommitSyncStatus, branch_status_of: BranchSyncStatus) -> Vec<SyncEvent> {
  let mut events = vec![grouping(vec![grouped("auth", &["a2", "a1"]), grouped("ui", &["u1"])])];
  for (branch, commit) in [("auth", "a1"), ("auth", "a2"), ("ui", "u1")] {
    events.push(synced(branch, commit, status));
  }
  events.push(branch_status("auth", branch_status_of));
  events.push(branch_status("ui", branch_status_of));
  events.push(SyncEvent::RemoteStatusUpdate(remote_status("auth", &["copy-a2"])));
  events
}

pub(crate) fn run_sync(state: &mut TableState, events: Vec<SyncEvent>) {
  state.begin_sync();
  for event in events {
    state.apply(event);
  }
  state.finish_sync(Ok(()));
}

/// A grouping starts each branch `Syncing` in the order of the grouping; the events of its build count its progress,
/// write what the sync made of each commit and end with its status
#[test]
fn the_events_of_a_sync_build_the_rows_of_the_table() {
  let mut state = TableState::new();
  state.begin_sync();
  assert_eq!(state.sync(), &SyncState::Running { step: SyncStep::Grouping });
  state.apply(grouping(vec![grouped("auth", &["a2", "a1"]), grouped("ui", &["u1"])]));
  assert_eq!(state.sync(), &SyncState::Running { step: SyncStep::Building });
  assert_eq!(state.order(), ["auth", "ui"]);
  let auth = state.branch("auth").unwrap();
  assert_eq!((auth.sync.status, auth.sync.processed_count, auth.commits.len()), (BranchStatus::Syncing, 0, 2));

  state.apply(synced("auth", "a1", CommitSyncStatus::Created));
  // a commit or a branch the table lacks changes nothing
  state.apply(synced("auth", "unknown", CommitSyncStatus::Created));
  state.apply(synced("gone", "a1", CommitSyncStatus::Created));
  let auth = state.branch("auth").unwrap();
  assert_eq!(auth.sync.processed_count, 1);
  let copy = CommitSync {
    hash: Some("copy-a1".to_string()),
    status: Some(CommitSyncStatus::Created),
    ..CommitSync::default()
  };
  assert_eq!(auth.copies, [CommitSync::default(), copy]);

  state.apply(synced("auth", "a2", CommitSyncStatus::Created));
  state.apply(branch_status("auth", BranchSyncStatus::Created));
  state.apply(SyncEvent::RemoteStatusUpdate(remote_status("auth", &[])));
  state.apply(SyncEvent::UnappliedBranchesFound { branches: Vec::new() });
  assert_eq!(state.sync(), &SyncState::Running { step: SyncStep::Detection });
  state.finish_sync(Ok(()));
  let auth = state.branch("auth").unwrap();
  assert_eq!((auth.sync.status, auth.sync.processed_count, auth.sync.has_error), (BranchStatus::Created, 2, false));
  assert_eq!(auth.remote.status, Some(remote_status("auth", &[])));
  assert_eq!(state.sync(), &SyncState::Idle);
}

/// A branch grouped as before, with the same commits, keeps what its row shows while it is rebuilt; a changed branch
/// starts `Syncing` with the remote status it had; a branch the grouping lacks leaves the table
#[test]
fn a_quiet_resync_keeps_the_shown_state_and_a_changed_branch_starts_syncing() {
  let mut state = TableState::new();
  run_sync(&mut state, sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created));
  let shown = state.branch("auth").unwrap().clone();

  state.begin_sync();
  state.apply(grouping(vec![grouped("auth", &["a2", "a1"]), grouped("ui", &["u2", "u1"])]));
  assert_eq!(state.branch("auth"), Some(&shown), "the quiet resync shows what the last sync made");
  let ui = state.branch("ui").unwrap();
  assert_eq!(
    (ui.sync.status, ui.sync.processed_count, &ui.copies),
    (BranchStatus::Syncing, 0, &vec![CommitSync::default(); 2])
  );
  // the reused copies of the quiet resync don't count as progress: the row shows no progress
  state.apply(synced("auth", "a1", CommitSyncStatus::Created));
  assert_eq!(state.branch("auth"), Some(&shown));

  state.apply(grouping(vec![grouped("ui", &["u2", "u1"])]));
  assert_eq!((state.order(), state.branch("auth")), (&["ui".to_string()][..], None));
}

/// A failed commit marks itself and its branch; a branch with an error, a conflict or a block is never a quiet resync
#[test]
fn a_commit_error_marks_the_commit_and_the_branch() {
  let mut state = TableState::new();
  run_sync(&mut state, sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created));
  state.begin_sync();
  state.apply(grouping(vec![grouped("auth", &["a2", "a1"]), grouped("ui", &["u1"])]));
  let error = BranchError::Generic("cannot copy".to_string());
  state.apply(SyncEvent::CommitError {
    branch_name: "auth".to_string(),
    commit_hash: "a2".to_string(),
    error: error.clone(),
  });
  let auth = state.branch("auth").unwrap();
  assert_eq!(
    (auth.sync.status, auth.sync.has_error, &auth.sync.error_details),
    (BranchStatus::Error, true, &Some(error.clone()))
  );
  assert_eq!((auth.copies[0].status, &auth.copies[0].error), (Some(CommitSyncStatus::Error), &Some(error)));

  state.apply(grouping(vec![grouped("auth", &["a2", "a1"])]));
  assert_eq!(state.branch("auth").unwrap().sync.status, BranchStatus::Syncing, "an error is rebuilt in view");
}

/// A stack problem marks the commits whose prefix it asks to reword; a blocked branch keeps its reason and is no error
#[test]
fn a_stack_problem_marks_its_commits_and_a_block_keeps_its_reason() {
  let mut problem_branch = grouped("login", &["l2", "l1"]);
  problem_branch.stack = Some(BranchStack {
    problems: vec![StackProblemView {
      problem: StackProblem::Syntax(vec!["l1".to_string()]),
      label: "invalid prefix".to_string(),
      reason: "login has an invalid prefix".to_string(),
      message: "The prefix has an empty level".to_string(),
      commits: vec!["l1".to_string()],
      fixes: Vec::new(),
    }],
    ..BranchStack::default()
  });
  let mut state = TableState::new();
  state.begin_sync();
  state.apply(grouping(vec![problem_branch, grouped("oauth", &["o1"])]));
  state.apply(branch_status("login", BranchSyncStatus::Error));
  state.apply(SyncEvent::BranchStatusUpdate {
    branch_name: "oauth".to_string(),
    status: BranchSyncStatus::Blocked,
    error: Some(BranchError::Generic("Blocked: the parent branch failed".to_string())),
  });
  let login = state.branch("login").unwrap();
  assert_eq!((login.sync.status, login.sync.has_error), (BranchStatus::Error, true));
  assert_eq!(login.copies.iter().map(|commit| commit.status).collect::<Vec<_>>(), [None, Some(CommitSyncStatus::Error)]);
  let oauth = state.branch("oauth").unwrap();
  assert_eq!((oauth.sync.status, oauth.sync.has_error), (BranchStatus::Blocked, false));
  assert_eq!(oauth.sync.blocked_reason.as_deref(), Some("Blocked: the parent branch failed"));
}

/// The archived names of a sync reconcile the section: the integration of a name it lacks goes, an integration that
/// came first adds its name, and a branch this sync archived is marked beside the earlier marks still archived
#[test]
fn the_archived_names_reconcile_the_integration_and_the_marks() {
  let info = |name: &str| {
    let mut info = integrated_branch_info();
    info.name = name.to_string();
    info
  };
  let mut state = TableState::new();
  state.begin_sync();
  state.apply(SyncEvent::BranchIntegrationDetected { info: info("early") });
  state.apply(SyncEvent::ArchivedBranchesFound {
    branch_names: vec!["old".to_string(), "new".to_string(), "old".to_string()],
    newly_archived: vec!["new".to_string()],
  });
  state.apply(SyncEvent::BranchIntegrationDetected { info: info("old") });
  state.apply(SyncEvent::BranchIntegrationDetected { info: info("late") });
  let archived = &state.sections().archived;
  assert_eq!(
    (&archived.names[..], &archived.newly_archived[..]),
    (&["old", "new", "late"].map(String::from)[..], &["new".to_string()][..])
  );
  assert_eq!(archived.integration.keys().collect::<Vec<_>>(), ["late", "old"]);

  state.apply(SyncEvent::ArchivedBranchesFound {
    branch_names: vec!["new".to_string(), "newer".to_string()],
    newly_archived: vec!["newer".to_string()],
  });
  let archived = &state.sections().archived;
  assert_eq!(archived.newly_archived, ["new", "newer"]);
  assert!(archived.integration.is_empty(), "{:?}", archived.integration);
}

/// A failed sync shows the table of the sync before, with the error; the rows of its grouping are gone
#[test]
fn a_failed_sync_restores_the_table_of_the_sync_before() {
  let mut state = TableState::new();
  run_sync(&mut state, sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created));
  let before = state.clone();
  state.begin_sync();
  state.apply(grouping(vec![grouped("auth", &["a3", "a2", "a1"])]));
  let error = ApiError::new(ErrorCode::Internal, "the sync failed");
  state.finish_sync(Err(error.clone()));
  assert_eq!(state.sync(), &SyncState::Failed { error });
  assert_eq!((state.order(), state.branch("ui")), (before.order(), before.branch("ui")));
  assert_eq!(state.branch("auth"), before.branch("auth"));
  // the index of the restored table is the one of its commits
  state.begin_sync();
  state.apply(synced("auth", "a1", CommitSyncStatus::Unchanged));
  assert_eq!(state.branch("auth").unwrap().copies[1].status, Some(CommitSyncStatus::Unchanged));
}

/// The grouping keeps the stack branches it announces until their update and drops the others; the baseline status
/// and its landed branches go into their sections
#[test]
fn the_grouping_keeps_the_announced_stack_branches_and_the_baseline_status_fills_its_section() {
  let stack = |root: &str| domain_types::stack::StackBranchUpdate {
    root: root.to_string(),
    branch_name: format!("test/stack/{root}"),
    members: vec![root.to_string()],
    commit_count: 1,
    state: domain_types::stack::StackBranchState::Built,
    tip: None,
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  };
  let mut state = TableState::new();
  state.begin_sync();
  state.apply(SyncEvent::StackBranchUpdate(Box::new(stack("auth"))));
  state.apply(SyncEvent::StackBranchUpdate(Box::new(stack("core"))));
  let SyncEvent::BranchesGrouped {
    branches,
    baseline_branch,
    issue_groups,
    stack_parents,
    parked,
    ..
  } = grouping(Vec::new())
  else {
    unreachable!()
  };
  state.apply(SyncEvent::BranchesGrouped {
    branches,
    baseline_branch,
    stacks: true,
    issue_groups,
    stack_parents,
    parked,
    stack_branches: vec!["auth".to_string()],
  });
  assert_eq!((state.stack("auth"), state.stack("core")), (Some(&stack("auth")), None));
  assert!(state.header().stacks_enabled);

  state.apply(SyncEvent::BaselineStatus {
    baseline_branch: "origin/master".to_string(),
    update: None,
    check_error: Some("a rebase is in progress".to_string()),
    landing: Vec::new(),
    landed: Vec::new(),
  });
  let status = state.sections().landed.baseline.clone().unwrap();
  assert_eq!(
    (status.baseline_branch.as_str(), status.check_error.as_deref()),
    ("origin/master", Some("a rebase is in progress"))
  );
}
