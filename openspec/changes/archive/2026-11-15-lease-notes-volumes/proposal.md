# Proposal

## Why

The review of the lease notes found that they break where the slot and the state of the pool are on two volumes (a
`branchdeck.leasePool` on another disk, on Windows a repository on `D:` with its pool under `C:\Users`): each move was
a rename, which no volume boundary lets through (`EXDEV`). The set-aside of what an older brd left failed, so the next
holder read it as its own; without links, the return could not move the notes, which passed from session to session.
Three smaller gaps: the keeper read a state it could not read as "no lease held", a get that failed left a directory
without a record that never expired, and a status with `status.showUntrackedFiles=all` named the files of a directory
`notes.local` as changes.

## What Changes

- `fs_retry::move_path`: a rename, or across two volumes a copy beside the target, renamed into place once whole, then
  the removal of the source (`move_across_volumes`); the set-aside and the return of the notes move through it.
- The expiry deletes nothing where the state of the pool can't be read; a directory of the notes without a record
  expires by its modification time; a get whose notes fail removes what it made of them.
- No status of the pool counts a file in `notes.local` as a change, only the link or the directory itself before.
