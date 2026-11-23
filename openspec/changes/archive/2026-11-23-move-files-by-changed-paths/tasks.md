# Tasks

- [x] `SmallIndex` and `WorktreeUpdate` in `crates/git-ops/src/worktree_update.rs`; `IndexLock::hold` and `HeldIndexLock` in `index_lock.rs`
- [x] `move_with_index_lock` and `files_follow` of `update_branch.rs` prepare the files before the branch moves; the new mode of a changed path (`paths.rs`)
- [x] `crates/git-ops/src/worktree_update_test.rs`: every fixture on the whole index and by the small one, the lock, the interrupted moves
- [x] The scan of `update-index --index-info` (`git_usage_scan_test.rs`); the size of the index file from which the small index runs (`SMALL_INDEX_FROM`), so the process budgets and the lock spans of the moves in a small repository stay
- [x] The exact dry run on the small index (`dry_run_worktree_update`)
- [x] "The files of a move", "Index lock", "Lock spans", "Pick" and "Update project" of docs/design/git-operations.md; git-rules.md; AGENTS.md
- [x] The numbers on a lease of the IntelliJ monorepo (`the_files_of_a_move_of_a_rig`)
