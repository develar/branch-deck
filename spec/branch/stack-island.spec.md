---
name: Stack Island
description: Requirements for a big stack of stacked branches as an island of its own below the branch table, its header (size, states that show their first branch, pull requests, the landing of a running Safe Push, Push with Children of the root), its commits on the lanes a line each, oldest first, its collapse into the header, and the keys, the preview and the filter across the table and the islands.
style: plain-1
targets:
  - ../../src/lib/stack-collapse.ts
  - ../../src/stores/stack-collapse.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/stack-state.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/commit/StackCommitLines.tsx
  - ../../src/components/commit/CommitList.tsx
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/components/branch/BranchSelectionBar.tsx
  - ../../src/components/shared/CollapsibleCard.tsx
  - ../../crates/test-utils/src/repo_template.rs
---

# Stack Island

Status: Implemented
Date: 2026-10-16

## Summary

A stack of 25 branches and 178 commits collapsed into one row of the branch table hides what it holds in a tooltip, and
expanded it is 25 rows that still don't say what lands, what failed and what goes out with the next push. A big stack is
therefore an island of its own below the table: its header sizes it and says what it holds, each state a way to its
first branch, and pushes the whole stack; its rows show their commits on the lanes, a line each, oldest first, so the
stack reads as its history. The rest of the table stays as it is ([Stacked Branches](stacked-branches.spec.md#ui)).

## Goals

- See a big stack whole: its branches, their commits, which of them the remote lacks; push it up to any commit.
- Know what to do next from the header: what failed, what runs and what it lands, the pull requests, the push.
- Keep the small stacks and the lone branches in the table, where they read well.

## Non-goals

- An island for a stack inside an issue family.

## Requirements

### Which stack is an island

- A stack at the top level of the table of 5 branches or more (`STACK_ISLAND_MIN_BRANCHES`), or of 40 commits or more
  (`STACK_ISLAND_MIN_COMMITS`, a short stack of long branches), must be an island below the table, in the order of
  the table; the stack of an issue branch that heads an issue family (its family follows its stack) and a stack inside
  a family stay in the table. The rule reads all rows (`stackIslandRoots` of `getBranchTableRows(null)`), not the rows
  shown: neither a collapse nor a filter moves a stack between the table and its island.
  [@test] ../../src/stores/branch-table-rows.test.ts (`makes a stack of five branches or forty commits an island, not one in an issue family or on an issue branch that heads one`)
- The island must take its stack out of the table whole (`layoutBranchTables`); the shown rows are in the order of the
  screen, the table first, then the islands, for the keys, the Shift-click range, the preview and the selection bar.
  [@test] ../../src/stores/branch-table-rows.test.ts (`moves an island out of the table, whole, and lists the shown rows in the order of the screen`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, with its commits on the lanes, oldest first`)

### Header

- The header must name the root and size the stack ("air", "Stack · 7 branches · 14 commits"); the root row has no
  stack chip, the header is it.
- After it, what the stack holds, each counted as the stack chip counts it (`summarizeStackState` over the whole
  stack, `getStackStateChips`), the strongest first: "1 failed", "1 running", "7 to push"; a click on a count shows the
  first branch in that state (`requestReveal`: it expands the island and the row), also from a collapsed island.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
- Below the name, muted: the open pull requests of the stack ("2 pull requests open · 1 approved · 1 changes
  requested") and what each running robot lands, with its time left ("Safe Push of runtime lands 4 branches · ~23m
  left", the branch and the ones it lands with it, `useLandingSpans`).
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
- The one button of the header must be "Push with Children (7 branches)" of the root, as its menu names it: the branches
  of the stack that have work for the remote, each after the branch it is built on (`stackChildrenPushOrder`); while
  it runs, "Pushing bazel". Without work beyond the root's own the root row's push button is the push.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header, parents first`)

### Rows and commits

- The rows of the island must be the rows of the branch table: the stack graph, the Status cell, the Actions cell, the
  menu; a table of its own without the column heads, with a lane column as wide as its stack.
- Each branch must show its commits below its row while the row is not expanded, a line each on its lane
  (`StackCommitLines`): subject, short hash, age; the push arrow on a commit the remote lacks (`unpushedCommits`), a
  muted subject for one the remote has. A click on the row expands the full commit list in its place. A click on the
  arrow opens Push Up to Here of the commit: what goes out, in what order, and the push
  ([Stacked Branches](stacked-branches.spec.md#push-up-to-here)).
- On the lane of a stack the commits must be listed oldest first, the lines and the full list of every table with
  stacks: time runs down with the graph, so the tip of a parent sits right above the row of its child. The amend target
  is then the last commit, and the squash dialog still gets the commits newest first.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, with its commits on the lanes, oldest first`)

### Collapse, keys and filter

- A click on the header collapses the island into it and expands it; the state is the collapsed stack of the table
  (`collapsedStacks`, per repository, by root), so a new window starts with it.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should collapse the island into its header, per repository`)
- ↑/↓ must step through the rows of the table and the islands as one list; ⌥← from a row of an island collapses it
  (after the side stack the row is in, [Stacked Branches](stacked-branches.spec.md#ui)), and its header takes the
  focus; F2 and a reveal of a branch in a collapsed island expand it.
- A side stack of the island collapses into its root as in the table, so a long side chain doesn't cut the main line
  apart; one with nothing to push and nothing wrong starts collapsed.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should collapse a side stack into its root, from its chip and with Alt+Left before the whole stack`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should step from the table into the island with the arrows, and collapse it with Alt+Left`)
- Under the filter, an island must show its matches and, dimmed, the branches they stack on, and not collapse; an
  island without a match is not shown; the table without a row, while islands show matches, has no "No branches match".

## Testing / Local Run

- `pnpm test:unit`: `src/stores/branch-table-rows.test.ts`, `src/lib/stack-state.test.ts`.
- `pnpm e2e`: `tests/e2e/stack-island` (template `stack_island`).

## References

- [Stacked Branches](stacked-branches.spec.md): the stack graph, the collapse, Push with Children
- [Branch Row](branch-row.spec.md): the rows of a table with stacks
