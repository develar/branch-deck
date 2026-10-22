//! The operations of Branch Deck over git-ops: the sync of the virtual branches, one core per operation
//! (`*_core(params, options)`: the repository lock, the expected HEAD and the order of the git steps), the trial syncs
//! of `bd check`, Update Project, the pushes and what GitHub and Patronus know of the branches. The app and `bd` call
//! these cores, git-ops directly only for reads that need no lock, and run no git of their own (AGENTS.md, "Adding a
//! git operation").
//!
//! The grouping, the stores of the issue groups, the stack parents and the parked branches, the repository lock and the
//! rewrites that need no sync (regroup, reword, squash, fold) are the crate `history-rewrite`.
//!
//! The sync:
//! - `sync`: `sync_branches`, the guards, the grouping, one build per branch and the detection of the archived ones
//! - `branch_processor`: the build of one virtual branch, then its ref move with compare-and-swap and its remote status
//! - `repo_facts`: `RepoFacts`, what a sync reads once at its start: the baseline, HEAD, the branches under the prefix
//! - `sync_inputs`: the fingerprint of what a sync reads, so that a focus syncs only when it changed
//! - `grouping`: the grouping of a sync without its builds, for the reads of `bd`
//! - `stack_branch`: the stack branch `{prefix}/stack/<root>` of a stack that forks: all its commits in one branch
//! - `issue_navigation`: the issue key patterns of `.idea/vcs.xml`
//! - `remote_status`: what a virtual branch has that its remote branch lacks and the reverse, by content
//! - `commit_walk`: one `rev-list` walk of commits that a caller matches across a rebase by `CommitKey`
//! - `commit_key`: the identity of a commit across a rebase and an amend
//! - `missing_commits`: the commits a conflicting commit likely needs, with their branches
//! - `cache_memo`: the merges, commit trees and conflict views in the object cache
//! - `conflict_chain`: the copies of a conflicting branch before its conflict, kept by a ref and remembered
//! - `landed`: the user's commits that reached the baseline, before and after an update
//! - `baseline_fetch`: the fetch of the baseline without an update
//!
//! Rewrites of the checked out branch, each under the repository lock its core takes:
//! - `stack_operations`: rename a branch, stack it on another one or make it a root, through the regroup
//! - `add_issue_reference`: an issue key into the subjects of commits
//! - `amend_to_branch`: uncommitted changes into a commit of a branch
//! - `unapply_branch`: the commits of a virtual branch out of HEAD with their files, the branch kept as an unapplied one
//! - `reapply_branch`: the commits of an unapplied branch back on top of HEAD with their prefixes and files
//! - `move_hunk`: the checks of `bd move-hunk` against the commits ahead of the baseline
//! - `delete_archived_branch`: the delete of an archived branch, with compare-and-swap on its tip
//! - `parked`: park and unpark a branch (no rewrite: the store `refs/branch-deck/parked`), and whether a branch is parked
//!
//! Stores and config:
//! - `config_snapshot`: `ConfigSnapshot`, the `branchdeck.*` keys and the branch descriptions from one `git config`
//! - `repository_config`: the `branchdeck.*` config that Settings shows and writes
//! - `branch_prefix`: the branch prefix by git config and the app setting
//!
//! The check, trial syncs that write objects only:
//! - `check`: `CheckSession`, the trial sync of a regroup plan
//! - `conflict_fix`: the regroup after which the conflicting branches apply cleanly, found by trial builds
//! - `dangling_references`: the declarations a branch removes while its tree still uses them, and by the summaries
//!   (`symbol_dependencies::declaration_checks`) those it changes or declares again
//! - `dangling_fix`: the fix of a mutual dangling reference, offered when a trial build proves it
//! - `inverted_stack`: a branch that needs a commit of a branch stacked on it, and its fixes
//! - `missing_declarations`: the names a built branch uses that a commit outside it adds
//! - `landing_gate`: what refuses the start of a Safe Push or a Dry Run of a `domain_types::landing::Subject`, for
//!   `bd safe-push` and the app: a name what lands uses and a branch outside it adds, a dangling reference a commit
//!   proves
//! - `dependencies`: which commits depend on which, from the hunks of `diff_scan`
//! - `diff_scan`: the changes of many commits from one `log --raw -z` and the blocks of the diff views
//! - `symbol_scan`: the syntax summaries of the changed source files, by blob id
//! - `symbol_dependencies`: the edges between commits by the names they declare and use
//! - `module_roots`: where the modules of source files begin
//!
//! Update Project:
//! - `update_project`: the fetch and the update onto the baseline, and the check every sync runs
//! - `pick`: the commits of another worktree landed on the checked out branch (`bd pick`), and the undo
//! - `resolve_session`: the resolve sessions of a conflicting `bd update`
//! - `resolve_copies`: the copies of a round that the agent edits, the only files of a session
//!
//! Remotes:
//! - `remote_command`: the policy of every git that talks to a remote (push, fetch)
//! - `transfer_progress`: the progress of a fetch or a push, from its stderr lines
//! - `push_lease`: the lease of a virtual branch push and `push_refs`, shared by the app and `bd push`
//! - `push`: the app's push of a virtual branch
//! - `remote_url`: the host and the repository path of a remote URL
//! - `review_fetch`: the commits of a pull request to review, into `refs/branch-deck/review/<n>/`
//!
//! GitHub:
//! - `pull_requests`: the pull request status of the virtual branches, from one GraphQL search
//! - `pull_request_create`: a new pull request of a virtual branch
//! - `pull_request_text`: the title and the body of a new pull request of a virtual branch, from its commits
//! - `stack_pull_request`: the title and the body of the pull request of the whole stack, from what the sync read
//! - `pull_request_writes`: the base of a stacked pull request, reviews, reviewers and the closing of landed ones
//! - `review_requests`: the pull requests of others that wait for the user's review
//! - `branch_rules`: the rules GitHub applies to pull requests into a branch
//!
//! Patronus:
//! - `patronus`: the Space repository the virtual branches are pushed to
//! - `patronus_command`: the `/patronus` comment of a pull request
//! - `patronus_review`: the commits of the code-owner pre-check of a Safe Push
//! - `patronus_start`: the robot of a branch or of the whole stack, and whether a Safe Push or a Dry Run can start
//! - (the crate `repository-view` derives what the robots land, landed and hold of the branch table from these)
//! - `review_readiness`: the review readiness of branches, shared by the app and `bd safe-push`
//! - `robot_commits`: the commits a robot checked, by the hashes of the checked out branch
//! - `robot_landing`: what a merged robot landed of the branches, by content: the commits it tested, and those after
//! - `robot_stats`: the Safe Push statistics: the robots of the last 70 days, kept in the object cache, read once
//! - `robot_stats_aggregate`: what the statistics count of them, pure (moves to `domain-policy` in s8)
//! - `target_conflict`: the check that Patronus can rebase what a robot lands onto the newest target
//!
//! The lease pool:
//! - `lease`: the slots agent sessions lease (`bd lease`), their state and lock in `$GIT_COMMON_DIR/branch-deck/`
//! - `agent_workspace`: where a coding agent works and how it writes a subject (`bd workspace`), before its first edit
//! - `lease_clean`: the clean rules of `.branchdeck.toml` that a return and `bd lease clean` apply to a slot
//! - `clone_dir`: a directory tree by one APFS clone, the new slot of a get on macOS
//!
//! The rest:
//! - `branch_review`: the stack and the commits of a branch review
//! - `branch_name_suggestion`: branch names for commits from the user's Claude Code
//! - `git_setup`: git, the identity and a branch prefix to suggest, checked before the first run
//! - `repository_validation`: whether a path is the working tree of a repository, without git
//! - `uncommitted_changes`: the uncommitted files and the diff of one of them

pub mod add_issue_reference;
pub mod agent_workspace;
#[cfg(test)]
mod agent_workspace_test;
pub mod amend_to_branch;
pub mod baseline_fetch;
pub mod branch_name_suggestion;
pub mod branch_prefix;
mod branch_processor;
pub mod branch_review;
pub mod branch_rules;
mod cache_memo;
pub mod check;
pub mod clone_dir;
pub mod commit_key;
pub mod commit_walk;
pub mod config_snapshot;
mod conflict_chain;
pub mod conflict_fix;
pub mod dangling_fix;
pub mod dangling_references;
pub mod delete_archived_branch;
pub mod dependencies;
pub mod diff_scan;
pub mod git_setup;
pub mod grouping;
pub mod inverted_stack;
pub mod issue_navigation;
pub mod landed;
pub mod landing_gate;
pub mod lease;
pub mod lease_clean;
pub mod missing_commits;
pub mod missing_declarations;
pub mod module_roots;
pub mod move_hunk;
pub mod parked;
pub mod patronus;
pub mod patronus_command;
pub mod patronus_review;
pub mod patronus_start;
pub mod pick;
pub mod pull_request_create;
pub mod pull_request_text;
pub mod pull_request_writes;
pub mod pull_requests;
pub mod push;
pub mod push_lease;
pub mod reapply_branch;
pub mod remote_command;
pub mod remote_status;
pub mod remote_url;
pub mod repo_facts;
pub mod repository_config;
pub mod repository_validation;
pub mod resolve_copies;
pub mod resolve_session;
pub mod review_fetch;
pub mod review_readiness;
pub mod review_requests;
pub mod robot_commits;
pub mod robot_landing;
pub mod robot_stats;
pub mod robot_stats_aggregate;
pub mod stack_branch;
pub mod stack_operations;
pub mod stack_pull_request;
pub mod symbol_dependencies;
pub mod symbol_scan;
pub mod sync;
pub mod sync_inputs;
pub mod target_conflict;
pub mod transfer_progress;
pub mod unapply_branch;
pub mod uncommitted_changes;
pub mod update_project;

#[cfg(test)]
mod branch_name_suggestion_test;
#[cfg(test)]
mod branch_prefix_test;
#[cfg(test)]
mod branch_review_test;
#[cfg(test)]
mod branch_rules_test;
#[cfg(test)]
mod conflict_report_test;
#[cfg(test)]
mod dependencies_test;
#[cfg(test)]
mod diff_scan_test;
#[cfg(test)]
mod git_setup_test;
#[cfg(test)]
mod issue_groups_test;
#[cfg(test)]
mod move_commits_test;
#[cfg(test)]
mod parked_test;
#[cfg(test)]
mod patronus_command_test;
#[cfg(test)]
mod patronus_start_test;
#[cfg(test)]
mod patronus_test;
#[cfg(test)]
mod process_budget_test;
#[cfg(test)]
mod pull_request_create_test;
#[cfg(test)]
mod pull_request_writes_test;
#[cfg(test)]
mod pull_requests_test;
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
mod resolve_copies_test;
#[cfg(test)]
mod review_readiness_test;
#[cfg(test)]
mod reword_test;
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

#[cfg(test)]
mod module_roots_test;

#[cfg(test)]
mod symbol_dependencies_test;

#[cfg(test)]
mod symbol_scan_test;
#[cfg(test)]
mod word_resolve_sync_test;
