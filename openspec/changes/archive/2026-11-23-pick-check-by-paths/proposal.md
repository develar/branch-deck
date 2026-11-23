# Proposal

## Why

On the IntelliJ monorepo (1.42M index entries) a pick ran four full `unpack_trees`: a dry run on a copy of the index of
each worktree before the lock, and the two moves under it. `read-tree -m` frees the cache tree before it merges, so each
of them walks the whole index (2 s of CPU) and builds all 310 000 trees of the cache tree again (3.4 s), a dry run with
`-n` too, although the cache tree at rest is whole (`GIT_TRACE2_PERF` on a copy of the index, 2026-11-23). The check under
the lock already went by the paths (`recheck_blocking_paths`), hashing them again.

## What Changes

- The check of the files before the lock goes by the paths each move changes (`blocking_paths_by_preview`): their index
  entries and content, the exact dry run only where that finds a path or in a sparse checkout. The check under the lock
  takes the probe of the check before it and reads only what changed since.
- The gaps of the preview close first: a directory the move replaces with a file passes only while every file in it is
  one the move deletes; an unmerged entry of the index on any path blocks the pick with its paths (one `ls-files -u`).
- A refusal of `read-tree -m -u` after the CAS moves the branch back and, after the lock, the exact dry run names the
  paths: `Blocked` (`uncommitted_overlap`), not the error "Files changed during the pick".
- Not changed: the refresh of the index under the lock stays whole (0.5 s with fsmonitor on the monorepo; a refresh by
  the paths would read and write the same index).

Capabilities: `cli/pick`.
