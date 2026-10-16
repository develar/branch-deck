pub mod add_issue_reference;
pub mod amend_to_branch;
pub mod autosquash;
pub mod baseline;
pub mod baseline_fetch;
pub mod branch_name_suggestion;
pub mod branch_prefix;
mod branch_processor;
pub mod branch_review;
pub mod check;
pub mod commit_grouper;
pub mod commit_ids;
pub mod commit_key;
pub mod dangling_references;
pub mod delete_archived_branch;
pub mod dependencies;
pub mod diff_scan;
pub mod git_setup;
pub mod grouping;
pub mod issue_groups;
pub mod issue_navigation;
pub mod landed;
pub mod move_commits;
pub mod move_plan;
pub mod patronus;
pub mod patronus_command;
pub mod patronus_review;
pub mod patronus_start;
pub mod pull_request_writes;
pub mod pull_requests;
pub mod push;
pub mod push_lease;
pub mod regroup;
pub mod remote_command;
pub mod remote_status;
pub mod remote_url;
pub mod repository_lock;
pub mod repository_validation;
pub mod resolve_copies;
pub mod resolve_session;
pub mod review_fetch;
pub mod review_requests;
pub mod reword_commit;
pub mod squash;
pub mod stack;
pub mod stack_operations;
pub mod stack_parents;
pub mod store_lines;
pub mod sync;
pub mod sync_inputs;
pub mod transfer_progress;
pub mod unapply_branch;
pub mod uncommitted_changes;
pub mod update_project;

#[cfg(test)]
mod baseline_test;
#[cfg(test)]
mod branch_name_suggestion_test;
#[cfg(test)]
mod branch_prefix_test;
#[cfg(test)]
mod branch_review_test;
#[cfg(test)]
mod dependencies_test;
#[cfg(test)]
mod diff_scan_test;
#[cfg(test)]
mod git_setup_test;
#[cfg(test)]
mod patronus_command_test;
#[cfg(test)]
mod patronus_start_test;
#[cfg(test)]
mod patronus_test;
#[cfg(test)]
mod pull_request_writes_test;
#[cfg(test)]
mod pull_requests_test;
#[cfg(test)]
mod push_test;
#[cfg(test)]
mod remote_command_test;
#[cfg(test)]
mod resolve_copies_test;
#[cfg(test)]
mod sync_inputs_test;
#[cfg(test)]
mod sync_test;
#[cfg(test)]
mod test_commit;
#[cfg(test)]
mod unapply_branch_test;
#[cfg(test)]
mod uncommitted_changes_test;
