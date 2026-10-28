//! The client of the remote runs (spec/cli/host.spec.md, "run" to "upgrade"; docs/design/host-runs.md): a run of a
//! checkout on a host, its commit of the working tree, its thin pack, its follow, its fetch and the sync of its base,
//! the host files, `setup <alias>` over ssh and the upgrade over the link. Its git runs in process through
//! `git_ops::host_client`, `git_ops::host_objects` and `git_ops::worktree_snapshot`: no `bd` child.
//!
//! - `interrupt`: Ctrl+C of a run, counted
//!
//! The port of `yonder-client` (`community/tools/yonder` of the IntelliJ monorepo) fills this crate.

pub mod interrupt;
