# Proposal

## Why

`brd lease trim` on the user's Mac reported 1,062 GB freed for six slots of idea-1, while the volume gained about 30 GB.
The delete counted the blocks of each file (`st_blocks`). The tracked tree of a slot is an APFS clone of its template,
so most of those blocks stay with the other slots, and a file with another hard link frees nothing at its unlink.
`brd lease disk` already counts what a delete frees (the private size); the delete itself did not.

## What Changes

- The delete of a trash, a return, a clean, a reclaim and a trim count what each unlink frees: nothing while another
  hard link keeps the file, else the private size of the file on macOS (`ATTR_CMNEXT_PRIVATESIZE`, read once a
  directory by `getattrlistbulk`), else its blocks.
- The private size is read before the deletes of its directory, so a count is never more than the volume gains.

## Capabilities

### Modified Capabilities

- `cli/lease`: the bytes a delete of the trash frees.

## Impact

- `crates/lease-pool/src/lease_clean.rs`, `crates/lease-pool/src/lease_clean_test.rs`, `crates/fs-walk/src/lib.rs`.
