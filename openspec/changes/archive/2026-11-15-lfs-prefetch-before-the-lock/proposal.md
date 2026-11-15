# The objects of Git LFS of a move are fetched before the repository lock

## Why

A move whose files follow writes them under the repository lock (`read-tree -m -u`): Update Project and its undo, an
unapply, a reapply, a drop, a pick and the undo of each. In a repository of Git LFS that runs the smudge filter, which
downloads every object the store lacks, so the lock was held across the network and every rewrite of the app and of
`brd` waited for the download. Pointers in the user's working tree, even for a moment, are no way out.

## What Changes

- Before each lock of such a move, its core fetches the objects of Git LFS of the files the move changes and the store
  lacks (`git_ops::lfs::prefetch_lfs_objects`): `git lfs fetch --include=<paths> <remote> <target>`, through the policy
  of a remote command, which moves from `sync_core::remote_command` to `git_ops::remote_command` (the variants with
  progress stay in sync-core). Under the lock the smudge reads the store only.
- Nothing without `$GIT_COMMON_DIR/lfs/objects`, no process; a fetch only for an object the store lacks, and none when
  the smudge does not download (`--skip`, `GIT_LFS_SKIP_SMUDGE`).
- A failed fetch is the error of the operation, before its lock, nothing written.
- Capabilities `repository/update-project`, `branch/unapply`, `commit/drop`, `cli/pick`.
