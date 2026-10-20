---
name: Branch Row
description: Requirements for a row of the branch table and of the archived table, the name cell with the stack graph and the issue family indent, the commit count, the Status cell with the status text and the pull request and Landed chips in columns shared by the table and its stack islands, the Actions cell with the compact push button, the Safe Push slot (the robot of the branch, or "Safe Push" with its popover) and the "⋯" menu, hover and click, the keys of the focused row, the dimmed row of the filter, the branch menu and the inline forms below a row (title, progress, focus); and for the archived table, its status words and colours, the merged-by line and the "New" mark.
style: plain-1
targets:
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../crates/sync-core/src/delete_archived_branch.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchContextMenuRow.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/lib/patronus-actions.ts
  - ../../src/components/shared/InlineForm.tsx
  - ../../src/components/shared/InlineInputDialog.tsx
  - ../../src/stores/inline-row-action.ts
  - ../../src/stores/operations.ts
  - ../../src/lib/rewrite.ts
  - ../../src/components/shared/MoreActionsMenu.tsx
  - ../../src/components/shared/MenuItemButton.tsx
  - ../../src/components/branch/BranchNameCell.tsx
  - ../../src/components/branch/IssueFamilyRow.tsx
  - ../../src/components/branch/StackLanes.tsx
  - ../../src/components/branch/LandingNodeHover.tsx
  - ../../src/components/repository/RobotRows.tsx
  - ../../src/stores/stack-view.ts
  - ../../src/components/branch/StackChip.tsx
  - ../../src/hooks/useStackPathPointer.ts
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/branch/BranchStatusBadge.tsx
  - ../../src/components/branch/StatusChip.tsx
  - ../../src/components/branch/PullRequestChip.tsx
  - ../../src/components/branch/PatronusChip.tsx
  - ../../src/components/branch/LandedChip.tsx
  - ../../src/components/branch/BranchActions.tsx
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/components/branch/StackBaseLabel.tsx
  - ../../src/components/branch/SafePushButton.tsx
  - ../../src/components/branch/NextStepSlot.tsx
  - ../../src/components/branch/SafePushPopover.tsx
  - ../../src/hooks/useSafePush.ts
  - ../../src/components/branch/BranchRowStatus.tsx
  - ../../src/hooks/useRowRobot.ts
  - ../../src/hooks/useLandingNode.ts
  - ../../src/lib/landing-node.ts
  - ../../src/components/branch/BranchGraphHelpPopover.tsx
  - ../../src/components/branch/LandingSpanLabel.tsx
  - ../../src/components/shared/RowActions.tsx
  - ../../src/components/branch/ActionTracks.tsx
  - ../../src/components/branch/BranchColumns.tsx
  - ../../src/lib/branch-columns.ts
  - ../../src/hooks/useBranchCopyActions.ts
  - ../../src/lib/clipboard.ts
  - ../../src/components/shared/MoreActionsMenu.tsx
  - ../../src/components/shared/MenuItems.tsx
  - ../../src/components/shared/ContextMenuItems.tsx
  - ../../src/components/shared/DropdownMenuItems.tsx
  - ../../src/components/ui/dropdown-menu.tsx
  - ../../src/components/shared/TableHeader.tsx
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/hooks/useRowExpansion.ts
  - ../../src/hooks/useBranchRowKeys.ts
  - ../../src/lib/branch-row-keys.ts
  - ../../src/hooks/usePreviewHotkeys.ts
  - ../../src/lib/remote-status.ts
  - ../../src/lib/push-summary.ts
  - ../../src/lib/status-chip.ts
  - ../../src/lib/ui-types.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/branch.ts
  - ../../crates/sync-core/src/sync.rs
  - ../../src/lib/stack-lanes.ts
  - ../../src/components/commit/StackCommitLines.tsx
  - ../../src/index.css
  - ../../src/components/branch/ArchivedBranchTableCard.tsx
  - ../../src/components/branch/ArchivedDateCell.tsx
  - ../../src/components/branch/ArchivedBranchesHelpPopover.tsx
  - ../../src/components/shared/SectionCardHeader.tsx
  - ../../src/components/branch/ArchivedBranchStatusBadge.tsx
  - ../../src/components/branch/ArchivedDateCell.tsx
  - ../../src/components/branch/ArchivedBranchExpanded.tsx
  - ../../src/lib/integration-commit.ts
  - ../../src/stores/archived-branches.ts
---

# Branch Row

Status: Implemented
Date: 2026-10-15

## Summary

The branch table lists the active virtual branches, one row per branch; the archived table below it lists the branches
a sync archived. A row reads from left to right: the folder that expands the commits and the branch name, the commit
count, the Status cell and the actions. The Status cell says the state of the branch in its life, its robot while one
runs or once it finished, else its status text, then up to two chips (the pull request, the landed commits), each kind in
its own column, so a look down the table compares like with like. The actions are compact: the copy icon, the push as
an arrow and its count, then the Safe Push slot, the one labelled action, the next step after the robot ("Safe Push",
"Restart Safe Push", the pull request), and "⋯". Every branch has a node at the left edge that says where it is on its way into master; stacked
branches are a graph of those nodes, root on top, with a lane per line that runs in parallel; the branches of an issue
hang one indent level under it. The commits of an expanded branch sit on its lane.

## Goals

- Read the state of every branch in one look: the status text, then each chip kind, in the same column of every row.
- Keep the table still where the user acts: a column never moves under the pointer after a click (a busy label is no
  wider than the label it replaces), nor on the tick of a robot's time; an answer that arrives after the sync (a robot,
  a chip, a step) may widen its column once, and a long subject truncates.
- Spend the width on the names: a column is as wide as its widest content in any card, and no row holds a place for
  a button or a chip that no row shows.
- One meaning per colour, in the row and in its chips: `info` merged or landed, `warning` attention, `destructive` a
  failure, the push colour for the push.
- Show a stack as one graph: a straight chain is one line, a fork opens a lane, and the commits of a branch sit on its
  line, which runs on into its children.

## Non-goals

- The commit rows of an expanded branch: [Commit Row](../commit/commit-row.spec.md).
- What the chips say and when they refresh: [Pull Requests](pull-requests.spec.md), [Patronus](patronus.spec.md),
  [Landed Commits](../repository/landed-commits.spec.md).
- The push labels, colours, descriptions and the push itself: [Outgoing Commits](../repository/outgoing.spec.md).
- What each menu item and inline form does: [Move Commits](move-commits.spec.md), [Unapply](unapply.spec.md),
  [Patronus](patronus.spec.md), [Squash Fixups](../commit/fixups.spec.md), [Amend Changes](../commit/amend.spec.md),
  [Conflict Fix](conflict-fix.spec.md) ("Fix Conflict…").
- The order of the rows and the placement of stacked branches: [Stacked Branches](stacked-branches.spec.md).

## Requirements

### Columns

- The branch table must have the columns Branch Name, Commits, Status and Actions. The name must take the free width,
  the other columns their content width. Commits and Status hide below `sm`.
- The table and its stack islands must share their columns: each column, and each kind of item inside Status and
  Actions (the state, the pull request chip, the Landed chip; the push, the next step, "⋯"), stands at the same place
  in every row of every card and in the header of an island, as wide as its widest item in the view; a kind that no
  row shows takes no room ([Branch columns](../../docs/design/ui-theme.md#branch-columns)).
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should line up the columns of the table, the island and its header, whatever each card shows`)
  [@test] ../../tests/e2e/branch-row-layout/column-widths.spec.ts (`keeps the Actions column at its buttons: no place is held for a push, a step or a copy that no row shows`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)
- A long name or subject must truncate, not wrap: the row stays one line high.
  [@test] ../../tests/e2e/branch-row-layout/column-widths.spec.ts (`gives the free width to the branch name and keeps the other columns at content width`)
- A subject with less than 6rem of free width must drop out instead of truncating to a letter or two.
  [@test] ../../tests/e2e/branch-row-layout/column-widths.spec.ts (`drops a subject that has no room to be read`)

### Name cell

- The name cell must show, in this order: the stack graph (in a table with stacks, [Tree](#tree)), the folder button
  that expands the commits, the branch name with its issue keys as links, the issue chip of a branch whose issue has no
  row above it, the fixup count, a mark (the archived "New", the stack chip of a stack root, [Stacked
  Branches](stacked-branches.spec.md#ui)), then the summary, muted.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display correct branch row structure`)
  [@test] ../../tests/e2e/issue-navigation/issue-navigation.spec.ts (`should render issue links in branch names`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from its commits and undo it`)
- The summary must be the title of the branch's description (`branch.<name>.description`, which `bd describe` and the
  `descriptions` of a `bd group` plan write) as written, read by the sync. Without one, a branch of an issue shows its
  latest subject that is not a cleanup, refactor or format commit, after the issue key, with its conventional-commit
  prefix dimmed; any other branch shows nothing.
  [@test] ../../tests/e2e/branch-row-layout/description-title.spec.ts (`shows the description title of a branch as written, in the row and the preview`)
  [@test] ../../src/lib/branch.test.ts (`shows the description title as written, prefix and all`, `splits the conventional-commit prefix off a subject, to be dimmed`)
- Signed in to YouTrack, the issue keys of its issues in the name show the issue on hover, and the menu of a branch
  whose issue key is of the YouTrack has "Mark ABC-1 Fixed" ([YouTrack](../repository/youtrack.spec.md#mark-fixed)).
  [@test] ../../tests/e2e/youtrack/issue-popover.spec.ts (`should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)
- A title that starts with the issue key the row shows already (the name starts with it, or the issue chip is it) must
  leave it out with the separator after it ("IJPL-1 Fix", "IJPL-1: Fix", "[IJPL-1] Fix" show "Fix"), as the summary of
  a subject does: a pull request title starts with its key, and "IJPL-1  IJPL-1 Fix…" spends the width the rest of the
  title needs. A key in the middle, another key and a key that is a part of the sentence ("TEST-1 and PROD-9: …") stay;
  a title that is the key alone shows nothing. The branch preview, which has the room, and the search keep the title as
  written (`getBranchSummary`, `getShownIssueKey`).
  [@test] ../../src/lib/branch.test.ts (`leaves out the issue key the row shows already, which a title starts with`, `is the chip of the row, else the key the name starts with`)
  [@test] ../../src/lib/commit.test.ts (`drops the key the row shows from %j`, `keeps a key in the middle, another key, a longer key and a key that is a part of the sentence`)
  [@test] ../../tests/e2e/branch-row-layout/description-title.spec.ts (`leaves out the issue key the row shows already, in the title and in the commits`)
  [@test] ../../crates/sync-core/src/sync_test.rs (`test_branches_grouped_carries_the_description_titles`)
- A branch without commits and without an error must have no folder button.

### Tree

- The name cell of every table must start with the graph: a column of lanes (`--bd-lane-width`, 1rem each, as many as
  the table needs), the node of the branch on its lane and, over the whole row height, the lanes passing the row, the
  line up into the node from its parent above, the line down from it to its children or its commits, and the curve
  from the parent's lane into the node of a side child. Every name starts at the same x. A table without a branch
  stacked on another has one lane, the node of each branch alone on it (`StackLanes.alone`), as IntelliJ's log keeps
  its graph column for a straight history.
  [@test] ../../src/stores/branch-table-rows.test.ts (`puts each branch under its parent, depth-first, wherever the parent is in the order`, `keeps a straight chain on one lane and forks a leaf sibling and a side chain off it`, `puts every branch on one lane while no branch is stacked on another: its node says where it is on its way into master`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)
  [@test] ../../tests/e2e/branch-graph/branch-graph.spec.ts (`should give every branch of a table without stacks its node, and keep the push on the dot of the status`)
- The divider above a row must run full width only between the top-level units of the table: a stack, a lone branch,
  an issue family. Inside a unit it starts at the folder buttons (`--bd-tree-x`) above a member of an issue family or
  a stack root in it, one `--bd-family-step` further above a branch stacked on another in a family, and at the
  folder buttons above the other rows of a stack and the rows below a branch row (its form, its amend conflict, its
  commits), so a stack reads as one block and its lanes run on past the dividers (`branchRowDivider`, `data-divider`,
  `bd-branch-rows`): where a long stack ends shows as a full-width line.
  [@test] ../../src/stores/branch-table-rows.test.ts (`runs the divider full width above each top-level unit, from the tree inside a stack or an issue family`)
- The node must be an 8px circle that says where the branch is on its way into master ([Landing](../../docs/design/landing.md)),
  the first state that matches (`getLandingNode`, `landingNodeLooks`), the left edge where a look down the table starts.
  Filled: the next step is the user's and on this row; hollow: it waits for others (people, the parent, the header of
  its island, a robot) or is done. The push is not the node's: it is the dot of the status text in every table.

  | Node | Legend | When |
  |---|---|---|
  | `success`, filled | Ready to land: Patronus counts the approvals, Safe Push lands it | the next step is "Safe Push" or "Restart Safe Push" ([Next Step](next-step.spec.md) rows 2, 3, 11) and Patronus judged the approvals: `approved` or `notRequired`; a bot's approval or a GitHub decision is no judge of what Patronus counts |
  | `primary`, filled | Your step first: a pull request, a review, a retarget, a Safe Push whose approvals Patronus checks at the start | rows 5, 6, 7 (not "In #12"), 9, the pull request of a robot that failed for approvals, and a Safe Push Patronus can't judge (`unknown`, `unavailable`, no pull requests GitHub lists: "Safe Push: Patronus checks the approvals when the robot starts"); without Patronus: no pull request, a draft, or (GitHub lists none) the remote lacks work of the branch and no push of it runs |
  | `destructive`, filled | Needs a fix | a sync error, a conflict, its analysis, a Safe Push that failed on the branch as it is, rows 8 and 8′ (changes requested); without Patronus the failing checks or changes requested of its open pull request |
  | `primary`, hollow | Landing now | its own Safe Push runs (the ring around it), or a running Safe Push of a branch built on it or of the whole stack lands it ("Lands with oauth"); also while its Safe Push starts or restarts (from the target check on, the push included), before the robot exists: "Landing: Safe Push starting", "Landing: Safe Push restarting", and the branches it lands beside: "Lands with oauth: its Safe Push starts" |
  | neutral, hollow | Waits for others | rows 10 and 10′ (awaiting, also the reviewer GitHub waits for while Patronus can't judge), 5′ (another base), "In #12", a robot of an ancestor that a start waits for, Blocked, a Dry Run running, starting or restarting, while its pull request is created or a review is requested in it ("Creating the pull request of auth", "Requesting a review on #1078"), no commits, before the first answer of Patronus; without Patronus an open pull request ("Lands through #7") |
  | `info`, hollow | In master | landed in the baseline, landed with a merged Safe Push, merged by Patronus or by its pull request |

  A sync and an inline action of the row pass: the node keeps the state they started from. A move of the row is made
  when the user makes it (`RunningMove`): a start or a restart of its robot (Push and Restart included), the creation of
  its pull request, a review request leave the filled nodes at the click, not when Patronus or GitHub answer; an action
  that fails gives the node back. Its action declares the move (`OperationSpec.move`; the start of a Safe Push from
  `safePushStore.starting`), so every surface that starts it moves the node alike. Without Patronus a running push is
  the push made. A retarget is no such move: the Safe Push after it is the user's again.
  [@test] ../../src/lib/landing-node.test.ts (`is ready to land once Patronus answered that a Safe Push can land it, with the branches it lands`, `is never ready while Patronus can't judge the approvals: a bot's approval is no judge, GitHub's wait is told`, `waits before the first answer of Patronus, never a green that turns blue`, `is the user's step before it lands: a pull request, a review request, a retarget, a draft`, `needs a fix: a sync error, a conflict, a robot that failed on the branch as it is, changes requested`, `is landing while its own Safe Push runs or one of a branch built on it lands it; a dry run lands nothing`, `waits for others: the reviewers, its parent, a robot that lands an ancestor, a base that may be meant`, `is in master once the baseline has it or a merge says so, also over a robot that failed before`, `keeps its state while a sync or an operation on the row runs`, `lands by its pull request in a repository that is not on Space`, `is the state after a running move of the row from its start on: Landing for a robot that comes, else Waiting`)
  [@test] ../../src/lib/running-move.test.ts (`finds the own move of the row, else a move of a row whose Safe Push lands it beside`, `finds none for a move that lands nothing beside, or of a row that doesn't land it`, `is a move from the target check on, Landing for a Safe Push, Waiting for a Dry Run; none at its checks`)
  [@test] ../../src/stores/operations.test.ts (`tells the moves of the running operations of the repository until they end`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should restart a robot once while its restart runs, running in the slot and in its details, landing from the click`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should push and restart a failed robot whose branch changed since, from the branch preview, landing from the click`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`, `should ask the code owner nobody asked yet from Request Review…, then wait for him from the click on`)
  [@test] ../../tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`, `should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does, the move made from the click`, `should show the push of the stack branch alone in the header's push when Request Review… of its pull request pushes it, the stack waiting from the click`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push popover of the focused row with S and start it with Enter, which takes it out of Your Move`, `should keep a branch under Your Move until the filter changes, once its move is made`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`, `should name in the chip of a collapsed stack only the robots that failed on its branches as they are`, `should say which branches a running Safe Push lands, and not start them on their own`)
- Hovering the node must say its state in the words the row uses elsewhere (the reason of the next step, the status):
  "Ready to land: Safe Push lands 2 branches", "Waits for the approval of IJPL Code Platform: Ilya Korennoy is
  requested on #1078", "Lands with oauth: its Safe Push runs", "Landed in origin/master". Its hover target is the lane's
  16px square around the 8px circle; a click goes on to the row, and never pins the hover.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`, `should say which branches a running Safe Push lands, and not start them on their own`)
  [@test] ../../tests/e2e/branch-graph/branch-graph.spec.ts (`should give every branch of a table without stacks its node, and keep the push on the dot of the status`, `should go on to the row on a click of the node, never pinning its hover`)
- What the hover names must open from it: it is a popover (`LandingNodeHover`), not a tooltip. The issue keys of the
  text are links to the tracker (the issue navigation of the project). A state of the robot of the branch (it failed on
  the branch as it is, it runs, a Dry Run runs) shows the robot below the text as the landing overview lists it
  (`RobotRow`, without the branch name): its state and time (the mode only for a Dry Run, a link that opens it in
  Patronus; the mode and the profile on hover), and its actions from the one list of the branch menu (`getGatedRobotActions`: Restart Safe Push, Copy Prompt for
  Agent, Copy Prompt with CI Details, Open in Patronus, Open Pull Request #N). The failed state says so: "Safe Push
  failed on ABC-123 as it is: fix it, or Restart Safe Push". A blocked branch offers "Show auth", the parent it waits
  for (`LandingNode.shows`). A click in the hover never toggles the row. The node is no
  tab stop: the row has its keys, and the same actions are in its slot, its menu and its robot chip.
  [@test] ../../src/lib/landing-node.test.ts (`needs a fix: a sync error, a conflict, a robot that failed on the branch as it is, changes requested`, `is landing while its own Safe Push runs or one of a branch built on it lands it; a dry run lands nothing`)
  [@test] ../../tests/e2e/branch-graph/branch-graph.spec.ts (`should open the issue and the failed Safe Push from the hover of the node, and restart it or copy the prompt there`)
  [@test] ../../src/lib/landing-node.test.ts (`waits for others: the reviewers, its parent, a robot that lands an ancestor, a base that may be meant`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
- The "Branch Name" head must end with the legend of the nodes, "About Branch Graph" (`BranchGraphHelpPopover`): one
  line on what a node says and what filled means, each kind as its node with its legend, and a line on the lines of
  a stack.
  [@test] ../../tests/e2e/branch-graph/branch-graph.spec.ts (`should explain the nodes in the legend after the Branch Name head`)
- The node of a branch whose robot runs must have the rotating ring of its robot around it. The lines of a
  landing span, from the root of the stack down to the branch whose running Safe Push lands the branches it is built
  on, must be drawn in the colour of the robot: the line down from each branch of the span, the lanes passing the side
  branches between them, and the line up or the fork curve into the next one, also beside the expanded commits and the
  inline forms (`landingSegments`, [Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)).
  [@test] ../../src/lib/stack-lanes.test.ts (`draws the span from the root down to the branch whose robot lands it, past the side branches between`, `curves into a side branch whose robot lands its parent, and leaves the main line alone`, `stops where the graph doesn't connect the branches of a span`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)
- The graph must answer "what is this branch built on" where the question comes up: the path from the root of the
  stack down to a branch (`ancestorSegments`: the line down from each ancestor, the lanes passing the side branches
  between them, the line up or the fork curve into the next one, also beside the expanded commits and the inline
  forms) is highlighted while the pointer rests on its row for 100 ms, at once while it is over the push button, the
  robot in the Status cell or the Safe Push slot of the row, while the keyboard focuses the row or a control in it, and while the popover
  of its Safe Push or Dry Run is open (`pinStackPath`; the pointer wins over the focus, the focus over the popover; `stack-view.ts`). Over the push button,
  the nodes of the branches the push sends are circled as well, so "↑ 18" shows which branches its 18 commits come
  from. A root has no path; a running robot's landing span is drawn over the path. An event from a menu or a popover
  above the table moves no path.
  [@test] ../../src/lib/stack-lanes.test.ts (`draws the path from the root down to a branch, past the side branches between`, `curves into a side branch and leaves the main line alone`, `has no path for a root or a branch without a row`)
  [@test] ../../src/stores/stack-view.test.ts (`highlights the path from the root to the hovered branch, and keeps its state while the branch doesn't change`, `prefers the pointer to the keyboard focus, and the focus to an open Safe Push popover`, `drops the path of a branch that left the rows`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should draw the path to the root of the hovered branch`, `should mark the branches a push sends when its button is hovered`)
- A branch under the branch of its issue or under its issue row must be indented one step (`--bd-family-step`, 1.75rem),
  with a guide over the whole row height under the icon of the row above; the lanes of a stack on it stay in the lane
  column. An issue row without a branch of its own heads its branches with the issue key and their size, the
  members and the stacks on them, muted: "3 branches · 12 commits"; its icon box has the size of the folder button and sits where the folder buttons are, so the guide of its
  branches sits under the icon.
  [@test] ../../src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`, `heads the branches of an issue without a branch of its own with an issue row`)
  [@test] ../../src/stores/branch-table-rows.test.ts (`indents a family member and the stack on it one level, with the lanes of the stack inside`)
  [@test] ../../src/lib/branch.test.ts (`formats the branches and their commits in one summary`)
- The commits of an expanded branch must sit on its lane, a dot per commit at its subject line, beside the lanes that
  run on below the row. In a table with stacks they are oldest first: time runs down with the graph, so the tip of a
  parent sits right above the row of its child ([Stack Island](stack-island.spec.md#rows-and-commits)); without stacks
  newest first, as no graph runs down with time. The lane runs on into the children below, or ends at the last commit
  of a branch without children; a family member keeps its guide beside them.
- The commit subjects must start at the branch name.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should carry the commits of a branch on its lane, into the lines of its children`)
- The inline form row and the amend conflict row of a branch must draw the lanes that run on
  and the family guide, so the graph doesn't pause; their content starts right of them, the title at the branch name.
  [@test] ../../src/lib/stack-lanes.test.ts (`runs the lanes passing a row on below it, and its own while it has children or shows its commits`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should draw the lanes beside the inline form of a stacked branch`)

### Commits cell

- The commits cell must show "1 commit" or "N commits", muted.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display correct branch row structure`)

### Status cell

- The Status cell must show three items in their columns ([Columns](#columns)): the state of the branch (its robot,
  else the status text), the pull request chip, then the Landed chip and the muted "lands with oauth" of a branch a
  running Safe Push of a branch built on it lands ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)),
  else the muted "on master" of a stacked branch built elsewhere because its parent has no commits
  ([Blocked](#blocked)). A missing chip leaves its column empty, and the next chip keeps its own column.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show each pull request state in the status cell`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)
- The first column must say the robot of the branch, the state of the branch in its life, while one runs (its ring,
  its time, the progress line under it) or once it finished on the branch as it is ("Failed", "Needs review", "Merged",
  "Dry run passed", "Canceled"), as the chip of the robot: hover for its details, a click opens it in Patronus
  ([Patronus](patronus.spec.md#robot-of-a-row)). A sync running, an error or a conflict, a block and a landed branch say
  more: the status text then, as for a branch without a robot. The robot reads as the status text it replaces
  (`text-sm`, its icon where the dot is).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
- The status text must follow the first that applies (a robot of the branch comes after the landed branch, before the
  remote status, as above): the sync progress ("syncing 3/10…") of a branch the sync builds anew (a new or changed
  branch, or one the last sync left with an error, a conflict or a block), an error or a conflict
  ("merge conflict"), "analyzing conflict…", a blocked stacked branch ("blocked by auth"), a branch the fetched
  baseline has all of ("landed", [Landed Commits](../repository/landed-commits.spec.md#before-the-update)), the remote
  status ("not pushed", "up to date", "3 (5)↑ · 2↓", "1↑ · 1 amended", "4↑ · restacked", "restacked",
  [What is outgoing](../repository/outgoing.spec.md#what-is-outgoing)), then the result of the sync ("created",
  "updated", "unchanged").
- A branch a sync groups as it was, with the same commits, which the last sync built without a problem, must keep what
  its row shows while the sync rebuilds it (a quiet resync, `isQuietResync`): its status, its node, its synced hashes,
  until the sync reports it, so a sync of 35 branches doesn't turn every row to "syncing…" for a moment. Its push stays
  enabled: a push pressed during the sync waits for it with the spinner of a push and pushes the branch as the sync left
  it; a rewrite (the menu, a drag, M) still waits for the sync to finish.
  [@test] ../../src/stores/branch-sync.test.ts (`keeps what the row of an unchanged branch shows while the sync rebuilds it, and resets a changed or failed one`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should push a branch pressed during a sync once the sync finished, the row as it was meanwhile`)
- Only an error or a conflict must be a badge (`destructive`; `warning` while the conflict is analysed). Every other
  status is muted text after a dot of its colour; the dot of a remote status has the push colour of the branch
  ([Push colours](../repository/outgoing.spec.md#push-colours)), in every table: the node of the row says where it is on
  its way into master ([Tree](#tree)), not its push.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display correct branch row structure`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
  [@test] ../../src/lib/push-summary.test.ts (`takes the hue from the risk: warning only for a push that drops remote commits`, `is neutral for a safe push of no commit of the user`)
- The status text must show as written: the texts are lower case already, and an issue key or an error text keeps its
  case ("blocked by IJPL-123"). Nothing changes the case in CSS; the branch preview shows the same text.
- Hovering the status text must open its description: the remote status in a sentence and the last push time, the
  reason of a block, the error.
- A chip must be a text-sized button. Hovering it opens its details; a click opens its item (the pull request, the
  robot), or the details when it has none (Landed), and never toggles the row. The stack chip after the name of a
  stack root is the one disclosure among them: a click collapses or expands the stack, its hover is a tooltip.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`)
- The details of every chip must have one anatomy: the state line (the icon and the state in the hue of the chip), the
  title or a sentence on what it means, `dl` rows, the notices, then the actions.
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

### Blocked

- A blocked stacked branch must have one treatment: a hollow neutral node ("Blocked by auth, which doesn't build": the
  fix is in the row of its parent), the muted status "blocked by <parent>" after a `warning` dot, a
  disabled push button whose description offers "Show <parent>" too, the hover of the node as well, and, expanded, a
  `warning` alert with the reason and "Show <parent>", which reveals the parent.
  Its commits carry the `warning` Blocked badge.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
- A stacked branch whose declared parent has no commits is not blocked but built elsewhere ("Base changed"): the muted
  "on master" in the third column of the Status cell, the reason in its tooltip ("login landed · oauth is built on
  master"), an enabled push button, and, expanded, an alert "Built on master: login landed." with "Unstack from login"
  ([Stacked Branches](stacked-branches.spec.md#ui)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)

### Actions cell

- The Actions cell must show, in this order: the push button, the Safe Push slot and "⋯", each in its column
  ([Columns](#columns)), so the push arrows, the slot's icons and "⋯" line up down the table and its islands; the
  column is as wide as its widest row and doesn't wrap.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should handle branch with commits state`)
  [@test] ../../tests/e2e/branch-row-layout/column-widths.spec.ts (`gives the free width to the branch name and keeps the other columns at content width`)
- A click in the cell must not toggle the row: on a button, on a disabled one (it passes the click on) and in the
  menu "⋯" opens (its popup is in the row's React tree; a right-click in it doesn't open the row menu either).
- ⌘C (Ctrl+C elsewhere) on the focused row must copy the full branch name, `{prefix}/virtual/{name}`, as "Copy Full
  Branch Name" of the menu, which shows the key; not while text is selected. The row has no copy button: a button on
  every row would hold its place in every row, and 35 copy icons down the table would be the loudest thing in it.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should copy the full branch name with ⌘C on the focused row, the key hint of Copy Full Branch Name`, `should handle branch with commits state`)
  [@test] ../../tests/e2e/stack-island/stack-island.spec.ts (`should line up the columns of the table, the island and its header, whatever each card shows`)
- The push button must follow [Outgoing Commits](../repository/outgoing.spec.md), compact: an up arrow and the count of
  the commits it sends ("↑ 3", the arrow alone without a count), "45%" after the spinner while it pushes. Its label
  ("Push (2)", "Force Push (1)", "Push Stack (3 branches)", "Pushing 45%"; one name for the first push too, whose description
  says "The branch isn't on the remote yet: the push creates it.") is its accessible name and the
  first line of its description on hover, which shows also while it is disabled, with the remote commits a force push
  drops, the question before a push that drops them, and the progress of a running push. Its colour is the push colour
  of the branch as the text of a ghost button (the arrow for `primary`), neutral when landed. A branch up to date with
  its remote has no button: the status text says it, and a disabled arrow in every pushed row would only repeat it; so has a branch a running Safe Push holds, whose Status cell shows the robot or
  "lands with oauth", and its Safe Push slot is empty too ([Patronus](patronus.spec.md#pushing-into-a-branch-whose-safe-push-runs));
  and so has a branch the remote lacks whose landing pushes it (`isFirstPushInLanding`): where Patronus answers or
  GitHub lists the pull requests and the branch has no live pull request, its Safe Push and Create Pull Request push
  it first, so its first push is a step of them, not a move of its own; the menu and P offer it, and Push with Children
  of a root. A branch with a live pull request keeps its button, as the push updates it, and so does a root whose
  Push with Children takes the stack branch of the open pull request of its whole stack along, and every branch of a
  repository with neither Patronus nor GitHub.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should take the whole stack branch along in Push with Children of the root once its pull request is open, from the push and the menu`)
  [@test] ../../src/hooks/usePushButton.test.ts (`leaves the first push of a branch to its landing where Safe Push or Create Pull Request pushes it`, `keeps a push that updates the remote branch, and the first push where nothing else pushes`, `keeps the push of a branch a Safe Push holds: its description says why it won't land and offers the robot`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`)
  A running push keeps its button until it finished, and a blocked branch keeps the
  disabled button with its reason. The focus in the button or its description when it leaves goes to the row, so P and
  Space keep working.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show the percent on the push button and git's last line on hover while the push runs`, `should describe a disabled push on hover`)
  The preview footer keeps the labelled button. A branch with an error or without commits has no push button.
  [@test] ../../src/lib/push-summary.test.ts (`follows the rules of the push button label`)
  [@test] ../../src/hooks/usePushButton.test.ts (`counts the commits the push sends in every label, the user's share in the description`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`, `should show the percent on the push button and git's last line on hover while the push runs`)
- On Space (Patronus answered), the Safe Push slot must show the next step of the branch, the first row of the table
  of [Next Step](next-step.spec.md) that matches: "Safe Push" for a branch with a push button and nothing in its way
  (a click starts it at once when it has nothing to say, else opens its popover at the slot,
  [Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)); the step
  after a robot ("Restart Safe Push"); a quiet wait that opens the pull request it waits in ("Awaiting Review",
  "Changes Requested", "Draft"); the action that helps ("Create Pull Request…", "Request Review…", "Retarget #7"); a stacked branch's own step ("Safe Push · 3"), its popover at its own row; nothing while a robot runs, after a merge, for a
  landed or held branch ([Patronus](patronus.spec.md#robot-of-a-row)). The product lands through Safe Push, so it is
  the one labelled action and not only behind the right-click; every other "Safe Push" (the menu, S, the preview,
  the review) is disabled with the reason of the step. The robot itself is in the Status cell. Nothing in the slot
  changes on the row hover.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`, `should restart a failed robot from its row, the state staying in the Status cell`)
  [@test] ../../tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)
- "⋯" must open the branch menu under the button: exactly the menu of the right-click, so its actions are found without
  knowing the right-click. It is disabled while the row runs an inline action, as the right-click opens no menu then.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the branch menu from the more actions button without toggling the row`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)

### Hover and click

- A row must show the pointer and `bg-muted` on hover; an expanded row keeps `bg-muted/50`.
- A click on the row or on its folder button must toggle its commits. A click on another button, a chip or a link in
  the row must not. Cmd/Ctrl-click and Shift-click must select the row instead (`data-selected`, `bg-primary/15`), for
  "Move to…" of the selected branches ([Move Commits](move-commits.spec.md)).
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should move the selected branches to a new branch with Move to…`)
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should show expanded branch row with commits`)
- A right-click must open the branch menu, the same as "⋯" in the Actions cell. A row running an inline action has no
  menu.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display context menu with copy options`)
- Space over a row must open the [branch preview](branch-preview.spec.md).
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the hovered branch with Space and step through the branches with the arrows`)
- A row with commits must be draggable: it drags the branch, or all selected branches; a collapsed stack root drags its
  own commits, not the stack, and a drop on it moves the commits into it.
- While commits are dragged, every row that takes them must show a dashed outline, and the row under the pointer the
  primary tint and a solid outline; its rows below take the drop too.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should drag the selection onto a branch, but not onto its own branch`)
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should drag an issue branch onto a branch: the issue joins it without a rewrite, and Undo moves it back`)
- A reveal (a branch clicked in the landing overview, "Show auth", moved commits) must expand the row and scroll it
  into view; a row the filter hides shows again first ([Branch Filter](branch-filter.spec.md)), a row a collapsed stack
  hides expands its stack.
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show every branch again for a reveal of a hidden one`, `should show a match deep in a collapsed stack, and expand the stack for a reveal`)
- A row the filter shows only because a match is stacked on it must be dimmed and stay a row: it expands, opens its
  menu and takes the keys ([Branch Filter](branch-filter.spec.md#rows)).
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

### Keyboard

- The keys of a row must act while the focus is on the branch row (one of its buttons too) or on the table, never in
  an input, a menu, a popover, an inline form or the commit list of an expanded row; and only without a modifier, but
  ⌥← and ⌥→, which collapse and expand the stack of the row ([Stacked Branches](stacked-branches.spec.md#ui)).
  A click on a row focuses it; the row is focusable, not a tab stop.
- ↑ and ↓ must move the focus to the previous and the next branch row, past the issue rows, the rows below a branch
  (its commits, an inline form) and the rows a collapsed stack hides, scrolling it into view; from the table itself to the first or the last row. The row
  they focus is the one Space falls back on.
  [@test] ../../src/lib/branch-row-keys.test.ts (`moves to the next or the previous branch row, from the table to the first or the last, and stops at the ends`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should walk the branch rows with the arrows and preview the focused one with Space`)
- Space must open the preview of the focused row while the pointer hasn't moved since the focus moved there, else of
  the row under the pointer ([Branch Preview](branch-preview.spec.md)).
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should walk the branch rows with the arrows and preview the focused one with Space`)
- → must expand the focused row. ← must collapse it, or on a collapsed row move the focus to the branch row it is nested
  under (the branch it is stacked on, or the branch of its issue), as in a tree; nothing under an issue row or at the
  top level.
  [@test] ../../src/lib/branch-row-keys.test.ts (`moves from a row to the branch it is nested under, as in a tree`, `has no parent row under an issue row`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should walk the branch rows with the arrows and preview the focused one with Space`)
- Enter on the focused row must toggle it; with selected rows, Enter opens "Move to…" instead
  ([Move Commits](move-commits.spec.md)). A focused button of the row keeps its Enter.
- P, D, O and M must run the item of the branch menu they are the hints of (`selectRowItem`): the same action, enabled
  as the item is, a running one joined. A disabled item says why in an `info` toast, its reason as the title ("The
  remote branch is up to date", "Unstacking oauth…"); a branch without the item (O without a pull request) does nothing.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show an unstack started from the menu on the row's ⋯ and let the other rewrites wait for it, from a key too`)
- P must push the focused branch as its push button does (`pushOrConfirm`, the stack where the button pushes it, the
  question before a push that drops remote commits), when that button is enabled (`canPushNow`, the rule of P in the
  preview).
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should keep a branch under Your Move until the filter changes, once its move is made`)
- S must open the Safe Push popover at the slot of the focused row when its "Safe Push" is enabled (Patronus answered,
  the row has a push, `canStartRobot`, no inline action running): S never starts by itself (`requestSafePush` with
  `via: "key"`). The popover focuses its start button once it can start, so Enter starts the robot. Otherwise S says
  why in an `info` toast, the reason the button's hover says.
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push popover of the focused row with S and start it with Enter, which takes it out of Your Move`)
- D must review the branch in its review window when its "Review Changes" is enabled. They are keys that look at a row, as Space (`usePreviewHotkeys`): the focused row while the
  pointer hasn't moved since, else the row under the pointer; never from a button, a checkbox or a field of an inline
  form in the row. The keys that act on a row (P, S, O, M) need its focus.
  [@test] ../../tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)
- O must open the pull request of the focused row, as "Open Pull Request" of its menu; nothing for a branch without one.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
- M without selected rows must select the focused row and open "Move to…" for it, as the item of its menu does, where
  the menu offers it (a synced branch with commits, no error); with selected rows M opens it for them
  ([Move Commits](move-commits.spec.md)).
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should open Move to… for the focused row with M, without a selection`)
- F2 and Shift+F2 must move the focus to the next or the previous branch row that is the user's move (the rows of
  Your Move, [Branch Filter](branch-filter.spec.md#state-chips)), the most urgent first: the fixes, then the steps of
  the user, then the branches ready to land, each in table order (`movesByUrgency`), around the ends, from anywhere in
  the window but a text field, a dialog or a menu, as F2 steps through the problems in IntelliJ: from no focused row,
  or one that is no move, the first or the last one. A row in a collapsed stack expands its stack; under a filter only
  the rows it shows count. Nothing happens without a move. The tooltip of the Your Move chip tells the keys.
  [@test] ../../src/lib/branch-row-keys.test.ts (`steps to the next or the previous move by urgency, around the ends`, `starts at the most urgent or the least urgent move from no row or a row that is no move, and finds none without a move`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should step through the user's moves with F2 and Shift+F2, a fix first, into a collapsed stack`)
- P, S, D, O and M work in any keyboard layout (the physical key). The menu items show them (the push `P`, "Safe
  Push" `S`, "Review Changes" `D`, "Open Pull Request" `O`, "Move to…" `M`), and so does the
  hover of "Safe Push" in the row (`S`): a key shown in the menu works on the row.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should push from the branch menu as P does, and say why a pushed branch can't be pushed`)

### Branch menu

- The groups must come in this order: copy ("Copy Full Branch Name", "Copy Branch Name", "Copy Pull Request Link" for a
  branch with a pull request, "Copy Retarget Command" for an open one into a virtual branch), the name ("Rename…" or
  "Name This Branch…", "Move to…", "Ungroup"), the stack ("Stack On…", "Unstack from …", "Collapse Stack" `⌥←` or
  "Expand Stack" `⌥→`, on every branch of a stack "Whole Stack ›", the actions of the whole stack of its root:
  [Whole Stack](whole-stack.spec.md)), the commits ("Add Issue Reference…", "Amend Changes…",
  "Unapply…"), the history ("Review Changes", "Squash N Fixups…", "Open Pull Request" or "Create Pull Request…") and the
  remote: the push of the branch, named as its push button, "Push oauth Only" after it while it pushes the stack
  ([Stacked Branches](stacked-branches.spec.md#pull-requests)), on a branch with branches stacked on it "Push with
  Children (4 branches)" beside it (disabled with the reason while it pushes nothing beyond the branch:
  [Stacked Branches](stacked-branches.spec.md#pull-requests)), then where Patronus answers "Safe Push" (`S`; "Safe Push · 3" for a stacked branch), "Safe
  Push With Profile…" (always the popover, its profile to choose; disabled for a start that comments on a pull request,
  which runs the default checks), "Dry Run" and "Check Approvals Again", then the actions of the branch's robot as
  every surface of the robot lists them (`getRobotActions`, [Patronus](patronus.spec.md#restart-and-cancel): "Restart
  Safe Push" (it pushes first while the remote lacks work), "Cancel Safe Push", "Open in Patronus", and "Open Pull Request #12" for a
  robot of a pull request other than the branch's own).
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
  [@test] ../../src/lib/patronus-actions.test.ts (`cancels a running robot, and opens it in Patronus and the pull request it was started from`)
- The menu of a branch that can't change now (no commits, a sync error, a sync running) keeps every item: the
  rewrites (Rename…, Move to…, Ungroup, Stack On…, Unstack, Add Issue Reference…, Amend Changes…, Unapply…, Squash
  Fixups…) are disabled with the reason ("Wait for the sync to finish", "The branch failed to sync", "The branch has no
  commits", `getRewriteBlocker`); what rewrites nothing on the branch stays as it is: Review Changes, the pull request
  and its review request, Push and Push with Children (they wait for the sync themselves), Safe Push, Safe Push With
  Profile… and Dry Run (disabled with the reasons of the [next step](next-step.spec.md)), the robot's actions.
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`keeps every item: the rewrites wait for the sync, disabled with the reason; the rest stays as it is`, `is offered while a sync runs and while the root is on the remote: the push waits for the sync`)
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep the actions of the branch menu during a sync, the rewrites disabled with the reason`)
- An action of the menu must run once ([Running Operations](../../docs/design/operations.md)): while it runs, until the
  sync that shows its result ends, its item is running (disabled, a spinner in place of its icon) on every surface, a
  second start joins it (a double click runs one `restack_branch`), and every other rewrite of the repository, in every
  branch and commit menu, is disabled with its busy state as the reason: "Unstacking oauth…". A rewrite that must wait
  says so in an info toast titled as its failure; its success toast with Undo comes after the sync.
  [@test] ../../src/stores/operations.test.ts (`runs a second start of the same action once: it joins the running one and resolves with it`, `lets a rewrite wait for any rewrite of the repository, never for a remote action`, `lets a remote action wait for one of the same object or of a branch it works on`, `ends the operation when its action fails`, `marks the item of the running action running, the items that wait for it disabled with its busy state`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should unstack once from the alert, its spinner on the button until the sync shows the branch`, `should show an unstack started from the menu on the row's ⋯ and let the other rewrites wait for it, from a key too`)
- An action started from the menu, a key or a toast must show its spinner on the "⋯" of its row (`aria-busy`, its
  hover the busy state); one started from a button that stays (the notice's "Unstack from login", a form, a dialog,
  the Safe Push popover) shows it there, and "⋯" is only disabled. Every row the operation works on is busy meanwhile:
  no right-click menu, "⋯" disabled, no drag, no push.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should unstack once from the alert, its spinner on the button until the sync shows the branch`, `should show an unstack started from the menu on the row's ⋯ and let the other rewrites wait for it, from a key too`)
- The same error toast must not stack: an error with the title and the description of an open error toast updates
  it in place; Activity records each.
- An item that opens a form must end with "…"; an item that acts at once must not: "Safe Push" and "Dry Run" start at
  once and open their popover only when they have something to say, "Safe Push With Profile…" always opens it. An item that doesn't apply now stays,
  disabled, and says why on hover, never in its name ("Add Issue Reference…" with "Every commit of the branch has an
  issue reference"); it is left out only where it never applies (no pull request, no robot). Every item has a stable
  `id`, what the surfaces and the review window name it by.
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`says why an item doesn't apply in its description, never in its name, and names every item by a stable id`)
- The push item must be the push button of the row as a menu item (`getPushButtonNow`), also where the row has no
  button for a first push: its name ("Push (2)", "Force Push (1)", "Push Stack (3 branches)" with "Pushes auth, login and oauth, parents first" on
  hover), what it pushes (`pushOrConfirm`: the stack only where the button pushes it, the question in the button's
  description before a push that drops remote commits), enabled as that button (`canPushNow`), else disabled with the
  reason ("The remote branch is up to date", the blocked parent, the running Safe Push that holds it, "The stack is
  being pushed" while its stack push runs, "Wait for the sync to finish"). It is the one push of the branch in the
  menu: no second "Push Stack" pushes an ancestor past the question its own push button asks. The branch preview and
  the review list the same item.
  [@test] ../../src/hooks/useBranchContextActions.test.ts (`is one item named as the push button, pushing the stack as far as the button does`, `pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row`, `is disabled with the reason while the stack push runs`)
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should push from the branch menu as P does, and say why a pushed branch can't be pushed`)
- An item shows its key only where the key works: the push `P`, "Safe Push" `S`, "Review Changes" `D`, "Open Pull
  Request" `O` and "Move to…" `M`, the keys of the row ([Keyboard](#keyboard)). "Review Changes" reviews the branch in
  its review window ([Branch Review](branch-review.spec.md)), disabled with the reason for a branch without commits, a
  failed or a blocked one.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request from the branch context menu`)
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should open Move to… for the focused row with M, without a selection`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)

### Inline forms

- A form opened from the menu must render in a row below the branch row, one form at a time, with a visible title
  ("Rename auth") and, where it adds something, a muted description.
- A form with an input must focus the input. A confirmation without one (Unapply, Ungroup) must
  focus its button once the button is enabled, so Enter confirms. A Safe Push or a Dry Run is no inline form: its
  popover opens at the Safe Push slot ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)). Escape cancels.
  [@test] ../../tests/e2e/issue-reference.spec.ts (`should focus input when opened`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)
- While its action runs, the form must be a progress row that says what runs ("Adding ABC-123 to auth…"), and close
  when the action is done, after the sync that shows its result. A failure of a form with an input (rename, issue reference, the archived delete) must show
  the form again with the typed text, focused; the error is a toast.
  [@test] ../../tests/e2e/issue-reference.spec.ts (`should add issue reference to branch via context menu`, `should keep the typed reference when adding it fails`)
- Adding an issue reference rewords the branch's commits under the repository lock: while another Branch Deck
  operation holds it, the reword waits up to 10 s, then it is refused with nothing changed, and the form comes back
  with the error toast that another operation holds the lock.
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
- Closing a form (Escape, Cancel, done) must give the focus back to the element that opened it, or to the row when a
  menu item opened it or the opener is gone. The row is focusable, not a tab stop. A focus the user moved elsewhere
  meanwhile stays.
  [@test] ../../tests/e2e/issue-reference.spec.ts (`should cancel with Escape key`)

### Archived table

- The archived table must have the columns Branch Name, Status, Integrated and Actions. The name shows the last
  segment of the archived branch path; the full name is copied.
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should render archived branches card`)
- The status must be one of three words, in muted text after its icon: "Merged" with the merge icon in `info` (the
  merged colour of the chips), "Partially merged · 1 of 2" with the pie icon, muted, and "Not merged" with the
  warning triangle in `warning`. A spinner in the Status column shows while the integration is detected; the
  Integrated column stays empty until then. Expanding a row loads its commits once, "Loading commits…" with its spinner
  below it; the row itself shows no progress.
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should show the loading of the commits of an expanded row once, below it`)
- A branch the finished sync reported no integration for (its detection failed, or the whole detection did) must show
  "Not checked" with the dashed circle, muted, and the tooltip "Integration could not be checked, see the log",
  instead of spinning until the next sync; its commits still expand. A later result replaces it.
  [@test] ../../src/stores/archived-branches.test.ts (`marks the placeholders a finished sync left without a result as unknown`)
- The tooltip of the status must hold the counts and the merge commit: "3 commits, merged as abc12345 by Safe Push
  Robot", "1 of 2 commits merged; the original commits are no longer in HEAD", "1 commit, none merged; the original
  commits are no longer in HEAD". An exact match adds a `success` "Exact confidence" badge.
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should name the integration status in words, with the counts in the tooltip`)
  [@test] ../../src/lib/integration-commit.test.ts (`names the commit count and the merge commit`, `says what it knows when the count or the merge commit is unknown`, `names the merged part, or says that none merged`)
- The Integrated column must show when the branch was integrated.
- Expanded, an integrated branch must show the merged-by line above its commits: the commit icon in `info`, "Merged as
  abc12345 by Safe Push Robot" (the hash in the monospace font, no time: the Integrated column shows it), the author on
  hover, and a button that copies the full hash.
  [@test] ../../src/lib/integration-commit.test.ts (`names the short hash and the committer, not the time the row already shows`)
  [@test] ../../src/stores/archived-branches.test.ts (`keeps the commit that merged the branch`)
- The header of the archived card must say, after its count, how many archived branches the baseline has none of the
  commits of: "3 not merged" in `warning`, with the tooltip "3 archived branches have commits the baseline has none of",
  so work that may be lost shows without expanding the card; nothing while none is.
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should name the integration status in words, with the counts in the tooltip`)
- The help after the count of the archived card must be titled "Archived Branches" and say when a sync archives a
  branch and which archived branches it deletes: "A sync archives a virtual branch when none of your commits group
  into it anymore. A merged one is deleted by the first sync 7 days after it was archived; the others stay until you
  delete them." It is no part of the toggle, which covers the rest of the header row.
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should explain archiving and cleanup in the help after the title, the header still toggling around it`)
- A branch a sync of this session archived must show an `info` "New" badge after its name until the Landed island is
  dismissed.
  [@test] ../../src/stores/archived-branches.test.ts (`marks the branches archived in this session until cleared`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should lead the toast with what landed and keep the island until it is dismissed`)
- The menu of an archived row must offer the copy items and "Delete Archived Branch…" in the `destructive` colour,
  which asks for the branch name; its Delete button is `destructive` too. The Actions cell has "⋯" with the same menu;
  ⌘C on the focused row copies the full name, as "Copy Full Branch Name".
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should delete archived branch via context menu`)
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should open the menu of an archived row from its more actions button`)
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should copy the full name of an archived branch with ⌘C on its focused row`)
- The delete must remove the archived branch only from the tip it read (one ref transaction with compare-and-swap),
  and must refuse a branch checked out in any worktree, naming that worktree, with the branch left as it is.
  [@test] ../../crates/sync-core/src/delete_archived_branch_test.rs (`deletes_the_archived_branch_only`, `refuses_a_branch_checked_out_in_a_worktree`)

## User Experience

### Row

- Every cell has the vertical padding `--bd-cell-y` (6px), so a row is 36px around the 24px folder button, as dense as
  an IntelliJ list. The name cell has `pl-(--bd-lane-start)`
  (1.25rem, the left edge of a lane-0 node under the header text) and `pr-6`, the folder button at `--bd-tree-x` after
  the lane column. The other cells have `px-4`, `w-px` and `whitespace-nowrap`; inside them each item is a `ColumnTrack`
  (`min-w-(--bd-track-<kind>)`), the Status and Actions items in a `flex` at least as wide as the label of their column.
- The table header is quiet: `text-xs font-medium text-muted-foreground` on the card, with a bottom border and no fill.
- An expanded row fades its commits in (150ms, opacity only).
- The folder button is a ghost `icon-xs` button in `text-muted-foreground`, `text-foreground` while expanded: it
  repeats on every row, so it carries no accent. The name is `text-sm font-medium`, the summary
  `text-xs text-muted-foreground`, the prefix of a subject `font-mono`, muted as the summary.
- The family guide is a `w-px bg-border` line over the cell (`--bd-family-step` in `src/index.css`).
- The lanes (`StackLanes.tsx`) are 1px strokes in `--bd-lane-line`, the muted foreground at 40% over the card: opaque,
  so the curve of a fork doesn't darken the line it leaves, and stronger than a separator. One SVG per cell stretches
  over the row (`preserveAspectRatio="none"`, `vector-effect: non-scaling-stroke`); the fork is a quadratic curve from
  the parent's lane at the top of the row into the left edge of the node, and the lines end at the edge of the node.
  The node is `size-2 rounded-full` with a 1.5px border in the colour of its kind (`currentColor`), `bg-current` when
  filled, centred in a `size-4` hover target (`LaneNodeDot`, which the legend and the island header draw too); a running robot puts its `RobotRing` around it (`lane-ring`: `size-4`, the lane's width, a thinner and wider
  ring than in the slot, so the node keeps its gap; `text-primary`, turning). The
  segments of a landing span are a 1.5px stroke in `--color-primary` over the lines (`landing-lanes`), the colour of
  the ring and of the robot's progress line. The path to the root is a 1.5px stroke in `--bd-lane-path`, the
  foreground at 70% over the card, under the landing span (`stack-path-lanes`), fading in over 120 ms (`bd-path-in`,
  `@starting-style`; none under reduced motion) and gone at once; a node the hovered push sends gets a 1.5px
  `outline-current` 2px off it. The commit dots are 5px in `--bd-lane-dot`
  (`bd-lane-rail`), the content of the rows below a branch row starts at `--bd-row-inset`. The running robot that tests
  the branch draws its line and its dots over them ([Patronus](patronus.spec.md#commits-a-running-robot-covers)).
- The commit count is `text-sm text-muted-foreground`.

### Status

- The items are the tracks `state` (capped at `max-w-40`, its text truncates), `pr` and `landed` (capped at `max-w-48`),
  each after an 8px gutter while its column has content; `landed` is a `flex items-center gap-2` of the Landed chip,
  then the `text-xs text-muted-foreground` "lands with oauth" or "on master". A running robot is at least `13ch` wide ("59m · ~1h 59m"),
  so its ticking time moves no column.
- The status text is `text-sm text-muted-foreground`, truncated in its column, after a `size-1.5` dot. The badge is the
  subtle badge of its colour. The robot in its place is its chip at `text-sm`, `h-6` without padding, a `size-3.5` icon
  (the ring while it runs) where the dot is; the progress line under its time is `h-0.5`, in the padding of the cell.
- A chip (`StatusChip`) is `h-5`, `text-xs` with tabular figures and a `size-3.5` icon, `rounded-2xl` as buttons and
  badges (its focus ring), without a border or an outline, and underlined on hover. A dot is `size-1.5 rounded-full
  shrink-0`. The details popover is `w-80` with `p-3`.
- The colours: `info` merged and landed; `warning` attention (blocked, a pull request or a robot behind the branch,
  "Needs review"); `destructive` a failure or a conflict; `success` approved or passed; the push colour (`primary`, or
  `warning` for a push that drops remote commits) for the push only.
- The blocked alert is the `warning` alert with the warning triangle; its "Show auth" link is `text-warning`. The alert
  of a changed base is `info` with the info icon for a landed parent, `warning` with the triangle for a parent without
  commits, the neutral alert for an unapplied one; its "Unstack from login" link takes the alert's colour.

### Actions

- The push button is a ghost `xs` button, `min-w-16 justify-start` (so "⟳ 100%" fits and the arrows stand in a column),
  with the `ArrowUpIcon` and the count in tabular figures, in the push colour
  (`ghostButtonColorClass`). Its description starts with the label in `font-medium`.
- The Safe Push slot is the track `step`, capped at `max-w-40` ([Patronus](patronus.spec.md#user-experience)). "Safe Push" is a ghost `xs` button with
  the `ShieldCheckIcon` in the foreground colour, "Restart Safe Push" the same with the `RotateCcwIcon`, the pull request
  step with its icon; disabled, they are dimmed to `opacity-50` and keep their hover (`focusableWhenDisabled`). While a
  start of the branch runs, "Safe Push" shows its step with a spinner instead of the shield, `aria-busy`, not dimmed,
  at the width it had at rest (`min-w-26` covers "Checking…"): "Checking…", "45%" of its push as the compact push
  shows it (named "Pushing 45%"), "Starting…".
- "⋯" is a ghost `icon-xs` button with the `EllipsisIcon`, muted, named "More actions for feature-auth", with the
  tooltip "More actions"; while an action of the row started from a menu, a key or a toast runs, a `Spinner` in place
  of the icon, `aria-busy`, the tooltip its busy state ("Unstacking oauth…"). Its menu is the shadcn `DropdownMenu` (Base UI Menu), `align="end"`, as wide as its items
  (`w-auto min-w-36`, as the context menu); one renderer, `MenuItems` with the primitives of either menu, draws both.
- The cell is `ActionTracks`: a `flex items-center` of the tracks `push`, `step` and `more`, the push and the step each
  with an 8px gutter after it while its column has content; the header of a stack island draws the same.

### Inline forms

- The form row is `bg-muted/50` with `bd-padding-content`. Its content is `flex max-w-3xl flex-col gap-4` (`gap-2` for
  a form with one input), so it doesn't stretch across a wide window.
- The title is `text-sm font-medium`, the description below it `text-sm text-muted-foreground`; the progress row keeps
  the title for the screen reader only.
- An input is `max-w-md`, followed by Cancel and the submit button (`xs`); the submit of a delete is `destructive`.
- A focused row is `bg-muted` with an inset `ring-3 ring-ring/30` (`focus-visible`, also when the arrows moved the
  focus there). A dimmed row is `opacity-55`, full on hover or focus.

### Archived table

- The status icon is `size-4`, the word `text-sm text-muted-foreground`. "New" is an `info` badge.
- An expanded archived row is `bg-muted`; its merged-by line is `text-sm text-muted-foreground`, behind the rail of the
  commits.

### Dark theme

- The rows use only shadcn semantic tokens and no `dark:` variants.

## Testing / Local Run

- `pnpm test:unit`: `src/stores/branch-table-rows.test.ts`, `src/lib/stack-lanes.test.ts`,
  `src/lib/branch-row-keys.test.ts`, `src/stores/branch-sync.test.ts`, `src/hooks/useStatusBadge.test.ts`,
  `src/lib/landing-node.test.ts`,
  `src/lib/push-summary.test.ts`, `src/lib/integration-commit.test.ts`, `src/stores/archived-branches.test.ts`.
- `pnpm e2e`: `tests/e2e/branch-row-layout`, `tests/e2e/stacked-branches`, `tests/e2e/branch-graph`, `tests/e2e/branch-filter` (the keys),
  `tests/e2e/archived-branches`, the chip specs and `tests/e2e/issue-reference.spec.ts` for the inline forms (a
  failure is a `page.route` of the command with status 500). E2E reads the status text by
  `data-testid="branch-status"` and its dot by `branch-status-dot` (`data-color`), the graph by `data-lane` on the row,
  `lane-node` (`data-kind`, `data-color`, `data-filled`, `data-ring`) in `lane-node-target`, its hover `lane-node-details` (`data-kind`) with `lane-node-robot`, `lane-ring`, `lane-lines`, `landing-lanes` and `stack-path-lanes` (a node the hovered push sends has
  `data-pushed`), the stack chip by `stack-chip` (`data-stack`: `expanded`, `collapsed`, `filtered`;
  `stack-chip-attention`) and `lane-collapsed`, the dividers by `data-divider`, the family guide by `family-guide`, the rail by `commit-rail`, a dimmed row by `data-dimmed` and the
  archived status by `archived-branch-status` (`data-status`: `merged`, `partial`, `not-merged`, `unknown`).

## Open Questions / Risks

- The columns follow their widest item a frame after it changed (a `ResizeObserver`, `ColumnRegistry`): a check of
  their alignment polls. A column whose widest item changes width relayouts the view once.
- The SVG units assume the 36px row: a taller row (a cell that wraps) stretches the fork curve, and the lines end a
  little off the edge of the node.
- The lane of a branch without children ends at its last commit through a line that starts far above it and is cut
  by the vertical clip of the rail (`clip-path: inset(0 -100vw)`), which also cuts a focus ring above or below it.
- The commit subjects start at the name from `sm` up only: below it the commit rows have `px-4`, the name cell `px-6`.
- No E2E test clicks the row itself (only its folder button) or checks the hover tint and the drop outlines' colours.
- A branch without commits has no folder button, so its name starts 2rem further left than the others.
- The Safe Push slot arrives with the first answer of Patronus, after the sync: the Actions column widens once then, and
  the name column gives the room; so does the Status column when a robot is wider than every status text.
- A robot of a blocked, landed or failing-to-sync branch is not in its row, as the status says more; the branch preview
  and the landing overview show it.
- The compact push button shows the count, not the kind of push: a force push and a first push read alike in the row
  (the status text and the colour tell them apart, the hover names them).

## References

- [Commit Row](../commit/commit-row.spec.md): the commits of an expanded row and their rail
- [Outgoing Commits](../repository/outgoing.spec.md): the push labels, the push colours and the push
- [Pull Requests](pull-requests.spec.md), [Patronus](patronus.spec.md), [Landed Commits](../repository/landed-commits.spec.md):
  the chips
- [Stacked Branches](stacked-branches.spec.md): the order of the rows and the lane layout
- [Branch Preview](branch-preview.spec.md), [Move Commits](move-commits.spec.md)
- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
