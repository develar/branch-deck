# Git LFS: a deadline for its commands, and the repositories that skip download errors

## Why

A repository that lives on `lfs.skipdownloaderrors` (an object that can not be downloaded becomes its pointer) failed
every get of a new slot, since `git lfs pull` exits non-zero, and every move whose fetch before the lock failed. And a
hung `git lfs pull` or `git lfs fetch` had no deadline: git-lfs does its own HTTP, which `http.lowSpeedLimit` of the
policy of a remote command does not reach.

## What Changes

- After a failed pull or fetch, `lfs.skipdownloaderrors` (or `GIT_LFS_SKIP_DOWNLOAD_ERRORS`) true lets the get go on
  with the pointers and the move go on, its smudge writing them as the repository asks.
- `git lfs pull` and `git lfs fetch` end at `LFS_DEADLINE` (30 min) of `git_ops::remote_command`, killed with the
  processes they started.
- Capabilities `cli/lease`, `commit/drop`.
