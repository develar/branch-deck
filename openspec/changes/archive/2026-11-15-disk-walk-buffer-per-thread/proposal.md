# Each thread of a disk walk reads every directory into one buffer of its own

## Why

On macOS the walk of `brd lease disk` allocated a new zeroed buffer of 128 KiB for each directory that it read with
`getattrlistbulk`. A sample of a slow `brd lease disk` on 2026-11-15 put about 5% of the time in this allocation, with
lock contention between the 8 threads of the walk.

## What Changes

- On macOS each thread of a walk makes one read buffer when it takes its first directory and reads every directory into
  it (`fs_walk::walk`). A walk of a file or of one directory thus makes at most one buffer.
- The walk builds the path of an entry only for a directory.
- Capability `cli/lease` ("Disk").
