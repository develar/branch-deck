# Proposal

## Why

A record of `bd-checks.jsonl` names the exact tree a green build or test saw, so an update of the lease onto a newer
home branch threw it away: on 2026-11-23 21% of the picks of the IntelliJ monorepo (36 of 168) passed
`--allow-unchecked`, most with the reason "green on its own base, then a clean replay onto master". The gate stops a
stale claim, a check that ran before the last edit; a clean merge of git onto a newer base is no edit, and the gate
does not stop a merge break anyway ("The gate stops a stale claim, not a merge break").

## What Changes

- A stale record carries over (`check.state: carried_over`) when its tree, merged onto the base of HEAD on the branch
  the commits land on, is `HEAD^{tree}`: `git merge-tree --write-tree --merge-base=<B> <record.tree> <base>`, B the
  merge base of the record's HEAD with that branch, the base the merge base of HEAD with it
  (`git_ops::carried_check`). The record's tree, not its HEAD: it may hold the changes that were uncommitted then.
- The test is the tree, exact for binaries and whitespace: an update that needed a resolution, by words too, writes
  another tree and stays `stale`, so "An update that applies after a conflict waits for a check of the new tree" holds.
- The answer names the commits of that branch in between that no check saw (`check.untested_master_commits`, one
  `rev-list --count`).
- `check_carries_over = false` in `[lease]` of `.branchdeck.toml` turns it off; true by default.
- Git only for a stale record: two `merge-base`, one `merge-tree`, one `rev-list --count` when it carries over, else
  the `diff-tree` of the files changed since as before.

Capabilities: `cli/lease`, `cli/pick`.
