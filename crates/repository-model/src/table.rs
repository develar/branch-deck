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
//! `bd sync` folds the events of its sync into a table of its own and summarizes it, so the app and `bd` read one
//! applier; beside the parts of the view it reads two facts the page has no use for: the order in which the sync
//! reported the stack branches (`stacks`) and the commits each branch left uncopied (`blocked_commits`).
//!
//! The session writes into the table outside a sync too (`OwnWrite`): the outcome of a push (`end_push`: the remote
//! statuses it left) and the deletion of an archived or unapplied branch. The runs and the writes take their numbers
//! from the one counter of the session (`Sequence` of `facts`): a run when it starts, a write when it is applied, so a
//! write is newer than every event of a run that started before it, and that run never undoes it (a remote status it
//! computed before the push, an archived branch it listed before the deletion); a failed run restores the table with it. While a push of a branch runs
//! (`begin_push`), a sync shows no remote status of it: the status it computed is kept aside and shown only when the
//! push fails, as the newest the session knows; a push that waits for a sync is not running yet.
//!
//! Beside the table the state keeps the operations of the remote that run or wait in the session (`operations`, in the
//! order they were admitted), the `operations` part of the view: `operations::registry` admits them by
//! `domain_policy::admission` and changes them through `apply_operation` on the actor of the session
//! (`Sessions::apply_with`), the one writer of that part. Among them are the operations of other processes of the
//! repository (`Origin::OtherProcess`, an agent's `bd safe-push`), as the operations registry of the repository has them
//! (`operations::registry_watch`): each read of it replaces them all (`OperationPatch::Foreign`), and the ones of this
//! process stay as they are. It keeps the rewrites of this process that moved HEAD too (`rewrites`, `RewriteLink`), which a
//! rewrite asked on the table before one of them names its commits through (`operations::rewrite`).

use crate::derive;
use crate::facts::{Seq, Sequence};
use domain_types::branch_names::{BranchKind, qualified_branch_name, to_final_branch_name, to_stack_branch_name};
use domain_types::commit::Commit;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::operation::{OperationId, OperationStep, Origin, RunningMove, RunningOperation};
use domain_types::session::{
  ArchivedSection, BaselineStatus, BranchHead, BranchRemote, BranchStatus, BranchStructure, BranchSync, BranchView, CommitSync, LandedSection, SyncState, SyncStep, TableHeader,
  TableSections,
};
use domain_types::stack::StackBranchUpdate;
use domain_types::{BranchError, BranchSyncStatus, CommitSyncStatus, GroupedBranchInfo, RemoteStatusUpdate, SyncEvent, UnappliedBranchInfo};
use std::collections::{BTreeMap, HashMap, HashSet};
use sync_core::push::PUSH_REMOTE;
use sync_utils::issue_pattern::{find_issue_number, issue_lead};

/// The parts of the table that a failed sync restores
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct TableParts {
  pub(crate) header: TableHeader,
  pub(crate) order: Vec<String>,
  pub(crate) branches: BTreeMap<String, BranchView>,
  pub(crate) stacks: BTreeMap<String, StackBranchUpdate>,
  pub(crate) sections: TableSections,
  /// Not a part of the view, which keys the stack branches by root: the roots in the order the grouping announced
  /// them, of each stack that forks, then those kept for their remote branch (the order of `bd sync`)
  pub(crate) stack_roots: Vec<String>,
  /// Not a part of the view, whose rows mark none of them: branch -> the commits the sync did not copy, oldest first,
  /// because an earlier commit failed or the branch was not built (`CommitsBlocked`; `bd sync` lists them)
  pub(crate) blocked: HashMap<String, Vec<String>>,
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
  /// The writes of the session since `before_sync` was taken, in order, each with its number; empty while no sync runs
  writes: Vec<(Seq, OwnWrite)>,
  /// The counter of the observations of the session, the runs and the writes among them
  sequence: Sequence,
  /// The number of the run in flight: the writes after it win over its events
  run: Seq,
  /// The operations of the remote that run or wait, in the order they were admitted (`apply_operation`); no part of the
  /// table, so no sync restores or drops one
  operations: Vec<RunningOperation>,
  /// The rewrites of this process that moved HEAD, oldest first, the last `REWRITE_LINKS` of them
  /// (`record_rewrite`): a rewrite asked on the table before one of them names its commits through them
  /// (`operations::rewrite`); no part of the table either
  rewrites: Vec<RewriteLink>,
}

/// How many rewrites that moved HEAD a session keeps (`TableState::rewrites`)
const REWRITE_LINKS: usize = 16;

/// What a rewrite of this process did to HEAD: the commit it found and the one it left, and the new id of each commit
/// its core named (`RewriteResult::rewritten`); a commit it does not name may have another id after it too, but after
/// a rewrite that only added commits on top (`appends`: a pick, a reapply)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RewriteLink {
  pub from: String,
  pub to: String,
  pub rewritten: BTreeMap<String, String>,
  pub appends: bool,
}

/// A change of the operations of a session, which `operations::registry` makes (the module's header)
#[derive(Clone, Debug)]
pub enum OperationPatch {
  /// An operation was admitted, to run or to wait: it comes after the others
  Admitted(RunningOperation),
  /// An operation went on to another step
  Step { id: OperationId, step: OperationStep },
  /// The moves of rows an operation makes now
  Moves { id: OperationId, moves: Vec<RunningMove> },
  /// A request of the same intent joined an operation
  Joined(OperationId),
  /// An operation needs more than it held, and was admitted again as a new one is: it comes after the others, with what
  /// it holds now and its step (`Queued` behind the one it waits for, or the step it goes on at)
  Requeued(RunningOperation),
  /// An operation ended, or stopped waiting
  Ended(OperationId),
  /// The operations of other processes in the session as the operations registry of the repository has them now, in
  /// place of the ones read before; those of this process stay
  Foreign(Vec<RunningOperation>),
}

/// What a push sends, as the table keys its remote status: a virtual branch by its name, the stack branch of the whole
/// stack of a root, an unapplied branch by the branch it was (`UnappliedBranchInfo::branch`)
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RemoteSubject {
  Branch(String),
  WholeStack(String),
  Unapplied(String),
}

impl RemoteSubject {
  /// The remote-tracking ref a push of the subject under `branch_prefix` moves (`refs/remotes/origin/<full name>`);
  /// `None` for a name no branch can have
  #[must_use]
  pub fn remote_ref(&self, branch_prefix: &str) -> Option<String> {
    let full_name = match self {
      Self::Branch(name) => to_final_branch_name(branch_prefix, name),
      Self::WholeStack(root) => to_stack_branch_name(branch_prefix, root),
      Self::Unapplied(name) => qualified_branch_name(branch_prefix, BranchKind::Unapplied, name),
    };
    full_name.ok().map(|full_name| format!("refs/remotes/{PUSH_REMOTE}/{full_name}"))
  }
}

/// The pushes of one subject in flight
#[derive(Clone, Debug, Default)]
struct Push {
  /// How many run: a second push of the same ref waits for the first in the registry of the operations
  /// (`operations::push`), yet runs from its call
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

  /// The commits ahead of the baseline the last grouping placed: of the branches in the order of the table, then the
  /// unassigned ones
  pub fn commits(&self) -> impl Iterator<Item = &Commit> {
    let branches = self.parts.order.iter().filter_map(|name| self.parts.branches.get(name));
    branches.flat_map(|branch| branch.commits.iter()).chain(self.parts.sections.unassigned.iter())
  }

  /// The rewrites of this process that moved HEAD, oldest first (`RewriteLink`)
  #[must_use]
  pub fn rewrites(&self) -> &[RewriteLink] {
    &self.rewrites
  }

  /// A rewrite of this process moved HEAD as `link` says: the oldest one beyond `REWRITE_LINKS` is forgotten
  pub fn record_rewrite(&mut self, link: RewriteLink) {
    if self.rewrites.len() == REWRITE_LINKS {
      self.rewrites.remove(0);
    }
    self.rewrites.push(link);
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

  /// The stack branches in the order the sync reported them: of each stack that forks, then those kept for their
  /// remote branch
  pub fn stacks(&self) -> impl Iterator<Item = &StackBranchUpdate> {
    self.parts.stack_roots.iter().filter_map(|root| self.parts.stacks.get(root))
  }

  /// The commits of `branch` that the last sync did not copy, oldest first: those after the commit that failed, or all
  /// of a branch it did not build
  #[must_use]
  pub fn blocked_commits(&self, branch: &str) -> &[String] {
    self.parts.blocked.get(branch).map_or(&[], Vec::as_slice)
  }

  #[must_use]
  pub const fn sections(&self) -> &TableSections {
    &self.parts.sections
  }

  /// The number of an observation of the session that starts now (a read of a fact), from the counter of its runs and
  /// writes
  pub const fn next_seq(&mut self) -> Seq {
    self.sequence.next()
  }

  /// A sync starts: it groups the commits first. The table stays as it is until its events come. Answers the number of
  /// the run, newer than every write before it; a retry starts after the writes so far, so its events are newer too
  pub fn begin_sync(&mut self) -> Seq {
    if !matches!(self.sync, SyncState::Running { .. }) {
      self.before_sync = Some(Box::new(self.parts.clone()));
    }
    self.run = self.sequence.next();
    self.sync = SyncState::Running { step: SyncStep::Grouping };
    self.run
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
        self.restore_before_sync();
        self.sync = SyncState::Failed { error };
      }
    }
  }

  /// The sync was refused before it built anything (a rebase in progress, a detached HEAD: `SyncDeferred`): the table
  /// of the sync before comes back with what the session wrote since, and nothing failed. The sync says why it waits;
  /// a run under a prefix the git config left (`prefix_changed`) is no state of this session, whose window opens the
  /// session of the new prefix
  pub fn defer_sync(&mut self, error: ApiError) {
    self.restore_before_sync();
    self.sync = if error.code == ErrorCode::PrefixChanged {
      SyncState::Idle
    } else {
      SyncState::Deferred { error }
    };
  }

  /// The table of the sync before comes back, with what the session wrote since
  fn restore_before_sync(&mut self) {
    if let Some(before) = self.before_sync.take() {
      self.parts = *before;
      for (_, write) in std::mem::take(&mut self.writes) {
        write_into(&mut self.parts, write);
      }
      self.commit_index = index_commits(&self.parts.branches);
    }
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
        target_branch,
        head,
        branch,
        store_tips,
      } => {
        self.set_step(SyncStep::Building);
        let header = &mut self.parts.header;
        header.baseline_branch = baseline_branch;
        header.target_branch = target_branch;
        header.head = head;
        header.branch = branch;
        header.store_tips = store_tips;
        header.stacks_enabled = stacks;
        header.issue_groups = issue_groups;
        header.stack_parents = stack_parents;
        header.parked = parked;
        self.apply_grouping(branches);
        // the stack branches the grouping announces keep what the last sync said of them until their update
        let announced: HashSet<&String> = stack_branches.iter().collect();
        self.parts.stacks.retain(|root, _| announced.contains(root));
        self.parts.stack_roots = stack_branches;
        // every branch builds anew
        self.parts.blocked.clear();
        derive::place(&mut self.parts);
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
      // the rows don't mark them
      SyncEvent::CommitsBlocked {
        branch_name,
        blocked_commit_hashes,
      } => {
        if self.parts.branches.contains_key(&branch_name) {
          self.parts.blocked.insert(branch_name, blocked_commit_hashes);
        }
      }
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
        if !self.parts.stack_roots.contains(&update.root) {
          self.parts.stack_roots.push(update.root.clone());
        }
        let root = update.root.clone();
        self.parts.stacks.insert(root.clone(), update);
        derive::place_stack(&mut self.parts, &root);
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
  /// Whether a push of `subject` (or a fetch run as one, `Sessions::fetch_moved_remotes`) is in flight
  #[must_use]
  pub fn is_pushing(&self, subject: &RemoteSubject) -> bool {
    self.pushes.contains_key(subject)
  }

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

  /// The operations of the remote that run or wait, in the order they were admitted
  #[must_use]
  pub fn operations(&self) -> &[RunningOperation] {
    &self.operations
  }

  /// Changes the operations as `patch` says; a step, moves, a join or an end of an operation the session lacks change
  /// nothing
  pub fn apply_operation(&mut self, patch: OperationPatch) {
    match patch {
      OperationPatch::Admitted(operation) => self.operations.push(operation),
      OperationPatch::Step { id, step } => {
        if let Some(operation) = self.operations.iter_mut().find(|operation| operation.id == id) {
          operation.step = step;
        }
      }
      OperationPatch::Moves { id, moves } => {
        if let Some(operation) = self.operations.iter_mut().find(|operation| operation.id == id) {
          operation.moves = moves;
        }
      }
      OperationPatch::Joined(id) => {
        if let Some(operation) = self.operations.iter_mut().find(|operation| operation.id == id) {
          operation.joined = operation.joined.saturating_add(1);
        }
      }
      OperationPatch::Ended(id) => self.operations.retain(|operation| operation.id != id),
      OperationPatch::Requeued(operation) => {
        self.operations.retain(|held| held.id != operation.id);
        self.operations.push(operation);
      }
      OperationPatch::Foreign(foreign) => {
        self.operations.retain(|operation| operation.origin == Origin::ThisProcess);
        // a start of this process whose answer was lost is in the file before its run ended here: its run shows it until
        // then, and the read after its end the record
        let own: HashSet<OperationId> = self.operations.iter().map(|operation| operation.id.clone()).collect();
        self.operations.extend(foreign.into_iter().filter(|operation| !own.contains(&operation.id)));
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
  /// Writes into the table, with the next number of the session; while a sync runs, also into the writes its events
  /// never undo and its failure restores
  fn write(&mut self, write: OwnWrite) {
    let seq = self.sequence.next();
    if self.before_sync.is_some() {
      self.writes.push((seq, write.clone()));
    }
    write_into(&mut self.parts, write);
  }

  /// Whether the session wrote what `matches` after the run in flight started
  fn written_in_run(&self, matches: impl Fn(&OwnWrite) -> bool) -> bool {
    self.writes.iter().any(|(seq, write)| *seq > self.run && matches(write))
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

/// The issue key the last level of a branch name starts with (`find_issue_number`)
fn name_issue_key(name: &str) -> Option<String> {
  find_issue_number(name.rsplit('/').next().unwrap_or(name)).map(str::to_owned)
}

fn head_of(info: &GroupedBranchInfo) -> BranchHead {
  BranchHead {
    name: info.name.clone(),
    latest_commit_time: info.latest_commit_time,
    summary: info.summary.clone(),
    description_title: info.description_title.clone(),
    description_lead: info.description_title.as_deref().and_then(issue_lead),
    name_issue_key: name_issue_key(&info.name),
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
    structure: BranchStructure::default(),
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
    structure: shown.structure,
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
