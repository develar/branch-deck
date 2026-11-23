# Proposal

## Why

The transcripts of 2026-11-23 show agents working around the answers of `brd lease`: 49 searches for their own lease
through `brd workspace`, 12 calls of a `lease list` that did not exist, a `slot_busy` of their own background command
that they read and acted on by hand, and the answers of a conflicting pick cut with `tail -c` because they carried the
whole message of the commit.

## What Changes

- `brd lease return --holder <name>` returns the one lease of the holder from any checkout; several are refused and
  named, each with its return by id.
- `brd lease list` is a hidden alias of `brd lease status`.
- A `slot_busy` of only commands of the holder names `brd lease return --workspace <slot> --stop-own` first in
  `next_steps`.
- The `update` of a conflict of `brd lease pick` leaves out the body of the commit of its round; its `patch` holds it.

Capabilities: `cli/lease`.
