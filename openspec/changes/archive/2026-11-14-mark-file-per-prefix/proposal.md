# Proposal

## Why

The review of 6a849284 found four low items: `last-sync-read` took the branch of a ternary (`a ? state.lastSyncTime : 0`)
for the key of an object; two syncs under two prefixes at once read, changed and wrote one file of the mark, and one
line could be lost; the mark of the build before (a time instead of a prefix) stayed as garbage; and the e2e of a
repository synced before did not hold the open sync, so a page that ignored `synced` of the view passed it.

## What Changes

- `last-sync-read` takes a name as a key only after the start of an object, a comma or the start of a line.
- The mark is a file per prefix (`branch-deck/synced.d/`, the hex of the prefix, written by a rename); the file of lines
  of a build before is read, a time naming no prefix, and the next mark moves its prefixes and removes it.
- The e2e of a repository synced before holds the open sync and sees no card meanwhile.
- Capability `repository/onboarding`: the mark is a file per prefix, with the scenarios of two syncs at once and of the
  mark of a build before.
