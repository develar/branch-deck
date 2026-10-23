//! The trial checks of a grouping: builds of the virtual branches that write objects only (no ref moves, no working
//! tree), for `bd check`, the conflict fix and the landing gate of a Safe Push, and everything that reads the diffs
//! (`file_diffs`) and the symbols (`code_symbols`) of the commits. A check builds with the branch build of the sync
//! (`sync_core::branch_processor`, `sync_core::cache_memo`); the sync itself reads no diff and no symbol, so a change
//! here rebuilds no sync.
//!
//! The check:
//! - `check`: `CheckSession`, the trial sync of a regroup plan, and whether a stacked branch builds without its parent
//!   (`get_stack_independence`)
//! - `missing_commits`: the commits a conflicting commit of a trial build likely needs, by the changed files
//! - `conflict_fix`: the regroup after which the conflicting branches apply cleanly, found by trial builds
//! - `dangling_references`: the declarations a branch removes while its tree still uses them, and by the summaries
//!   (`symbol_dependencies::declaration_checks`) those it changes or declares again
//! - `dangling_fix`: the fix of a mutual dangling reference, offered when a trial build proves it
//! - `inverted_stack`: a branch that needs a commit of a branch stacked on it, and its fixes
//! - `missing_declarations`: the names a built branch uses that a commit outside it adds
//! - `landing_gate`: what refuses the start of a Safe Push or a Dry Run of a `domain_types::landing::Subject`, for
//!   `bd safe-push` and the app: a name what lands uses and a branch outside it adds, a dangling reference a commit
//!   proves
//!
//! The diffs and the symbols of the commits:
//! - `dependencies`: which commits depend on which, from the hunks of `diff_scan`
//! - `diff_scan`: the changes of many commits from one `log --raw -z` and the blocks of the diff views
//! - `symbol_scan`: the syntax summaries of the changed source files, by blob id
//! - `symbol_dependencies`: the edges between commits by the names they declare and use
//! - `module_roots`: where the modules of source files begin
//!
//! The one rewrite of the checked out branch here, because it reads the blocks of the commits' files:
//! - `move_hunk`: `bd move-hunk` under the repository lock, its checks against the commits ahead of the baseline

pub mod check;
pub mod conflict_fix;
pub mod dangling_fix;
pub mod dangling_references;
pub mod dependencies;
pub mod diff_scan;
pub mod inverted_stack;
pub mod landing_gate;
pub mod missing_commits;
pub mod missing_declarations;
pub mod module_roots;
pub mod move_hunk;
pub mod symbol_dependencies;
pub mod symbol_scan;

#[cfg(test)]
mod dependencies_test;
#[cfg(test)]
mod diff_scan_test;
#[cfg(test)]
mod module_roots_test;
#[cfg(test)]
mod process_budget_test;
#[cfg(test)]
mod symbol_dependencies_test;
#[cfg(test)]
mod symbol_scan_test;
#[cfg(test)]
mod word_resolve_sync_test;
