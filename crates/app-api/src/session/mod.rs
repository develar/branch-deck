//! The commands of the sessions (docs/design/rust-domain-authority.md, "Представление: снимок и патчи"): a window opens
//! the session of a repository, subscribes to its patches on a channel and asks it for syncs; the session
//! (`repository_model::session`) owns the table. A command of a session names the repository by the path of its
//! `params`, and the frame of its kind (`session` of `registry.rs`) resolves the key of the session with the prefix
//! Rust resolves (`AppContext::session_key`) before its body runs: a repository without a prefix is refused
//! (`no_branch_prefix`), and the body gets the key, never the path to resolve again.
//!
//! The facts of the remote services are read for the sessions by the scheduler of the facts
//! (`remote_facts::scheduler`), which each host starts beside them on the same clock; a window asks a fact again with
//! `refresh_facts`.
//!
//! - `view`: the view of a session, the subscription of a window to its patches, its focus and its demand, its syncs,
//!   its facts asked again, the group a typed message puts a commit into.
//! - `operations`: the operations of the remote, as the page asks them: their plans, their runs, their cancel.
//! - `reviews`: the pull requests the user adds to Reviews by their links.
//! - `windows`: what each window holds open in the sessions.

pub mod operations;
pub mod reviews;
pub mod view;
pub mod windows;

#[cfg(test)]
mod windows_test;
