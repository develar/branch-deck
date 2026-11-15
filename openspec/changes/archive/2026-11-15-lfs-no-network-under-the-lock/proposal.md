# No network of Git LFS under the repository lock, and an idle deadline for its commands

## Why

The review of 336c87f0f found three gaps. Where the repository skips download errors, a move whose fetch before the
lock failed tried the download once more under the lock. A `git lfs pull` that failed in part, where the get goes on,
could leave content that the pull got as pointers. And the 30 minute deadline of the commands of LFS killed an honest
slow download as well as a hang.

## What Changes

- A move whose fetch before the lock failed, where the repository skips download errors, runs its `read-tree -m -u`
  with `-c lfs.fetchexclude=*`: the smudge reads the store and writes the pointer of the rest, with no network.
- A failed `git lfs pull` of a new slot, where the get goes on, is followed by `git lfs checkout`, no network.
- The commands of LFS are killed after 10 minutes without a line of progress (`GIT_LFS_PROGRESS` or stderr), with no
  cap on the whole.
- Capabilities `commit/drop`, `repository/update-project`, `cli/lease`.
