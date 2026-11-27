# Proposal

## Why

A live check found that `brd --prefix slc sync`, in a repository whose config has `branchdeck.branchPrefix=develar`,
names its push as `brd -C <repo> push sl1`: run as printed, it acts on `develar/virtual/sl1`, another branch, or fails
with `no_branch_prefix` where no prefix is configured. Every command brd prints for a worktree had the same gap.

## What Changes

- The global options of the command line that change what a command acts on (`bd_core::bd_command::GlobalFlags`) go
  into every command brd names for a worktree: `--prefix`. The others change only how a command reads and prints
  (`--full-ids`, `--no-cache`, `-v`) or are the worktree each names itself (`-C`). `in_worktree` takes the flags, so no
  builder can leave them out: `RepoContext::bd` and `RepoContext::bd_in` (another worktree: a lease, the home
  checkout), the `LeaseContext` of a lease command (`with_flags`), `brd workspace`, the refusals of `brd update` and
  `brd drop` in a lease. A command of the lease pool that acts on no branch (`lease get`, `return`, `notes`) takes none.
- The app's command for an agent has no command line of brd: it names none.

Capabilities: `cli/agent-cli`.
