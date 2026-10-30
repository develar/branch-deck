//! The git of Branch Deck on objects and refs, one process per phase, never the working tree except the guarded update
//! (docs/design/git-operations.md). A module owns one operation or one reader or writer that operations share; the
//! repository lock, the expected HEAD and the order of the steps are the cores' (`history-rewrite`, `sync-core`,
//! `update-core`, `trial-check`; docs/contributing/adding-git-operation.md, "Checklist").
//!
//! Reading the repository:
//! - `revs`: the worktree, HEAD and its branch, commits by name, ref tips, the hash of the refs and the config,
//!   ancestry and merge bases, `short_id`,
//!   `is_object_id`
//! - `history`: the first-parent history of HEAD a rewrite works on, never the whole history
//! - `commit_list`: the commits ahead of the baseline (`domain_types::commit::Commit`), streamed from one `log`
//! - `commit_utils`: commits by id from one `log --no-walk --stdin`; the empty tree and blob ids without a process
//! - `cat_file_batch`: many objects from one `cat-file --batch --buffer -Z`, streamed; types from `--batch-check`
//! - `object_texts`: the bytes, texts and tree entries of many names, through `cat_file_batch`
//! - `paths`: the paths two trees differ in, as bytes; the commits that change some paths
//! - `notes`: the mapping notes of the sync (original commit -> its virtual copy) and the notes of many commits, with
//!   their committer times and subjects; the source and the resolve a note records are `domain_types::commit`'s
//! - `attributes`: the git attributes of paths at a tree-ish, one `check-attr --stdin -z`
//! - `branch_description`: `branch.<name>.description`, the pull request text of a virtual branch
//! - `ownership`: the code owners of paths by the `OWNERSHIP` files of a tree
//! - `commit_files`: the changed files of commits, their patches cut as they stream, blob contents for the diff view
//! - `tree_archive`: the files of a commit's tree as a tar
//! - `push_contents`: the files a push would send, with their sizes, from the diffs of its commits
//!
//! Writing objects and refs:
//! - `ref_transaction`: `RefTransaction`, every ref move in one `update-ref --stdin` with compare-and-swap
//! - `commit_writer`: commits, store commits and notes from one `fast-import`, with exact metadata, and temporary refs
//! - `reviewed_refs`: the base and the tip a local branch had when the user marked it reviewed
//! - `carried_review`: the reviewed change of a branch carried onto its base now, for the diff since the review
//!   (`refs/branch-deck/reviewed/`), listed with the branches and written with compare-and-swap
//!
//! The copies of a sync:
//! - `copy_commit`: one commit of a virtual branch, reused by its mapping note or cherry-picked
//! - `cherry_pick`: the tree of a cherry-pick by `merge-tree --write-tree`
//! - `commit_chain`: `CommitChain`, one `merge-tree --stdin` and one `fast-import` per branch
//! - `tree_id_cache`: `TreeIdCache`, the trees and dates a sync reads once, with the merge memo
//! - `merge_tree_output`: the `-z` output of `merge-tree --write-tree`, and one merge of a `--stdin` run
//! - `merge_memo`: `MergeOutcome` and the `MergeMemo` of merges, commit trees and conflict views
//! - `merge_conflict`: the report of a conflicting cherry-pick, its markers and its view
//! - `conflict_analysis`: the missing commits, the divergence and the file contents of a conflict view (the types are
//!   `domain_types::conflict`'s)
//! - `word_resolve`: the resolve by words of a conflicting cherry-pick, all or nothing
//!
//! Rewrites of the checked out branch:
//! - `rewrite_guard`: the guards of a rewrite, `HeadMoved`, `IndexUpdate` of the user's index
//! - `rewrite_plan`: drop commits or replace them (the drop of an unapply, squash), the kept ones replayed
//! - `replay`: `git replay` of a range and `merge-tree` of three trees, objects only
//! - `reword_commits`: new messages for many commits, one `fast-import`
//! - `amend_operations`: uncommitted changes into a commit, also an older one
//! - `squash_commits`: commits into older ones, and the undo of a rewrite (`restore_branch_head`)
//! - `move_hunk`: the change of a file from one commit into another (`bd move-hunk`)
//!
//! Update Project:
//! - `update_branch`: the check and the apply of an update onto the upstream, and its undo (the types are
//!   `domain_types::update_project`'s)
//! - `worktree_update`: the one write of the working tree, guarded
//! - `index_lock`: the wait of a write of the user's index for the lock of another process, the removal of a lock no
//!   process owns
//! - `update_resolve`: the resolve sessions of `bd update`, one round per conflicting commit
//! - `resolve_check`: the checks of a resolved file of a round
//! - `word_replay`: the local commits replayed onto the upstream, each conflict resolved by words, all or nothing
//! - `landed_pairing`: a local commit paired with the upstream commit that carries its change, by identity
//!   (`CommitKey`, the walks by author), by patch id and by the ladder of proofs of an update, before its replay and
//!   after a conflict (`pair_landed`); also for the landed report, the remote status, the robot commit map and the pick
//! - `autosquash_targets`: the target of each fixup with git's autosquash rules, pure
//! - `pick`: the commits of another worktree landed on the checked out branch (`bd pick`), the worktree following
//! - `unapply`: the commits of a virtual branch out of the checked out branch, the files following, and its undo
//! - `reapply`: the commits of an unapplied branch back on top, their prefixes restored, the files following, its undo
//!
//! Safe Push:
//! - `trial_rebase`: the rebase of a branch onto the target tip that Patronus runs, tried on objects only
//!
//! The lease pool:
//! - `lease_slot`: the git of a slot of `bd lease`, the one other writer of a working tree: a slot is pool property
//! - `worktree_snapshot`: the tree of the files of a worktree as `git add --all` stages them, in a copy of its index,
//!   and the patch between two trees into a file (the scans of a host slot, the commit of a working tree)
//! - `host_client`: the git of a client of a remote run: the commit it ships, the commits the host may have, the thin
//!   pack into a private file, and the apply of the changes of the command to the working tree; of Unix only, as its
//!   one caller, host-client
//! - `host_objects`: the commits a host has, the pack of a client into its object store, the refs that keep the
//!   commits of the runs (`refs/branch-deck/host/`), the blob text and the tree ids of a commit (`bd-host`)

pub mod amend_operations;
pub mod attributes;
pub mod autosquash_targets;
pub mod branch_description;
pub mod carried_review;
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
#[cfg(unix)]
pub mod host_client;
pub mod host_objects;
pub mod index_lock;
pub mod landed_pairing;
pub mod lease_slot;
pub mod merge_conflict;
pub mod merge_memo;
pub mod merge_tree_output;
pub mod move_hunk;
pub mod notes;
pub mod object_texts;
pub mod ownership;
pub mod paths;
pub mod pick;
pub mod push_contents;
pub mod reapply;
pub mod ref_transaction;
pub mod replay;
pub mod resolve_check;
pub mod reviewed_refs;
pub mod revs;
pub mod reword_commits;
pub mod rewrite_guard;
pub mod rewrite_plan;
pub mod squash_commits;
pub mod tree_archive;
pub mod tree_id_cache;
pub mod trial_rebase;
pub mod unapply;
pub mod update_branch;
pub mod update_resolve;
pub mod word_replay;
pub mod word_resolve;
pub mod worktree_snapshot;
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

#[cfg(all(test, unix))]
mod host_client_test;

#[cfg(test)]
mod host_objects_test;

#[cfg(test)]
mod worktree_snapshot_test;

#[cfg(test)]
mod history_test;

#[cfg(test)]
mod lease_slot_test;
#[cfg(test)]
mod merge_conflict_tests;

#[cfg(test)]
mod move_hunk_test;

#[cfg(test)]
mod notes_test;

#[cfg(test)]
mod object_texts_test;

#[cfg(test)]
mod carried_review_test;

#[cfg(test)]
mod ownership_test;

#[cfg(test)]
mod push_contents_test;

#[cfg(test)]
mod ref_transaction_test;

#[cfg(test)]
mod reviewed_refs_test;

#[cfg(test)]
mod rewrite_plan_test;

#[cfg(test)]
mod rewrite_test_utils;

#[cfg(test)]
mod squash_commits_test;
