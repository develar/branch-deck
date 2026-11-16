# Proposal

## Why

AGENTS.md puts the logs of a session into `notes.local`, and since the notes of a lease (c938f0c00) `notes.local` of a
lease is a link to its notes beside the state of the pool. A fetch of brd-host follows no local link, so
`brd-host run studio --fetch notes.local -- …` failed with `fetch_failed` (`notes.local is a link`), and the log of a
heavy run on the Studio could not come back where the rules want it.

## What Changes

- A fetch writes through `notes.local` at the root of the checkout while it links to the notes of a lease of the same
  repository: an absolute target without `.` or `..`, a directory and no link, right in
  `$GIT_COMMON_DIR/branch-deck/notes/` (`git_ops::lease_slot::links_to_lease_notes`). Every other link takes no write.
- The layout of the notes (`STATE_DIR`, `LEASE_NOTES_DIR`, `lease_notes_root`) moves to `git_ops::lease_slot`, beside
  `NOTES_LINK`, and lease-pool builds its paths from it, so the pool and the fetch name one directory.
- Capability `cli/host`.
