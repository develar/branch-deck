# Proposal

## Why

A user of the monorepo (2026-11-20) committed fixes of several issues on master, each needing the one before
(RUST-21311 needs RUST-21295). The sync built each issue on origin/master, so the later ones conflicted; Stack On… of a
conflicting branch was disabled ("The branch failed to sync"), though that restack is the fix, and the Conflict Fix
offered it in 10 steps, the same restack up to four times. He went back to cherry-picks in IDEA.

## What Changes

- A rewrite says what it reads of the branch: its original commits (every rewrite but Unapply) or its build (Unapply,
  as the push and the review). A merge conflict holds back only what reads the build; an error of the sync holds back
  every rewrite, as before. So Stack On…, the drop on the "Stack on ‹name›" chip, Stack Together, Unstack, Move to…,
  Ungroup and the other rewrites of the originals run on a conflicting branch, and the sync after them builds it again.
- A fix of several conflicts names each step once: a candidate of a later round that carries a step of an earlier one
  adds it no more.

Capabilities: `branch/branch-row`, `branch/move-commits`, `branch/stacked-branches`, `branch/conflict-fix`.
