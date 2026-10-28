//! The agent of the remote runs on a host (spec/cli/host.spec.md, "serve", "setup", "upgrade"; docs/design/host-runs.md):
//! `serve`, the supervisor of each run, the run directories, the login environment, the prepare steps of `[host]`, the
//! keeper, the power assertions and the upgrade. It lends the host slots and does the git of the host in process,
//! through `lease_pool::host_slots`, `lease_pool::host_refs` and `lease_pool::host_git`: no `bd` child.
//!
//! The port of `yonderd` (`community/tools/yonder/bins/yonderd` of the IntelliJ monorepo) fills this crate.
