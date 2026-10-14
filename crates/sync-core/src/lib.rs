pub mod add_issue_reference;
pub mod amend_to_branch;
pub mod autosquash;
pub mod baseline;
pub mod baseline_fetch;
pub mod branch_prefix;
mod branch_processor;
pub mod check;
pub mod commit_grouper;
pub mod commit_ids;
pub mod delete_archived_branch;
pub mod grouping;
pub mod issue_groups;
pub mod issue_navigation;
pub mod landed;
pub mod move_commits;
pub mod patronus;
pub mod pull_requests;
pub mod push;
pub mod push_lease;
pub mod regroup;
pub mod remote_command;
pub mod remote_status;
pub mod remote_url;
pub mod repository_lock;
pub mod repository_validation;
pub mod reword_commit;
pub mod squash;
pub mod stack;
pub mod stack_operations;
pub mod sync;
pub mod transfer_progress;
pub mod unapply_branch;
pub mod uncommitted_changes;
pub mod update_project;

#[cfg(test)]
mod branch_prefix_test;
#[cfg(test)]
mod patronus_test;
#[cfg(test)]
mod pull_requests_test;
#[cfg(test)]
mod push_test;
#[cfg(test)]
mod remote_command_test;
#[cfg(test)]
mod sync_test;
#[cfg(test)]
mod test_commit;
#[cfg(test)]
mod unapply_branch_test;
#[cfg(test)]
mod uncommitted_changes_test;
