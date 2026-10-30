//! The warm start (docs/design/rust-domain-authority.md, "s13-warm-start"; docs/design/git-operations.md, "Warm
//! start"): the frame of a session, which a later start of the app shows before its first sync.
//!
//! - **What a frame is** (`CachedSession`): the key of its session, the parts of its table as the last sync committed
//!   them (`CachedTable`: the header with the prefix, the order, the rows, the stack branches, the sections, of each
//!   commit what its view leaves out but a rewrite finds it again by, `CommitIdentity`, and what that sync built of the
//!   rows beside them, `CachedBuild`, which the next steps, the pushes and the actions read) and the facts of the remote
//!   services as the view had them (`CachedFacts`: the robots, the Patronus view, the landing needs with the inputs
//!   they are of, the pull requests, the rules, the merge profiles, the readiness, the review requests, the sign-ins).
//!   The parts the rules
//!   decide (the pushes, the starts, what landed, the next steps, the actions, the robots that need the user) are not
//!   kept: the session derives them again from the frame, by the rules of the binary that reads it, as from every
//!   state. No running sync, no operation, no news: a frame is of no process.
//! - **Where**: the object cache of the repository, store `session_frame`, one frame per worktree (`frame_key`, the
//!   hash of its path) under the version of the schema (`FRAME_SCHEMA`), so the start reads it without a git process and
//!   a frame of another schema is never read. The record names its key: a session seeds from it only under the same key
//!   (`CachedSession::seeds`), so a frame of another prefix seeds nothing.
//! - **The seed** (`CachedSession::seed`: `TableState::from_cached`, `FactStore::from_cached`, the table of the last
//!   sync rebuilt from the rows and the build, `CachedBuild::table`): the table is `cached` until the first sync of
//!   the session commits, each fact of the frame that stood (`Fresh`, `Stale`, `Cached`) is `Cached` until an answer of
//!   this process replaces it; a failure of that first read leaves what a failure without an answer leaves
//!   (`Unavailable`, the value gone), so no value of an earlier run stands as an answer of this one.

use crate::branch_table::{BranchTable, RemoteFacts};
use crate::facts::{FactStore, Needs};
use crate::session::SessionKey;
use crate::table::{TableParts, TableState};
use domain_types::GroupedBranchInfo;
use domain_types::commit::Commit;
use domain_types::patronus::PatronusStartOptions;
use domain_types::pull_request::BranchRules;
use domain_types::session::{BranchView, Fact, PatronusPart, PullRequestsView, ReviewRequestsView, RobotsView, SubjectReadiness, TableHeader, TableSections};
use domain_types::sign_in::AuthStatus;
use domain_types::stack::BlockReason;
use domain_types::stack::StackBranchUpdate;
use object_cache::cache::ObjectCache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use tracing::debug;

/// The version of the schema of a frame: bump it when a type a frame holds changes, and no frame of the old one is read
pub const FRAME_SCHEMA: u32 = 4;

/// The frame of a session (the module's header)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedSession {
  pub schema: u32,
  pub key: SessionKey,
  pub table: CachedTable,
  pub facts: CachedFacts,
}

impl CachedSession {
  /// Whether the session of `key` seeds from the frame: of this schema and of the same worktree and prefix
  #[must_use]
  pub fn seeds(&self, key: &SessionKey) -> bool {
    self.schema == FRAME_SCHEMA && self.key == *key
  }

  /// The table state and the facts the frame seeds: its rows `cached`, its facts `Cached`, and the table of its last
  /// sync (`FactStore::table`) rebuilt from the rows and what that sync built of them, `Cached` too, so the next steps,
  /// the pushes and the actions of the rows are derived as over a table this process committed
  #[must_use]
  pub fn seed(self) -> (TableState, FactStore) {
    let Self { key, mut table, facts, .. } = self;
    let built = table.built.take();
    let state = TableState::from_cached(&key.prefix, table);
    let mut store = FactStore::from_cached(facts, &state.header().baseline_branch);
    if let Some(built) = built {
      store.seed_table(built.table(&key.prefix, &state.parts));
    }
    (state, store)
  }
}

/// The parts of a table a frame keeps: what the last sync committed, without what the rules derive of it
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedTable {
  pub header: TableHeader,
  pub order: Vec<String>,
  pub branches: BTreeMap<String, BranchView>,
  pub stacks: BTreeMap<String, StackBranchUpdate>,
  pub sections: TableSections,
  /// By original id, of each commit of the rows and of the unassigned ones
  pub identities: BTreeMap<String, CommitIdentity>,
  /// What the sync that committed the rows built of them; none for a frame of a table no sync of its run committed
  #[serde(default)]
  pub built: Option<CachedBuild>,
}

/// What the table of a sync (`BranchTable`) has beside the rows of the frame, whose branches, commits and stack
/// branches it rebuilds from them (`table`): the baseline, the virtual tips, the branches not built, the copy of each
/// commit of the rows (the one the sync wrote, else the one its mapping note named, which the commits of a frame don't
/// keep), the notes of the parked branches, what the sync knew of the remote branches and what the baseline has whole
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedBuild {
  pub baseline: String,
  pub tips: BTreeMap<String, String>,
  pub not_built: BTreeMap<String, BlockReason>,
  pub copies: BTreeMap<String, String>,
  pub parked: BTreeMap<String, String>,
  pub remote: RemoteFacts,
  pub in_baseline: BTreeSet<String>,
}

impl CachedBuild {
  /// What `table` built beside its rows
  #[must_use]
  pub fn of(table: &BranchTable) -> Self {
    let copies = table
      .branches
      .iter()
      .flat_map(|branch| &branch.commits)
      .filter_map(|commit| Some((commit.id.clone(), table.copy_of(commit)?.to_owned())))
      .collect();
    Self {
      baseline: table.baseline.clone(),
      tips: table.tips.iter().map(|(name, tip)| (name.clone(), tip.clone())).collect(),
      not_built: table.not_built.iter().map(|(name, reason)| (name.clone(), *reason)).collect(),
      copies,
      parked: table.parked.clone(),
      remote: table.remote.clone(),
      in_baseline: table.in_baseline.clone(),
    }
  }

  /// The table of the sync under `prefix` with the rows of `parts`: its branches in the order of the table, each with
  /// the head and the commits of its row, and the stack branches of the rows
  pub(crate) fn table(self, prefix: &str, parts: &TableParts) -> BranchTable {
    let branches = parts.order.iter().filter_map(|name| parts.branches.get(name)).map(grouped_of).collect();
    BranchTable {
      prefix: prefix.to_owned(),
      baseline: self.baseline,
      branches,
      tips: self.tips.into_iter().collect(),
      not_built: self.not_built.into_iter().collect(),
      copies: self.copies.into_iter().collect(),
      stack_branches: parts.stacks.values().cloned().collect(),
      parked: self.parked,
      remote: self.remote,
      in_baseline: self.in_baseline,
    }
  }
}

/// The grouped branch of a row, as the sync that built the row grouped it
fn grouped_of(row: &BranchView) -> GroupedBranchInfo {
  let head = &row.head;
  GroupedBranchInfo {
    name: head.name.clone(),
    commits: row.commits.clone(),
    latest_commit_time: head.latest_commit_time,
    summary: head.summary.clone(),
    description_title: head.description_title.clone(),
    all_commits_have_issue_references: head.all_commits_have_issue_references,
    my_email: head.my_email.clone(),
    origin: head.origin,
    issue_key: head.issue_key.clone(),
    stack: head.stack.clone(),
  }
}

/// What a frame keeps of a commit beside its row, whose view leaves it out: what finds the commit again on a HEAD that
/// moved since (`git_ops::landed_pairing::CommitKey`, the author's email, the author time of the row and the subject),
/// so a rewrite asked on the seeded table resolves the commits it names once the first sync commits
/// (`operations::rewrite`)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitIdentity {
  pub author_email: String,
  /// The first line of the message as git has it, the prefix kept
  pub subject: String,
}

impl CommitIdentity {
  /// The identity of `commit`
  #[must_use]
  pub fn of(commit: &Commit) -> Self {
    Self {
      author_email: commit.author_email.clone(),
      subject: commit.subject.clone(),
    }
  }

  /// `commit`, which a frame seeded without them, with this identity
  pub fn restore(self, commit: &mut Commit) {
    commit.author_email = self.author_email;
    commit.subject = self.subject;
  }
}

/// The facts of the remote services a frame keeps, as the view had them; the details of the robots go with the demand
/// of a window, so no frame keeps them
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedFacts {
  pub robots: RobotsView,
  pub patronus: PatronusPart,
  /// The landing needs of the last computation that answered them, with what they are of
  pub landing_needs: Option<CachedNeeds>,
  pub pull_requests: PullRequestsView,
  pub rules: BTreeMap<String, Fact<BranchRules>>,
  /// Of the target of the table's baseline: a value of another baseline is not kept
  pub profiles: Fact<PatronusStartOptions>,
  pub readiness: BTreeMap<String, Fact<SubjectReadiness>>,
  pub review_requests: ReviewRequestsView,
  pub auth: Fact<AuthStatus>,
}

/// The landing needs a frame keeps (`PatronusFact`): by branch, with the inputs they were computed of
/// (`facts::landing_inputs`, the hash of the binary that computed them: a binary that hashes otherwise computes them
/// again)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CachedNeeds {
  pub inputs: u64,
  pub needs: Needs,
}

/// The frame of the session of `key` with `state` and `facts`: the table as its last sync committed it (while a sync
/// runs, the one before it) and the facts as they stand
#[must_use]
pub fn frame_of(key: &SessionKey, state: &TableState, facts: &FactStore) -> CachedSession {
  let mut table = state.committed_frame();
  table.built = facts.table.value().map(|table| CachedBuild::of(table));
  CachedSession {
    schema: FRAME_SCHEMA,
    key: key.clone(),
    table,
    facts: facts.frame(),
  }
}

/// The key of the frame of `worktree` in the object cache: the hash of its path
#[must_use]
pub fn frame_key(worktree: &str) -> [u8; 32] {
  Sha256::digest(worktree.as_bytes()).into()
}

/// The frame of `worktree` in `cache`; `None` without one, of another schema, or unreadable
#[must_use]
pub fn read_frame(cache: &ObjectCache, worktree: &str) -> Option<CachedSession> {
  let text = cache.get_session_frame(FRAME_SCHEMA, &frame_key(worktree))?;
  serde_json::from_str(&text)
    .inspect_err(|error| debug!(worktree, "The frame of the session is unreadable: {error}"))
    .ok()
}

/// Keeps `frame` as the frame of its worktree in `cache`, in place of the one before; nothing for a disabled cache
pub fn write_frame(cache: &ObjectCache, frame: &CachedSession) {
  if !cache.is_enabled() {
    return;
  }
  match serde_json::to_string(frame) {
    Ok(text) => cache.put_session_frame(FRAME_SCHEMA, &frame_key(&frame.key.worktree), &text),
    Err(error) => debug!(worktree = %frame.key.worktree, "The frame of the session is not encodable: {error}"),
  }
}

#[cfg(test)]
#[path = "warm_start_test.rs"]
pub(crate) mod warm_start_test;
