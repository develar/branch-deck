# Tasks

- [x] The policy of a remote command in git-ops, `git_ops::lfs` (the config, the prefetch, the pull of a new lease slot)
- [x] The prefetch before the lock of a computed rewrite (`Prepared::worktree_moves` in `lock_witnessed`), of Update
  Project and of a pick; the tests with a fake smudge filter and a fake `git lfs fetch` that log the lock, one per
  family with its undo, with mutations (the fetch under the lock of each path, no fetch), and the process budget
- [x] Git Operations, "Lock spans" and "Git LFS"; the usage scan counts `git lfs` as a network command
