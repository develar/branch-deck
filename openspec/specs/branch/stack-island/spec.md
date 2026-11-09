---
name: Stack Island
description: Requirements for a big stack of stacked branches as an island of its own below the branch table, its header (size, states that show their first branch, pull requests, the landing of a running Safe Push, the actions of a branch row for the whole stack: the push, the Safe Push of the whole stack with its popover at the header's slot, S on the header and the menu, none of them while a Safe Push runs in it), its rows those of the table, its collapse into the header, and the keys, the preview and the filter across the table and the islands.
style: plain-1
targets:
  - src/lib/stack-collapse.ts
  - src/stores/stack-collapse.ts
  - src/stores/branch-sync.ts
  - crates/domain-policy/src/stack.rs
  - src/lib/stack-state.ts
  - src/hooks/useStackStates.ts
  - src/components/branch/BranchTableCard.tsx
  - src/components/branch/StackIslandCard.tsx
  - src/components/branch/ChildrenPushButton.tsx
  - src/hooks/useSettledLayout.ts
  - src/components/branch/LandingNodeHover.tsx
  - src/presenters/push-button.ts
  - src/hooks/useBranchPushes.ts
  - src/hooks/useWholeStackActions.ts
  - src/components/branch/OrphanedStackPullRequest.tsx
  - src/components/branch/NextStepSlot.tsx
  - src/components/branch/WholeStackSlot.tsx
  - src/hooks/useStackNextStep.ts
  - src/components/branch/SafePushButton.tsx
  - src/lib/stack.ts
  - src/components/branch/BranchRow.tsx
  - src/components/branch/BranchActions.tsx
  - src/components/branch/BranchPushButton.tsx
  - src/lib/push-run.ts
  - src/hooks/useBranchRowKeys.ts
  - src/components/branch/BranchSelectionBar.tsx
  - src/components/shared/CollapsibleCard.tsx
  - src/stores/inline-row-action.ts
  - src/components/branch/SafePushPopover.tsx
  - src/hooks/useSafePush.ts
  - src/stores/safe-push.ts
  - crates/test-utils/src/repo_template.rs
  - src/hooks/useLandingNode.ts
  - src/presenters/landing-node.ts
  - src/presenters/next-step.ts
  - crates/domain-policy/src/landing_node.rs
  - crates/domain-policy/src/next_step.rs
  - crates/repository-model/src/derive.rs
  - crates/operations/src/push.rs
  - crates/operations/src/pull_request.rs
  - crates/operations/src/close.rs
  - src/hooks/useOrphanedStackPullRequest.ts
  - src/presenters/push.ts
---

# Stack Island

Status: Implemented
Date: 2026-10-16, its rows those of the table 2026-10-25

## Purpose

A stack of 25 branches and 178 commits collapsed into one row of the branch table hides what it holds in a tooltip, and
expanded it is 25 rows that still don't say what lands, what failed and what goes out with the next push. A big stack is
therefore an island of its own below the table: its header sizes it and says what it holds, each state a way to its
first branch, and offers the actions of a branch row for the whole stack: the Safe Push of the whole stack first (one
robot lands every branch of it, side branches included), the push of all of it second, nothing while a Safe Push runs
in it; its rows are the rows of the table, so the stack reads as its branches. The rest of the table stays as it is
([Stacked Branches](../stacked-branches/spec.md#ui)).

## Goals

- See a big stack whole: its branches, a row each, as the table shows a branch.
- Know what to do next from the header: what failed, what runs and what it lands, the pull requests, the Safe Push,
  the push; as a branch row offers them, so the header reads as the row of the stack.
- Keep the small stacks and the lone branches in the table, where they read well.

## Non-goals

- An island for a stack inside an issue family.

## Requirements

### Requirement: Which stack is an island: A big stack at the top level is an island below the table

A stack at the top level of the table of 5 branches or more (`STACK_ISLAND_MIN_BRANCHES`), or of 40 commits or more
(`STACK_ISLAND_MIN_COMMITS`, a short stack of long branches), MUST be an island below the table, in the order of the
table.

#### Scenario: A stack of five branches or of forty commits
- **GIVEN** a stack at the top level of the table of 5 branches or more, or a short stack of long branches of 40 commits or more
- **WHEN** the table lays out its rows
- **THEN** the stack is an island below the table, in the order of the table
  [@test] src/stores/branch-table-rows.test.ts (`makes a stack of five branches or forty commits an island, not one in an issue family or on an issue branch that heads one`)

### Requirement: Which stack is an island: A stack of an issue family stays in the table

The stack of an issue branch that heads an issue family (its family follows its stack) and a stack inside a family
MUST stay in the table.

#### Scenario: An issue branch that heads a family, a stack inside a family
- **GIVEN** the stack of an issue branch that heads an issue family, or a stack inside a family, big enough for an island
- **WHEN** the table lays out its rows
- **THEN** the stack stays in the table, the family following its stack
  [@test] src/stores/branch-table-rows.test.ts (`makes a stack of five branches or forty commits an island, not one in an issue family or on an issue branch that heads one`)

### Requirement: Which stack is an island: The rule reads all rows, not the rows shown

The rule MUST read all rows (`stackIslandRoots` of `getBranchTableRows(null)`), not the rows shown: neither a collapse
nor a filter moves a stack between the table and its island.

#### Scenario: A collapse or a filter hides rows of the stack
- **WHEN** a collapse or a filter hides rows of a stack
- **THEN** the stack stays where all rows put it, in the table or in its island

### Requirement: Which stack is an island: The island takes its stack out of the table whole

The island MUST take its stack out of the table whole (`layoutBranchTables`).

#### Scenario: The table without the stack
- **WHEN** a stack is an island
- **THEN** the table holds no row of it, and the island holds every row of it
  [@test] src/stores/branch-table-rows.test.ts (`moves an island out of the table, whole, and lists the shown rows in the order of the screen`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

### Requirement: Which stack is an island: The shown rows are in the order of the screen

The shown rows MUST be in the order of the screen, the table first, then the islands, for the keys, the Shift-click
range, the preview and the selection bar.

#### Scenario: The keys, the Shift-click range, the preview and the selection bar
- **WHEN** the keys, the Shift-click range, the preview or the selection bar go through the shown rows
- **THEN** they go through the rows of the table first, then the rows of the islands, as the screen shows them
  [@test] src/stores/branch-table-rows.test.ts (`moves an island out of the table, whole, and lists the shown rows in the order of the screen`)

### Requirement: Header: The header names the root and sizes the stack

The header MUST name the root and size the stack ("air", "Stack · 7 branches · 14 commits"); the root row has no stack
chip, the header is it.

#### Scenario: The header of the air stack
- **WHEN** the stack of air is an island
- **THEN** its header names "air" and sizes it "Stack · 7 branches · 14 commits"
- **AND** the root row has no stack chip: the header is it
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

### Requirement: Header: The node of the whole stack says where it is on its way into master

Before the name, the node of the whole stack MUST say where it is on its way into master, as the node of a row says it
of a branch ([Branch Row](../branch-row/spec.md#tree)): of its stack branch while the stack forks, of its last branch
for a straight chain (that row's node); the page only words it (`useStackLandingNode`, `presentLandingNode`).

#### Scenario: A stack that forks
- **GIVEN** a stack that forks
- **WHEN** the header shows its node
- **THEN** it is the node of its stack branch, the node Rust decides of it (`nextSteps` of the view under `stack:<root>`, `domain_policy::landing_node`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_carries_the_next_step_and_the_node_of_each_subject_and_a_patch_only_those_that_changed`)
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should say in the island header where the whole stack is on its way into master`)

#### Scenario: A straight chain
- **GIVEN** a straight chain, which has no stack branch
- **WHEN** the header shows its node
- **THEN** it is the node of its last branch, that row's node

### Requirement: Header: The hover of the node says it, also of a collapsed island

The hover of the node of the whole stack MUST say where the stack is ("Ready to land: Safe Push lands 7 branches"), so
a collapsed island still answers it.

#### Scenario: The hover of a stack ready to land
- **GIVEN** a stack whose Safe Push lands 7 branches, ready to land
- **WHEN** the pointer rests on the node of the header, also of a collapsed island
- **THEN** its hover says "Ready to land: Safe Push lands 7 branches"
  [@test] crates/domain-policy/src/landing_node_test.rs (`is_ready_to_land_once_patronus_judged_the_approvals_else_the_start_is_the_users_step`)

### Requirement: Header: The hover of the node is that of the node of a row

The hover MUST be that of the node of a row ([Branch Row](../branch-row/spec.md#tree), `LandingNodeHover`): a failed
or running robot shows below the text with its actions, the robot of the whole stack while the stack forks, else that
of its last branch.

#### Scenario: The failed robot of the whole stack
- **GIVEN** a stack that forks, whose robot of the whole stack failed
- **WHEN** the pointer rests on the node of the header
- **THEN** the robot shows below the text with its actions
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the failed robot of the whole stack in the hover of the island's node, with its actions`)

#### Scenario: The robot of a straight chain
- **GIVEN** a straight chain whose last branch has a failed or running robot
- **WHEN** the pointer rests on the node of the header
- **THEN** the robot of its last branch shows below the text with its actions

### Requirement: Header: The states count the branches by the nodes of their rows

After the name, among the actions of the header (after the button of the pull request of the whole stack, before
the push), the header MUST say where the branches of the stack are on their way into master, by the nodes of their
rows as they show them, as the stack chip counts them (`summarizeStackState` over the whole stack,
`getStackStateChips`), the strongest first in the colours of the nodes.

#### Scenario: The counts of the header
- **WHEN** the branches of the stack are in several states
- **THEN** the header counts them, the strongest first in the colours of the nodes: "1 to fix", "4 landing" with the ring of the robot, "2 waiting"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)

#### Scenario: The kind a row shows
- **WHEN** a branch counts
- **THEN** its kind is the one Rust decides with the move that runs on the row laid over it (`getNodeKindNow`, `presentNodeKind`)
  [@test] src/presenters/landing-node.test.ts (`counts the kind a row shows without its words: the move that runs and the push from its click laid over the node`)

### Requirement: Header: A click on a count shows the first branch in that state

A click on a count MUST show the first branch in that state (`requestReveal`: it expands the island and the row), also
from a collapsed island.

#### Scenario: A count of a collapsed island
- **GIVEN** a collapsed island
- **WHEN** the user clicks a count of its header
- **THEN** the island and the row of the first branch in that state expand
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)

### Requirement: Header: Every branch the robot of the whole stack lands is landing

The robot of the whole stack runs on its stack branch, no branch's: while it runs, every branch it lands MUST be
landing ("7 landing").

#### Scenario: The robot of the whole stack runs
- **WHEN** the robot of the whole stack runs
- **THEN** the header counts every branch it lands as landing, "7 landing"
- **AND** a click on the count shows the root
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should count the robot of the whole stack in the header of its island, a click on it showing the root`)

### Requirement: Header: A failure of the robot of the whole stack is the root's fix

A failure of the robot of the whole stack on the stack branch as the sync built it MUST be one more fix, the root's
("1 to fix"), its count showing the root.

#### Scenario: The robot failed on the stack branch as the sync built it
- **WHEN** the robot of the whole stack failed on the stack branch as the sync built it
- **THEN** the header counts one more fix, the root's, "1 to fix", whose count shows the root
  [@test] src/lib/stack-state.test.ts (`counts a fix of the whole stack, whose stack branch has no row, as the root's`)
  [@test] tests/e2e/patronus/dock.spec.ts (`should count the failed robot of the whole stack only while the sync built its stack branch`)

### Requirement: Header: The line below the name counts the open pull requests of the stack

Below the name, muted, with " · " between its items as between the parts of each, the header MUST name the open pull
requests of the stack ("2 pull requests open · 1 approved · 1 changes requested").

#### Scenario: Two open pull requests of the branches
- **GIVEN** two branches of the stack with open pull requests, one approved, one with changes requested
- **WHEN** the header shows its line below the name
- **THEN** it reads "2 pull requests open · 1 approved · 1 changes requested", muted
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)

### Requirement: Header: The line says what each running robot lands, without a ring

The line below the name MUST say what each running robot lands, with its time left, without a ring: the ring of the
robot is on the count of the branches it lands ("4 landing").

#### Scenario: The robot of a branch runs
- **WHEN** the robot of runtime runs
- **THEN** the line says "Safe Push of runtime lands 4 branches · ~23m left": the branch and the ones its commit has of those it is built on (`useLandingSpans`)
- **AND** the line shows no ring: the ring is on "4 landing"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)

#### Scenario: The robot of the whole stack runs
- **WHEN** the robot of the whole stack runs
- **THEN** the line says "Safe Push of the whole stack lands 7 branches" ([Whole Stack](../whole-stack/spec.md#the-whole-stack-in-the-app)), without a ring
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should count the robot of the whole stack in the header of its island, a click on it showing the root`)

### Requirement: Header: The pull request of the whole stack is counted apart, first

The pull request of the whole stack MUST be counted apart, first, as where the approvals of its Safe Push count:
"Approvals count in #12, the pull request of the whole stack" (" · approved", " · changes requested" as GitHub says).

#### Scenario: The pull request of the whole stack is open
- **GIVEN** #12, the open pull request of the whole stack, and a pull request of a branch
- **WHEN** the header shows its line below the name
- **THEN** it says "Approvals count in #12, the pull request of the whole stack" first, apart from the pull requests of the branches
- **AND** it adds " · approved" or " · changes requested" as GitHub says
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should count the approvals of the whole stack in its pull request: the header says so apart from the branches', and a branch without its own points to it`)

### Requirement: Header: A pull request of the whole stack that no longer matches says so

A pull request of the whole stack that no longer matches the stack MUST say so in `text-warning` instead: "#12 no
longer matches the air stack: auth landed".

#### Scenario: A piece landed a branch of the stack
- **GIVEN** #12, the pull request of the whole stack, after a piece landed a branch of it
- **WHEN** the header shows its line below the name
- **THEN** it says so in `text-warning` instead, as "#12 no longer matches the air stack: auth landed"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should offer the close of the pull request of the whole stack in the island header once it no longer matches`)

### Requirement: Header: The pull request the lines name is a button before the state chips

The lines are inside the toggle of the header, so the pull request they name MUST be a button before the state chips
(`StackPullRequestButton`): "#12" opens it, "Close Pull Request #12…" in `warning` asks to close one that no longer
matches (in the root's row, as from "⋯").

#### Scenario: The pull request of the whole stack opens
- **WHEN** the user clicks "#12" before the state chips
- **THEN** the pull request of the whole stack opens
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should count the approvals of the whole stack in its pull request: the header says so apart from the branches', and a branch without its own points to it`)

#### Scenario: The close of one that no longer matches
- **GIVEN** a pull request of the whole stack that no longer matches it
- **WHEN** the user clicks "Close Pull Request #12…", in `warning`
- **THEN** the confirmation of the close opens in the root's row, as from "⋯"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should offer the close of the pull request of the whole stack in the island header once it no longer matches`)

### Requirement: Header: The line takes no room while it has no item

The line below the name MUST take no room while it has no item.

#### Scenario: No pull request open, no robot running
- **WHEN** the stack has no open pull request and no running robot
- **THEN** the header has no line below the name

### Requirement: Header: The line moves no row when the pull requests come after the rows

The line MUST move no row when the pull requests come after the rows: until the facts of the remote are in, an island
whose line showed once they were in the last time keeps its place, one text line high and empty; the answer fills it
there.

#### Scenario: The pull requests come back after the rows
- **GIVEN** an island whose line showed once the facts of the remote were in the last time
- **WHEN** the rows show before the facts of the remote are in
- **THEN** the line keeps its place, one text line high and empty (`useSettledStackLine`, the roots kept in `stackLines` of `settledLayout` in the repository state, as the columns and Your Turn keep theirs, [Next Step](../next-step/spec.md#facts-without-an-answer))
- **AND** the answer fills it there
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the second header line of a stack island in its place while the pull requests come back`)

### Requirement: Header: The header ends with the Actions cell of a branch row

The header MUST end with the Actions cell of a branch row, for the whole stack, its push, Safe Push slot and "⋯" in
the columns of the rows' ([Branch columns](../../../../docs/design/ui-theme.md#branch-columns)), also while its slot is
empty: the compact push, the Safe Push slot and "⋯".

#### Scenario: The header's slot is empty
- **GIVEN** a Safe Push runs in the stack, so the header's slot is empty
- **WHEN** the table and the island show
- **THEN** the compact push, the Safe Push slot and "⋯" of the header stand in the columns of the rows' Actions cells
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should line up the columns of the table, the island and its header, whatever each card shows`)

### Requirement: Header: The header has no filled button

The header MUST NOT show a filled button: a push is not what lands the stack.

#### Scenario: The push of the header
- **WHEN** the header shows its push
- **THEN** it is no filled button

### Requirement: Header: The push is the compact push of the root's Push with Children

The push MUST be the compact push of a row: "↑ 41", the commits of "Push with Children (7 branches)" of the root, its
accessible name, as the root's menu names it: the branches of the stack that have work for the remote, each after the
branch it is built on (`domain_policy::push::push_plan`, the `children` of `RepositoryView.pushes`), the landed ones
and the ones a running Safe Push holds left out.

#### Scenario: Seven branches with work for the remote
- **GIVEN** the 7 branches of the stack with work for the remote
- **WHEN** the header shows its push
- **THEN** it is the compact push of the commits of "Push with Children (7 branches)" of the root, its accessible name, each branch after the branch it is built on
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`)

### Requirement: Header: The push takes the stack branch of the whole stack along

The push MUST take the stack branch of the whole stack along once it is on the remote or its pull request is open
([Stacked Branches](../stacked-branches/spec.md#pull-requests)): one more branch, a commit it and the branches both send
counted once.

#### Scenario: The stack branch is on the remote
- **GIVEN** the stack branch on the remote, lacking commits the sync built
- **WHEN** the header shows its push
- **THEN** it counts one more branch, "Push with Children (8 branches)", a commit it and the branches both send counted once
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu`)

#### Scenario: Only the stack branch has commits to push
- **GIVEN** the branches on the remote and the stack branch lacking a commit there
- **WHEN** the header shows its push
- **THEN** it is "Push with Children (the whole stack branch)" alone
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu`)

### Requirement: Header: The hover of the push names it and lists the pushes

The hover of the push MUST name it and list the pushes, a line per branch, the stack branch last, and the branches
whose push drops remote commits, which ask first in their rows.

#### Scenario: The hover of the push of the stack
- **WHEN** the pointer rests on the header's push
- **THEN** it says "Pushes 8 branches, parents first:", a line per branch
- **AND** the stack branch last: "user-name/stack/air · the whole stack branch · 2 commits"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`, `should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu`)

#### Scenario: Branches whose push drops remote commits
- **GIVEN** branches of the stack whose push drops remote commits
- **WHEN** the pointer rests on the header's push
- **THEN** the hover lists them: they ask first in their rows

### Requirement: Header: The root's push that drops remote commits asks first

The root's own push that drops remote commits MUST ask first, as Push with Children does
([Stacked Branches](../stacked-branches/spec.md#pull-requests)): nothing goes before the push button of the root opened
its description with them, whose Force Push runs the header's push again on that drop; a branch whose drop nobody
showed stays out.

#### Scenario: The remote branch of the root has commits of another
- **GIVEN** a remote commit the push of the root drops, and one the push of side-test drops
- **WHEN** the user clicks the header's push
- **THEN** nothing goes: the push button of the root opens its description with the commit it drops
- **AND** its Force Push runs the header's push on that drop, side-test, whose drop nobody showed, left out
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should ask in the row of the root before the header's push drops its remote commits, and leave out a branch whose drop nobody showed`)

### Requirement: Header: The push is one atomic push that leaves its hover open

The push MUST be one atomic push of all of them ([Stacked Branches](../stacked-branches/spec.md#pull-requests)): a
click pushes and leaves the hover as it is ([hover descriptions](../../../../docs/design/ui-theme.md)), open while the
pointer stays, also a click well after it opened.

#### Scenario: A click well after the hover opened
- **GIVEN** the hover of the header's push open
- **WHEN** the user clicks the push well after it opened
- **THEN** one push sends every branch of it, parents first
- **AND** the hover stays open while the pointer stays
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`)

### Requirement: Header: The header's push is the one indicator of the stack push

While the push of the stack runs, the header's push MUST show the spinner and the percent, "Pushing 7 branches" its
name and the line of git on hover, the one indicator of the stack push (also of Push with Children from the root's
menu), and every row it pushes MUST show the same progress on its own push button.

#### Scenario: The push of seven branches runs
- **WHEN** the push of the 7 branches runs
- **THEN** the header's push shows the spinner and the percent, named "Pushing 7 branches", the line of git on hover
- **AND** every row it pushes shows the same progress on its own push button
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`)

#### Scenario: Push with Children from the root's menu
- **WHEN** Push with Children from the root's menu runs
- **THEN** the header's push is its indicator too

### Requirement: Header: The push of the stack branch alone shows in the header's push

The push of the stack branch alone, the push step of the restart of its robot, of Request Review… of its pull request
and of the creation of its pull request, MUST show in the header's push: "Pushing the whole stack 45%" its name, the
line of git on hover ("Pushing the whole stack branch of air…" until git reports), no row of a branch busy.

#### Scenario: The restart of the robot of the whole stack pushes it
- **WHEN** the restart of the robot of the whole stack pushes the stack branch
- **THEN** the header's push is named "Pushing the whole stack 45%", the line of git on hover ("Pushing the whole stack branch of air…" until git reports)
- **AND** no row of a branch is busy
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`)

#### Scenario: Request Review… of its pull request pushes it
- **WHEN** Request Review… of the pull request of the whole stack pushes the stack branch
- **THEN** the header's push shows that push, "Pushing the whole stack 45%"
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when Request Review… of its pull request pushes it, the stack waiting from the click`)

#### Scenario: The creation of its pull request pushes it
- **WHEN** the creation of the pull request of the whole stack pushes the stack branch
- **THEN** the header's push shows that push, "Pushing the whole stack 45%"

### Requirement: Header: The start of the Safe Push of the whole stack shows its push in its own button

The start of the Safe Push of the whole stack MUST show its push in its own button instead (the spinner and "45%",
named "Pushing 45%", below), the header's push disabled meanwhile: one indicator, decided once for the header's push
and the root's push button (`useStackBranchPushAlone`).

#### Scenario: The start pushes the stack branch
- **WHEN** the start of the Safe Push of the whole stack pushes its stack branch
- **THEN** its button shows the spinner and "45%", named "Pushing 45%"
- **AND** the header's push is disabled, without the progress
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

### Requirement: Header: The push stays disabled while it would push nothing beyond the root

The push MUST stay in its place, disabled ("↑", named "Push with Children") with the reason in its hover, while it would
push nothing beyond the root.

#### Scenario: The stack and its stack branch are on the remote
- **WHEN** the stack is on the remote and its stack branch has nothing to push either
- **THEN** the push is disabled, "↑", with "Nothing to push: the stack is on the remote" in its hover

#### Scenario: The branches are on the remote, the stack branch is not published
- **WHEN** the branches of the stack are on the remote, and its stack branch is neither on the remote nor in an open pull request
- **THEN** the push is disabled, named "Push with Children", with "The branches of the stack are on the remote. Its stack branch goes there with the pull request or the Safe Push of the whole stack" in its hover
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should push the whole stack from the header in one push, parents first`)

#### Scenario: The branches are on the remote, the stack branch stays
- **WHEN** the branches of the stack are on the remote, and its stack branch stays: the sync did not build it, its push is held, or its push would drop remote commits it lacks
- **THEN** its hover says "The branches of the stack are on the remote." and then why the stack branch stays
  [@test] src/presenters/actions.test.ts (`says why Push with Children leaves the stack branch of the whole stack as it is, and names the children that ask first`)

#### Scenario: Only the root has commits to push
- **WHEN** only the root has commits to push
- **THEN** its hover says "Only air has commits to push: its own push sends them"

#### Scenario: The branches beyond the root ask first
- **WHEN** the only branches beyond the root with work for the remote are ones whose push drops remote commits, which ask first in their rows (`NothingBeyond::AskFirst`), whether the root has commits of its own or not
- **THEN** its hover names the branches that ask first: "login and oauth ask first in their rows: the push drops remote commits"
  [@test] src/presenters/actions.test.ts (`says why Push with Children leaves the stack branch of the whole stack as it is, and names the children that ask first`)
  [@test] crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)

#### Scenario: Merged Safe Pushes landed every branch of the stack
- **WHEN** merged Safe Pushes landed every branch of the stack, its stack branch held too
- **THEN** its hover says "Landed in master with deploy and side-test: nothing of air and its children is left to push"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

### Requirement: Header: The push stays disabled while a Safe Push runs in the stack

The push MUST stay in its place, disabled with the reason of the hold in its hover, while a Safe Push runs in the stack.

#### Scenario: The Safe Push of runtime runs
- **WHEN** the Safe Push of runtime runs in the stack
- **THEN** the push is disabled, named "Push with Children", its hover "A Safe Push of runtime is running: it lands 2 commits of air with it"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)

### Requirement: Header: The push of a stack whose root landed sends what is left

A root a merged Safe Push landed MUST hold nothing: the header pushes what is left of the stack without the landed
branches, and its slot shows what is left to land.

#### Scenario: A commit added after the landing
- **GIVEN** merged Safe Pushes that landed the branches of the stack, and a commit of side-test added since
- **WHEN** the user clicks the header's push
- **THEN** it pushes side-test and the stack branch, without the landed branches
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

#### Scenario: The slot of a stack whose root landed
- **GIVEN** a root a merged Safe Push landed
- **WHEN** the header shows its slot
- **THEN** the slot shows what is left to land

### Requirement: Header: The Safe Push slot is the Safe Push of the whole stack

The Safe Push slot of the header MUST be the Safe Push of the whole stack: one robot lands every branch of it, side
branches included ([Whole Stack](../whole-stack/spec.md#the-safe-push-of-the-whole-stack-in-the-app)).

#### Scenario: A stack with side branches
- **GIVEN** a stack with side branches
- **WHEN** the header shows its Safe Push slot
- **THEN** its Safe Push is one robot that lands every branch of the stack, side branches included
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

### Requirement: Header: Of a stack that forks the slot is the next step of its stack branch

Of a stack that forks, the slot MUST be the next step of its stack branch as Rust decides it (`nextSteps` under
`stack:<root>`, `WholeStackSlot`, [Next Step](../next-step/spec.md#the-whole-stack)): its Safe Push; else the restart
of its robot, "Create Pull Request…" (of the Stack, as its menus say it), Request Review… or the wait in its pull
request.

#### Scenario: The Safe Push of a stack that forks
- **GIVEN** a stack that forks, whose next step is its Safe Push
- **WHEN** the header shows its slot
- **THEN** the slot is "Safe Push · 7"
- **AND** its hover says what it lands, the main line first, then the side branches, which only it lands: "Lands 7 branches · 14 commits into master in one Safe Push: air, bazel, … side-fix and side-test"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)
  [@test] crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)

#### Scenario: Another step of the stack branch
- **GIVEN** a stack that forks, whose next step is not its Safe Push
- **WHEN** the header shows its slot
- **THEN** the slot is the restart of its robot, "Create Pull Request…" of the Stack, Request Review… or the wait in its pull request, as Rust decides it
  [@test] crates/domain-policy/src/next_step_test.rs (`the_whole_stack_takes_the_rows_of_its_stack_branch`)

### Requirement: Header: Of a straight chain the slot is that of the last branch of the main line

Of a straight chain, which has no stack branch, the slot MUST be the slot of the last branch of the main line
(`BranchStructure.mainLine` of its root, which Rust places: at each fork the child with the longest chain), whose one
robot lands it all.

#### Scenario: A straight chain up to deploy
- **GIVEN** a straight chain whose main line ends at deploy
- **WHEN** the header shows its slot
- **THEN** it is the slot of deploy: "Safe Push deploy · lands 5 branches into master"
  [@test] crates/domain-policy/src/stack_test.rs (`the_main_line_takes_the_longest_chain_at_each_fork_and_the_last_of_equal_ones`)

### Requirement: Header: A click on the slot opens its popover at the header's slot

A click MUST ask the start (`requestSafePush` of `stack:<root>`, placement `stack-header`); it lands more than a row,
so it opens its popover at the header's slot (`SafePushPopover`), also while the island is collapsed: nothing expands,
nothing scrolls.

#### Scenario: A click on the slot of a collapsed island
- **GIVEN** a collapsed island
- **WHEN** the user clicks the Safe Push of the whole stack in its header
- **THEN** its popover opens at the header's slot
- **AND** the island stays collapsed: nothing expands, nothing scrolls
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

### Requirement: Header: The popover says what lands and what it pushes

The title of the popover MUST be what lands, then "Pushes only its stack branch user-name/stack/air." with what stays
said only of the branches on the remote, and, with its pull request open and its approvals needed, "Approvals count in
#12, the pull request of the whole stack."

#### Scenario: No branch of the stack on the remote
- **GIVEN** no branch of the stack on the remote
- **WHEN** the popover of the whole stack opens
- **THEN** its title is what lands, then "Pushes only its stack branch user-name/stack/air."
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

#### Scenario: Branches of the stack on the remote, no approvals needed
- **GIVEN** branches of the stack on the remote, and the pull request of the whole stack open with no approvals needed
- **WHEN** the popover of the whole stack opens
- **THEN** it says what stays of the branches on the remote ("…: the branches of the stack stay as they are on the remote.")
- **AND** it says nothing of where the approvals count
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say in the popover of the whole stack only what holds: the branches on the remote stay, no approvals where none are needed`)

#### Scenario: Its pull request open, its approvals needed
- **GIVEN** the pull request of the whole stack open and its approvals needed
- **WHEN** the popover of the whole stack opens
- **THEN** it says "Approvals count in #12, the pull request of the whole stack."
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should say in the popover of the whole stack that its approvals count in its pull request while Patronus judges them`)

### Requirement: Header: Enter pushes the stack branch alone and starts the robot from it

Enter in the popover MUST push the stack branch alone (leased, the push step of the start) and start the robot from it
(`stack: true`), the button busy meanwhile ("Checking…", the spinner and "45%" of its push, "Starting…", at the width it
had at rest), the one indicator of its push: the header's push waits, disabled, without it.

#### Scenario: Enter in the popover of the whole stack
- **WHEN** the user presses Enter in the popover of the whole stack
- **THEN** the stack branch alone is pushed, the button busy with the spinner and "45%" of its push, at the width it had at rest
- **AND** the header's push waits, disabled, without the progress
- **AND** then the robot starts from the stack branch
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should land the whole stack in one Safe Push from the header, its popover at the header`)

#### Scenario: The checks and the start
- **WHEN** the start checks and then starts the robot
- **THEN** the button says "Checking…", then "Starting…", at the width it had at rest

### Requirement: Header: S on the focused header opens the popover of the whole stack

S on the focused header MUST open the same popover of the Safe Push of the whole stack.

#### Scenario: S on the header
- **GIVEN** the header has the focus
- **WHEN** the user presses S
- **THEN** the popover of the Safe Push of the whole stack opens at the header's slot
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should start the whole stack from the island's ⋯ and with S on its header, the popover at the header`)

### Requirement: Header: The Safe Push of the whole stack is disabled with the reason of the whole stack

The Safe Push of the whole stack MUST be disabled with the reason of the whole stack: its stack branch not built, a
robot of a branch of it, its approvals missing in its own pull request.

#### Scenario: The stack branch conflicts
- **GIVEN** the stack branch is not built: it conflicts
- **WHEN** the header shows its slot
- **THEN** its Safe Push is disabled, its hover "The stack branch conflicts at 0caf9066 in config.js: the stack lands in pieces"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)

#### Scenario: A robot of a branch, approvals missing
- **WHEN** a robot of a branch of the stack runs, or the approvals are missing in its own pull request
- **THEN** its Safe Push is disabled with that reason

### Requirement: Header: A stack whose stack branch is not built lands in pieces

While its stack branch is not built, the stack MUST land in pieces: the "⋯" item "Safe Push deploy" (the last branch of
the main line), the action of the notice of S and "Land it in pieces" of the popover open that Safe Push at the
header's slot.

#### Scenario: The item of the menu and the action of the notice of S
- **GIVEN** a stack whose stack branch conflicts
- **WHEN** the user chooses "Safe Push deploy" in "⋯", or the action of the notice of S
- **THEN** the Safe Push of deploy, the last branch of the main line, opens at the header's slot
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)

#### Scenario: Land it in pieces from the popover
- **GIVEN** a stack whose stack branch is not built
- **WHEN** the user chooses "Land it in pieces" in the popover
- **THEN** the Safe Push of the last branch of the main line opens at the header's slot

### Requirement: Header: The rows keep their own popovers at their slots

The rows of the island MUST keep their own popovers at their slots, one popover at a time (`SafePushAsk`).

#### Scenario: The Safe Push of a row of the island
- **WHEN** the user opens the Safe Push of deploy in its row
- **THEN** its popover opens at the slot of the row, the only popover open
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should keep the popover of a row of the island at its slot, and review the whole stack in the window of its stack branch`)

### Requirement: Header: The header shows no key hints

The header MUST NOT show key hints: it is no row.

#### Scenario: The focused header
- **WHEN** the header has the focus
- **THEN** it shows no key hints

### Requirement: Header: While a Safe Push runs in the stack the header offers no Safe Push

While a Safe Push runs in the stack (each one lands the root:
[Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)) the header's push MUST be disabled with the
reason of the hold and the header MUST offer no Safe Push: the landing line and "4 landing" say why, as the Status cell
of a row whose robot runs.

#### Scenario: The Safe Push of a branch of the stack runs
- **WHEN** the Safe Push of runtime runs in the stack
- **THEN** the header's push is disabled with the reason of the hold, and its slot offers no Safe Push
- **AND** the landing line and "4 landing" say why
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)

#### Scenario: The branches it holds and doesn't land
- **WHEN** a Safe Push runs in the stack and holds branches it doesn't land
- **THEN** the header counts them as waiting, "2 waiting"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should say what the stack holds in the header, each state a way to its first branch`)

### Requirement: Header: "⋯" is the menu of the whole stack

"⋯" MUST be the menu of the stack ("More actions for the air stack"), built when it opens: the actions of the whole
stack, the list the menu of every branch of the stack shows under "Whole Stack ›" (`getWholeStackMenuItems`,
[Whole Stack](../whole-stack/spec.md#the-whole-stack-in-the-app)), then "Collapse Stack" / "Expand Stack".

#### Scenario: The items of the menu
- **WHEN** the user opens "⋯" of the header
- **THEN** the menu "More actions for the air stack" lists, in this order: "Safe Push Whole Stack" and "Dry Run Whole Stack", "Safe Push deploy" while the stack lands in pieces, "Create Pull Request of the Stack…" or "Open Pull Request #12" with "Request Review…" of it while it is open, "Push with Children (7 branches)", "Review Whole Stack" and the actions of the robot of the whole stack
- **AND** then "Collapse Stack" / "Expand Stack"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should start the whole stack from the island's ⋯ and with S on its header, the popover at the header`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)

### Requirement: Header: The Safe Push and the Dry Run of the whole stack in the menu open at the header

"Safe Push Whole Stack" and "Dry Run Whole Stack" MUST open their popover at the header's slot, and MUST be disabled
with the reasons of the whole stack.

#### Scenario: Safe Push Whole Stack
- **WHEN** the user chooses "Safe Push Whole Stack" in "⋯"
- **THEN** the popover of the Safe Push of the whole stack opens at the header's slot
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should start the whole stack from the island's ⋯ and with S on its header, the popover at the header`)

#### Scenario: Dry Run Whole Stack
- **WHEN** the user chooses "Dry Run Whole Stack" in "⋯"
- **THEN** the popover of the Dry Run of the whole stack opens at the header's slot
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should check the whole stack in a Dry Run from the island's ⋯, its popover at the header`)

#### Scenario: Disabled with the reasons of the whole stack
- **GIVEN** a stack whose stack branch conflicts
- **WHEN** the user opens "⋯"
- **THEN** "Safe Push Whole Stack" and "Dry Run Whole Stack" are disabled, each with the reason of the whole stack
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)

### Requirement: Header: The menu offers the piece of the main line while the stack lands in pieces

While the stack lands in pieces, "⋯" MUST offer "Safe Push deploy", the Safe Push of the last branch of the main line.

#### Scenario: The stack branch conflicts
- **GIVEN** a stack whose stack branch conflicts
- **WHEN** the user opens "⋯"
- **THEN** it offers "Safe Push deploy"
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should disable the whole stack's Safe Push while its stack branch conflicts, and land it in pieces from the header`)

### Requirement: Header: The menu creates or opens the pull request of the whole stack

"⋯" MUST offer "Create Pull Request of the Stack…" while the whole stack has no pull request, or "Open Pull Request #12"
with "Request Review…" of it while it is open.

#### Scenario: Create Pull Request of the Stack…, then Open Pull Request #12
- **GIVEN** a stack whose whole stack has no pull request
- **WHEN** the user creates it with "Create Pull Request of the Stack…" from "⋯"
- **THEN** "⋯" offers "Open Pull Request #12", which opens it
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should create the pull request of the whole stack from the island's menu and the root's menu, and open it from both`)

### Requirement: Header: Request Review… of the whole stack opens its popover at the header's slot

"Request Review…" of the pull request of the whole stack MUST open its popover at the header's slot, also while a Safe
Push runs in the stack and the slot is empty.

#### Scenario: Request Review… from the island's ⋯
- **GIVEN** the pull request of the whole stack is open
- **WHEN** the user chooses "Request Review…" in "⋯"
- **THEN** its popover opens at the header's slot
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should ask for a review of the pull request of the whole stack from the island's ⋯ and the root's menu, the popover at the header`)

#### Scenario: A Safe Push runs and the slot is empty
- **GIVEN** a Safe Push runs in the stack and the slot of the header is empty
- **WHEN** the user chooses "Request Review…" in "⋯"
- **THEN** its popover opens at the header's slot
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should open Request Review… of the whole stack at the header while its Safe Push runs and the header offers no step`)

### Requirement: Header: Push with Children in the menu is that of the root's menu

"Push with Children (7 branches)" of "⋯" MUST count the stack branch while it goes along, and MUST be disabled with the
reason while a Safe Push holds the root and while it pushes nothing beyond the root, as the root's menu:
[Stacked Branches](../stacked-branches/spec.md#pull-requests).

#### Scenario: The stack branch alone
- **GIVEN** the branches on the remote and the stack branch lacking a commit there
- **WHEN** the user chooses "Push with Children (the whole stack branch)" in "⋯"
- **THEN** it pushes the stack branch alone
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should take the whole stack branch along in the header's push once it is on the remote, and push it alone from the menu`)

#### Scenario: It pushes nothing beyond the root
- **GIVEN** merged Safe Pushes landed every branch of the stack
- **WHEN** the user opens "⋯"
- **THEN** "Push with Children" is disabled with the reason, as in the root's menu
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should hold the push of the whole stack branch once every branch of the stack landed, and push what is left without the landed ones`)

#### Scenario: A Safe Push holds the root
- **GIVEN** a Safe Push holds the root
- **WHEN** the user opens "⋯"
- **THEN** "Push with Children" is disabled with the reason of the hold

### Requirement: Header: Review Whole Stack opens the review window of the stack branch

"Review Whole Stack" MUST open the review window of the stack branch; of a straight chain, of its last branch from the
root; disabled with the reason of the branch menu's (`reviewBlockerOf`, as Rust decides it: no commits, failed,
blocked).

#### Scenario: Review Whole Stack of a stack that forks
- **WHEN** the user chooses "Review Whole Stack" in "⋯"
- **THEN** the review window of the stack branch opens
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should keep the popover of a row of the island at its slot, and review the whole stack in the window of its stack branch`)

#### Scenario: Review Whole Stack of a straight chain
- **GIVEN** a straight chain
- **WHEN** the user chooses "Review Whole Stack" in "⋯"
- **THEN** the review window of its last branch from the root opens

#### Scenario: Nothing to review
- **WHEN** the review has no commits, failed or is blocked, as Rust decides it
- **THEN** "Review Whole Stack" is disabled with the reason of the branch menu's

### Requirement: Header: The menu holds the actions of the robot of the whole stack

"⋯" MUST hold the actions of the robot of the whole stack.

#### Scenario: The robot of the whole stack failed
- **GIVEN** the robot of the whole stack failed on an older stack branch
- **WHEN** the user opens "⋯"
- **THEN** it offers "Restart Safe Push" of the whole stack
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`)

### Requirement: Rows: The rows of the island are the rows of the branch table

The rows of the island MUST be the rows of the branch table, with nothing of their own: the stack graph, the Status
cell, the Actions cell, the menu, the keys, the expansion and the commits it shows
([Branch Row](../branch-row/spec.md)).

#### Scenario: A row of the island
- **WHEN** the island shows a branch of its stack
- **THEN** its row is the row of the branch table: the stack graph, the Status cell, the Actions cell, the menu, the keys, the expansion and the commits it shows
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

### Requirement: Rows: The island is a table of its own with the columns of the branch table

The island MUST be a table of its own without the column heads, with a lane column as wide as its stack, its other
columns those of the branch table ([Branch Row](../branch-row/spec.md#columns)).

#### Scenario: The columns of the island
- **WHEN** the island shows its rows
- **THEN** it has no column heads, its lane column is as wide as its stack, its other columns those of the branch table
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should line up the columns of the table, the island and its header, whatever each card shows`)

### Requirement: Rows: Only the header takes the stack's own parts from its root row

Only the header MUST take the stack's own parts from its root row: the size, and the push and the step of the whole
stack (above).

#### Scenario: The root row of the island
- **WHEN** the island shows its root row
- **THEN** the size, the push and the step of the whole stack are in the header, not in the root row
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

### Requirement: Collapse, keys and filter: A click on the header collapses and expands the island

A click on the header MUST collapse the island into it and expand it; the state is the collapsed stack of the table
(`collapsedStacks`, per repository, by root), so a new window starts with it.

#### Scenario: Collapse, then a new window
- **WHEN** the user clicks the header of an expanded island
- **THEN** the island collapses into its header, which still says what the stack holds
- **AND** the repository keeps the collapsed root, so a new window starts with the island collapsed
- **AND** the next click expands it
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should collapse the island into its header, per repository`)

### Requirement: Collapse, keys and filter: The collapse closes an inline form, never the popover of the header

The collapse MUST close an inline form below a row of the island, and MUST NOT close the Safe Push popover at the
header's slot, which the collapsed header still shows.

#### Scenario: An inline form below a row
- **GIVEN** an inline form open below a row of the island
- **WHEN** the island collapses
- **THEN** the form closes

#### Scenario: The popover at the header's slot
- **GIVEN** the Safe Push popover open at the header's slot
- **WHEN** the island collapses
- **THEN** the popover stays, at the collapsed header

### Requirement: Collapse, keys and filter: The arrows step through the table and the islands as one list

↑/↓ MUST step through the rows of the table and the islands as one list.

#### Scenario: From the last row of the table into the island
- **GIVEN** the last row of the table has the focus
- **WHEN** the user presses ↓, then ↑
- **THEN** the focus goes to the first row of the island, then back
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should step from the table into the island with the arrows, and collapse it with Alt+Left`)

### Requirement: Collapse, keys and filter: Alt+Left from a row of an island collapses it

⌥← from a row of an island MUST collapse it (after the side stack the row is in,
[Stacked Branches](../stacked-branches/spec.md#ui)), and its header takes the focus.

#### Scenario: A row of the main line
- **GIVEN** a row of the main line of the island has the focus
- **WHEN** the user presses ⌥←
- **THEN** the island collapses and its header takes the focus
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should step from the table into the island with the arrows, and collapse it with Alt+Left`)

#### Scenario: A row of a side stack
- **GIVEN** a row of a side stack of the island has the focus
- **WHEN** the user presses ⌥←, then ⌥← again
- **THEN** the side stack collapses first, its root taking the focus, then the island
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should collapse a side stack into its root, from its chip and with Alt+Left before the whole stack`)

### Requirement: Collapse, keys and filter: F2 and a reveal expand a collapsed island

F2 and a reveal of a branch in a collapsed island MUST expand it.

#### Scenario: A reveal of a branch in a collapsed island
- **GIVEN** a collapsed island
- **WHEN** F2 or a reveal goes to a branch in it
- **THEN** the island expands

### Requirement: Collapse, keys and filter: S on the focused header opens the popover, Enter starts it

S on the focused header MUST open the popover of the Safe Push of the whole stack, and Enter MUST start it.

#### Scenario: S, then Enter
- **GIVEN** the header has the focus
- **WHEN** the user presses S, then Enter
- **THEN** the popover of the Safe Push of the whole stack opens, and Enter starts it
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should start the whole stack from the island's ⋯ and with S on its header, the popover at the header`)

### Requirement: Collapse, keys and filter: A side stack of the island is a side stack of the table

A side stack of the island MUST be a side stack of the table ([Stacked Branches](../stacked-branches/spec.md#ui)).

#### Scenario: The chip of a side stack
- **WHEN** the user clicks the stack chip of a side stack of the island
- **THEN** the side stack collapses into its root, as in the table, and the next click expands it
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should collapse a side stack into its root, from its chip and with Alt+Left before the whole stack`)

### Requirement: Collapse, keys and filter: Under the filter an island shows its matches and does not collapse

Under the filter, an island MUST show its matches and, dimmed, the branches they stack on, and MUST NOT collapse.

#### Scenario: A filter that matches a branch of the island
- **WHEN** the filter matches a branch of the island
- **THEN** the island shows it and, dimmed, the branches it stacks on, without collapsing
  [@test] src/stores/branch-table-rows.test.ts (`moves an island out of the table, whole, and lists the shown rows in the order of the screen`)

### Requirement: Collapse, keys and filter: An island without a match is not shown

Under the filter, an island without a match MUST NOT be shown.

#### Scenario: A filter that matches nothing of the island
- **WHEN** the filter matches no branch of the island
- **THEN** the island is not shown

### Requirement: Collapse, keys and filter: An empty table says nothing while islands show matches

The table without a row, while islands show matches, MUST NOT show "No branches match".

#### Scenario: Only the islands match
- **WHEN** the filter matches no row of the table and some of an island
- **THEN** the table has no "No branches match"

## Testing / Local Run

- `pnpm test:unit`: `src/stores/branch-table-rows.test.ts`, `src/lib/stack-state.test.ts`,
  `src/presenters/landing-node.test.ts`; `cargo nextest run -p domain-policy -p repository-model`: the step and the node
  of the whole stack (`next_step_test.rs`, `landing_node_test.rs`, `derive_test.rs`).
- `pnpm e2e`: `tests/e2e/stack-island` (templates `stack_island`, `stack_side_conflict`: the stack branch conflicts).

## References

- [Stacked Branches](../stacked-branches/spec.md): the stack graph, the collapse, Push with Children
- [Branch Row](../branch-row/spec.md): the rows of a table with stacks
