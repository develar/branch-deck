# Proposal

## Why

`notes.local` of a leased worktree linked into `$GIT_COMMON_DIR/branch-deck/notes/<lease id>/`; for a main checkout that
is `<repository>/.git/branch-deck/notes/…`, outside the directories of the sessions that lead agents. Claude Code
matches a link by its target too, so every read of an agent's notes by its lead (`cat`, `tail`, Read, Monitor, also
through `<slot>/<repo>/notes.local`) asked the user; one such prompt stopped a whole team of agents for a night. The
pool directory (`~/.branch-deck/pool`, `branchdeck.leasePool`) is one of those directories already.

## What Changes

- The notes of a new lease and their record live in `notes/` of the pool directory, beside the slots, and
  `notes.local` links there; the root follows from the slot (`<pool dir>/<n>/<name>` → `<pool dir>/notes`), so no file
  names it. `notes` of `brd lease get`, `path` of `brd lease notes` name it.
- Readers look there, then in the old place `$GIT_COMMON_DIR/branch-deck/notes/`, read only: nothing moves the notes of
  a brd before; they expire there, and the read of the old place goes once the last of them expired.
- A fetch of brd-host writes through `notes.local` only into notes whose record names its checkout as the slot.
- No slot scan takes `notes/` for a slot; the trim and the keeper delete only expired notes of it.

Capabilities: `cli/lease`.
