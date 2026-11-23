# Proposal

## Why

Every move of HEAD that the files follow (Update Project, a pick and its follow, the unapply, the reapply, the drop,
their undo) writes the working tree with one two-way `read-tree -m -u` on the user's index, under the repository
lock. `read-tree -m` frees the cache tree before it merges, so on the IntelliJ monorepo (1.42M index entries) it walks
every entry and builds every tree again: 7 s of CPU for a move of 50 paths, 7–11 s of wall at a load average of 20–40
and 37–74 s at 60–200. The picks of 10–30 agents wait for each other behind it.

## What Changes

- Where the index file has 16 MB or more, the merge runs on an index of the paths the move changes: their entries
  are cut out of the user's index into a temporary one, the same `read-tree -m -u` merges it with the two trees of
  those paths alone, and the entries it changed go back into a copy of the user's index, which takes its place. Git
  still checks every path before it writes a file, and still writes the files. The exact dry run of a move merges the
  same index with `-n`. Below 16 MB the whole index merges as before: the small index is ten processes where the
  whole one is two, 0.5–0.7 s against 0.2–0.3 s in a repository of 4 300 files.
- This process holds the index lock of the worktree from the cut to the put back, taken before the branch moves, so no
  git of the user commits the old index over the new files, and a lock of another process is waited for while nothing
  has moved.
- The whole index merges as before where the small one does not model it: a sparse checkout, a skip-worktree or
  unmerged entry, an assume-unchanged entry or a submodule on a path of the move, a small index that cannot be built.
- A move killed between its files and its index is finished by the next one, not moved back; an index that cannot
  follow takes the files back.
- `update-index --index-info` is the second writer of the user's index, in `worktree_update.rs` only.
- Not changed: the index of a rewrite whose tree changes (`IndexUpdate::TwoWay`, a `read-tree -m -i` of the whole
  index) and the check of a move by its paths.

Capabilities: `repository/update-project`, `cli/pick`, `branch/unapply`.
