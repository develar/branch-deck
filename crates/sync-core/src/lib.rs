pub mod add_issue_reference;
pub mod amend_to_branch;
pub mod branch_prefix;
mod branch_processor;
pub mod commit_grouper;
pub mod create_branch;
pub mod delete_archived_branch;
pub mod issue_navigation;
pub mod remote_status;
pub mod repository_validation;
pub mod sync;
pub mod uncommitted_changes;

#[cfg(test)]
mod branch_prefix_test;
#[cfg(test)]
mod create_branch_test;
#[cfg(test)]
mod sync_test;
#[cfg(test)]
mod uncommitted_changes_test;
