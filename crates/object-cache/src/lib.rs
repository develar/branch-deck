//! A disk cache keyed by git object ids, shared by the app and the `bd` CLI (`$GIT_COMMON_DIR/branch-deck/cache`), and
//! the answers of the Patronus pre-check (`review_answer`).
//! Free of Tauri and sync dependencies so that git-ops and sync-core can use it.

pub mod blob_summary;
pub mod cache;
pub mod commit_diff;
pub mod commit_files;
pub mod commit_lines;
pub mod commit_tree;
pub mod merge_result;
pub mod review_answer;

#[cfg(test)]
mod cache_test;
