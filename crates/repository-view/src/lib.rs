//! What Rust derives of the branch table, so the page and `bd` read one rule (docs/design/git-operations.md,
//! "Repository view"). The app keeps the table of its last sync and answers `get_repository_view` from it; `bd` builds
//! the table from the report of its own sync or from a grouping, and reads the same view.
//!
//! - `branch_table`: the branch table a sync left: the branches in the order of the table, their tips and copies, the
//!   stack branches, the parked branches, the remote facts of the sync (`remote_facts`), `stack_ancestors`
//! - `patronus_view`: what the robots land, landed and hold of the table, by the hashes and else by content
//!   (`sync_core::robot_landing`), once per state (`ContentMemo`)
//! - `push_guard`: the checks of the app's push under the repository lock, by the rules of `bd push`: parked, landed
//!   whole, held by a running Safe Push; the words of the holds for both sides
//! - `views`: what the app keeps to answer `get_repository_view`: the table of the last sync and the memo, by repository

pub mod branch_table;
pub mod patronus_view;
pub mod push_guard;
pub mod views;
