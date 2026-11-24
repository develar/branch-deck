# Proposal

## Why

A review of lease-notes-in-pool found two ways the expiry of the notes could delete user data. Any slot whose parent is a
number counted as `<pool dir>/<n>/<name>`, so a worktree adopted where it is, `~/work/2026/repo-wt`, made `~/work/notes`
a root, and the trim and the keeper deleted every directory there without a record. And two repositories that share
one pool directory: the trim of one knows only its own held leases, so the held lease of the other lost its notes once
past the keep.

## What Changes

- A slot gives a root only in a pool directory of Branch Deck: the one of the config, one of `~/.branch-deck/pool`, or
  one whose root has the marker `.branch-deck-notes`, which every get writes; an adopted worktree elsewhere takes the
  root of the get's pool directory.
- A directory without a record expires only in a marked root or the old place in the common dir.
- A record names its repository; the expiry and `brd lease notes` take only the notes of their own repository (a record
  without the field by its slot).

Capabilities: `cli/lease`.
