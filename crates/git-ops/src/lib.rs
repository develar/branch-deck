//! The git of Branch Deck on objects and refs, one process per phase, never the working tree except the guarded update
//! (docs/design/git-operations.md). A module owns one operation or one reader or writer that operations share; the
//! repository lock, the expected HEAD and the order of the steps are sync-core's (AGENTS.md, "Adding a git operation").
//!
//! Reading the repository:
//! - `revs`: the worktree, HEAD and its branch, commits by name, ref tips, ancestry and merge bases, `short_id`,
//!   `is_object_id`
//! - `history`: the first-parent history of HEAD a rewrite works on, never the whole history
//! - `commit_list`: the commits ahead of the baseline, streamed from one `log`
//! - `commit_utils`: commits by id from one `log --no-walk --stdin`; the empty tree and blob ids without a process
//! - `cat_file_batch`: many objects from one `cat-file --batch --buffer -Z`, streamed; types from `--batch-check`
//! - `object_texts`: the bytes, texts and tree entries of many names, through `cat_file_batch`
//! - `paths`: the paths two trees differ in, as bytes; the commits that change some paths
//! - `notes`: the mapping notes of the sync (original commit -> its virtual copy) and the notes of many commits
//! - `attributes`: the git attributes of paths at a tree-ish, one `check-attr --stdin -z`
//! - `branch_description`: `branch.<name>.description`, the pull request text of a virtual branch
//! - `ownership`: the code owners of paths by the `OWNERSHIP` files of a tree
//! - `commit_files`: the changed files of commits, their patches cut as they stream, blob contents for the diff view
//! - `tree_archive`: the files of a commit's tree as a tar
//! - `model`: the types of a sync and its conflicts, the branch namespaces under the prefix
//!
//! Writing objects and refs:
//! - `ref_transaction`: `RefTransaction`, every ref move in one `update-ref --stdin` with compare-and-swap
//! - `commit_writer`: commits, store commits and notes from one `fast-import`, with exact metadata, and temporary refs
//!
//! The copies of a sync:
//! - `copy_commit`: one commit of a virtual branch, reused by its mapping note or cherry-picked
//! - `cherry_pick`: the tree of a cherry-pick by `merge-tree --write-tree`
//! - `commit_chain`: `CommitChain`, one `merge-tree --stdin` and one `fast-import` per branch
//! - `cache`: `TreeIdCache`, the trees and dates a sync reads once, with the merge memo
//! - `merge_tree_output`: the `-z` output of `merge-tree --write-tree`, and one merge of a `--stdin` run
//! - `merge_memo`: `MergeOutcome` and the `MergeMemo` of merges, commit trees and conflict views
//! - `merge_conflict`: the report of a conflicting cherry-pick, its markers and its view
//! - `conflict_analysis`: the missing commits, the divergence and the file contents of a conflict view
//!
//! Rewrites of the checked out branch:
//! - `rewrite_guard`: the guards of a rewrite, `HeadMoved`, `IndexUpdate` of the user's index
//! - `rewrite_plan`: drop commits or replace them (unapply, squash), the kept ones replayed
//! - `replay`: `git replay` of a range and `merge-tree` of three trees, objects only
//! - `reword_commits`: new messages for many commits, one `fast-import`
//! - `amend_operations`: uncommitted changes into a commit, also an older one
//! - `squash_commits`: commits into older ones, and the undo of a rewrite (`restore_branch_head`)
//! - `move_hunk`: the change of a file from one commit into another (`bd move-hunk`)
//!
//! Update Project:
//! - `update_branch`: the check and the apply of an update onto the upstream, and its undo
//! - `worktree_update`: the one write of the working tree, guarded
//! - `update_resolve`: the resolve sessions of `bd update`, one round per conflicting commit
//! - `resolve_check`: the checks of a resolved file of a round

pub mod amend_operations;
pub mod attributes;
pub mod branch_description;
pub mod cache;
pub mod cat_file_batch;
pub mod cherry_pick;
pub mod commit_chain;
pub mod commit_files;
pub mod commit_list;
pub mod commit_utils;
pub mod commit_writer;
pub mod conflict_analysis;
pub mod copy_commit;
pub mod history;
pub mod merge_conflict;
pub mod merge_memo;
pub mod merge_tree_output;
pub mod model;
pub mod move_hunk;
pub mod notes;
pub mod object_texts;
pub mod ownership;
pub mod paths;
pub mod ref_transaction;
pub mod replay;
pub mod resolve_check;
pub mod revs;
pub mod reword_commits;
pub mod rewrite_guard;
pub mod rewrite_plan;
pub mod squash_commits;
pub mod tree_archive;
pub mod update_branch;
pub mod update_resolve;
pub mod worktree_update;

#[cfg(test)]
mod amend_operations_test;

#[cfg(test)]
mod branch_description_test;

#[cfg(test)]
mod cherry_pick_test_utils;

#[cfg(test)]
mod commit_files_test;

#[cfg(test)]
mod commit_utils_test;

#[cfg(test)]
mod conflict_analysis_tests;

#[cfg(test)]
mod copy_commit_test;

#[cfg(test)]
mod git_command_test;

#[cfg(test)]
mod history_test;

#[cfg(test)]
mod merge_conflict_tests;

#[cfg(test)]
mod move_hunk_test;

#[cfg(test)]
mod notes_test;

#[cfg(test)]
mod object_texts_test;

#[cfg(test)]
mod ownership_test;

#[cfg(test)]
mod rewrite_plan_test;

#[cfg(test)]
mod rewrite_test_utils;

#[cfg(test)]
mod squash_commits_test;
