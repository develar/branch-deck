# The object pass of the cargo rule holds the objects of every session of `incremental/`

## Why

A review of the object pass found that the newest output of a unit does not guard every object of a running build.
rustc links a reused part of a crate from its session into `out/` as a hard link, which keeps the old mtime of the
inode. So that object is older than the new rmeta and the old rlib. Only the sessions keep it, and the pass read the
sessions of the kept dirs of `incremental/` alone. A stale dir that a build wrote after the walk stays in place, as
built again, but its sessions did not count. A build after the release of the locks then lost the objects it links,
and its rlib failed. The object pass runs after the release at every `age` since the last change, so the window was
open for every `age`.

## What Changes

- The pass reads the sessions of every dir of `incremental/` as they are at the pass, kept or stale.
- A session that rustc renames between the list and its read is read again by a new list. When a read fails at each
  try, the objects of the profile dir stay.
- A new requirement, the module doc and Git Operations name the two guards of a running build: `age`, measured from
  the start of the run, and the sessions. The newest output is no such guard.
- Capability `cli/lease`.
