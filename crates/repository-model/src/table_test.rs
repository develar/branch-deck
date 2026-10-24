use crate::table::{RemoteSubject, TableState};
use domain_types::commit::Commit;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::session::{BranchStatus, CommitSync, SyncState, SyncStep};
use domain_types::stack::{BranchStack, StackBranchState, StackBranchUpdate, StackProblem, StackProblemView};
use domain_types::{BranchError, BranchOrigin, BranchSyncStatus, CommitSyncStatus, GroupedBranchInfo, RemoteStatusUpdate, SyncEvent, UnappliedBranchInfo};
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

/// A grouping of no branch with stacked branches on, which announces the stack branches of `roots`
fn stack_grouping(roots: &[&str]) -> SyncEvent {
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
  SyncEvent::BranchesGrouped {
    branches,
    baseline_branch,
    stacks: true,
    issue_groups,
    stack_parents,
    parked,
    stack_branches: roots.iter().map(ToString::to_string).collect(),
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

/// The commits a failed branch left uncopied stay in the table, unmarked in its rows, until the next grouping builds it
/// anew
#[test]
fn the_blocked_commits_of_a_branch_stay_unmarked_until_the_next_grouping() {
  let mut state = TableState::new();
  state.begin_sync();
  state.apply(grouping(vec![grouped("auth", &["a3", "a2", "a1"])]));
  state.apply(SyncEvent::CommitsBlocked {
    branch_name: "auth".to_string(),
    blocked_commit_hashes: vec!["a2".to_string(), "a3".to_string()],
  });
  state.apply(SyncEvent::CommitsBlocked {
    branch_name: "gone".to_string(),
    blocked_commit_hashes: vec!["g1".to_string()],
  });
  state.finish_sync(Ok(()));
  assert_eq!(
    (state.blocked_commits("auth"), state.blocked_commits("gone")),
    (["a2".to_string(), "a3".to_string()].as_slice(), [].as_slice())
  );
  assert!(state.branch("auth").unwrap().copies.iter().all(|copy| *copy == CommitSync::default()), "the rows mark none");

  state.begin_sync();
  state.apply(grouping(vec![grouped("auth", &["a3", "a2", "a1"])]));
  assert_eq!(state.blocked_commits("auth"), [] as [String; 0]);
}

/// The stack branches keep the order the grouping announced them in, of the stacks that fork first, not the order of
/// their roots
#[test]
fn the_stack_branches_keep_the_order_the_grouping_announced() {
  let mut state = TableState::new();
  state.begin_sync();
  state.apply(stack_grouping(&["ui", "auth"]));
  for root in ["ui", "auth"] {
    state.apply(SyncEvent::StackBranchUpdate(Box::new(stack_branch(root, None))));
  }
  state.finish_sync(Ok(()));
  assert_eq!(state.stacks().map(|stack| stack.root.as_str()).collect::<Vec<_>>(), ["ui", "auth"]);
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
  let stack = |root: &str| StackBranchUpdate {
    root: root.to_string(),
    branch_name: format!("test/stack/{root}"),
    members: vec![root.to_string()],
    commit_count: 1,
    state: StackBranchState::Built,
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
  state.apply(stack_grouping(&["auth"]));
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

/// The remote status of `branch` at the remote tip `tip`, nothing to push
pub(crate) fn remote_at(branch: &str, tip: &str) -> RemoteStatusUpdate {
  RemoteStatusUpdate {
    remote_tip: Some(tip.to_string()),
    ..remote_status(branch, &[])
  }
}

fn shown_tip(state: &TableState, branch: &str) -> Option<String> {
  state.branch(branch)?.remote.status.as_ref()?.remote_tip.clone()
}

fn stack_tip(state: &TableState, root: &str) -> Option<String> {
  state.stack(root)?.remote.as_ref()?.remote_tip.clone()
}

fn unapplied_tip(state: &TableState, branch: &str) -> Option<String> {
  let unapplied = state.sections().unapplied.iter().find(|unapplied| unapplied.branch == branch)?;
  unapplied.remote.as_ref()?.remote_tip.clone()
}

fn stack_branch(root: &str, remote: Option<RemoteStatusUpdate>) -> StackBranchUpdate {
  StackBranchUpdate {
    root: root.to_string(),
    branch_name: format!("test/stack/{root}"),
    members: vec![root.to_string()],
    commit_count: 1,
    state: StackBranchState::Built,
    tip: Some("stack-built".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote,
  }
}

pub(crate) fn unapplied(branch: &str, remote: Option<RemoteStatusUpdate>) -> UnappliedBranchInfo {
  UnappliedBranchInfo {
    name: format!("test/unapplied/{branch}"),
    branch: branch.to_string(),
    tip: format!("{branch}-tip"),
    subject: format!("Subject of {branch}"),
    checked_out: false,
    remote,
  }
}

/// A table synced once: `auth` and `ui`, the stack branch of `auth`, the unapplied `docs`, each at the remote tip
/// `synced`
fn synced_with_remotes() -> TableState {
  let mut state = TableState::new();
  let mut events = sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created);
  events.push(SyncEvent::RemoteStatusUpdate(remote_at("auth", "synced")));
  events.push(SyncEvent::RemoteStatusUpdate(remote_at("ui", "synced")));
  events.push(SyncEvent::StackBranchUpdate(Box::new(stack_branch("auth", Some(remote_at("auth", "synced"))))));
  events.push(SyncEvent::UnappliedBranchesFound {
    branches: vec![unapplied("docs", Some(remote_at("docs", "synced")))],
  });
  run_sync(&mut state, events);
  state
}

/// `auth`, the stack branch of `auth` and the unapplied `docs`, as one push names them
fn pushed_subjects() -> Vec<RemoteSubject> {
  vec![
    RemoteSubject::Branch("auth".to_string()),
    RemoteSubject::WholeStack("auth".to_string()),
    RemoteSubject::Unapplied("docs".to_string()),
  ]
}

/// The remote statuses a push of `pushed_subjects` left, each at `tip`
fn pushed_statuses(tip: &str) -> Vec<RemoteStatusUpdate> {
  vec![remote_at("auth", tip), remote_at("auth", tip), remote_at("docs", tip)]
}

/// The events of a sync that computed every remote status at `tip`
fn remote_events(tip: &str) -> Vec<SyncEvent> {
  vec![
    SyncEvent::RemoteStatusUpdate(remote_at("auth", tip)),
    SyncEvent::RemoteStatusUpdate(remote_at("ui", tip)),
    SyncEvent::StackBranchUpdate(Box::new(stack_branch("auth", Some(remote_at("auth", tip))))),
    SyncEvent::UnappliedBranchesFound {
      branches: vec![unapplied("docs", Some(remote_at("docs", tip)))],
    },
  ]
}

/// The remote tips the table shows of `auth`, `ui`, the stack branch of `auth` and the unapplied `docs`
fn tips(state: &TableState) -> [String; 4] {
  [shown_tip(state, "auth"), shown_tip(state, "ui"), stack_tip(state, "auth"), unapplied_tip(state, "docs")].map(Option::unwrap_or_default)
}

/// The outcome of a push goes into the table without a sync: the branch, the stack branch and the unapplied branch it
/// sent. Written while a sync runs, it is newer than the remote statuses that run computes, and a run that fails restores
/// the table with it; a run that starts after it shows what it computes
#[test]
fn the_outcome_of_a_push_stays_over_the_remote_statuses_of_a_sync_that_started_before_it() {
  let mut state = synced_with_remotes();
  state.begin_push(&pushed_subjects());
  state.end_push(&pushed_subjects(), Some(pushed_statuses("pushed")));
  assert_eq!(tips(&state), ["pushed", "synced", "pushed", "pushed"], "a push without a sync");

  state.begin_sync();
  state.begin_push(&pushed_subjects());
  state.end_push(&pushed_subjects(), Some(pushed_statuses("pushed again")));
  for event in remote_events("computed before the push") {
    state.apply(event);
  }
  assert_eq!(
    tips(&state),
    ["pushed again", "computed before the push", "pushed again", "pushed again"],
    "the run started before the push: its statuses of what the push sent are older"
  );
  state.finish_sync(Err(ApiError::new(ErrorCode::Internal, "the sync failed")));
  assert_eq!(tips(&state), ["pushed again", "synced", "pushed again", "pushed again"], "restored with the push");

  state.begin_sync();
  for event in remote_events("computed after the push") {
    state.apply(event);
  }
  state.finish_sync(Ok(()));
  assert_eq!(tips(&state), ["computed after the push"; 4]);
}

/// While a push of a branch runs, a sync shows no remote status of what it sends; the push's outcome replaces the one the
/// sync computed, and a push that fails leaves that one, the newest the session knows. Two pushes of one branch run until
/// both ended
#[test]
fn a_sync_shows_no_remote_status_of_a_running_push_and_a_failed_push_leaves_the_one_it_computed() {
  let mut state = synced_with_remotes();
  state.begin_push(&pushed_subjects());
  run_sync(&mut state, remote_events("fetched"));
  assert_eq!(tips(&state), ["synced", "fetched", "synced", "synced"], "only ui, which no push sends");
  state.end_push(&pushed_subjects(), None);
  assert_eq!(tips(&state), ["fetched"; 4], "the push failed: what the sync computed");

  state.begin_push(&pushed_subjects());
  state.begin_push(&pushed_subjects());
  run_sync(&mut state, remote_events("fetched again"));
  state.end_push(&pushed_subjects(), Some(pushed_statuses("pushed")));
  assert_eq!(tips(&state), ["pushed", "fetched again", "pushed", "pushed"]);
  run_sync(&mut state, remote_events("while the second push runs"));
  assert_eq!(tips(&state), ["pushed", "while the second push runs", "pushed", "pushed"]);
  state.end_push(&pushed_subjects(), None);
  assert_eq!(tips(&state), ["while the second push runs"; 4]);
}

/// A deleted archived or unapplied branch leaves its section at once; a sync that listed it before the deletion does not
/// bring it back, nor does one that fails
#[test]
fn a_deleted_archived_or_unapplied_branch_stays_deleted_by_the_sync_that_listed_it_before_and_by_one_that_fails() {
  let info = |name: &str| {
    let mut info = integrated_branch_info();
    info.name = name.to_string();
    info
  };
  let archived = |names: &[&str]| SyncEvent::ArchivedBranchesFound {
    branch_names: names.iter().map(ToString::to_string).collect(),
    newly_archived: names.iter().map(ToString::to_string).collect(),
  };
  let listed = || SyncEvent::UnappliedBranchesFound {
    branches: vec![unapplied("docs", None), unapplied("api", None)],
  };
  let mut state = TableState::new();
  run_sync(
    &mut state,
    vec![
      archived(&["test/archived/a", "test/archived/b"]),
      SyncEvent::BranchIntegrationDetected { info: info("test/archived/a") },
      listed(),
    ],
  );
  state.remove_archived("test/archived/a");
  let section = &state.sections().archived;
  assert_eq!(
    (&section.names[..], &section.newly_archived[..]),
    (&["test/archived/b".to_string()][..], &["test/archived/b".to_string()][..])
  );
  assert!(section.integration.is_empty());

  state.begin_sync();
  state.remove_archived("test/archived/b");
  state.remove_unapplied("test/unapplied/docs");
  state.apply(archived(&["test/archived/b"]));
  state.apply(SyncEvent::BranchIntegrationDetected { info: info("test/archived/b") });
  state.apply(listed());
  let names = |state: &TableState| {
    let sections = state.sections();
    let unapplied: Vec<String> = sections.unapplied.iter().map(|branch| branch.branch.clone()).collect();
    (sections.archived.names.clone(), sections.archived.integration.len(), unapplied)
  };
  assert_eq!(names(&state), (Vec::new(), 0, vec!["api".to_string()]));
  state.finish_sync(Err(ApiError::new(ErrorCode::Internal, "the sync failed")));
  assert_eq!(names(&state), (Vec::new(), 0, vec!["api".to_string()]), "restored without them");
}
