# Proposal

## Why

The review of a generator's run over the monorepo (jps-bridge of idea-1: 7596 files, about 266 owners by the
`OWNERSHIP` files) showed a chip per owner in the header of the files. The header does not shrink, so the chips took
the whole sidebar and the tree of the files had no room left: the window showed no file at all.

## What Changes

- The header shows "All", then the chips in their order, eight at most, and the chosen chip wherever it stands; the
  others are under "N More", a search of them by name, with the shield of a group that still needs its approval.
- Owners of as many files come in the order of the numbers in their names (`Team 2` before `Team 10`).

Capabilities: `branch/branch-review`.
