# Proposal

## Why

A new slot is an APFS clone, but in slots of idea-1 a tracked file unchanged since May had its own inode and the birth
time of the get. The refresh of the index of the template ran with `core.checkStat=minimal`. With that check an entry
matches by its mtime and its size, so git wrote no new inode or ctime into the index. The `checkout --force` that
followed used the default check, saw every entry as changed and wrote every tracked file again: about 14 GB of blocks
of its own and 16 minutes of writes for each new slot of idea-1 (1.4M files).

## What Changes

- `git_ops::cloned_index::patch_cloned_index` writes the ctime, the device, the inode, the owner and the group of each
  cloned file into its entry when the clone kept its mtime, size and mode, without a hash. It leaves a racy entry, an
  entry with another stage, flags or stat data, and an index it does not know (split index, sparse index, an unknown
  extension) to the refresh.
- `refresh_cloned_index` runs the patch, then `update-index -q --refresh` with the default check, which hashes only
  the entries that the patch left.

Capabilities: `cli/lease`.
