# A primary that can not be read is unknown, and a companion unlinks its side

## Why

A companion of a primary whose config could not be read, or whose path was gone, was taken over by the next add of
another primary: a disk not mounted for a while lost the user's link. And a companion whose primary did not list it
could only be repaired from a primary.

## What Changes

- A primary that can not be read (its path gone, no git repository, a config that fails to read) is unknown, not
  absent: an add of its companion by another primary is refused with the primary it names and how to unlink it.
- Unlink takes back the side of a half link on a companion (`companion_write::unlink_primary`): Unlink in Settings →
  Landing of the companion (`unlink_primary` of the app, the one builder `primaryLinkItems`) and `brd companion unlink`;
  refused while the primary lists it.
- Capability `branch/declared-needs`.
