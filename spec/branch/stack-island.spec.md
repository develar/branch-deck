---
name: Stack Island
description: Requirements for a big stack of stacked branches as an island of its own below the branch table, its header (size, states that show their first branch, pull requests, the landing of a running Safe Push, the actions of a branch row for the whole stack: the push, the Safe Push of the main line with its popover at the header's slot and the menu, none of them while a Safe Push runs in it), its commits on the lanes a line each, oldest first, its collapse into the header, and the keys, the preview and the filter across the table and the islands.
style: plain-1
targets:
  - ../../src/lib/stack-collapse.ts
  - ../../src/stores/stack-collapse.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/stack-state.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/hooks/useWholeStackActions.ts
  - ../../src/components/branch/OrphanedStackPullRequest.tsx
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/lib/stack.ts
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchActions.tsx
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/lib/git/stack-push.ts
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/commit/StackCommitLines.tsx
  - ../../src/components/commit/CommitList.tsx
  - ../../src/hooks/useCommitMoveList.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/components/branch/BranchSelectionBar.tsx
  - ../../src/components/shared/CollapsibleCard.tsx
  - ../../src/stores/inline-row-action.ts
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/hooks/useSafePush.ts
  - ../../src/stores/safe-push.ts
  - ../../crates/test-utils/src/repo_template.rs
---

# Stack Island

Status: Implemented
Date: 2026-10-16

## Summary

A stack of 25 branches and 178 commits collapsed into one row of the branch table hides what it holds in a tooltip, and
expanded it is 25 rows that still don't say what lands, what failed and what goes out with the next push. A big stack is
therefore an island of its own below the table: its header sizes it and says what it holds, each state a way to its
first branch, and offers the actions of a branch row for the whole stack: the Safe Push of its main line first, the push
of all of it second, nothing while a Safe Push runs in it; its rows show their commits on the lanes, a line each, oldest first, so the
stack reads as its history. The rest of the table stays as it is ([Stacked Branches](stacked-branches.spec.md#ui)).

## Goals

- See a big stack whole: its branches, their commits, which of them the remote lacks; push it up to any commit.
- Know what to do next from the header: what failed, what runs and what it lands, the pull requests, the Safe Push,
  the push; as a branch row offers them, so the header reads as the row of the stack.
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
  stack, `getStackStateChips`), the strongest first: "1 failed", "1 running", "3 to push"; a click on a count shows the
  first branch in that state (`requestReveal`: it expands the island and the row), also from a collapsed island.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
- Below the name, muted: the open pull requests of the stack ("2 pull requests open · 1 approved · 1 changes
  requested") and what each running robot lands, with its time left ("Safe Push of runtime lands 4 branches · ~23m
  left", the branch and the ones it lands with it, `useLandingSpans`; "Safe Push of the whole stack lands 7 branches"
  for the robot of the whole stack, [Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app)), without a ring: the
  ring of the robot is on "1 running". A pull request of the whole stack that no longer matches it says so in
  `text-warning`: "#12 no longer matches the air stack: auth landed".
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
- The header must end with the Actions cell of a branch row, for the whole stack, at the widths of the rows below, so
  the columns line up: the compact push, the Safe Push slot and "⋯". No filled button: a push is not what lands the
  stack.
- The push is the compact push of a row: "↑ 41", the commits of "Push with Children (7 branches)" of the root, its
  accessible name, as the root's menu names it: the branches of the stack that have work for the remote, each after
  the branch it is built on (`stackChildrenPushOrder`), the landed ones and the ones a running Safe Push holds left
  out, so it counts as the "to push" chip does. Its hover names it and lists the pushes ("Pushes 7 branches, parents
  first:", a line per branch), and the branches whose push drops remote commits, which ask first in their rows. It is
  one atomic push of all of them ([Stacked Branches](stacked-branches.spec.md#pull-requests)): while it runs, the spinner
  and the percent, "Pushing 7 branches" its name and the line of git on hover, the one indicator of the stack push
  (also of Push with Children from the root's menu), and every row it pushes shows the same progress on its own push
  button. It stays in its place, disabled ("↑", named "Push with Children") with the reason in its hover, while it
  would push nothing beyond the root ("Nothing to push: the stack is on the remote", "Only air has commits to push:
  its own push sends them", the branches that ask first) and while a Safe Push runs in the stack (the reason of the
  hold, "A Safe Push of runtime is running: it lands air with it").
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`, `should say what the stack holds in the header, each state a way to its first branch`)
- The Safe Push slot is the one of the last branch of the main line (`stackMainLineIn`, lane 0 of the graph): its one
  robot lands the whole line ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)). Its hover says whose it
  is, what it lands and which side branches it doesn't: "Safe Push deploy · lands 5 of 7 branches into master ·
  side-fix and side-test stay: not on the main line". Its button is the branch's ("Safe Push · 5"): a click asks the
  start of that branch (`requestSafePush`, placement `stack-header`) and, since a stacked branch always confirms, opens
  its popover at the header's slot (`SafePushPopover`), also while the island is collapsed: the row is not revealed,
  nothing expands, nothing scrolls to the end of the line. The start is the branch's own: its facts, its holds, its
  progress in the button ("Checking…", "Pushing 5 branches 45%", "Starting…") and its start (Enter) are those of the
  branch, the path it lands is drawn while the popover is open; its row's slot, S and menu open the popover at the row
  (placement `row`), one popover at a time (`SafePushAsk`). Disabled with the reason of the branch (blocked, a robot
  that lands an ancestor, its approvals missing in its own pull request, a draft: [Next
  Step](next-step.spec.md#the-table)); the step of the branch otherwise ("Restart Safe Push", Create Pull Request…, the
  wait); never another branch. The "⋯" item "Safe Push deploy" reads the same reason and opens the same popover at the
  header's slot. The key hints of a row (S, O) are not in the header: it is no row.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should open the Safe Push of the main line under the header`)
- While a Safe Push runs in the stack (each one lands the root: [Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs))
  the header's push is disabled with the reason of the hold and it offers no Safe Push: the landing line and "1
  running" say why, as the Status cell of a row whose robot runs. The branches it lands are not to push: "3 to push"
  counts the others.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
- "⋯" is the menu of the stack ("More actions for the air stack"), built when it opens: "Safe Push deploy", then the
  actions of the whole stack, the list the root's menu shows under "Whole Stack ›" (`getWholeStackMenuItems`,
  [Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app)): "Create Pull Request of the Stack…" or "Open Pull
  Request #12", "Push with Children (7 branches)" (disabled with the reason while a Safe Push holds the root and while
  it pushes nothing beyond the root, as the root's menu: [Stacked Branches](stacked-branches.spec.md#pull-requests)),
  "Review Whole Stack" (the review window of the stack branch; of a straight chain, of its last branch from the root,
  disabled with the reason of the branch menu's, `branchReviewBlocker`: no commits, failed, blocked) and the actions of
  the robot of the whole stack; then "Collapse Stack" / "Expand Stack".
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should open the Safe Push of the main line under the header`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)

### Rows and commits

- The rows of the island must be the rows of the branch table: the stack graph, the Status cell, the Actions cell, the
  menu; a table of its own without the column heads, with a lane column as wide as its stack.
- Each branch must show its commits below its row while the row is not expanded, a line each on its lane
  (`StackCommitLines`): subject, short hash, the short time of the commit ("Today 2:32 PM", "Oct 3"); the push arrow on a commit the remote lacks (`unpushedCommits`), a
  muted subject for one the remote has. A click on the row expands the full commit list in its place. A click on the
  arrow opens Push Up to Here of the commit: what goes out, in what order, and the push
  ([Stacked Branches](stacked-branches.spec.md#push-up-to-here)).
- On the lane of a stack the commits must be listed oldest first, the lines and the full list of every table with
  stacks: time runs down with the graph, so the tip of a parent sits right above the row of its child. The amend target
  is then the last commit, and the squash dialog still gets the commits newest first.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, with its commits on the lanes, oldest first`)

### Collapse, keys and filter

- A click on the header collapses the island into it and expands it; the state is the collapsed stack of the table
  (`collapsedStacks`, per repository, by root), so a new window starts with it. The collapse closes an inline form below
  a row of the island, never the Safe Push popover at the header's slot, which the collapsed header still shows.
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
