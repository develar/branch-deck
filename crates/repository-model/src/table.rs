//! The branch table of a session as the events of its syncs leave it (docs/design/git-operations.md, "Repository
//! view"): `TableState::apply` folds each `SyncEvent` into the parts of `domain_types::session`, in the order the sync
//! sends them, as the page's applier did (`branch-sync.ts`). A grouping keeps the row of a branch it groups as before,
//! with the same commits, which the last sync built without a problem: the row shows what it showed until the sync
//! reports the branch again (a quiet resync), so an unchanged sync changes no part. Any other grouped branch starts
//! `Syncing`; a branch the grouping no longer has leaves the table. The remote status of a branch stays until a sync
//! computes a new one.
//!
//! A sync that fails restores the table of the sync before (`finish_sync`): the rows of a failed sync are never shown
//! as the table. The texts of the statuses, the marks of what to expand and the badges are the page's presentation.
//!
//! The session writes into the table outside a sync too (`OwnWrite`): the outcome of a push (`end_push`: the remote
//! statuses it left) and the deletion of an archived or unapplied branch. Such a write is newer than every event of a
//! run that started before it, so that run never undoes it (a remote status it computed before the push, an archived
//! branch it listed before the deletion), and a failed run restores the table with it. While a push of a branch runs
//! (`begin_push`), a sync shows no remote status of it: the status it computed is kept aside and shown only when the
//! push fails, as the newest the session knows; a push that waits for a sync is not running yet.

use domain_types::error::ApiError;
use domain_types::session::{
  ArchivedSection, BaselineStatus, BranchHead, BranchRemote, BranchStatus, BranchSync, BranchView, CommitSync, LandedSection, SyncState, SyncStep, TableHeader, TableSections,
};
use domain_types::stack::StackBranchUpdate;
use domain_types::{BranchError, BranchSyncStatus, CommitSyncStatus, GroupedBranchInfo, RemoteStatusUpdate, SyncEvent, UnappliedBranchInfo};
use std::collections::{BTreeMap, HashMap, HashSet};
use sync_core::push::StackPushBranch;

/// The parts of the table that a failed sync restores
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TableParts {
  pub(crate) header: TableHeader,
  pub(crate) order: Vec<String>,
  pub(crate) branches: BTreeMap<String, BranchView>,
  pub(crate) stacks: BTreeMap<String, StackBranchUpdate>,
  pub(crate) sections: TableSections,
}

/// The branch table of a session, folded from the events of its syncs
#[derive(Clone, Debug, Default)]
pub struct TableState {
  pub(crate) sync: SyncState,
  pub(crate) parts: TableParts,
  /// Branch -> original id -> the index of the commit in `BranchView::commits`
  commit_index: HashMap<String, HashMap<String, usize>>,
  /// The parts when the running sync started, restored when it fails
  before_sync: Option<Box<TableParts>>,
  /// The pushes in flight, by what they send
  pushes: HashMap<RemoteSubject, Push>,
  /// The writes of the session since `before_sync` was taken, in order, each with its number (`write_count` after it);
  /// empty while no sync runs
  writes: Vec<(u64, OwnWrite)>,
  write_count: u64,
  /// `write_count` when the run in flight started: the writes after it win over its events
  run_start: u64,
}

/// What a push sends, as the table keys its remote status: a virtual branch by its name, the stack branch of the whole
/// stack of a root (`StackPushBranch::whole_stack`), an unapplied branch by the branch it was
/// (`UnappliedBranchInfo::branch`, `StackPushBranch::unapplied`)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RemoteSubject {
  Branch(String),
  WholeStack(String),
  Unapplied(String),
}

impl RemoteSubject {
  /// What the branch of a push sends
  #[must_use]
  pub fn of(branch: &StackPushBranch) -> Self {
    let name = branch.branch_name.clone();
    if branch.whole_stack {
      Self::WholeStack(name)
    } else if branch.unapplied {
      Self::Unapplied(name)
    } else {
      Self::Branch(name)
    }
  }
}

/// The pushes of one subject in flight
#[derive(Clone, Debug, Default)]
struct Push {
  /// How many run: a second push of the same ref waits in Rust for the first (`PushingRefs`), yet runs from its call
  running: u32,
  /// What a sync saw of the remote while they ran, which a push that fails leaves
  seen: Option<BranchRemote>,
}

/// A change of the table the session made outside a sync
#[derive(Clone, Debug)]
enum OwnWrite {
  /// What a push left on the remote of a subject
  Remote(RemoteSubject, BranchRemote),
  /// An archived branch deleted, by its full name
  RemovedArchived(String),
  /// An unapplied branch deleted, by its full name
  RemovedUnapplied(String),
}

impl TableState {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  #[must_use]
  pub const fn sync(&self) -> &SyncState {
    &self.sync
  }

  #[must_use]
  pub const fn header(&self) -> &TableHeader {
    &self.parts.header
  }

  /// The names of the branches in the order of the table
  #[must_use]
  pub fn order(&self) -> &[String] {
    &self.parts.order
  }

  #[must_use]
  pub fn branch(&self, name: &str) -> Option<&BranchView> {
    self.parts.branches.get(name)
  }

  #[must_use]
  pub fn stack(&self, root: &str) -> Option<&StackBranchUpdate> {
    self.parts.stacks.get(root)
  }

  #[must_use]
  pub const fn sections(&self) -> &TableSections {
    &self.parts.sections
  }

  /// A sync starts: it groups the commits first. The table stays as it is until its events come
  pub fn begin_sync(&mut self) {
    if !matches!(self.sync, SyncState::Running { .. }) {
      self.before_sync = Some(Box::new(self.parts.clone()));
    }
    // a retry starts after the writes so far: its events are newer than they are
    self.run_start = self.write_count;
    self.sync = SyncState::Running { step: SyncStep::Grouping };
  }

  /// The sync ended: on success the table is what its events left; on a failure the table of the sync before comes
  /// back, with the error
  pub fn finish_sync(&mut self, result: Result<(), ApiError>) {
    match result {
      Ok(()) => {
        self.before_sync = None;
        self.writes.clear();
        self.sync = SyncState::Idle;
      }
      Err(error) => {
        self.defer_sync();
        self.sync = SyncState::Failed { error };
      }
    }
  }

  /// The sync was refused before it built anything (a rebase in progress, a detached HEAD: `SyncDeferred`): the table
  /// of the sync before comes back with what the session wrote since, and nothing failed
  pub fn defer_sync(&mut self) {
    if let Some(before) = self.before_sync.take() {
      self.parts = *before;
      for (_, write) in std::mem::take(&mut self.writes) {
        write_into(&mut self.parts, write);
      }
      self.commit_index = index_commits(&self.parts.branches);
    }
    self.sync = SyncState::Idle;
  }

  /// The sync runs once more for `branches` (`SyncReport::retry`: another process moved what it built): their rows say
  /// `Syncing` until the retry reports them; the sync goes on, so a failed retry still restores the table before both
  pub fn resync<'a>(&mut self, branches: impl IntoIterator<Item = &'a str>) {
    for name in branches {
      if let Some(branch) = self.parts.branches.get_mut(name) {
        branch.sync = BranchSync::SYNCING;
      }
    }
  }

  /// Folds one event of the running sync into the table
  pub fn apply(&mut self, event: SyncEvent) {
    match event {
      SyncEvent::IssueNavigationConfig { config } => {
        // a sync without a configuration keeps the one found before
        if config.is_some() {
          self.parts.header.issue_navigation_config = config;
        }
      }
      SyncEvent::BranchesGrouped {
        branches,
        baseline_branch,
        stacks,
        issue_groups,
        stack_parents,
        parked,
        stack_branches,
      } => {
        self.set_step(SyncStep::Building);
        let header = &mut self.parts.header;
        header.baseline_branch = baseline_branch;
        header.stacks_enabled = stacks;
        header.issue_groups = issue_groups;
        header.stack_parents = stack_parents;
        header.parked = parked;
        self.apply_grouping(branches);
        // the stack branches the grouping announces keep what the last sync said of them until their update
        let announced: HashSet<String> = stack_branches.into_iter().collect();
        self.parts.stacks.retain(|root, _| announced.contains(root));
      }
      SyncEvent::UnassignedCommits { commits } => self.parts.sections.unassigned = commits,
      SyncEvent::CommitSynced {
        branch_name,
        commit_hash,
        new_hash,
        status,
        resolved,
      } => self.update_commit(&branch_name, &commit_hash, |branch, index| {
        count_processed(&mut branch.sync);
        let commit = &mut branch.copies[index];
        commit.hash = Some(new_hash);
        commit.status = Some(status);
        commit.resolved = resolved;
      }),
      SyncEvent::CommitError { branch_name, commit_hash, error } => self.update_commit(&branch_name, &commit_hash, |branch, index| {
        // errors count as processed too
        count_processed(&mut branch.sync);
        let commit = &mut branch.copies[index];
        commit.status = Some(CommitSyncStatus::Error);
        commit.error = Some(error.clone());
        branch.sync.status = BranchStatus::Error;
        branch.sync.has_error = true;
        branch.sync.error_details = Some(error);
      }),
      // not displayed
      SyncEvent::CommitsBlocked { .. } => {}
      SyncEvent::BranchStatusUpdate { branch_name, status, error } => self.apply_branch_status(&branch_name, status, error),
      SyncEvent::BranchIntegrationDetected { info } => {
        if self.written_in_run(|write| matches!(write, OwnWrite::RemovedArchived(name) if *name == info.name)) {
          return;
        }
        let archived = &mut self.parts.sections.archived;
        if !archived.names.contains(&info.name) {
          archived.names.push(info.name.clone());
        }
        archived.integration.insert(info.name.clone(), info);
      }
      SyncEvent::ArchivedBranchesFound {
        mut branch_names,
        mut newly_archived,
      } => {
        // the run listed them before a deletion of this session
        let deleted = |name: &String| self.written_in_run(|write| matches!(write, OwnWrite::RemovedArchived(removed) if removed == name));
        branch_names.retain(|name| !deleted(name));
        newly_archived.retain(|name| !deleted(name));
        apply_archived_names(&mut self.parts.sections.archived, branch_names, newly_archived);
      }
      SyncEvent::UnappliedBranchesFound { branches } => {
        // sent once the branches and the stack branches are built
        self.set_step(SyncStep::Detection);
        self.apply_unapplied(branches);
      }
      SyncEvent::RemoteStatusUpdate(status) => {
        let name = status.branch_name.clone();
        if let Some(remote) = self.remote_of_run(&RemoteSubject::Branch(name.clone()), Some(status))
          && let Some(branch) = self.parts.branches.get_mut(&name)
        {
          branch.remote = remote;
        }
      }
      SyncEvent::StackBranchUpdate(update) => {
        let mut update = *update;
        let subject = RemoteSubject::WholeStack(update.root.clone());
        let shown = self.parts.stacks.get(&update.root).and_then(|stack| stack.remote.clone());
        update.remote = self.remote_of_run(&subject, update.remote.take()).map_or(shown, |remote| remote.status);
        self.parts.stacks.insert(update.root.clone(), update);
      }
      SyncEvent::BaselineStatus {
        baseline_branch,
        update,
        check_error,
        landing,
        landed,
      } => {
        let baseline = Some(BaselineStatus {
          baseline_branch,
          update: update.map(|update| *update),
          check_error,
        });
        self.parts.sections.landed = LandedSection { baseline, landing, landed };
      }
    }
  }

  fn set_step(&mut self, step: SyncStep) {
    if matches!(self.sync, SyncState::Running { .. }) {
      self.sync = SyncState::Running { step };
    }
  }

  /// A push of `subjects` starts sending: a sync meanwhile shows no remote status of them until it ends
  pub fn begin_push(&mut self, subjects: &[RemoteSubject]) {
    for subject in subjects {
      self.pushes.entry(subject.clone()).or_default().running += 1;
    }
  }

  /// A push of `subjects` (as `begin_push` named them) ended: `statuses`, in their order, are the remote statuses it left
  /// (pushed, or fetched when the remote moved), newer than any sync that started before; `None` for a push that failed,
  /// which leaves the status a sync computed meanwhile, if one did
  pub fn end_push(&mut self, subjects: &[RemoteSubject], statuses: Option<Vec<RemoteStatusUpdate>>) {
    let mut statuses = statuses.map(Vec::into_iter);
    for subject in subjects {
      let seen = match self.pushes.get_mut(subject) {
        Some(push) if push.running > 1 => {
          push.running -= 1;
          None
        }
        Some(_) => self.pushes.remove(subject).and_then(|push| push.seen),
        None => None,
      };
      let remote = match statuses.as_mut() {
        Some(statuses) => statuses.next().map(|status| BranchRemote { status: Some(status) }),
        None => seen,
      };
      if let Some(remote) = remote {
        self.write(OwnWrite::Remote(subject.clone(), remote));
      }
    }
  }

  /// The archived branch `name` (its full name) was deleted
  pub fn remove_archived(&mut self, name: &str) {
    self.write(OwnWrite::RemovedArchived(name.to_owned()));
  }

  /// The unapplied branch `name` (its full name) was deleted
  pub fn remove_unapplied(&mut self, name: &str) {
    self.write(OwnWrite::RemovedUnapplied(name.to_owned()));
  }
}

impl TableState {
  /// Writes into the table; while a sync runs, also into the writes its events never undo and its failure restores
  fn write(&mut self, write: OwnWrite) {
    if self.before_sync.is_some() {
      self.write_count += 1;
      self.writes.push((self.write_count, write.clone()));
    }
    write_into(&mut self.parts, write);
  }

  /// Whether the session wrote what `matches` since the run in flight started
  fn written_in_run(&self, matches: impl Fn(&OwnWrite) -> bool) -> bool {
    self.writes.iter().any(|(number, write)| *number > self.run_start && matches(write))
  }

  /// What the table shows of `subject` after the remote status the running sync computed (`observed`, `None`: no remote
  /// branch): that status, unless a push wrote the subject since the run started (it stays as the push left it) or a
  /// push of it runs (the status is kept for a push that fails); `None` keeps what the table shows
  fn remote_of_run(&mut self, subject: &RemoteSubject, observed: Option<RemoteStatusUpdate>) -> Option<BranchRemote> {
    let observed = BranchRemote { status: observed };
    if self.written_in_run(|write| matches!(write, OwnWrite::Remote(written, _) if written == subject)) {
      return None;
    }
    if let Some(push) = self.pushes.get_mut(subject) {
      push.seen = Some(observed);
      return None;
    }
    Some(observed)
  }

  /// The unapplied branches of the sync, without the ones this session deleted since the run started, each with its
  /// remote status by `remote_of_run`
  fn apply_unapplied(&mut self, branches: Vec<UnappliedBranchInfo>) {
    let mut kept = Vec::with_capacity(branches.len());
    for mut branch in branches {
      if self.written_in_run(|write| matches!(write, OwnWrite::RemovedUnapplied(name) if *name == branch.name)) {
        continue;
      }
      let shown = self
        .parts
        .sections
        .unapplied
        .iter()
        .find(|shown| shown.name == branch.name)
        .and_then(|shown| shown.remote.clone());
      let subject = RemoteSubject::Unapplied(branch.branch.clone());
      branch.remote = self.remote_of_run(&subject, branch.remote.take()).map_or(shown, |remote| remote.status);
      kept.push(branch);
    }
    self.parts.sections.unapplied = kept;
  }

  /// The branches in the order of the grouping, each a quiet resync of its row or a fresh `Syncing` one
  fn apply_grouping(&mut self, grouped: Vec<GroupedBranchInfo>) {
    let mut previous = std::mem::take(&mut self.parts.branches);
    let mut order = Vec::with_capacity(grouped.len());
    for info in grouped {
      if self.parts.branches.contains_key(&info.name) {
        continue;
      }
      let branch = match previous.remove(&info.name) {
        Some(shown) if is_quiet_resync(&shown, &info) => keep_shown_state(shown, info),
        shown => fresh_branch(info, shown.map_or_else(BranchRemote::default, |shown| shown.remote)),
      };
      order.push(branch.head.name.clone());
      self.parts.branches.insert(branch.head.name.clone(), branch);
    }
    self.parts.order = order;
    self.commit_index = index_commits(&self.parts.branches);
  }

  /// Changes the commit `commit_hash` of `branch_name`; nothing when the table lacks either
  fn update_commit(&mut self, branch_name: &str, commit_hash: &str, update: impl FnOnce(&mut BranchView, usize)) {
    let Some(index) = self.commit_index.get(branch_name).and_then(|index| index.get(commit_hash)).copied() else {
      return;
    };
    if let Some(branch) = self.parts.branches.get_mut(branch_name) {
      update(branch, index);
    }
  }

  fn apply_branch_status(&mut self, branch_name: &str, status: BranchSyncStatus, error: Option<BranchError>) {
    let Some(branch) = self.parts.branches.get_mut(branch_name) else {
      return;
    };
    if status == BranchSyncStatus::Blocked {
      // not an error of this branch: its parent can't be built on, the reason says why
      branch.sync.status = BranchStatus::Blocked;
      branch.sync.blocked_reason = match error {
        Some(BranchError::Generic(reason)) => Some(reason),
        _ => None,
      };
      return;
    }
    branch.sync.status = status.into();
    branch.sync.blocked_reason = None;
    if status == BranchSyncStatus::Error {
      // the commits whose prefix the stack problem asks to reword carry the error mark, the others stay as they are
      let problem_commits = branch.head.stack.as_ref().and_then(|stack| stack.problems.first()).map(|problem| problem.commits.clone());
      let index = self.commit_index.get(branch_name);
      for hash in problem_commits.unwrap_or_default() {
        if let Some(&position) = index.and_then(|index| index.get(&hash)) {
          branch.copies[position].status = Some(CommitSyncStatus::Error);
        }
      }
    }
    if let Some(error) = error {
      branch.sync.error_details = Some(error);
      branch.sync.has_error = true;
    }
    if matches!(status, BranchSyncStatus::MergeConflict | BranchSyncStatus::Error) {
      branch.sync.has_error = true;
    }
  }
}

/// The progress of a row counts while it is `Syncing`; a quiet resync keeps the count it showed
const fn count_processed(sync: &mut BranchSync) {
  if matches!(sync.status, BranchStatus::Syncing) {
    sync.processed_count = sync.processed_count.saturating_add(1);
  }
}

fn index_commits(branches: &BTreeMap<String, BranchView>) -> HashMap<String, HashMap<String, usize>> {
  branches
    .iter()
    .map(|(name, branch)| {
      let index = branch.commits.iter().enumerate().map(|(position, commit)| (commit.id.clone(), position)).collect();
      (name.clone(), index)
    })
    .collect()
}

fn head_of(info: &GroupedBranchInfo) -> BranchHead {
  BranchHead {
    name: info.name.clone(),
    latest_commit_time: info.latest_commit_time,
    summary: info.summary.clone(),
    description_title: info.description_title.clone(),
    all_commits_have_issue_references: info.all_commits_have_issue_references,
    my_email: info.my_email.clone(),
    origin: info.origin,
    issue_key: info.issue_key.clone(),
    stack: info.stack.clone(),
  }
}

/// A row that starts `Syncing`, with the remote status the branch had
fn fresh_branch(info: GroupedBranchInfo, remote: BranchRemote) -> BranchView {
  BranchView {
    head: head_of(&info),
    sync: BranchSync::SYNCING,
    copies: vec![CommitSync::default(); info.commits.len()],
    commits: info.commits,
    remote,
  }
}

/// A branch the sync groups as it was, with the same commits, which the last sync built without a problem
fn is_quiet_resync(shown: &BranchView, info: &GroupedBranchInfo) -> bool {
  let built = matches!(shown.sync.status, BranchStatus::Created | BranchStatus::Updated | BranchStatus::Unchanged);
  built && !shown.sync.has_error && shown.commits.len() == info.commits.len() && shown.commits.iter().zip(&info.commits).all(|(shown, commit)| shown.id == commit.id)
}

/// The row of a quiet resync: the grouping of now, and what the last sync made of the branch and of each commit until
/// this one reports them
fn keep_shown_state(shown: BranchView, info: GroupedBranchInfo) -> BranchView {
  BranchView {
    head: head_of(&info),
    commits: info.commits,
    sync: shown.sync,
    copies: shown.copies,
    remote: shown.remote,
  }
}

/// `write` applied to the parts of the table: a remote status to the row, the stack branch or the unapplied branch it is
/// of (nothing when the table has none), a deleted branch out of its section
fn write_into(parts: &mut TableParts, write: OwnWrite) {
  match write {
    OwnWrite::Remote(RemoteSubject::Branch(name), remote) => {
      if let Some(branch) = parts.branches.get_mut(&name) {
        branch.remote = remote;
      }
    }
    OwnWrite::Remote(RemoteSubject::WholeStack(root), remote) => {
      if let Some(stack) = parts.stacks.get_mut(&root) {
        stack.remote = remote.status;
      }
    }
    OwnWrite::Remote(RemoteSubject::Unapplied(name), remote) => {
      if let Some(branch) = parts.sections.unapplied.iter_mut().find(|branch| branch.branch == name) {
        branch.remote = remote.status;
      }
    }
    OwnWrite::RemovedArchived(name) => {
      let archived = &mut parts.sections.archived;
      archived.names.retain(|archived| *archived != name);
      archived.newly_archived.retain(|archived| *archived != name);
      archived.integration.remove(&name);
    }
    OwnWrite::RemovedUnapplied(name) => parts.sections.unapplied.retain(|branch| branch.name != name),
  }
}

/// The archived branches of the sync: the names it found; the integration of the others goes, and a branch this sync
/// archived is marked beside the earlier marks that are still archived
fn apply_archived_names(archived: &mut ArchivedSection, branch_names: Vec<String>, newly_archived: Vec<String>) {
  let mut seen = HashSet::with_capacity(branch_names.len());
  archived.names = branch_names.into_iter().filter(|name| seen.insert(name.clone())).collect();
  archived.integration.retain(|name, _| seen.contains(name));
  let mut marked: Vec<String> = Vec::with_capacity(archived.newly_archived.len() + newly_archived.len());
  for name in std::mem::take(&mut archived.newly_archived).into_iter().chain(newly_archived) {
    if seen.contains(&name) && !marked.contains(&name) {
      marked.push(name);
    }
  }
  archived.newly_archived = marked;
}

#[cfg(test)]
#[path = "table_test.rs"]
pub(crate) mod table_test;
