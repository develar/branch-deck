//! The operations of Branch Deck over git-ops: the sync of the virtual branches, one core per operation
//! (`*_core(params, options)`: the repository lock, the expected HEAD and the order of the git steps) and the pushes.
//! It talks to no remote service but through git: what GitHub and Patronus know of the branches (pull requests, review
//! requests, branch rules, review readiness, the Patronus command, the Safe Push statistics) is the crate
//! `remote-facts`, over HTTP, built on this one. The app and `bd` call these cores, git-ops directly only for reads that
//! need no lock, and run no git of their own (docs/contributing/adding-git-operation.md, "Checklist").
//!
//! The grouping, the stores of the issue groups, the stack parents and the parked branches, the repository lock and the
//! rewrites that need no sync (regroup, reword, squash, fold) are the crate `history-rewrite`.
//! Update Project, `bd pick` and the resolve sessions of a conflicting `bd update` are the crate `update-core`; the
//! baseline, its fetch and the update check every sync runs stay here (`baseline`, `baseline_fetch`), as does the
//! landed report an update records (`landed`). The trial checks (`bd check`, the conflict fix, the landing gate),
//! `bd move-hunk` and everything that reads the diffs and the symbols of the commits are the crate `trial-check`; it
//! builds with `branch_processor` and `cache_memo` of this crate.
//! The lease pool of agent worktrees (`bd lease`), its clean rules and the agent workspace (`bd workspace`) are the crate
//! `lease-pool`; it reads `repository_config`, `uncommitted_changes` and `baseline` of this crate.
//!
//! The sync:
//! - `sync`: `sync_branches`, the guards, the grouping, one build per branch and the detection of the archived ones
//! - `branch_processor`: the build of one virtual branch, then its ref move with compare-and-swap and its remote status
//! - `repo_facts`: `RepoFacts`, what a sync reads once at its start: the baseline, HEAD, the branches under the prefix
//! - `sync_inputs`: what a sync reads (`read_sync_inputs`), so that a focus syncs only when it changed
//! - `grouping`: the grouping of a sync without its builds, for the reads of `bd`
//! - `stack_branch`: the stack branch `{prefix}/stack/<root>` of a stack that forks: all its commits in one branch
//! - `issue_navigation`: the issue key patterns of `.idea/vcs.xml`
//! - `remote_status`: what a virtual branch has that its remote branch lacks and the reverse, by content
//! - `commit_owners`: where the commits of a sync go, so a conflict names the branch of each commit it likely needs
//! - `cache_memo`: the merges, commit trees and conflict views in the object cache
//! - `conflict_chain`: the copies of a conflicting branch before its conflict, kept by a ref and remembered
//! - `landed`: the user's commits that reached the baseline, before and after an update (the pairing with the upstream
//!   commits is `git_ops::landed_pairing`)
//! - `baseline`: the remote branch the baseline tracks, its fetch, and the update check every sync runs, without the
//!   network
//! - `baseline_fetch`: the fetch of the baseline without an update
//! - `fetch_lock`: the lock of the fetches of the baseline, `fetch.lock`: a second fetch of a tracking ref, of any
//!   process, waits for the first one and takes its result
//!
//! Rewrites of the checked out branch, each under the repository lock its core takes:
//! - `add_issue_reference`: an issue key into the subjects of commits
//! - `amend_to_branch`: uncommitted changes into a commit of a branch
//! - `unapply_branch`: the commits of a virtual branch out of HEAD with their files, the branch kept as an unapplied one
//! - `reapply_branch`: the commits of an unapplied branch back on top of HEAD with their prefixes and files
//! - `delete_archived_branch`: the delete of an archived branch, with compare-and-swap on its tip
//! - `parked`: park and unpark a branch (no rewrite: the store `refs/branch-deck/parked`), and whether a branch is parked
//!
//! Stores and config:
//! - `config_snapshot`: `ConfigSnapshot`, the `branchdeck.*` keys and the branch descriptions from one `git config`;
//!   the symbol mode (`SymbolMode`)
//! - `github_repository`: the GitHub repository of the branches, from the config, `origin` and the Space mirror, no HTTP
//! - `repository_config`: the `branchdeck.*` config that Settings shows and writes
//! - `branch_prefix`: the branch prefix by git config and the app setting
//!
//! Remotes:
//! - `remote_command`: the policy of every git that talks to a remote (push, fetch)
//! - `transfer_progress`: the progress of a fetch or a push, from its stderr lines
//! - `moved_remote`: the fetch of the remote branches whose pull request shows a head their remote-tracking ref lacks
//!   (someone else pushed to them), with their remote statuses against what it fetched
//! - `push_lease`: the lease of a virtual branch push and `push_refs`, shared by the app and `bd push`
//! - `push`: the app's push of a virtual branch
//! - `push_size`: the size check of every push, refused over `branchdeck.pushSizeLimit` before the network
//! - `baseline_push`: the push of a copy into the branch of the baseline on its remote, a fast-forward by the push URL,
//!   where the repository lands by a push (`branchdeck.landing=push`)
//! - `remote_url`: the host and the repository path of a remote URL
//! - `review_fetch`: the commits of a pull request to review, into `refs/branch-deck/review/<n>/`
//!
//! Patronus, by the content of the repository (no HTTP):
//! - `patronus`: the Space repository the virtual branches are pushed to, the commits and the paths of a review
//! - `patronus_start`: whether a robot runs and pushes, the modes a profile allows, and the push that fails a running
//!   Safe Push (which robot is a branch's is `domain_policy::robot_match`; whether a start can begin is
//!   `repository_model::patronus_view`)
//! - (the crate `repository-model` derives what the robots land, landed and hold of the branch table from these)
//! - `robot_commits`: the commits a robot checked, by the hashes of the checked out branch
//! - `robot_landing`: what a merged robot landed of the branches, by content: the commits it tested, and those after
//! - `target_conflict`: the check that Patronus can rebase what a robot lands onto the newest target
//!
//! The rest:
//! - `branch_review`: the stack and the commits of a branch review
//! - `branch_name_suggestion`: branch names for commits from the user's Claude Code
//! - `git_setup`: the branch prefix the git identity suggests, which a session without a prefix answers
//! - `repository_validation`: whether a path is the working tree of a repository, without git
//! - `uncommitted_changes`: the uncommitted files and the diff of one of them

pub mod add_issue_reference;
pub mod amend_to_branch;
pub mod baseline;
pub mod baseline_fetch;
pub mod baseline_push;
pub mod branch_name_suggestion;
pub mod branch_prefix;
pub mod branch_processor;
pub mod branch_review;
pub mod cache_memo;
pub mod commit_owners;
pub mod config_snapshot;
pub mod conflict_chain;
pub mod delete_archived_branch;
pub mod fetch_lock;
pub mod git_setup;
pub mod github_repository;
pub mod grouping;
pub mod issue_navigation;
pub mod landed;
pub mod moved_remote;
pub mod parked;
pub mod patronus;
pub mod patronus_start;
pub mod push;
pub mod push_lease;
pub mod push_size;
pub mod reapply_branch;
pub mod remote_command;
pub mod remote_status;
pub mod remote_url;
pub mod repo_facts;
pub mod repository_config;
pub mod repository_validation;
pub mod review_fetch;
pub mod robot_commits;
pub mod robot_landing;
pub mod stack_branch;
pub mod sync;
pub mod sync_inputs;
pub mod target_conflict;
pub mod transfer_progress;
pub mod unapply_branch;
pub mod uncommitted_changes;

#[cfg(test)]
mod branch_name_suggestion_test;
#[cfg(test)]
mod branch_prefix_test;
#[cfg(test)]
mod branch_review_test;
#[cfg(test)]
mod conflict_report_test;
#[cfg(test)]
mod detection_archive_step_test;
#[cfg(test)]
mod detection_integration_test;
#[cfg(test)]
mod detection_merge_test;
#[cfg(test)]
mod detection_rebase_test;
#[cfg(test)]
mod detection_recheck_test;
#[cfg(test)]
mod detection_remote_status_test;
#[cfg(test)]
mod detection_squash_test;
#[cfg(test)]
mod detection_stack_test;
#[cfg(test)]
mod detection_test_helpers;
#[cfg(test)]
mod git_setup_test;
#[cfg(test)]
mod github_repository_test;
#[cfg(test)]
mod issue_groups_test;
#[cfg(test)]
mod move_commits_test;
#[cfg(test)]
mod parked_test;
#[cfg(test)]
mod patronus_start_test;
#[cfg(test)]
mod patronus_test;
#[cfg(test)]
mod process_budget_test;
#[cfg(test)]
mod push_fixture;
#[cfg(test)]
mod push_size_test;
#[cfg(test)]
mod push_test;
#[cfg(test)]
mod reapply_branch_test;
#[cfg(test)]
pub(crate) mod regroup_test;
#[cfg(test)]
mod remote_command_test;
#[cfg(test)]
mod repository_config_test;
#[cfg(test)]
mod repository_lock_test;
#[cfg(test)]
mod reword_test;
#[cfg(test)]
mod stack_operations_test;
#[cfg(test)]
mod stack_parents_test;
#[cfg(test)]
mod sync_inputs_test;
#[cfg(test)]
mod sync_test;
#[cfg(test)]
mod unapply_branch_test;
#[cfg(test)]
mod uncommitted_changes_test;
