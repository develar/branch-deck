---
name: Branch Filter
description: Requirements for the filter bar of the branch table, the search over the branch names, issue keys, summaries and commit subjects, the state chips All, Your Move, Landing, Waiting and In Master with their counts by the node of each row, the dimmed stack ancestors of the matches, the empty result, and how the filter meets the selection, a reveal, a collapsed stack and the keys of the rows.
style: plain-1
targets:
  - src/components/branch/BranchFilterBar.tsx
  - src/components/shared/FilterChips.tsx
  - src/components/ui/toggle-group.tsx
  - src/components/branch/BranchTableCard.tsx
  - src/components/branch/BranchRow.tsx
  - src/components/branch/BranchRowElement.tsx
  - src/components/branch/BranchSelectionBar.tsx
  - src/hooks/useBranchFilter.ts
  - src/hooks/useBranchRowKeys.ts
  - src/lib/branch-filter.ts
  - src/lib/branch-row-keys.ts
  - src/lib/text-search.ts
  - src/components/shared/SearchField.tsx
  - src/hooks/useSlashFocus.ts
  - src/stores/branch-filter.ts
  - src/stores/branch-sync.ts
  - crates/test-utils/src/repo_template.rs
---

# Branch Filter

Status: Implemented
Date: 2026-10-19

## Purpose

A repository with 28 branches and a stack of 15 no longer fits in one look. The filter bar at the top of the branch
table narrows it: a search over what a row shows (the name, the issue key, the summary, the commit subjects) and one
state chip at a time (Your Move, Landing, Waiting, In Master: where a branch is on its way into master, as the node of
its row says it), each with its count of all branches. The matches stay in their
place in the stack graph: the branches they are built on show as dimmed context, so the lines stay connected. A branch
that stops matching while the user looks at it (its move made, its robot done) stays until the filter changes.

## Goals

- Find a branch by any word of its row, and the branches in a state, in a table of many.
- Keep the stack readable: a match deep in a stack shows what it is built on.
- Never pull a row away from the user: the row just pushed with P stays, with the focus on it.
- Stay out of the way of a small table.

## Non-goals

- A saved filter, or one per repository across restarts: the filter is per window and in memory.
- Fuzzy matching or highlighting the matched words.
- The keys of the rows themselves: [Branch Row](../branch-row/spec.md#keyboard).

## Requirements

### Requirement: The bar: The bar sits at the top of the branch card

The bar MUST sit at the top of the branch card, above the column heads and the selection bar over them: the search,
then the chips.

#### Scenario: The bar over the column heads
- **WHEN** the branch card shows its filter bar
- **THEN** the bar is at its top, above the column heads and the selection bar over them
- **AND** it holds the search, then the chips

### Requirement: The bar: The bar shows from 8 branches on or while a filter is active

The bar MUST show from 8 branches on (`FILTER_BAR_MIN_BRANCHES`), and below that while a filter is active, so a
filter that a sync left on a smaller table can still be cleared. A table of fewer branches is read in one look: the
bar would be the only chrome on it, and `/` does nothing there.

#### Scenario: A table of a few branches
- **GIVEN** a table of fewer than 8 branches and no active filter
- **WHEN** the branch table shows
- **THEN** there is no filter bar
- **AND** `/` does nothing there
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should have no filter bar in a table of a few branches`)

#### Scenario: A filter left on a smaller table
- **GIVEN** a filter is active
- **WHEN** a sync leaves fewer than 8 branches
- **THEN** the bar still shows, so the filter can be cleared

### Requirement: The bar: The filter is per window, in memory and of one repository

The filter MUST be per window, in memory, and of one repository: another repository, or a restart, starts without
one (`src/stores/branch-filter.ts`).

#### Scenario: Another repository or a restart
- **GIVEN** a filter is set in a window
- **WHEN** the window changes to another repository, or the app restarts
- **THEN** it starts without a filter
  [@test] src/stores/branch-filter.test.ts (`has no filter in another repository: a repository change starts without one`)

### Requirement: Search: Slash focuses the search

`/` MUST focus the search from anywhere in the window and select its text while the branches show (the Reviews view
has its own, [Reviews](../../repository/reviews/spec.md#search)), except while typing in another input, in a menu or
in a dialog.

#### Scenario: Slash from anywhere in the window
- **GIVEN** the branches show
- **WHEN** the user presses `/` outside another input, a menu and a dialog
- **THEN** the search takes the focus and its text is selected
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

#### Scenario: Slash while typing in another input, a menu or a dialog
- **WHEN** the user presses `/` while typing in another input, in a menu or in a dialog
- **THEN** the search does not take the focus

### Requirement: Search: Escape and the down arrow leave the search

Escape in the search MUST clear its text, and in an empty search move the focus to the table, whose keys then work.
↓ in the search MUST move the focus to the first branch row.

#### Scenario: Escape clears, then leaves for the table, and the down arrow goes to the first row
- **GIVEN** the search has the focus and holds text
- **WHEN** the user presses Escape
- **THEN** its text is cleared
- **AND** Escape in the empty search moves the focus to the table, whose keys then work
- **AND** ↓ in the search moves the focus to the first branch row
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

### Requirement: Search: A branch matches when every word is in what its row shows

A branch MUST match when every word of the search (split at white space) is in its name, its issue key, its summary,
the title of its description or the subject of one of its commits, ignoring case.

#### Scenario: Every word somewhere in the row
- **WHEN** the search holds words split at white space
- **THEN** a branch matches when every word is in its name, its issue key, its summary, the title of its description or the subject of one of its commits, ignoring case
  [@test] src/lib/branch-filter.test.ts (`matches every word in the name, the issue key, the summary, the description title or a commit subject, ignoring case`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

### Requirement: State chips: The chips are the sections of the landing overview with counts

The chips MUST be All, Your Move, Landing, Waiting and In Master, each with its count of all branches, whatever the
search: the words of the sections of the [landing overview](../../repository/landing-overview/spec.md#overview), so
the table, the node of each row and the header tell one story ([Landing](../../../../docs/design/landing.md)). A push
is no state of its own: the steps push what they need.

#### Scenario: The chips and their counts
- **WHEN** the filter bar shows
- **THEN** its chips are All, Your Move, Landing, Waiting and In Master
- **AND** each has its count of all branches, whatever the search
- **AND** no chip is a push state
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)

### Requirement: State chips: A chip shows while it counts a branch or is chosen

A chip MUST show while it counts a branch or is chosen; All always shows.

#### Scenario: A chip that counts no branch
- **GIVEN** no branch is in master and In Master is not chosen
- **WHEN** the filter bar shows
- **THEN** the In Master chip is not shown
- **AND** All is shown
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)

### Requirement: State chips: One chip is chosen at a time and combines with the search

One chip MUST be chosen at a time, All by default; the chosen chip again MUST go back to All. A chip combines with
the search.

#### Scenario: One chip at a time, the chosen one again goes back to All
- **GIVEN** no chip was chosen, so All is
- **WHEN** the user chooses a chip, then another, then the chosen one again
- **THEN** only the last chosen chip is chosen each time
- **AND** the chosen one again goes back to All
  [@test] src/stores/branch-filter.test.ts (`chooses one chip at a time, and the chosen one again goes back to All`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)

#### Scenario: A chip with the search
- **WHEN** a chip is chosen and the search holds text
- **THEN** the table shows the branches of the chosen chip that match the text, in table order
- **AND** an inactive filter shows all branches
  [@test] src/stores/branch-filter.test.ts (`keeps the text with the chip and clears both`)
  [@test] src/lib/branch-filter.test.ts (`shows the branches of the chosen chip that match the text, in table order; an inactive filter shows all`)

### Requirement: State chips: A chip takes a branch by the node of its row

A chip MUST take a branch by the node of its row (`getLandingNodeNow`, [Branch Row](../branch-row/spec.md#tree)). A
branch without a node yet MUST be in All only.

#### Scenario: Your Move
- **WHEN** the node of a row is a filled one: a fix (`needsFix`: a conflict, an error, a failed robot of the branch as it is, changes requested, failing checks), a step of the user (`yourStep`) or a branch ready to land (`ready`)
- **THEN** the branch is in Your Move
  [@test] src/lib/branch-filter.test.ts (`puts a branch into the chip of the node of its row, the filled nodes into Your Move`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)

#### Scenario: Landing
- **WHEN** a Safe Push lands the branch, its own or one of a branch built on it or of the whole stack, from its start on (`landing`)
- **THEN** the branch is in Landing
  [@test] src/lib/branch-filter.test.ts (`puts a branch into the chip of the node of its row, the filled nodes into Your Move`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)

#### Scenario: Waiting
- **WHEN** the branch waits for others, reviewers, its parent (a blocked branch too), a robot that holds it, a Dry Run, the first answer of Patronus (`waiting`)
- **THEN** the branch is in Waiting
  [@test] src/lib/branch-filter.test.ts (`puts a branch into the chip of the node of its row, the filled nodes into Your Move`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should filter by a state chip, with the counts of all branches`)

#### Scenario: In Master
- **WHEN** the baseline or a merge has the branch, until the update archives it (`inMaster`)
- **THEN** the branch is in In Master
  [@test] src/lib/branch-filter.test.ts (`puts a branch into the chip of the node of its row, the filled nodes into Your Move`)

#### Scenario: A branch without a node yet
- **WHEN** a branch has no node yet
- **THEN** it is in All only
  [@test] src/lib/branch-filter.test.ts (`puts a branch into the chip of the node of its row, the filled nodes into Your Move`)

### Requirement: State chips: A parked branch is in no chip and no row of the table

A parked branch MUST be in no chip and in no row of the table: its row is in the Parked card
([Parked Branches](../parked-branches/spec.md)).

#### Scenario: A parked branch
- **GIVEN** a parked branch
- **WHEN** the chips count and the table shows its rows
- **THEN** the parked branch is in no chip and no row of the table, its row is in the Parked card
  [@test] src/lib/branch-filter.test.ts (`puts a parked branch into All only: it has a card of its own, no chip, no move for F2`)

### Requirement: State chips: The chips count branches, the header counts moves

The chips MUST count branches, the rows the table shows; the header counts the units that land in one move.

#### Scenario: A ready stack of five branches
- **GIVEN** a ready stack of five branches
- **WHEN** the chips and the header count it
- **THEN** it is five rows of Your Move and one move of the header

### Requirement: State chips: The tooltip of Your Move tells F2 and Shift+F2

The tooltip of Your Move, while it counts a branch, SHALL tell F2 and Shift+F2, which step through its rows, a fix
first, from anywhere in the window ([Branch Row](../branch-row/spec.md#keyboard)).

#### Scenario: The tooltip of Your Move
- **GIVEN** Your Move counts a branch
- **WHEN** the user points at the chip
- **THEN** its tooltip tells F2 and Shift+F2
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should step through the user's moves with F2 and Shift+F2, a fix first, into a collapsed stack`)

#### Scenario: F2 and Shift+F2 step through the rows of Your Move, a fix first
- **WHEN** the user presses F2 or Shift+F2 anywhere in the window
- **THEN** the focus steps through the rows of Your Move, a fix first, each kind in table order
  [@test] src/lib/branch-filter.test.ts (`orders the moves for F2 by urgency, a fix first, each kind in table order`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should step through the user's moves with F2 and Shift+F2, a fix first, into a collapsed stack`)

### Requirement: Rows: The matches show in table order with the branches they stack on dimmed

The table MUST show the matches in table order and, dimmed, the branches they are stacked on, laid out as one graph
(`useVisibleBranchTableRows`, [Stacked Branches](../stacked-branches/spec.md#ui)).

#### Scenario: A match built on another branch
- **WHEN** a filter matches a branch stacked on others that it does not match
- **THEN** the table shows the matches in table order and, dimmed, the branches they are stacked on
- **AND** the rows are laid out as one graph, its lines connected
  [@test] src/stores/branch-table-rows.test.ts (`prunes the rows to the visible branches and the branches they stack on, which are dimmed`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

### Requirement: Rows: A dimmed row is a row

A dimmed row MUST be a row: it expands, opens its menu and takes the keys.

#### Scenario: A dimmed row expands
- **GIVEN** a dimmed row of a branch the matches are stacked on
- **WHEN** the user clicks it, opens its menu or presses its keys
- **THEN** it expands, opens its menu and takes the keys as any row
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

### Requirement: Rows: A shown branch stays until the filter changes

A branch shown under the filter MUST stay until the filter changes, also when it stops matching (pushed or started
under Your Move, its robot done under Landing); a new match shows at once.

#### Scenario: A branch stops matching
- **GIVEN** a branch shown under the filter
- **WHEN** it stops matching: pushed or started under Your Move, its robot done under Landing
- **THEN** it stays until the filter changes
  [@test] src/lib/branch-filter.test.ts (`keeps the branches shown since the filter changed, and adds the new matches`)
  [@test] src/stores/branch-filter.test.ts (`remembers the shown branches until the filter changes`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should keep a branch under Your Move until the filter changes, once its move is made`)

#### Scenario: A new match
- **WHEN** a branch starts to match the filter
- **THEN** it shows at once
  [@test] src/lib/branch-filter.test.ts (`keeps the branches shown since the filter changed, and adds the new matches`)

### Requirement: Rows: The counts follow the branches at once

The counts MUST follow the branches at once: a branch leaves Your Move at the click of its move (a start or restart
of its Safe Push, the creation of its pull request, a review request, without Patronus its push), not when it ends.

#### Scenario: The click of the move
- **GIVEN** a branch in Your Move
- **WHEN** the user clicks its move: a start or restart of its Safe Push, the creation of its pull request, a review request, without Patronus its push
- **THEN** the branch leaves the count of Your Move at the click, not when the move ends
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should keep a branch under Your Move until the filter changes, once its move is made`, `should open the Safe Push popover of the focused row with S and start it with Enter, which takes it out of Your Move`)

### Requirement: Rows: A filter that leaves no row says so in one quiet line

A filter that leaves no row MUST show one quiet line in the table instead of the rows and the column heads, with a
Clear Filter button that shows all branches again.

#### Scenario: Nothing matches the text
- **WHEN** the text of the filter leaves no row
- **THEN** the table shows `No branches match “foo”` (`No branches of your move match “foo”` with a chip) instead of the rows and the column heads
  [@test] src/lib/branch-filter.test.ts (`says what found nothing`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should say when nothing matches and clear the filter`)

#### Scenario: Nothing is in the chip alone
- **WHEN** a chip alone leaves no row
- **THEN** the table shows "Nothing is your move", "Nothing is landing", "Nothing waits for others", "Nothing is in master"
  [@test] src/lib/branch-filter.test.ts (`says what found nothing`)

#### Scenario: Clear Filter
- **GIVEN** the line of a filter that leaves no row
- **WHEN** the user clicks Clear Filter
- **THEN** all branches show again
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should say when nothing matches and clear the filter`)

### Requirement: The filter and the rest of the table: A reveal of a hidden branch clears the filter

A reveal (a branch clicked in the landing overview or Activity, "Show auth", moved commits) of a branch the filter
hides MUST clear the filter, so the row can expand and scroll into view.

#### Scenario: A branch the filter hides is revealed
- **GIVEN** the filter hides a branch
- **WHEN** a reveal of it comes: the branch clicked in the landing overview or Activity, "Show auth", moved commits
- **THEN** the filter is cleared
- **AND** the row expands and scrolls into view
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show every branch again for a reveal of a hidden one`)

### Requirement: The filter and the rest of the table: Cmd+A and Shift-click select the shown rows

Cmd+A MUST select the rows the filter matched, not the dimmed ones. Shift-click MUST select a range of the shown
rows.

#### Scenario: Cmd+A under a filter
- **GIVEN** a filter shows its matches and, dimmed, the branches they stack on
- **WHEN** the user presses Cmd+A
- **THEN** the matched rows are selected, not the dimmed ones
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should select only the matches with Cmd+A and keep a hidden selected branch`)

#### Scenario: Shift-click under a filter
- **WHEN** the user Shift-clicks a row under a filter
- **THEN** a range of the shown rows is selected

### Requirement: The filter and the rest of the table: A hidden selected branch stays selected

A selected branch the filter hides MUST stay selected, and the selection bar MUST say so ("2 branches selected
· 1 hidden"): Move to… takes it too.

#### Scenario: The filter hides a selected branch
- **GIVEN** a selected branch
- **WHEN** the filter changes and hides it
- **THEN** it stays selected and the selection bar says so ("2 branches selected · 1 hidden")
- **AND** Move to… takes it too
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should select only the matches with Cmd+A and keep a hidden selected branch`)

### Requirement: The filter and the rest of the table: The preview steps through the shown rows

The branch preview MUST step through the shown rows.

#### Scenario: The preview under a filter
- **WHEN** the user steps the branch preview under a filter
- **THEN** it goes through the shown rows only

### Requirement: The filter and the rest of the table: The counters read all branches

The counters of the header, the landing overview and the selection MUST read all branches.

#### Scenario: The counters under a filter
- **WHEN** a filter hides branches
- **THEN** the counters of the header, the landing overview and the selection still read all branches

### Requirement: The filter and the rest of the table: A filter wins over a collapsed stack

A filter MUST win over a collapsed stack ([Stacked Branches](../stacked-branches/spec.md#ui)): under it the table
shows the matches in their stacks, also deep in a collapsed one, and the chip of the root says how many of its
branches show ("3 of 22 branches") and toggles nothing. The filter stores nothing, so clearing it brings the
collapse back.

#### Scenario: A match deep in a collapsed stack
- **GIVEN** a collapsed stack
- **WHEN** a filter matches a branch deep in it
- **THEN** the table shows the match in its stack
- **AND** the chip of the root says how many of its branches show ("3 of 22 branches") and toggles nothing
- **AND** clearing the filter brings the collapse back
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show a match deep in a collapsed stack, and expand the stack for a reveal`)

### Requirement: The filter and the rest of the table: The rows of a collapsed stack count as hidden

Without a filter, the rows a collapsed stack hides MUST count as hidden like the rows the filter hides: Cmd+A,
Shift-click and the preview skip them, the selection bar counts them in "· 1 hidden", and a reveal of one expands
its stack instead of clearing a filter.

#### Scenario: Cmd+A and the selection bar with a collapsed stack
- **GIVEN** no filter and a collapsed stack
- **WHEN** the user presses Cmd+A, Shift-clicks or steps the preview
- **THEN** the rows the stack hides are skipped
- **AND** a selected one of them is counted in "· 1 hidden" of the selection bar
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should not select the branches of a collapsed stack with Cmd+A, and count them as hidden`)

#### Scenario: A reveal of a row of a collapsed stack
- **GIVEN** no filter and a collapsed stack
- **WHEN** a reveal of a row it hides comes
- **THEN** the stack expands instead of a filter being cleared
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show a match deep in a collapsed stack, and expand the stack for a reveal`)

## User Experience

- The bar is `flex flex-wrap items-center gap-x-3 gap-y-2 border-b border-border px-4 py-2`, on the card like the
  column heads below it.
- The search is the `SearchField` of every list with a search (the files of a review too): an `InputGroup` `h-7`,
  here `w-60`, with the `SearchIcon`, the placeholder "Filter branches" (also its accessible name), the `Kbd` `/` at
  its end while empty and an `icon-xs` X button ("Clear the filter text") once typed.
- The chips are `FilterChips` in a group named "Branch state": the stock `ToggleGroup` (`outline`, `sm`, `spacing={1}`,
  wrapping), the chosen one in its pressed state (`aria-pressed`), no colour of its own; the label in Title Case as a tab (All, Your Move, Landing, Waiting, In Master), the count after it in tabular
  figures at 70% opacity. E2E finds them by `data-chip`.
- A dimmed row is `opacity-55`, its node and lines too, and full opacity on hover or focus (`data-dimmed`).
- The empty line is `text-sm text-muted-foreground` in a `px-6 py-4` cell; Clear Filter is a `link` `xs` button.
- The filter applies as it is typed: the table is small enough for no debounce.

## Testing / Local Run

- `pnpm test:unit`: `src/lib/branch-filter.test.ts`, `src/stores/branch-filter.test.ts`,
  `src/stores/branch-table-rows.test.ts`.
- `pnpm e2e tests/e2e/branch-filter`: the `branch_filter` template (eleven branches: the stack `core` → `api` → `web`
  with `cli` forked off `core`, issue branches, `search` of IJPL-200, and "the API" in the subject of `docs`); robots
  through the `patronus` option of `setupRepo`, pushes to the remote of the repository (`remotePushes`, a push held
  there with `holdRemotePushes`). The counts are checked against the nodes of
  the rows (`lane-node`, `data-kind`), which follow the same rule.

## Open Questions / Risks

- The lane column follows the rows shown: when the filter leaves fewer lanes, or none, the names move left.
- A match by a commit subject doesn't say which commit matched; the row has to be expanded.
- A sync that renames a branch drops it from the shown branches; it shows again only if it matches.
- Selected branches hidden by the filter are moved by Move to… too; the selection bar names their count, not them.

## References

- [Branch Row](../branch-row/spec.md): the rows, their keys and the dimmed row
- [Stacked Branches](../stacked-branches/spec.md): the pruned graph
- [Landing Overview](../../repository/landing-overview/spec.md): the sections the chips are named by
- [Next Step](../next-step/spec.md): what decides the node of a row
- [Patronus](../patronus/spec.md): the robots that land a branch or failed on it
- [Move Commits](../move-commits/spec.md): the selection
