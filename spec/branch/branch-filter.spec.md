---
name: Branch Filter
description: Requirements for the filter bar of the branch table, the search over the branch names, issue keys, summaries and commit subjects, the state chips All, To Push, Running and Attention with their counts, the dimmed stack ancestors of the matches, the empty result, and how the filter meets the selection, a reveal, a collapsed stack and the keys of the rows.
style: plain-1
targets:
  - ../../src/components/branch/BranchFilterBar.tsx
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchContextMenuRow.tsx
  - ../../src/components/branch/BranchSelectionBar.tsx
  - ../../src/hooks/useBranchFilter.ts
  - ../../src/lib/branch-filter.ts
  - ../../src/lib/text-search.ts
  - ../../src/components/shared/SearchField.tsx
  - ../../src/hooks/useSlashFocus.ts
  - ../../src/stores/branch-filter.ts
  - ../../src/stores/branch-sync.ts
  - ../../crates/test-utils/src/repo_template.rs
---

# Branch Filter

Status: Implemented
Date: 2026-10-15

## Summary

A repository with 28 branches and a stack of 15 no longer fits in one look. The filter bar at the top of the branch
table narrows it: a search over what a row shows (the name, the issue key, the summary, the commit subjects) and one
state chip at a time (To Push, Running, Attention), each with its count of all branches. The matches stay in their
place in the stack graph: the branches they are built on show as dimmed context, so the lines stay connected. A branch
that stops matching while the user looks at it (pushed, its robot done) stays until the filter changes.

## Goals

- Find a branch by any word of its row, and the branches in a state, in a table of many.
- Keep the stack readable: a match deep in a stack shows what it is built on.
- Never pull a row away from the user: the row just pushed with P stays, with the focus on it.
- Stay out of the way of a small table.

## Non-goals

- A saved filter, or one per repository across restarts: the filter is per window and in memory.
- Fuzzy matching or highlighting the matched words.
- The keys of the rows themselves: [Branch Row](branch-row.spec.md#keyboard).

## Requirements

### The bar

- The bar must sit at the top of the branch card, above the selection bar and the column heads: the search, then the
  chips.
- It must show from 8 branches on (`FILTER_BAR_MIN_BRANCHES`), and below that while a filter is active, so a filter
  that a sync left on a smaller table can still be cleared. A table of fewer branches is read in one look: the bar
  would be the only chrome on it, and `/` does nothing there.
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should have no filter bar in a table of a few branches`)
- The filter must be per window, in memory, and of one repository: another repository, or a restart, starts without
  one (`src/stores/branch-filter.ts`).
  [@test] ../../src/stores/branch-filter.test.ts (`has no filter in another repository: a repository change starts without one`)

### Search

- `/` must focus the search from anywhere in the window and select its text, except while typing in another input, in
  a menu or in a dialog. Escape in the search must clear its text, and in an empty search move the focus to the table,
  whose keys then work. ↓ in the search must move the focus to the first branch row.
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)
- A branch must match when every word of the search (split at white space) is in its name, its issue key, its summary,
  the title of its description or the subject of one of its commits, ignoring case.
  [@test] ../../src/lib/branch-filter.test.ts (`matches every word in the name, the issue key, the summary, the description title or a commit subject, ignoring case`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

### State chips

- The chips must be All, To Push, Running and Attention, each with its count of all branches, whatever the search.
  A chip must show while it counts a branch or is chosen; All always shows.
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)
- One chip must be chosen at a time, All by default; the chosen chip again must go back to All. A chip combines with
  the search.
  [@test] ../../src/stores/branch-filter.test.ts (`chooses one chip at a time, and the chosen one again goes back to All`, `keeps the text with the chip and clears both`)
  [@test] ../../src/lib/branch-filter.test.ts (`shows the branches of the chosen chip that match the text, in table order; an inactive filter shows all`)
- To Push must take the branches the header counts as having work for the remote (`summarizePushState`): synced
  without an error, with commits, not blocked, not landed, and a push that is not "up to date"; an amended or
  restacked branch is a force push (`isRemoteUpToDate`), so it counts.
  [@test] ../../src/lib/branch-filter.test.ts (`puts a branch into To Push by the rule of the outgoing counts`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)
- Running must take the branches whose robot (`findBranchRobot`) runs or cancels (`isRobotActive`).
  [@test] ../../src/lib/branch-filter.test.ts (`puts a branch whose robot runs or cancels into Running`)
- Attention must take what waits for the user: an error or a conflict, a branch blocked by its stack parent, a failed
  robot of the branch as it is (also one that failed for missing approvals; `needsRobotAttention`), and an open pull
  request with changes requested or failing checks (`formatAuthorAttention`), as the dot of the outgoing indicator.
  [@test] ../../src/lib/branch-filter.test.ts (`puts an error, a conflict, a blocked branch, a failed robot of the branch as it is and a pull request waiting for its author into Attention`)
  The tooltip of Attention, while it counts a branch, tells F2 and Shift+F2, which step through its rows from anywhere
  in the window ([Branch Row](branch-row.spec.md#keyboard)).
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)

### Rows

- The table must show the matches in table order and, dimmed, the branches they are stacked on, laid out as one graph
  (`useVisibleBranchTableRows`, [Stacked Branches](stacked-branches.spec.md#ui)). A dimmed row is a row: it expands,
  opens its menu and takes the keys.
  [@test] ../../src/stores/branch-table-rows.test.ts (`prunes the rows to the visible branches and the branches they stack on, which are dimmed`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)
- A branch shown under the filter must stay until the filter changes, also when it stops matching (pushed under To
  Push, its robot done under Running); a new match shows at once. The counts follow the branches at once.
  [@test] ../../src/lib/branch-filter.test.ts (`keeps the branches shown since the filter changed, and adds the new matches`)
  [@test] ../../src/stores/branch-filter.test.ts (`remembers the shown branches until the filter changes`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should keep a pushed branch under To Push until the filter changes`)
- A filter that leaves no row must show one quiet line in the table instead of the rows and the column heads: `No
  branches match “foo”` (`No branches to push match “foo”` with a chip), or for a chip alone "Nothing to push", "No
  robot is running", "Nothing needs attention", with a Clear Filter button that shows all branches again.
  [@test] ../../src/lib/branch-filter.test.ts (`says what found nothing`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should say when nothing matches and clear the filter`)

### The filter and the rest of the table

- A reveal (a branch clicked in the outgoing overview or Activity, "Show auth", moved commits) of a branch the filter
  hides must clear the filter, so the row can expand and scroll into view.
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show every branch again for a reveal of a hidden one`)
- Cmd+A must select the rows the filter matched, not the dimmed ones. Shift-click must select a range of the shown
  rows. A selected branch the filter hides must stay selected, and the selection bar must say so ("2 branches selected
  · 1 hidden"): Move to… takes it too.
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should select only the matches with Cmd+A and keep a hidden selected branch`)
- The branch preview must step through the shown rows; the counters of the header, the outgoing overview and the
  selection read all branches.
- A filter must win over a collapsed stack ([Stacked Branches](stacked-branches.spec.md#ui)): under it the table shows
  the matches in their stacks, also deep in a collapsed one, and the chip of the root says how many of its branches
  show ("3 of 22 branches") and toggles nothing; the filter stores nothing, so clearing it brings the collapse back.
  Without a filter, the rows a collapsed stack hides count as hidden like the rows the filter hides: Cmd+A, Shift-click
  and the preview skip them, the selection bar counts them in "· 1 hidden", and a reveal of one expands its stack
  instead of clearing a filter.
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show a match deep in a collapsed stack, and expand the stack for a reveal`, `should not select the branches of a collapsed stack with Cmd+A, and count them as hidden`)

## User Experience

- The bar is `flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-border px-4 py-2`, on the card like the
  selection bar below it.
- The search is the `SearchField` of every list with a search (the files of a review too): an `InputGroup` `h-7`,
  here `w-60`, with the `SearchIcon`, the placeholder "Filter branches" (also its accessible name), the `Kbd` `/` at
  its end while empty and an `icon-xs` X button ("Clear the filter text") once typed.
- The chips are `ChoiceChip`s (`xs`) in a group named "Branch state": the chosen one soft primary (`aria-pressed`), the
  others `secondary`; the label in Title Case as a tab (All, To Push, Running, Attention), the count after it in tabular
  figures at 70% opacity. E2E finds them by `data-chip`.
- A dimmed row is `opacity-55`, its node and lines too, and full opacity on hover or focus (`data-dimmed`).
- The empty line is `text-sm text-muted-foreground` in a `px-6 py-4` cell; Clear Filter is a `link` `xs` button.
- The filter applies as it is typed: the table is small enough for no debounce.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/branch-filter.test.ts`, `src/stores/branch-filter.test.ts`,
  `src/stores/branch-table-rows.test.ts`.
- `pnpm e2e tests/e2e/branch-filter`: the `branch_filter` template (eleven branches: the stack `core` → `api` → `web`
  with `cli` forked off `core`, issue branches, `search` of IJPL-200, and "the API" in the subject of `docs`); robots
  through the `patronus` option of `setupRepo`, pushes through `routePush`. To Push is compared with the branch count
  of the outgoing indicator, which follows the same rule.

## Open Questions / Risks

- The lane column follows the rows shown: when the filter leaves fewer lanes, or none, the names move left.
- A match by a commit subject doesn't say which commit matched; the row has to be expanded.
- A sync that renames a branch drops it from the shown branches; it shows again only if it matches.
- Selected branches hidden by the filter are moved by Move to… too; the selection bar names their count, not them.

## References

- [Branch Row](branch-row.spec.md): the rows, their keys and the dimmed row
- [Stacked Branches](stacked-branches.spec.md): the pruned graph
- [Outgoing Commits](../repository/outgoing.spec.md): the push rule of To Push and the attention of the indicator
- [Patronus](patronus.spec.md): the robots of Running and Attention
- [Move Commits](move-commits.spec.md): the selection
