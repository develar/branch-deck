---
name: Stack Island
description: Requirements for a big stack of stacked branches as an island of its own below the branch table, its header (size, states that show their first branch, pull requests, the landing of a running Safe Push, the actions of a branch row for the whole stack: the push, the Safe Push of the whole stack with its popover at the header's slot, S on the header and the menu, none of them while a Safe Push runs in it), its rows those of the table, its collapse into the header, and the keys, the preview and the filter across the table and the islands.
style: plain-1
targets:
  - ../../src/lib/stack-collapse.ts
  - ../../src/stores/stack-collapse.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/stack-state.ts
  - ../../src/hooks/useStackStates.ts
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/StackIslandCard.tsx
  - ../../src/hooks/useSettledLayout.ts
  - ../../src/components/branch/LandingNodeHover.tsx
  - ../../src/hooks/usePushButton.ts
  - ../../src/hooks/useWholeStackActions.ts
  - ../../src/components/branch/OrphanedStackPullRequest.tsx
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/components/branch/WholeStackSlot.tsx
  - ../../src/hooks/useStackNextStep.ts
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/lib/stack.ts
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchActions.tsx
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/lib/git/stack-push.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/components/branch/BranchSelectionBar.tsx
  - ../../src/components/shared/CollapsibleCard.tsx
  - ../../src/stores/inline-row-action.ts
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/hooks/useSafePush.ts
  - ../../src/stores/safe-push.ts
  - ../../crates/test-utils/src/repo_template.rs
  - ../../src/hooks/useLandingNode.ts
---

# Stack Island

Status: Implemented
Date: 2026-10-16, its rows those of the table 2026-10-25

## Summary

A stack of 25 branches and 178 commits collapsed into one row of the branch table hides what it holds in a tooltip, and
expanded it is 25 rows that still don't say what lands, what failed and what goes out with the next push. A big stack is
therefore an island of its own below the table: its header sizes it and says what it holds, each state a way to its
first branch, and offers the actions of a branch row for the whole stack: the Safe Push of the whole stack first (one
robot lands every branch of it, side branches included), the push of all of it second, nothing while a Safe Push runs
in it; its rows are the rows of the table, so the stack reads as its branches. The rest of the table stays as it is
([Stacked Branches](stacked-branches.spec.md#ui)).

## Goals

- See a big stack whole: its branches, a row each, as the table shows a branch.
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
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

### Header

- The header must name the root and size the stack ("air", "Stack · 7 branches · 14 commits"); the root row has no
  stack chip, the header is it. Before the name, the node of the whole stack says where it is on its way into master,
  as the node of a row says it of a branch ([Branch Row](branch-row.spec.md#tree)): of its stack branch while the stack
  forks, of its last branch for a straight chain; its hover says it ("Ready to land: Safe Push lands 7 branches"), so
  a collapsed island still answers it. The hover is that of the node of a row ([Branch Row](branch-row.spec.md#tree),
  `LandingNodeHover`): a failed or running robot shows below the text with its actions, the robot of the whole stack
  while the stack forks, else that of its last branch.
  [@test] ../../tests/e2e/branch-graph/branch-graph.spec.ts (`should say in the island header where the whole stack is on its way into master`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should show the failed robot of the whole stack in the hover of the island's node, with its actions`)
- After it, where the branches of the stack are on their way into master, by the nodes of their rows, as the stack
  chip counts them (`summarizeStackState` over the whole stack, `getStackStateChips`), the strongest first in the
  colours of the nodes: "1 to fix", "4 landing" with the ring of the robot, "2 waiting"; a click on a count shows the
  first branch in that state (`requestReveal`: it expands the island and the row), also from a collapsed island. The
  robot of the whole stack runs on its stack branch, no branch's: while it runs every branch it lands is landing
  ("7 landing"); a failure of it on the stack branch as the sync built it is one more fix, the root's ("1 to fix"),
  its count showing the root.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should count the robot of the whole stack in the header of its island, a click on it showing the root`)
  [@test] ../../src/lib/stack-state.test.ts (`counts a fix of the whole stack, whose stack branch has no row, as the root's`)
  [@test] ../../tests/e2e/patronus/dock.spec.ts (`should count the failed robot of the whole stack only while the sync built its stack branch`)
- Below the name, muted, " · " between its items as between the parts of each: the open pull requests of the stack
  ("2 pull requests open · 1 approved · 1 changes requested") and what each running robot lands, with its time left ("Safe Push of runtime lands 4 branches · ~23m
  left", the branch and the ones its commit has of those it is built on, `useLandingSpans`; "Safe Push of the whole stack lands 7 branches"
  for the robot of the whole stack, [Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app)), without a ring: the
  ring of the robot is on the count of the branches it lands ("4 landing"). The pull request of the whole stack is counted apart, first, as where the
  approvals of its Safe Push count: "Approvals count in #12, the pull request of the whole stack" (" · approved", " ·
  changes requested" as GitHub says). One that no longer matches it says so in `text-warning` instead: "#12 no longer
  matches the air stack: auth landed". The lines are inside the toggle of the header, so the pull request they name is
  a button before the state chips (`StackPullRequestButton`): "#12" opens it, "Close Pull Request #12…" in `warning`
  asks to close one that no longer matches (in the root's row, as from "⋯").
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should count the approvals of the whole stack in its pull request: the header says so apart from the branches', and a branch without its own points to it`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should offer the close of the pull request of the whole stack in the island header once it no longer matches`)
- The line takes no room while it has no item, and moves no row when the pull requests come after the rows: until the
  facts of the remote are in, an island whose line showed once they were in the last time keeps its place, one text
  line high and empty (`useSettledStackLine`, the roots kept in `stackLines` of `settledLayout` in the repository state,
  as the columns and Your Turn keep theirs, [Next Step](next-step.spec.md#facts-without-an-answer)); the answer fills
  it there.
  [@test] ../../tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the second header line of a stack island in its place while the pull requests come back`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should say in the popover of the whole stack only what holds: the branches on the remote stay, no approvals where none are needed`)
- The header must end with the Actions cell of a branch row, for the whole stack, its push, Safe Push slot and "⋯"
  in the columns of the rows' ([Branch columns](../../docs/design/ui-theme.md#branch-columns)), also while its slot is
  empty: the compact push, the Safe Push slot and "⋯".
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should line up the columns of the table, the island and its header, whatever each card shows`)
  No filled button: a push is not what lands the stack.
- The push is the compact push of a row: "↑ 41", the commits of "Push with Children (7 branches)" of the root, its
  accessible name, as the root's menu names it: the branches of the stack that have work for the remote, each after
  the branch it is built on (`stackChildrenPushOrder`), the landed ones and the ones a running Safe Push holds left
  out; and the stack branch of the whole stack while it goes along, once it
  is on the remote or its pull request is open ([Stacked Branches](stacked-branches.spec.md#pull-requests)): one more
  branch ("Push with Children (8 branches)", "Push with Children (the whole stack branch)" alone), a commit it and the
  branches both send counted once. Its hover names it and lists the pushes ("Pushes 8 branches, parents first:", a
  line per branch, the stack branch last: "user-name/stack/air · the whole stack branch · 2 commits"), and the
  branches whose push drops remote commits, which ask first in their rows. It is
  one atomic push of all of them ([Stacked Branches](stacked-branches.spec.md#pull-requests)): a click pushes and leaves
  the hover as it is ([hover descriptions](../../docs/design/ui-theme.md)), open while the pointer stays, also a
  click well after it opened; while it runs, the spinner
  and the percent, "Pushing 7 branches" its name and the line of git on hover, the one indicator of the stack push
  (also of Push with Children from the root's menu), and every row it pushes shows the same progress on its own push
  button. So does the push of the stack branch alone, a step of the restart of its robot, of Request Review… of its
  pull request and of the creation of its pull request (`pushWholeStack`): "Pushing the whole stack 45%" its name, the
  line of git on hover ("Pushing the whole stack branch of air…" until git reports), no row of a branch busy; the
  start of the Safe Push of the whole stack shows its push in its own button instead (the spinner and "45%", named
  "Pushing 45%", below), the
  header's push disabled meanwhile: one indicator, decided once for the header's push and the root's push button
  (`useStackBranchPushAlone`). It stays in its place, disabled ("↑", named "Push with Children") with the reason in its hover, while it
  would push nothing beyond the root ("Nothing to push: the stack is on the remote" only while its stack branch has
  nothing to push either; "The branches of the stack are on the remote. Its stack branch goes there with the pull
  request or the Safe Push of the whole stack"; "Only air has commits to push: its own push sends them", the branches
  that ask first; once merged Safe Pushes landed every branch of the stack, its stack branch held too: "Landed in master
  with deploy and side-test: nothing of air and its children is left to push") and while a Safe Push runs in the stack
  (the reason of the hold, "A Safe Push of runtime is running: it lands 2 commits of air with it"). A root a merged Safe Push landed
  holds nothing: the header pushes what is left of the stack without the landed branches, and its slot shows what is
  left to land.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`, `should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu`, `should say what the stack holds in the header, each state a way to its first branch`, `should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`, `should show the push of the stack branch alone in the header's push when Request Review… of its pull request pushes it, the stack waiting from the click`)
- The Safe Push slot is the Safe Push of the whole stack: one robot lands every branch of it, side branches included
  ([Whole Stack](whole-stack.spec.md#the-safe-push-of-the-whole-stack-in-the-app)). Of a stack that forks it is the
  next step of its stack branch (`WholeStackSlot`, [Next Step](next-step.spec.md#the-whole-stack)): "Safe Push · 7",
  its hover what it lands, "Lands 7 branches · 14 commits into master in one Safe Push: air, bazel, … side-fix and
  side-test" (the main line first, then the side branches, which only it lands); else the restart of its robot,
  "Create Pull Request…" (of the Stack, as its menus say it), Request Review… or the wait in its pull request. Of a straight chain, which has
  no stack branch, it is the slot of the last branch of the main line (`stackMainLineIn`), whose one robot lands it all:
  "Safe Push deploy · lands 5 branches into master". A click asks the start (`requestSafePush` of `stack:<root>`,
  placement `stack-header`); it lands more than a row, so it opens its popover at the header's slot (`SafePushPopover`),
  also while the island is collapsed: nothing expands, nothing scrolls. Its title is what lands, then "Pushes only its
  stack branch user-name/stack/air." with what stays said only of the branches on the remote ("…: the branches of the
  stack stay as they are on the remote.") and, with its pull request open and its approvals needed, "Approvals count
  in #12, the pull request of the whole stack."; Enter pushes the stack branch alone
  (leased, `pushWholeStack`) and starts the robot from it (`stack: true`), the button busy meanwhile ("Checking…",
  the spinner and "45%" of its push, "Starting…", at the width it had at rest), the one indicator of its push: the header's push waits, disabled, without it. S on the focused header opens the same popover. Disabled with the reason of the whole stack:
  its stack branch not built ("The stack branch conflicts at 0caf9066 in config.js: the stack lands in pieces"), a
  robot of a branch of it, its approvals missing in its own pull request. While its stack branch is not built, the stack
  lands in pieces: the "⋯" item "Safe Push deploy" (the last branch of the main line), the action of the notice of S
  and "Land it in pieces" of the popover open that Safe Push at the header's slot. The rows keep their own popovers at
  their slots, one popover at a time (`SafePushAsk`). The header shows no key hints: it is no row.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`, `should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)
- While a Safe Push runs in the stack (each one lands the root: [Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs))
  the header's push is disabled with the reason of the hold and it offers no Safe Push: the landing line and "4
  landing" say why, as the Status cell of a row whose robot runs; the branches it holds and doesn't land are "2
  waiting".
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)
- "⋯" is the menu of the stack ("More actions for the air stack"), built when it opens: the actions of the whole
  stack, the list the menu of every branch of the stack shows under "Whole Stack ›" (`getWholeStackMenuItems`,
  [Whole Stack](whole-stack.spec.md#the-whole-stack-in-the-app)): "Safe Push Whole Stack" and "Dry Run Whole Stack"
  (the popover at the header's slot, disabled with the reasons of the whole stack), "Safe Push deploy" while the stack
  lands in pieces, then "Create Pull Request of the Stack…" or "Open Pull
  Request #12" with "Request Review…" of it while it is open (its popover at the header's slot, also while a Safe Push
  runs in the stack and the slot is empty), "Push with Children (7 branches)" (the stack branch counted while it goes along, disabled with the
  reason while a Safe Push holds the root and while it pushes nothing beyond the root, as the root's menu:
  [Stacked Branches](stacked-branches.spec.md#pull-requests)),
  "Review Whole Stack" (the review window of the stack branch; of a straight chain, of its last branch from the root,
  disabled with the reason of the branch menu's, `branchReviewBlocker`: no commits, failed, blocked) and the actions of
  the robot of the whole stack; then "Collapse Stack" / "Expand Stack".
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`, `should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`, `should ask for a review of the pull request of the whole stack from the island's ⋯ and the root's menu, the popover at the header`, `should open Request Review… of the whole stack at the header while its Safe Push runs and the header offers no step`)

### Rows

- The rows of the island must be the rows of the branch table, with nothing of their own: the stack graph, the Status
  cell, the Actions cell, the menu, the keys, the expansion and the commits it shows
  ([Branch Row](branch-row.spec.md)); a table of its own without the column heads, with a lane column as wide as its
  stack, its other columns those of the branch table ([Branch Row](branch-row.spec.md#columns)). Only the header takes
  the stack's own parts from its root row: the size, and the push and the step of the whole stack (above).
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

### Collapse, keys and filter

- A click on the header collapses the island into it and expands it; the state is the collapsed stack of the table
  (`collapsedStacks`, per repository, by root), so a new window starts with it. The collapse closes an inline form below
  a row of the island, never the Safe Push popover at the header's slot, which the collapsed header still shows.
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should collapse the island into its header, per repository`)
- ↑/↓ must step through the rows of the table and the islands as one list; ⌥← from a row of an island collapses it
  (after the side stack the row is in, [Stacked Branches](stacked-branches.spec.md#ui)), and its header takes the
  focus; F2 and a reveal of a branch in a collapsed island expand it. S on the focused header opens the popover of the
  Safe Push of the whole stack, Enter starts it. A side stack of the island is a side stack of the table
  ([Stacked Branches](stacked-branches.spec.md#ui)).
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should step from the table into the island with the arrows, and collapse it with Alt+Left`)
- Under the filter, an island must show its matches and, dimmed, the branches they stack on, and not collapse; an
  island without a match is not shown; the table without a row, while islands show matches, has no "No branches match".

## Testing / Local Run

- `pnpm test:unit`: `src/stores/branch-table-rows.test.ts`, `src/lib/stack-state.test.ts`.
- `pnpm e2e`: `tests/e2e/stack-island` (templates `stack_island`, `stack_side_conflict`: the stack branch conflicts).

## References

- [Stacked Branches](stacked-branches.spec.md): the stack graph, the collapse, Push with Children
- [Branch Row](branch-row.spec.md): the rows of a table with stacks
