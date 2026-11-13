//! A disk cache keyed by git object ids (or a hash of them, `remote_status`, `conflict_chain`, `stack_branch`), shared
//! by the app and the `bd` CLI (`$GIT_COMMON_DIR/branch-deck/cache`), the answers of the Patronus pre-check
//! (`review_answer`), the user's robots `brd status` reads (`patronus_robots`), the robots of the Safe Push statistics
//! (`patronus_history`), the passed builds of `brd check --build` (`build_result`), the last frame of the session of
//! each worktree, which the next start of the app shows before its first sync (`session_frame`), and what a range of
//! two commits leaves to read once it is final (`range_reading`).
//! Free of Tauri and sync dependencies so that git-ops and sync-core can use it.

pub mod blob_summary;
pub mod build_result;
pub mod cache;
pub mod commit_diff;
pub mod commit_files;
pub mod commit_lines;
pub mod commit_tree;
pub mod conflict_chain;
pub mod conflict_info;
pub mod file_diff;
pub mod file_merge;
pub mod merge_result;
pub mod patronus_history;
pub mod patronus_robots;
pub mod range_reading;
pub mod remote_status;
pub mod review_answer;
pub mod session_frame;
pub mod stack_branch;
pub mod word_resolution;

#[cfg(test)]
mod cache_test;
