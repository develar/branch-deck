# Proposal

## Why

The copies of a resolve session, which an agent reads and edits during the conflict of `brd lease pick` or `brd
update`, lived in `branch-deck/resolve/` of the worktree's git dir; for a slot of the lease pool that is
`.git/worktrees/<id>` of the main checkout, so each edit asked the user, and a prompt stops an unattended night as the
notes of a lease did (change lease-notes-in-pool).

## What Changes

- In a slot whose `notes.local` links to the notes of its lease, a session without `--dir` keeps its copies in
  `resolve/` of those notes, in the pool directory, by the path the link resolves to; the finish and the abort remove
  them as before. Any other worktree, and `--dir`, stay as they are.

Capabilities: `cli/update`.
