# The files of Git LFS of a new slot are downloaded after the clones lock

## Why

A get holds the clones lock until its new slot is checked out, and in a repository of Git LFS that checkout ran the
smudge filter, which downloads every object the store of the repository lacks: every get that adds a slot waited for
that download. The lock of the pool's `worktree` commands holds no checkout (the registration is `--no-checkout`).

## What Changes

- Where the smudge filter of LFS downloads (`filter.lfs.process`, else `filter.lfs.smudge`, without `--skip`, and no
  `GIT_LFS_SKIP_SMUDGE`), the checkout of a new slot under the clones lock writes the pointers (`LfsFiles::Pointers`:
  the filter's commands empty, not `required`), and the get pulls the content (`git lfs pull`, the policy of a remote
  command) after the release, where the repository keeps objects of LFS (`lfs/objects`, or of `lfs.storage`).
- A pull that fails fails the get and deletes the index of the slot, so the next get finds it dirty and never reuses
  its pointers as content.
- A reused slot moves without a lock of the pool, its smudge filter as git runs it.
- Capability `cli/lease`.
