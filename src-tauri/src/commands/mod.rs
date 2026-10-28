pub mod activity;
pub mod agent_cli;
pub mod branch_review;
pub mod cli_tools;
pub mod conflict_fix;
pub mod file_diffs;
pub mod github;
pub mod patronus;
#[cfg(test)]
mod patronus_test;
pub mod pull_requests;
pub mod repository_browser;
pub mod repository_config;
pub mod review_requests;
pub mod settings;
pub mod sign_in;
pub mod suggest_branch_names;
pub mod window_management;
#[cfg(test)]
mod window_management_test;
pub mod window_menu;
pub mod youtrack;
