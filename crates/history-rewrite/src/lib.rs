//! The rewrites of the checked out branch and what they read, without the sync, GitHub or Patronus: the grouping of
//! the commits ahead of the baseline, the stores of the issue groups, the stack parents and the parked branches, the
//! repository lock, and one core per rewrite (`*_core(params, options)`: the lock, the expected HEAD and the order of
//! the git steps). The app and `bd` call these cores through sync-core or directly, and run no git of their own
//! (AGENTS.md, "Adding a git operation").
//!
//! What a rewrite reads:
//! - `baseline`: `detect_baseline_branch`, the baseline outside a sync
//! - `commit_grouper`: the groups of the commits ahead of the baseline by prefix, issue group and issue key
//! - `stack`: stacked branches, the parents a prefix declares, the forest and where each branch goes
//! - `snapshot`: the commits ahead of the baseline and what groups them, read once per operation
//! - `commit_ids`: the commit ids of a request, abbreviated or full, among the commits ahead of the baseline
//!
//! Stores:
//! - `store_lines`: `LineStore`, a map of `key<TAB>value` lines in a ref of its own
//! - `issue_groups`: `ISSUE_GROUPS`, issue key -> branch name
//! - `stack_parents`: `STACK_PARENTS`, branch -> stack parent
//! - `parked`: `PARKED`, branch -> note: the branches kept local, never pushed; park and unpark
//!
//! Rewrites, each under the repository lock its core takes:
//! - `repository_lock`: `$GIT_COMMON_DIR/branch-deck.lock`, `RewriteOptions` and `ensure_expected_head`
//! - `regroup`: one reword that sets the prefixes of a plan, with the issue groups and the stack parents, and its undo
//! - `move_commits`: the app's move of commits to a branch, through the regroup
//! - `move_plan`: the smallest regroup of a move: by the issue groups, by prefixes, or both
//! - `reword`: new messages of commits ahead of the baseline as a regroup (`bd reword`, the app's edit of a message),
//!   with its undo, and the writes of every regroup
//! - `fixup_targets`: the fixups a rewrite of messages may not fold elsewhere
//! - `squash`: commits into the oldest of them, and the undo
//! - `autosquash`: the `fixup!`, `squash!` and `amend!` commits into their targets (`bd fold`)

pub mod autosquash;
pub mod baseline;
pub mod commit_grouper;
pub mod commit_ids;
mod fixup_targets;
pub mod issue_groups;
pub mod move_commits;
pub mod move_plan;
pub mod parked;
pub mod regroup;
pub mod repository_lock;
pub mod reword;
pub mod snapshot;
pub mod squash;
pub mod stack;
pub mod stack_parents;
pub mod store_lines;
#[cfg(any(test, feature = "test-commit"))]
pub mod test_commit;

#[cfg(test)]
mod baseline_test;
