# Tasks

- [x] `fs_retry::move_path` and `move_across_volumes`, with their tests through a rename that refuses two volumes
- [x] The set-aside (its record first) and the return of a directory of notes move through it; a seam of the tests
  moves as across two volumes, and the tests tell a copy from a rename by the inode
- [x] `held_leases` of a state the caller read: the keeper deletes nothing without it, the trim and `brd lease notes`
  fail as before; directories without a record expire by age (`holder` absent in `expired_notes`)
- [x] A get whose notes fail removes the link and the empty directory it made
- [x] `slot_changes` skips everything under `notes.local/`; the test with `status.showUntrackedFiles=all`
- [x] The spec, git-operations ("The notes of a lease"), contributing/leases.md
