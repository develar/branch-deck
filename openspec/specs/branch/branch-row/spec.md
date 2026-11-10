---
name: Branch Row
description: Requirements for a row of the branch table and of the archived table, the name cell with the stack graph and the issue family indent, the commit count, the Status cell with the status text and the pull request and Landed chips in columns shared by the table and its stack islands, the Actions cell with the compact push button, the Safe Push slot (the robot of the branch, or "Safe Push" with its popover) and the "⋯" menu, hover and click, the keys of the focused row, the dimmed row of the filter, the branch menu and the inline forms below a row (title, progress, focus); and for the archived table, its status words and colours, the merged-by line and the "New" mark.
style: plain-1
targets:
  - src/components/branch/BranchTableCard.tsx
  - crates/sync-core/src/delete_archived_branch.rs
  - crates/history-rewrite/src/repository_lock.rs
  - src/components/branch/BranchRow.tsx
  - src/components/branch/BranchRowElement.tsx
  - src/components/shared/TableContextMenu.tsx
  - src/hooks/useLazyRoot.ts
  - src/lib/pointer-moves.ts
  - patches/@base-ui__react@1.9.0.patch
  - src/components/ui/tooltip.tsx
  - src/hooks/useBranchContextActions.ts
  - src/hooks/useSourceMemo.ts
  - crates/domain-policy/src/actions.rs
  - crates/domain-policy/src/landing_node.rs
  - crates/repository-model/src/action_view.rs
  - crates/operations/src/rewrite.rs
  - crates/repository-model/src/warm_start.rs
  - src/presenters/actions.ts
  - src/lib/robot-run.ts
  - src/components/shared/InlineForm.tsx
  - src/components/shared/InlineInputDialog.tsx
  - src/stores/inline-row-action.ts
  - src/stores/indicators.ts
  - src/hooks/useOperationWords.ts
  - src/hooks/useRowOperation.ts
  - src/lib/rewrite-run.ts
  - src/components/shared/MoreActionsMenu.tsx
  - src/lib/menu.ts
  - src/components/shared/MenuItemButton.tsx
  - src/components/branch/BranchNameCell.tsx
  - src/components/branch/IssueFamilyRow.tsx
  - src/components/branch/StackLanes.tsx
  - src/components/branch/LandingNodeHover.tsx
  - src/components/repository/RobotRows.tsx
  - src/stores/stack-view.ts
  - src/components/branch/StackChip.tsx
  - src/hooks/useStackPathPointer.ts
  - src/components/branch/ActiveBranchExpanded.tsx
  - src/components/commit/CommitList.tsx
  - src/hooks/useCommitMoveList.ts
  - src/components/branch/BranchStatusBadge.tsx
  - src/components/branch/StatusChip.tsx
  - src/components/branch/PullRequestChip.tsx
  - src/components/branch/PatronusChip.tsx
  - src/components/branch/LandedChip.tsx
  - src/components/branch/BranchActions.tsx
  - src/components/branch/BranchPushButton.tsx
  - src/components/branch/ChildrenPushButton.tsx
  - src/components/branch/StackBaseLabel.tsx
  - src/components/branch/SafePushButton.tsx
  - src/components/branch/NextStepSlot.tsx
  - src/components/branch/SafePushPopover.tsx
  - src/hooks/useSafePush.ts
  - src/components/branch/BranchRowStatus.tsx
  - src/hooks/useRowRobot.ts
  - src/presenters/row-status.ts
  - crates/domain-policy/src/row_status.rs
  - crates/domain-policy/src/own_facts.rs
  - src/hooks/useLandingNode.ts
  - src/presenters/landing-node.ts
  - src/components/branch/BranchGraphHelpPopover.tsx
  - src/components/branch/LandingSpanLabel.tsx
  - src/components/shared/RowActions.tsx
  - src/components/branch/ActionTracks.tsx
  - src/components/branch/BranchColumns.tsx
  - src/lib/branch-columns.ts
  - src/hooks/useBranchCopyActions.ts
  - src/lib/clipboard.ts
  - src/components/shared/MenuItems.tsx
  - src/components/shared/ContextMenuItems.tsx
  - src/components/shared/DropdownMenuItems.tsx
  - src/components/ui/dropdown-menu.tsx
  - src/components/shared/TableHeader.tsx
  - src/presenters/push-button.ts
  - src/hooks/useBranchPushes.ts
  - src/hooks/useRowExpansion.ts
  - src/hooks/useBranchRowKeys.ts
  - src/lib/branch-row-keys.ts
  - src/hooks/usePreviewHotkeys.ts
  - src/lib/remote-status.ts
  - src/lib/push-summary.ts
  - crates/domain-policy/src/push.rs
  - crates/domain-types/src/remote_status.rs
  - src/lib/status-chip.ts
  - src/lib/ui-types.ts
  - src/stores/branch-sync.ts
  - src/lib/branch.ts
  - crates/sync-core/src/sync.rs
  - src/lib/stack-lanes.ts
  - src/index.css
  - src/components/branch/ArchivedBranchTableCard.tsx
  - crates/domain-policy/src/kept.rs
  - src/hooks/useKeptBranchMenu.ts
  - src/components/branch/ArchivedDateCell.tsx
  - src/components/branch/ArchivedBranchesHelpPopover.tsx
  - src/components/shared/SectionCardHeader.tsx
  - src/components/branch/ArchivedBranchStatusBadge.tsx
  - src/components/branch/ArchivedBranchExpanded.tsx
  - src/lib/integration-commit.ts
  - src/stores/archived-branches.ts
  - crates/repository-model/src/table.rs
  - crates/repository-model/src/session.rs
  - crates/operations/src/push.rs
  - crates/app-api/src/reads/commits.rs
  - src/lib/git/delete-kept-branch.ts
  - src/model/store.ts
  - src/model/branch-row.ts
  - crates/domain-policy/src/admission.rs
  - crates/domain-policy/src/reasons.rs
  - src/presenters/rewrite.ts
  - src/presenters/running-move.ts
  - crates/operations/src/close.rs
  - crates/operations/src/registry.rs
  - src/model/intents.ts
  - src/hooks/useAddIssueReference.ts
  - src/components/branch/InlineIssueReferenceInput.tsx
  - crates/sync-core/src/add_issue_reference.rs
  - crates/branch-integration/src/detector.rs
  - crates/branch-integration/src/strategy.rs
  - crates/branch-integration/src/archive.rs
  - crates/branch-integration/src/rebase.rs
  - crates/branch-integration/src/merge.rs
  - crates/branch-integration/src/squash.rs
  - crates/branch-integration/src/detection_notes.rs
  - crates/branch-integration/src/common.rs
  - crates/domain-types/src/operation.rs
  - crates/domain-types/src/rewrite.rs
  - crates/domain-types/src/branch_integration.rs
  - src/presenters/lock-wait.ts
  - src/presenters/operations.ts
  - src/presenters/reasons.ts
---

# Branch Row

Status: Implemented
Date: 2026-10-15

## Purpose

The branch table lists the active virtual branches, one row per branch; the archived table below it lists the branches
a sync archived. A row reads from left to right: the folder that expands the commits and the branch name, the commit
count, the Status cell and the actions. The Status cell says what happens to the branch, its robot while one runs or
once it finished, else its status text, with the facts the first covers beside it as a mark (a block beside a running
Safe Push), then up to two chips (the pull request, the landed commits), each kind in
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

- The commit rows of an expanded branch: [Commit Row](../../commit/commit-row/spec.md).
- What the chips say and when they refresh: [Pull Requests](../pull-requests/spec.md), [Patronus](../patronus/spec.md),
  [Landed Commits](../../repository/landed-commits/spec.md).
- The push labels, colours, descriptions and the push itself: [Outgoing Commits](../../repository/outgoing/spec.md).
- What each menu item and inline form does: [Move Commits](../move-commits/spec.md), [Unapply](../unapply/spec.md),
  [Patronus](../patronus/spec.md), [Squash Fixups](../../commit/fixups/spec.md), [Amend Changes](../../commit/amend/spec.md),
  [Conflict Fix](../conflict-fix/spec.md) ("Fix Conflict…").
- The order of the rows and the placement of stacked branches: [Stacked Branches](../stacked-branches/spec.md).

## Requirements

### Requirement: Columns: The table has four columns and the name takes the free width

The branch table MUST have the columns Branch Name, Commits, Status and Actions. The name MUST take the free width,
the other columns their content width. Commits and Status hide below `sm`.

#### Scenario: A wide window
- **WHEN** the branch table shows in a wide window
- **THEN** it has the columns Branch Name, Commits, Status and Actions
- **AND** the name takes the free width, the other columns their content width
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`gives the free width to the branch name and keeps the other columns at content width`)

#### Scenario: A window below sm
- **WHEN** the window is below `sm`
- **THEN** Commits and Status hide

### Requirement: Columns: The table and its stack islands share their columns

The table and its stack islands MUST share their columns: each column, and each kind of item inside Status and Actions
(the state, the pull request chip, the Landed chip; the push, the next step, "⋯"), stands at the same place in every
row of every card and in the header of an island, as wide as its widest item in the view; a kind that no row shows
takes no room ([Branch columns](../../../../docs/design/ui-theme.md#branch-columns)).

#### Scenario: The cards show different items
- **WHEN** the table, a stack island and its header show different items in their rows
- **THEN** each column and each kind of item stands at the same place in every row of every card and in the header of the island
- **AND** it is as wide as its widest item in the view
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should line up the columns of the table, the island and its header, whatever each card shows`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)

#### Scenario: A kind that no row shows
- **WHEN** no row shows a kind of item (a push, a step, a copy)
- **THEN** that kind takes no room in its column
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`keeps the Actions column at its buttons: no place is held for a push, a step or a copy that no row shows`)

### Requirement: Columns: A running sync moves no column

A running sync MUST move no column: while a sync runs a column only widens, as the texts of the run ("syncing 3/7…")
change what its items show; once the sync ended, the column MUST follow what its rows show, in the frame that renders
the end and in every frame after it, also when an item showed only in that frame
([Branch columns](../../../../docs/design/ui-theme.md#branch-columns), "A running sync moves no column", "A read is confirmed").

#### Scenario: While a sync runs
- **WHEN** a sync runs and the texts of the run ("syncing 3/7…") change what the items of a column show
- **THEN** the column only widens
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`holds the width of a column while a sync runs, and follows what the rows show once it ended`)
  [@test] src/lib/branch-columns.test.ts (`only widens a track while held, and follows the items as laid out once the hold ends`, `writes no track before the observer measured an item, then every track at once`, `keeps a track at its floor until the floors go, then at what its items show`)

#### Scenario: Once the sync ended
- **WHEN** the sync ended
- **THEN** the column follows what its rows show, in the frame that renders the end and in every frame after it
- **AND** also when an item showed only in that frame ("Checking…" of a step whose fact came before the next frame)
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`holds the width of a column while a sync runs, and follows what the rows show once it ended`, `drops a subject that has no room to be read`)
  [@test] src/lib/branch-columns.test.ts (`follows an item read wider as the hold ends and back at its reported width before the next rendering update`)

### Requirement: Columns: A long name or subject truncates

A long name or subject MUST truncate, not wrap: the row stays one line high.

#### Scenario: A long subject
- **WHEN** a branch has a long name or subject
- **THEN** it truncates and the row stays one line high
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`gives the free width to the branch name and keeps the other columns at content width`)

### Requirement: Columns: A subject without room drops out

A subject with less than 6rem of free width MUST drop out instead of truncating to a letter or two.

#### Scenario: Less than 6rem of free width
- **WHEN** a subject has less than 6rem of free width
- **THEN** it drops out instead of truncating to a letter or two
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`drops a subject that has no room to be read`)

### Requirement: Name cell: The cell shows its parts in a fixed order

The name cell MUST show, in this order: the stack graph (in a table with stacks, [Tree](#tree)), the folder button
that expands the commits, the branch name with its issue keys as links, the issue chip of a branch whose issue has no
row above it, the fixup count, a mark (the archived "New", the stack chip of a stack root), then the summary, muted.

#### Scenario: The structure of a row
- **WHEN** a branch row shows
- **THEN** its name cell has the folder button, the branch name and then the summary, muted, in this order
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display correct branch row structure`)

#### Scenario: Issue keys in the name
- **WHEN** the branch name holds issue keys
- **THEN** they show as links
  [@test] tests/e2e/issue-navigation/issue-navigation.spec.ts (`should render issue links in branch names`)

#### Scenario: A table with stacks
- **WHEN** the table has stacks
- **THEN** the stack graph comes first, and a stack root has the stack chip as its mark ([Stacked
  Branches](../stacked-branches/spec.md#ui))
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)

#### Scenario: A branch with fixups
- **WHEN** a branch has fixups
- **THEN** the fixup count is a button that opens the squash of the fixups ([Squash
  Fixups](../../commit/fixups/spec.md#ui))
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should mark fixups, squash those of a branch from the fixup badge of its row and undo it`)

### Requirement: Name cell: The summary is the title of the branch's description

The summary MUST be the title of the branch's description (`branch.<name>.description`, which `brd describe` and the
`descriptions` of a `brd group` plan write) as written, read by the sync.

#### Scenario: A branch with a description
- **WHEN** a branch has a description
- **THEN** its summary is the title of the description as written, prefix and all
  [@test] tests/e2e/branch-row-layout/description-title.spec.ts (`shows the description title of a branch as written, in the row and the preview`)
  [@test] src/lib/branch.test.ts (`shows the description title as written, prefix and all`)

### Requirement: Name cell: Without a description an issue branch shows its latest subject

Without a description, a branch of an issue MUST show its latest subject that is not a cleanup, refactor or format
commit, after the issue key, with its conventional-commit prefix dimmed; any other branch MUST show nothing.

#### Scenario: A branch of an issue without a description
- **WHEN** a branch of an issue has no description
- **THEN** it shows its latest subject that is not a cleanup, refactor or format commit, after the issue key
- **AND** the conventional-commit prefix of the subject is dimmed
  [@test] src/lib/branch.test.ts (`splits the conventional-commit prefix off a subject, to be dimmed`)

#### Scenario: Any other branch without a description
- **WHEN** a branch that is not of an issue has no description
- **THEN** it shows no summary

### Requirement: Name cell: Signed in to YouTrack, the issue keys show the issue

Signed in to YouTrack, the issue keys of its issues in the name SHALL show the issue on hover, and the menu of a branch
whose issue key is of the YouTrack SHALL have "Mark ABC-1 Fixed" ([YouTrack](../../repository/youtrack/spec.md#mark-fixed)).

#### Scenario: Signed in to YouTrack
- **GIVEN** the user is signed in to YouTrack
- **WHEN** an issue key of its issues in the name is hovered, or the menu of a branch whose issue key is of the YouTrack opens
- **THEN** the issue shows, and the menu has "Mark ABC-1 Fixed"
  [@test] tests/e2e/youtrack/issue-popover.spec.ts (`should open on the keyboard focus, and mark the issue of a branch fixed from its menu`)

### Requirement: Name cell: A title leaves out the issue key the row shows already

A title that starts with the issue key the row shows already (the name starts with it, or the issue chip is it) MUST
leave it out with the separator after it, as the summary of a subject does: a pull request title starts with its key,
and "IJPL-1  IJPL-1 Fix…" spends the width the rest of the title needs.

#### Scenario: A title led by the key the row shows
- **WHEN** the title starts with the issue key the row shows already
- **THEN** "IJPL-1 Fix", "IJPL-1: Fix", "[IJPL-1] Fix" show "Fix", in the title and in the commits
  [@test] src/lib/branch.test.ts (`leaves out the issue key the row shows already, at which Rust split the title`)
  [@test] tests/e2e/branch-row-layout/description-title.spec.ts (`leaves out the issue key the row shows already, in the title and in the commits`)

#### Scenario: A key that does not lead, another key, a key of the sentence
- **WHEN** the key stands in the middle, is another key, or is a part of the sentence ("TEST-1 and PROD-9: …")
- **THEN** the title stays as written

#### Scenario: A title that is the key alone
- **WHEN** the title is the key alone
- **THEN** it shows nothing

### Requirement: Name cell: The preview and the search keep the title as written

The branch preview, which has the room, and the search MUST keep the title as written (`getBranchSummary`).

#### Scenario: The title in the preview
- **WHEN** the branch preview or the search shows a title that starts with the key of the row
- **THEN** it keeps the title as written

### Requirement: Name cell: Rust splits the title at the key it starts with

Rust MUST split the title at the key it starts with (`descriptionLead` of the branch head, `issue_lead`) and name the
key the name starts with (`nameIssueKey`); the page compares keys, no grammar of its own.

#### Scenario: A title led by an issue key
- **WHEN** the sync reads a title led by an issue key
- **THEN** Rust splits it at the key and its separator, and names the key the name starts with
- **AND** the page only compares the keys
  [@test] crates/sync-utils/src/issue_pattern.rs (`a_title_led_by_an_issue_key_splits_at_it_and_its_separator`)
  [@test] crates/sync-core/src/sync_test.rs (`test_branches_grouped_carries_the_description_titles`)

### Requirement: Name cell: A branch without commits has no folder button

A branch without commits and without an error MUST have no folder button.

#### Scenario: A branch without commits
- **WHEN** a branch has no commits and no error
- **THEN** its row has no folder button

### Requirement: Tree: The name cell of every table starts with the graph

The name cell of every table MUST start with the graph: a column of lanes (`--bd-lane-width`, 1rem each, as many as
the table needs), the node of the branch on its lane and, over the whole row height, the lanes passing the row, the
line up into the node from its parent above, the line down from it to its children or its commits, and the curve
from the parent's lane into the node of a side child. Every name starts at the same x.

#### Scenario: A table with stacks
- **WHEN** a table has branches stacked on others
- **THEN** each branch stands under its parent, depth-first, wherever the parent is in the order
- **AND** a straight chain keeps one lane, and a leaf sibling and a side chain fork off it, by the curve from the parent's lane
  [@test] src/stores/branch-table-rows.test.ts (`puts each branch under its parent, depth-first, wherever the parent is in the order`, `keeps a straight chain on one lane and forks a leaf sibling and a side chain off it`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`)

### Requirement: Tree: A table without stacked branches keeps one lane

A table without a branch stacked on another MUST have one lane, the node of each branch alone on it
(`StackLanes.alone`), as IntelliJ's log keeps its graph column for a straight history.

#### Scenario: No branch is stacked on another
- **WHEN** no branch of the table is stacked on another
- **THEN** every branch is on one lane, its node alone on it, saying where it is on its way into master
  [@test] src/stores/branch-table-rows.test.ts (`puts every branch on one lane while no branch is stacked on another: its node says where it is on its way into master`)
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should give every branch of a table without stacks its node, and keep the push on the dot of the status`)

### Requirement: Tree: The divider runs full width only between top-level units

The divider above a row MUST run full width only between the top-level units of the table: a stack, a lone branch, an
issue family. Inside a unit it starts at the folder buttons, so a stack reads as one block and its lanes run on past
the dividers (`branchRowDivider`, `data-divider`, `bd-branch-rows`): where a long stack ends shows as a full-width line.

#### Scenario: The dividers of a unit
- **WHEN** rows of the same unit follow each other
- **THEN** the divider starts at the folder buttons (`--bd-tree-x`) above a member of an issue family or a stack root in it
- **AND** one `--bd-family-step` further above a branch stacked on another in a family
- **AND** at the folder buttons above the other rows of a stack and the rows below a branch row (its form, its amend conflict, its commits)
  [@test] src/stores/branch-table-rows.test.ts (`runs the divider full width above each top-level unit, from the tree inside a stack or an issue family`)

#### Scenario: Between top-level units
- **WHEN** a row starts a stack, a lone branch or an issue family
- **THEN** the divider above it runs full width
  [@test] src/stores/branch-table-rows.test.ts (`runs the divider full width above each top-level unit, from the tree inside a stack or an issue family`)

### Requirement: Tree: The node says where the branch is on its way into master

The node MUST be an 8px circle that says where the branch is on its way into master
([Landing](../../../../docs/design/landing.md)), the first state of the table that matches (`getLandingNode`,
`landingNodeLooks`), the left edge where a look down the table starts. Filled: the next step is the user's and on this
row; hollow: it waits for others (people, the parent, the header of its island, a robot) or is done.

#### Scenario: The table
- **WHEN** a branch row shows its node
- **THEN** the node is the first state of the table that matches
  | Node | Legend | When |
  |---|---|---|
  | `success`, filled | Ready to land: Patronus counts the approvals, Safe Push lands it | the next step is "Safe Push" or "Restart Safe Push" ([Next Step](../next-step/spec.md) rows 2, 3, 11) and Patronus judged the approvals: `approved` or `notRequired`; a bot's approval or a GitHub decision is no judge of what Patronus counts |
  | `primary`, filled | Your step first: a pull request, a review, a retarget, a branch its build needs, a Safe Push whose approvals Patronus checks at the start | rows 1′ (the branch its Safe Push needs, [Next Step](../next-step/spec.md#what-the-safe-push-needs)), 5, 6, 7 (not "In #12"), 9, the pull request of a robot that failed for approvals, and a Safe Push Patronus can't judge (`unknown`, `unavailable`, no pull requests GitHub lists: "Safe Push: Patronus checks the approvals when the robot starts"); without Patronus: no pull request, a draft, or (GitHub lists none) the remote lacks work of the branch and no push of it runs |
  | `destructive`, filled | Needs a fix | a sync error, a conflict, its analysis (also while its own robot runs, with the ring, or a Safe Push of a branch built on it lands it: the conflict came after the push, of new work or of a baseline the robot meets too, and the robot lands the remote copy), a Safe Push that failed on the branch as it is (unless a running Safe Push of a branch it is built on lands a part of it: it waits for that one), rows 8 and 8′ (changes requested); without Patronus the failing checks or changes requested of its open pull request |
  | `primary`, hollow | Landing now | its own Safe Push runs (the ring around it), or a running Safe Push of a branch built on it or of the whole stack lands it ("Lands with oauth"), also a blocked branch: the Safe Push lands the copy it took, whatever its parent builds to now, so the move is the robot's (not a Dry Run, nor one that will fail or cancels); also while its Safe Push starts or restarts (from the target check on, the push included), before the robot exists: "Landing: Safe Push starting", "Landing: Safe Push restarting", and the branches it lands beside: "Lands with oauth: its Safe Push starts" |
  | neutral, hollow | Waits for others | rows 10 and 10′ (awaiting, also the reviewer GitHub waits for while Patronus can't judge), 5′ (another base), "In #12", a robot of an ancestor that a start waits for (also over a failure of its own earlier piece; "Show auth" in the hover), Blocked (no Safe Push that will land it runs), a Dry Run running, starting or restarting, while its pull request is created or a review is requested in it ("Creating the pull request of auth", "Requesting a review on #1078"), no commits, before the first answer of Patronus; without Patronus an open pull request ("Lands through #7") |
  | neutral, hollow | Parked: kept local, never pushed | the user parked it or a branch it is built on ([Parked Branches](../parked-branches/spec.md)), also with a conflict ("It doesn't build: fix it before you unpark it"); not while a robot runs on it, a Safe Push of a branch built on it lands it, a move of it runs, or it landed |
  | `info`, hollow | In master | landed in the baseline, landed with a merged Safe Push, merged by Patronus or by its pull request; also with a conflict of what is left to build, which the update archives |
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_node_of_the_subject_itself_parked_failing_landed_blocked_or_its_robot_running_stays_through_a_move`, `is_ready_to_land_once_patronus_judged_the_approvals_else_the_start_is_the_users_step`, `waits_for_others_the_reviewers_a_base_that_may_be_meant_and_is_the_users_step_otherwise`, `waits_for_the_first_answers_as_the_step_does_and_says_a_read_that_failed`)
  [@test] src/presenters/landing-node.test.ts (`words where a start lands it: ready with the branches it lands, the user's step while Patronus checks the approvals at the start`, `words the step of the node as its slot says it: a pull request, a review request, a wait, In #12, Checking…`, `waits for the first answers and says a read that failed`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`, `should name in the chip of a collapsed stack only the robots that failed on its branches as they are`, `should say which branches a running Safe Push lands, and not start them on their own`)

### Requirement: Tree: The push is not the node's

The push MUST NOT be shown by the node: it is the dot of the status text in every table.

#### Scenario: A branch with commits to push
- **WHEN** a branch has commits to push
- **THEN** its node keeps its landing state, and the push is the dot of its status text
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should give every branch of a table without stacks its node, and keep the push on the dot of the status`)

### Requirement: Tree: A sync and an inline action of the row leave the node

A sync and an inline action of the row MUST pass the node by: the node keeps the state they started from.

#### Scenario: A sync runs
- **WHEN** a sync or an inline action of the row runs
- **THEN** the node keeps the state they started from
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_node_of_the_subject_itself_parked_failing_landed_blocked_or_its_robot_running_stays_through_a_move`)

### Requirement: Tree: A move of the row is made when the user makes it

A move of the row MUST be made when the user makes it (`RunningMove`): a start or a restart of its robot (Push and
Restart included), the creation of its pull request, a review request leave the filled nodes at the click, not when
Patronus or GitHub answer; an action that fails gives the node back. A retarget is no such move: the Safe Push after it
is the user's again.

#### Scenario: A start or a restart of the robot
- **WHEN** the user starts or restarts the robot of the row (Push and Restart included)
- **THEN** the node leaves the filled nodes at the click, not when Patronus answers
  [@test] tests/e2e/patronus/patronus.spec.ts (`should restart a robot once while its restart runs, running in the slot and in its details, landing from the click`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should push and restart a failed robot whose branch changed since, from the branch preview, landing from the click`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push popover of the focused row with S and start it with Enter, which takes it out of Your Move`, `should keep a branch under Your Move until the filter changes, once its move is made`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should show the push of the stack branch alone in the header's push when the restart of its robot pushes it, the stack landing from the click`)

#### Scenario: The creation of a pull request and a review request
- **WHEN** the user creates the pull request of the row or requests a review in it
- **THEN** the node leaves the filled nodes at the click, not when GitHub answers
  [@test] tests/e2e/next-step/next-step.spec.ts (`should create the pull request of a branch not on the remote yet: Create pushes it, Cancel pushes nothing, the move made from the click`, `should ask the code owner nobody asked yet from Request Review…, then wait for him from the click on`)
  [@test] tests/e2e/stack-island/whole-stack.spec.ts (`should ask the code owners of the whole stack in the pull request its header creates, as the step of a branch does, the move made from the click`, `should show the push of the stack branch alone in the header's push when Request Review… of its pull request pushes it, the stack waiting from the click`)

#### Scenario: The state of the subject itself
- **WHEN** a move of the row runs while the sync of the branch failed or it conflicts (parked or not), all of it is in master, it is blocked or its own robot runs
- **THEN** the node keeps the state of the subject itself, which a move doesn't replace
- **AND** a move of a parked branch without a conflict, or the restart of a robot that failed, shows in place of the node
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_node_of_the_subject_itself_parked_failing_landed_blocked_or_its_robot_running_stays_through_a_move`)

#### Scenario: An action that fails
- **WHEN** the action of the move fails
- **THEN** the node is given back

#### Scenario: A retarget
- **WHEN** the user retargets the pull request of the branch
- **THEN** it is no move: the Safe Push after it is the user's again

### Requirement: Tree: The operation of the session declares the move

The operation of the session MUST declare the move (`RunningOperation.moves` of the view, from its admission: a start,
a restart, a creation of a pull request, a review request), so every surface that starts it moves the node alike, in
every window. Without Patronus a running push is the push made.

#### Scenario: The moves of the operations of the session
- **WHEN** an operation of the session runs a move
- **THEN** the node of its own row, and of a row whose Safe Push lands it beside, says the move
- **AND** a move that lands nothing beside, or of a row that doesn't land it, moves no other node
  [@test] src/presenters/running-move.test.ts (`finds the own move of the row, else a move of a row whose Safe Push lands it beside`, `finds none for a move that lands nothing beside, or of a row that doesn't land it`, `words the moves of the operations of the session as the node of each row says them`)

#### Scenario: A push without Patronus
- **WHEN** a repository without Patronus runs a push of the branch
- **THEN** the running push is the push made
  [@test] src/presenters/landing-node.test.ts (`is the pushed branch from the click of its push where it asks for the push, in a repository that is not on Space`)

### Requirement: Tree: Hovering the node says its state in the words of the row

Hovering the node MUST say its state in the words the row uses elsewhere (the reason of the next step, the status).
Its hover target is the lane's 16px square around the 8px circle ([hover descriptions](../../../../docs/design/ui-theme.md)).

#### Scenario: The words of the hover
- **WHEN** the pointer rests on the node
- **THEN** the hover says its state as the row does: "Ready to land: Safe Push lands 2 branches", "Waits for the approval of IJPL Code Platform: Ilya Korennoy is
  requested on #1078", "Lands with oauth: its Safe Push runs", "Landed in origin/master"
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show the branches of a stack under their parents`, `should say which branches a running Safe Push lands, and not start them on their own`)
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should give every branch of a table without stacks its node, and keep the push on the dot of the status`)

### Requirement: Tree: A click on the node goes on to the row

A click on the node MUST go on to the row, and MUST neither pin nor close the hover.

#### Scenario: A click on the node
- **WHEN** the user clicks the node
- **THEN** the click goes on to the row, and the hover is neither pinned nor closed
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should go on to the row on a click of the node, never pinning its hover`)

### Requirement: Tree: What the hover of the node names opens from it

What the hover names MUST open from it: it is a popover (`LandingNodeHover`), not a tooltip. The issue keys of the text
are links to the tracker (the issue navigation of the project).

#### Scenario: The issue of the hover
- **WHEN** the hover of the node names an issue
- **THEN** its key is a link to the tracker that opens the issue
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should open the issue and the failed Safe Push from the hover of the node, and restart it or copy the prompt there`)

### Requirement: Tree: The hover of the node shows the robot of the branch

A state of the robot of the branch (it failed on the branch as it is, it runs, a Dry Run runs) MUST show the robot below
the text as the landing overview lists it (`RobotRow`, without the branch name), with its actions from the one list of
the branch menu (`getRobotItems`).

#### Scenario: A robot that failed on the branch as it is
- **WHEN** the Safe Push of the branch failed on the branch as it is
- **THEN** the hover says so: "Safe Push
  failed on ABC-123 as it is: fix it, or Restart Safe Push"
- **AND** the robot shows below the text, its state and time (the mode only for a Dry Run, a link that opens it in Patronus; the mode and the profile on hover)
- **AND** its actions: Restart Safe Push, Copy Prompt for Agent, Copy Prompt with CI Details, Open in Patronus, Open Pull Request #N
  [@test] crates/domain-policy/src/landing_node_test.rs (`the_node_of_the_subject_itself_parked_failing_landed_blocked_or_its_robot_running_stays_through_a_move`)
  [@test] src/presenters/landing-node.test.ts (`words the state of the subject itself: parked, a failure, landed, blocked, its robot running, which a move doesn't replace`)
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should open the issue and the failed Safe Push from the hover of the node, and restart it or copy the prompt there`)

#### Scenario: A running robot or Dry Run
- **WHEN** the robot of the branch runs, or a Dry Run of it runs
- **THEN** the robot shows below the text of the hover as the landing overview lists it

### Requirement: Tree: The hover of a blocked branch offers the parent it waits for

The hover of a blocked branch MUST offer "Show auth", the parent it waits for (`LandingNode.shows`).

#### Scenario: A blocked branch
- **WHEN** the user hovers the node of a branch blocked by its parent
- **THEN** the hover offers "Show auth", the parent it waits for
  [@test] crates/domain-policy/src/landing_node_test.rs (`waits_for_others_the_reviewers_a_base_that_may_be_meant_and_is_the_users_step_otherwise`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)

### Requirement: Tree: The node is no tab stop and its hover toggles no row

A click in the hover MUST NOT toggle the row. The node MUST NOT be a tab stop: the row has its keys, and the same
actions are in its slot, its menu and its robot chip.

#### Scenario: A click in the hover
- **WHEN** the user clicks in the hover of the node
- **THEN** the row does not expand or collapse
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should open the issue and the failed Safe Push from the hover of the node, and restart it or copy the prompt there`)

#### Scenario: Tab through the row
- **WHEN** the user tabs through the row
- **THEN** the node takes no focus

### Requirement: Tree: The Branch Name head ends with the legend of the nodes

The "Branch Name" head MUST end with the legend of the nodes, "About Branch Graph" (`BranchGraphHelpPopover`): one line
on what a node says and what filled means, each kind as its node with its legend, and a line on the lines of a stack.

#### Scenario: The legend
- **WHEN** the user opens "About Branch Graph" after the "Branch Name" head
- **THEN** it explains the nodes, each kind as its node with its legend, and the lines of a stack
  [@test] tests/e2e/branch-graph/branch-graph.spec.ts (`should explain the nodes in the legend after the Branch Name head`)

### Requirement: Tree: A running robot rings its node and colours its landing span

The node of a branch whose robot runs MUST have the rotating ring of its robot around it. The lines of a landing span,
from the root of the stack down to the branch whose running Safe Push lands the branches it is built on, MUST be drawn
in the colour of the robot (`landingSegments`, [Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)).

#### Scenario: The span of a running Safe Push
- **WHEN** the Safe Push of a branch runs and lands the branches it is built on
- **THEN** its node has the rotating ring of its robot
- **AND** the line down from each branch of the span, the lanes passing the side branches between them, and the line up or the fork curve into the next one are in the colour of the robot, also beside the expanded commits and the inline forms
  [@test] src/lib/stack-lanes.test.ts (`draws the span from the root down to the branch whose robot lands it, past the side branches between`, `curves into a side branch whose robot lands its parent, and leaves the main line alone`, `stops where the graph doesn't connect the branches of a span`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)

### Requirement: Tree: The landing span holds the branches the robot's commit has

The landing span MUST hold the branches the robot's commit has: a root a restack placed below the branch after the start
is left out, the line starts below it ([Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: A root placed after the start
- **WHEN** a restack placed a root below the branch after the start of its robot
- **THEN** the span leaves it out and starts below it
  [@test] src/lib/stack-lanes.test.ts (`starts below a root the robot's commit doesn't have: a restack placed it there after the start`)

### Requirement: Tree: The graph highlights the path to the root of a branch

The graph MUST answer "what is this branch built on" where the question comes up: the path from the root of the stack
down to a branch (`ancestorSegments`) is highlighted while the pointer rests on its row for 100 ms, at once over its
push button, robot or Safe Push slot, while the keyboard focuses the row or a control in it, and while the popover of
its Safe Push or Dry Run is open (`pinStackPath`; `stack-view.ts`).

#### Scenario: The pointer rests on a row
- **WHEN** the pointer rests on the row of a branch for 100 ms, or at once while it is over the push button, the robot in the Status cell or the Safe Push slot of the row
- **THEN** the path from the root of the stack down to the branch is highlighted: the line down from each ancestor, the lanes passing the side branches between them, the line up or the fork curve into the next one, also beside the expanded commits and the inline forms
  [@test] src/lib/stack-lanes.test.ts (`draws the path from the root down to a branch, past the side branches between`, `curves into a side branch and leaves the main line alone`)
  [@test] src/stores/stack-view.test.ts (`highlights the path from the root to the hovered branch, and keeps its state while the branch doesn't change`, `drops the path of a branch that left the rows`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should draw the path to the root of the hovered branch`)

#### Scenario: The focus and the popover
- **WHEN** the keyboard focuses the row or a control in it, or the popover of its Safe Push or Dry Run is open
- **THEN** the path of the branch is highlighted
- **AND** the pointer wins over the focus, the focus over the popover
  [@test] src/stores/stack-view.test.ts (`prefers the pointer to the keyboard focus, and the focus to an open Safe Push popover`)

#### Scenario: A root
- **WHEN** the branch is a root
- **THEN** it has no path
  [@test] src/lib/stack-lanes.test.ts (`has no path for a root or a branch without a row`)

#### Scenario: A running robot's landing span
- **WHEN** a running robot's landing span crosses the path
- **THEN** the span is drawn over the path

#### Scenario: An event above the table
- **WHEN** an event comes from a menu or a popover above the table
- **THEN** it moves no path

### Requirement: Tree: Over the push button the nodes the push sends are circled

Over the push button, the nodes of the branches the push sends MUST be circled as well, so "↑ 18" shows which branches
its 18 commits come from, and Push with Children in its place the branches stacked on it; the circles follow a push
while the pointer rests on the button.

#### Scenario: The pointer over the push button
- **WHEN** the pointer is over the push button of a row
- **THEN** the nodes of the branches the push sends are circled, the children of a branch with nothing of its own to send for Push with Children
- **AND** the circles follow a push while the pointer rests on the button
  [@test] src/stores/stack-view.test.ts (`circles what the hovered push sends, the children of a branch with nothing of its own to send, and follows a push`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should mark the branches a push sends when its button is hovered`)

### Requirement: Tree: A branch of an issue family is indented one step

A branch under the branch of its issue or under its issue row MUST be indented one step (`--bd-family-step`, 1.75rem),
with a guide over the whole row height under the icon of the row above; the lanes of a stack on it stay in the lane
column.

#### Scenario: A member of an issue family
- **WHEN** a branch is under the branch of its issue, wherever that is in the order
- **THEN** it is indented one step with a guide under the icon of the row above
- **AND** a stack on it is indented with it, its lanes inside the lane column
  [@test] src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`)
  [@test] src/stores/branch-table-rows.test.ts (`indents a family member and the stack on it one level, with the lanes of the stack inside`)

### Requirement: Tree: An issue row heads the branches of an issue without a branch

An issue row without a branch of its own MUST head its branches with the issue key and their size, the members and the
stacks on them, muted: "3 branches · 12 commits"; its icon box has the size of the folder button and sits where the
folder buttons are, so the guide of its branches sits under the icon.

#### Scenario: An issue without a branch of its own
- **WHEN** the branches of an issue have no issue branch among them
- **THEN** an issue row heads them with the issue key and "3 branches · 12 commits", muted
  [@test] src/stores/branch-sync.test.ts (`heads the branches of an issue without a branch of its own with an issue row`)
  [@test] src/lib/branch.test.ts (`formats the branches and their commits in one summary`)

### Requirement: Tree: The commits of an expanded branch sit on its lane

The commits of an expanded branch MUST sit on its lane, a dot per commit at its subject line, beside the lanes that run
on below the row; a collapsed row shows none ([Hover and click](#hover-and-click)). The lane runs on into the children
below, or ends at the last commit of a branch without children; a family member keeps its guide beside them. The rows of
a commit: [Commit Row](../../commit/commit-row/spec.md).

#### Scenario: An expanded and a collapsed row
- **WHEN** a branch is expanded
- **THEN** its commits sit on its lane, a dot per commit at its subject line
- **AND** a collapsed row shows none
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

### Requirement: Tree: The commits are oldest first in a table with stacks

In a table with stacks the commits MUST be oldest first: time runs down with the graph, so the tip of a parent sits right
above the row of its child; the amend target is then the last commit, and the squash dialog still gets the commits
newest first. Without stacks they MUST be newest first, as no graph runs down with time.

#### Scenario: A table with stacks
- **WHEN** a branch of a table with stacks is expanded
- **THEN** its commits are oldest first, the amend target the last commit
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

#### Scenario: A table without stacks
- **WHEN** a branch of a table without stacks is expanded
- **THEN** its commits are newest first

### Requirement: Tree: The commit subjects start at the branch name

The commit subjects MUST start at the branch name.

#### Scenario: The commits on the lane
- **WHEN** a branch is expanded
- **THEN** the subjects of its commits start at the branch name, its lane carried into the lines of its children
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should carry the commits of a branch on its lane, into the lines of its children`)

### Requirement: Tree: The form and conflict rows draw the lanes that run on

The inline form row and the amend conflict row of a branch MUST draw the lanes that run on and the family guide, so the
graph doesn't pause; their content starts right of them, the title at the branch name.

#### Scenario: The inline form of a stacked branch
- **WHEN** the inline form or the amend conflict row of a branch shows
- **THEN** it draws the lanes passing the row and its own while it has children, and the family guide
- **AND** its content starts right of them, the title at the branch name
  [@test] src/lib/stack-lanes.test.ts (`runs the lanes passing a row on below it, and its own while it has children or shows its commits`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should draw the lanes beside the inline form of a stacked branch`)

### Requirement: Commits cell: The count is a number under the Commits head

The Commits cell MUST show the count of the commits as a number, muted, right-aligned in tabular figures under the head
"Commits": "1 commit" and "2 commits" on every row were words that said the head again. A sentence about a count keeps
its words ("ABC-123 · 2 commits" of a drag, a tooltip, a toast).

#### Scenario: The count of a row
- **WHEN** a branch row shows
- **THEN** its Commits cell shows the count as a number, muted, right-aligned in tabular figures
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display correct branch row structure`)

#### Scenario: A sentence about a count
- **WHEN** a drag, a tooltip or a toast says a count
- **THEN** it keeps its words: "ABC-123 · 2 commits"

### Requirement: Status cell: The cell shows three items in their columns

The Status cell MUST show three items in their columns ([Columns](#columns)): the state of the branch (its robot, else
the status text), the pull request chip, then the Landed chip and the muted "lands with oauth" of a branch a running
Safe Push of a branch built on it lands ([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)), else the
muted "on master" of a stacked branch built elsewhere because its parent has no commits ([Blocked](#blocked)).

#### Scenario: The items of the cell
- **WHEN** a branch has a state, a pull request and landed commits
- **THEN** its Status cell shows the state, the pull request chip and the Landed chip, each in its column
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show each pull request state in the status cell`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`)

#### Scenario: A missing chip
- **WHEN** a branch has no pull request chip or no Landed chip
- **THEN** the missing chip leaves its column empty, and the next chip keeps its own column

### Requirement: Status cell: The first column says what happens to the branch

The first column MUST say what happens to the branch, as Rust decides it (`RowStatus`, `domain_policy::row_status`, for
the app and `brd status` alike; the page words it, `presentRowCell`): its robot while one runs or once it finished on
the branch as it is, as the chip of the robot. The robot reads as the status text it replaces (`text-sm`, its icon
where the dot is).

#### Scenario: A running robot
- **WHEN** a robot of the branch runs (also a Dry Run, one that will fail and one that cancels)
- **THEN** the first column shows its robot: its ring, its time, the progress line under it
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should say which branches a running Safe Push lands, and not start them on their own`)

#### Scenario: A robot that finished on the branch as it is
- **WHEN** the robot finished on the branch as it is
- **THEN** the first column shows it as the chip of the robot: "Failed", "Needs review", "Merged", "Dry run passed", "Canceled"
- **AND** hover shows its details, a click opens it in Patronus ([Patronus](../patronus/spec.md#robot-of-a-row))

### Requirement: Status cell: Work in flight never hides behind the state of the row

Work in flight MUST NOT hide behind the state of the row, nor the state behind it: the first that applies is the first
column, a running robot, then what landed all of it ("landed", its own merged robot "Merged"), a failed sync ("merge
conflict", "analyzing conflict…"), parked, a blocked stacked branch ("blocked by auth"), its robot finished on the
branch as it is.

#### Scenario: The order of the first column
- **WHEN** several of these facts apply to a branch
- **THEN** the first that applies is the first column
- **AND** only a Safe Push that lands it outranks a block, and nothing outranks a fix but master
  [@test] crates/domain-policy/src/row_status_test.rs (`only_a_safe_push_that_lands_it_outranks_a_block_and_nothing_outranks_a_fix_but_master`)

### Requirement: Status cell: The facts the first column covers stay beside it as a dot

The exceptional facts the first column covers (a failed sync, a block, what landed it, parked, a failed robot) MUST stay
beside it as a dot in the colour of the strongest (`destructive` for a failed sync or robot, `warning` for a block or
parked), each a line of its hover and of the hover of the node.

#### Scenario: A running Safe Push of a branch whose parent conflicts
- **WHEN** a Safe Push runs of a branch whose parent conflicts
- **THEN** the row shows the robot with "Blocked: parent auth has a merge conflict" beside it
  [@test] crates/domain-policy/src/row_status_test.rs (`a_safe_push_of_a_blocked_branch_is_landing_and_its_block_stays_beside_the_robot`)
  [@test] src/presenters/row-status.test.ts (`shows the robot Rust puts first, a block and a conflict beside it as a mark that names them, and a sync over all of it`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show a running Safe Push of a blocked branch first, its block beside it, and its parent's conflict as the fix`)

#### Scenario: A finished robot beside another fact
- **WHEN** a finished robot neither failed nor landed all of it, and another fact applies
- **THEN** the robot covers nothing beside that fact
  [@test] crates/domain-policy/src/row_status_test.rs (`a_finished_robot_shows_alone_and_beside_another_fact_only_when_it_failed`)

### Requirement: Status cell: One table pins every combination of the facts of a row

Every combination MUST be pinned by one table (`no_fact_of_a_row_drops_and_the_node_and_the_cell_tell_one_story`): no
fact drops from the row, the node and the cell tell one story, and a row whose next step is settled says why.

#### Scenario: Every combination of facts
- **WHEN** any combination of the facts of a row applies
- **THEN** no fact drops from the row, the node and the cell tell one story, and a row whose next step is settled says why
  [@test] crates/domain-policy/src/row_status_test.rs (`no_fact_of_a_row_drops_and_the_node_and_the_cell_tell_one_story`)

### Requirement: Status cell: Without such a fact the text says the push or the sync

Without such a fact the status text MUST say the push state or what the sync did; a sync lays over the cell as it
passes: the sync progress of a branch the sync builds anew (a new or changed branch, or one the last sync left with an
error, a conflict or a block), then the remote status, then the result of the sync.

#### Scenario: A sync passes over the cell
- **WHEN** a sync builds a branch anew
- **THEN** its status text says the sync progress ("syncing 3/10…")
- **AND** then the remote status ("not pushed", "3 (5)↑ · 2↓",
  "1↑ · 1 amended", "4↑ · restacked", "restacked", [What is outgoing](../../repository/outgoing/spec.md#what-is-outgoing))
- **AND** then the result of the sync ("created", "updated", "unchanged")
  [@test] src/presenters/row-status.test.ts (`is in the push colour for an amended or restacked branch: its push replaces older versions`)

#### Scenario: A branch the fetched baseline has all of
- **WHEN** the fetched baseline has all of a branch
- **THEN** its status says "landed" ([Landed Commits](../../repository/landed-commits/spec.md#before-the-update))
  [@test] src/presenters/row-status.test.ts (`says landed, in the merged hue, for a branch the baseline has all of: its push state no longer matters`)
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)

### Requirement: Status cell: A branch up to date with its remote shows nothing

A branch up to date with its remote (pushed, the remote as the branch) MUST show nothing in the first column: it needs
no action, and on most rows "up to date" drowned the states that do. Its node, its chips and the rest of the row stay.

#### Scenario: A branch up to date with its remote
- **WHEN** a branch is pushed and the remote is as the branch
- **THEN** its first column shows nothing, and its node, its chips and the rest of the row stay
- **AND** the branch preview says "up to date" with the last push (`StatusTexts.quiet`)
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should show nothing in the Status cell of a branch up to date with its remote, which needs no action`)
  [@test] src/presenters/row-status.test.ts (`is quiet for a branch up to date with its remote, which needs no action: the row shows nothing, the preview the text`)
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should push the branch with P`)

### Requirement: Status cell: A quiet resync keeps what the row shows

A branch a sync groups as it was, with the same commits, which the last sync built without a problem, MUST keep what its
row shows while the sync rebuilds it (a quiet resync, `TableState` of the session): its status, its node, its synced
hashes, until the sync reports it, so a sync of 35 branches doesn't turn every row to "syncing…" for a moment.

#### Scenario: A sync of unchanged branches
- **WHEN** a sync rebuilds a branch it groups as it was, with the same commits, that the last sync built without a problem
- **THEN** its row keeps its status, its node, its synced hashes until the sync reports it
- **AND** a changed branch starts syncing
  [@test] crates/repository-model/src/table_test.rs (`a_quiet_resync_keeps_the_shown_state_and_a_changed_branch_starts_syncing`)
  [@test] src/model/store.test.ts (`keeps the records of the branches a patch leaves alone, and of the commits of a branch whose state alone changed`)

### Requirement: Status cell: A push pressed during a sync waits for it

The push of a branch MUST stay enabled during a sync: a push pressed during the sync waits for it in the session
(`WaitingForSync`, the operation of the view) with the spinner of a push and pushes the branch as the sync left it, over
the remote status the sync brought.

#### Scenario: A fetch moved the remote branch
- **WHEN** a push is pressed during a sync whose fetch moved the remote branch
- **THEN** it waits for the sync with the spinner of a push
- **AND** it makes one push over its new tip, not a refused one and a second, the row as it was meanwhile
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should push a branch pressed during a sync once, over the remote tip the sync brought, the row as it was meanwhile`)

### Requirement: Status cell: A rewrite during a sync goes to the session at once

A rewrite (the menu, a drag, M) during a sync MUST go to the session at once, named on the table the row shows, and is
decided on HEAD as it is when it runs.

#### Scenario: A rewrite started during a sync
- **WHEN** the user starts a rewrite of a row from the menu, a drag or M while a sync runs
- **THEN** it goes to the session at once, named on the table the row shows, and is decided on HEAD as it is when it runs

### Requirement: Status cell: A push during a rewrite waits behind it and its sync

A push started while a rewrite of the repository runs MUST wait for it in the session (`Queued` behind it, holding no
HEAD, so a rewrite after it runs at once) and for the sync it ends with, and push the branches as the rewrite left them.

#### Scenario: A push clicked during a rewrite
- **WHEN** the user clicks a push while a rewrite of the repository runs
- **THEN** it waits behind the rewrite and its sync, and pushes what the rewrite left
  [@test] crates/operations/src/rewrite_test.rs (`a_push_clicked_during_a_rewrite_waits_behind_it_and_its_sync_and_pushes_what_it_left`)

### Requirement: Status cell: A sync renders only what it changed

A sync MUST render only what it changed. A branch whose status alone changed renders the status of its row, nothing
else of the row; a branch whose commits, texts or remote status changed renders its row; no other row renders, nor the
branch table or a stack island whose rows stay.

#### Scenario: A sync that changes one branch, and one that changes nothing
- **WHEN** a sync changes the commits, texts or remote status of one branch
- **THEN** only the row of that branch renders
- **AND** of a sync that changes nothing (each branch `Created` to `Unchanged`; each step of the progress of a sync), only the status of each row renders
  [@test] tests/e2e/branch-row-layout/row-renders.spec.ts (`should render only the row of the branch a sync changed, and of a sync that changes nothing the status of each row`)

### Requirement: Status cell: The status of a row is a leaf of its own

The status of a row MUST be a leaf of its own that reads its branch by name (`BranchRowStatus`); the row and its cells
read what the row shows beside the status, one object the store keeps while it is equal (`Branch.row`,
[Rendering Performance](../../../../docs/design/rendering-performance.md) Key Decision 3), with the status text only of
a branch the sync did not build (empty while it syncs and once it built it).

#### Scenario: A patch that changes only the status
- **WHEN** a patch changes only the status of a branch
- **THEN** the store keeps what its row shows as one object
- **AND** a change the row shows builds it anew
  [@test] src/model/store.test.ts (`keeps what the row of a branch shows as one object while a patch changes only its status, and builds it anew for a change the row shows`)

### Requirement: Status cell: The remote status of a row is the session's

The remote status of a row, of a stack branch and of an unapplied branch MUST be the session's, which orders a push
against the syncs: the outcome of a push (the statuses it left, or the ones it fetched when the remote moved) reaches the
rows as a patch with no sync after it, and a sync that started before the push ended never replaces it, also when that
sync fails and its table comes back.

#### Scenario: A push and a sync that started before it ended
- **WHEN** a sync started before a push ended, and the push left its remote statuses
- **THEN** the outcome of the push stays over the remote statuses of that sync, also when that sync fails and its table comes back
  [@test] crates/repository-model/src/table_test.rs (`the_outcome_of_a_push_stays_over_the_remote_statuses_of_a_sync_that_started_before_it`)
  [@test] src/model/store.test.ts (`shows the remote status every patch brings, also while a push of the branch runs: Rust orders the push's outcome against the syncs`)

#### Scenario: A push without a sync
- **WHEN** a push ends
- **THEN** its outcome reaches the rows as a patch with no sync after it
  [@test] crates/repository-model/src/session_test.rs (`a_push_without_a_sync_patches_the_remote_status_it_left`)
  [@test] crates/test-server/src/session_routes_test.rs (`a_push_answers_the_version_of_its_patch_and_runs_no_sync`)
  [@test] src/model/store.test.ts (`lets a command whose result the session applied wait for the patch of the version it answers`)

### Requirement: Status cell: A sync shows no remote status of what a push sends

While a push sends, a sync MUST show no remote status of what it sends; a push that fails leaves the status that sync
computed. A push that only waits for a sync sends nothing yet, so the status that sync brings reaches the row, and the
push goes over it.

#### Scenario: A sync while a push sends
- **WHEN** a sync ends while a push of the branch sends
- **THEN** the sync shows no remote status of what the push sends
- **AND** a push that fails leaves the status that sync computed
  [@test] crates/repository-model/src/table_test.rs (`a_sync_shows_no_remote_status_of_a_running_push_and_a_failed_push_leaves_the_one_it_computed`)

#### Scenario: A push that waits for a sync
- **WHEN** a push only waits for a sync
- **THEN** the status that sync brings reaches the row, and the push goes over it
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should push a branch pressed during a sync once, over the remote tip the sync brought, the row as it was meanwhile`)

### Requirement: Status cell: The outcome of a push goes to its own session

The outcome of a push MUST go to the session of the repository the push resolved, never to the branch of the same name
in another one; the page keeps only the spinner of a push.

#### Scenario: A branch of the same name in another repository
- **WHEN** a push of a branch ends and another session has a branch of the same name
- **THEN** its outcome leaves the branch of the other session alone
  [@test] crates/repository-model/src/session_test.rs (`the_outcome_of_a_push_in_one_session_leaves_the_branch_of_the_same_name_in_another`)

### Requirement: Status cell: A push of a branch the last sync did not build pushes nothing

A push of a branch the last sync did not build (a merge conflict, an error, a blocked parent) MUST push nothing and say
why: its virtual branch is the build of a sync before, of commits it may no longer have. The backend refuses it by the
table of the last sync (`not_built`), as `brd push` does, so a push pressed during that sync (the button, P) is refused too.

#### Scenario: A push pressed during a sync that could not build the branch
- **WHEN** the user pushes a branch the last sync did not build, also during that sync (the button, P)
- **THEN** nothing is pushed and the app says why: "Nothing was pushed: JIRA-456 is not synced (a merge conflict)"
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should refuse a push pressed during a sync that could not build the branch, its virtual branch an older build`)
  [@test] crates/domain-policy/src/push_test.rs (`a_branch_is_kept_out_parked_then_landed_then_held_unless_replaced_then_not_built`)
  [@test] crates/sync-core/src/sync_test.rs (`the_report_says_what_the_sync_read_and_the_tips_it_left`)

#### Scenario: An action that pushes first
- **WHEN** an action that pushes first (Create Pull Request, Request Review, a Safe Push start) runs on such a branch
- **THEN** it stops with that reason instead of going on with the remote branch as it is

### Requirement: Status cell: Only an error or a conflict is a badge

Only an error or a conflict MUST be a badge (`destructive`; `warning` while the conflict is analysed). Every other
status but up to date is muted text after a dot of its colour; the dot of a remote status has the push colour of the
branch ([Push colours](../../repository/outgoing/spec.md#push-colours)), in every table.

#### Scenario: A failed sync
- **WHEN** the sync of a branch failed, or its conflict is analysed
- **THEN** its status is a badge, `destructive`, or `warning` while the conflict is analysed
  [@test] src/presenters/row-status.test.ts (`says a failed sync as the badge of its error, a stack problem by its message`)

#### Scenario: Any other status
- **WHEN** a branch has another status than up to date
- **THEN** it is muted text after a dot of its colour
- **AND** the dot of a remote status has the push colour of the branch: warning only for a push that drops remote commits
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display correct branch row structure`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
  [@test] src/lib/push-summary.test.ts (`takes the hue from the risk: warning only for a push that drops remote commits`, `is neutral for a safe push of no commit of the user`)
  [@test] src/presenters/row-status.test.ts (`is in the push colour for an amended or restacked branch: its push replaces older versions`)

### Requirement: Status cell: The node, not the status, says the way into master

In every table the node of the row MUST say where it is on its way into master ([Tree](#tree)), not its push.

#### Scenario: The node and the dot
- **WHEN** a branch has a remote status
- **THEN** its push is the dot of the status, and the node says where it is on its way into master

### Requirement: Status cell: The status text shows as written

The status text MUST show as written: the texts are lower case already, and an issue key or an error text keeps its case
("blocked by IJPL-123"). Nothing changes the case in CSS; the branch preview shows the same text.

#### Scenario: A text with an issue key
- **WHEN** the status text holds an issue key or an error text
- **THEN** it keeps its case, "blocked by IJPL-123", in the row and in the branch preview

### Requirement: Status cell: Hovering the status text opens its description

Hovering the status text MUST open its description: the remote status in a sentence and the last push time, the reason
of a block, the error.

#### Scenario: Hover over the status text
- **WHEN** the user hovers the status text
- **THEN** its description opens: the remote status in a sentence and the last push time, the reason of a block, or the error

### Requirement: Status cell: A chip is a text-sized button

A chip MUST be a text-sized button. Hovering it opens its details; a click opens its item (the pull request, the robot)
and leaves the details as they are ([hover descriptions](../../../../docs/design/ui-theme.md)), or opens the details when
it has none (Landed), and never toggles the row.

#### Scenario: Hover and click on a chip
- **WHEN** the user hovers a chip, then clicks it
- **THEN** the hover shows its details, and the click opens the pull request or the robot, the details as they are
- **AND** the row does not toggle
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`)

#### Scenario: A chip without an item
- **WHEN** the user clicks the Landed chip
- **THEN** it opens its details

### Requirement: Status cell: The stack chip is the one disclosure among the chips

The stack chip after the name of a stack root MUST be the one disclosure among them: a click collapses or expands the
stack, its hover is a tooltip.

#### Scenario: A click on the stack chip
- **WHEN** the user clicks the stack chip of a stack root
- **THEN** the stack collapses or expands, and its hover is a tooltip

### Requirement: Status cell: The details of every chip have one anatomy

The details of every chip MUST have one anatomy: the state line (the icon and the state in the hue of the chip), the
title or a sentence on what it means, `dl` rows, the notices, then the actions.

#### Scenario: The details of a chip
- **WHEN** the details of a Landed chip or of a robot chip open
- **THEN** they show the state line, the title or a sentence on what it means, `dl` rows, the notices, then the actions
  [@test] tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

### Requirement: Blocked: A blocked stacked branch has one treatment

A blocked stacked branch MUST have one treatment: a hollow neutral node, the muted status "blocked by <parent>" after a
`warning` dot, a disabled push button whose description offers "Show <parent>" too, the hover of the node as well, and,
expanded, a `warning` alert with the reason and "Show <parent>", which reveals the parent. Its commits carry the
`warning` Blocked badge.

#### Scenario: The children of a parent that doesn't build
- **GIVEN** a stacked branch whose parent auth doesn't build
- **WHEN** the table shows it
- **THEN** its node is hollow and neutral, "Blocked by auth, which doesn't build": the fix is in the row of its parent
- **AND** its status is "blocked by <parent>" after a `warning` dot, its push button disabled, its description and the hover of the node offering "Show <parent>"
- **AND** expanded, a `warning` alert with the reason and "Show <parent>" reveals the parent, and its commits carry the `warning` Blocked badge
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)

### Requirement: Blocked: A branch whose parent has no commits is built elsewhere

A stacked branch whose declared parent has no commits SHALL NOT be blocked but built elsewhere ("Base changed"): the
muted "on master" in the third column of the Status cell, the reason in its tooltip, an enabled push button, and,
expanded, an alert with "Unstack from login" ([Stacked Branches](../stacked-branches/spec.md#ui)).

#### Scenario: The parent landed
- **GIVEN** a stacked branch oauth whose parent login landed and has no commits
- **WHEN** the table shows it
- **THEN** its Status cell shows "on master" in the third column, with the tooltip "login landed · oauth is built on
  master", and its push button is enabled
- **AND** expanded, the alert "Built on master: login landed." offers "Unstack from login", which unstacks it
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)

### Requirement: Blocked: A branch with a stack problem shows the problem and its fixes

A stacked branch with a stack problem MUST have the label of the problem as its `destructive` badge, its commits, and,
expanded, a `destructive` alert with the message and a `text-destructive` link per fix, the items of its menu
([Stacked Branches](../stacked-branches/spec.md#ui)). Its children are blocked by it.

#### Scenario: Conflicting parents
- **GIVEN** a stacked branch publish-viewer with conflicting parents
- **WHEN** it is expanded
- **THEN** a `destructive` alert names the problem, its commits are kept, and its links ("Stack On ci-traces",
  "Unstack", "Rename…") fix it, as the items of its menu
- **AND** its children say "Blocked: parent publish-viewer has a stack problem (conflicting parents)"
  [@test] tests/e2e/stacked-branches/stack-problems.spec.ts (`should name conflicting parents, keep the commits and stack on a parent from the alert`)

### Requirement: Actions cell: The cell shows the push, the Safe Push slot and the menu

The Actions cell MUST show, in this order: the push button, the Safe Push slot and "⋯", each in its column
([Columns](#columns)), so the push arrows, the slot's icons and "⋯" line up down the table and its islands; the column is
as wide as its widest row and doesn't wrap.

#### Scenario: A branch with commits
- **WHEN** a branch with commits shows
- **THEN** its Actions cell shows the push button, the Safe Push slot and "⋯", each in its column
- **AND** the column is as wide as its widest row and doesn't wrap
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should handle branch with commits state`)
  [@test] tests/e2e/branch-row-layout/column-widths.spec.ts (`gives the free width to the branch name and keeps the other columns at content width`)

### Requirement: Actions cell: A click in the cell does not toggle the row

A click in the cell MUST NOT toggle the row: on a button, on a disabled one (it passes the click on) and in the menu "⋯"
opens (its popup is in the row's React tree; a right-click in it doesn't open the row menu either).

#### Scenario: A click on a button, a disabled button and in the menu
- **WHEN** the user clicks a button of the cell, a disabled one, or in the menu "⋯" opens
- **THEN** the row does not toggle
- **AND** a right-click in that menu doesn't open the row menu either

### Requirement: Actions cell: ⌘C on the focused row copies the full branch name

⌘C (Ctrl+C elsewhere) on the focused row MUST copy the full branch name, `{prefix}/virtual/{name}`, as "Copy Full Branch
Name" of the menu's "Copy ›", which shows the key; not while text is selected.

#### Scenario: ⌘C on the focused row
- **WHEN** the user presses ⌘C (Ctrl+C elsewhere) on the focused row and no text is selected
- **THEN** the full branch name `{prefix}/virtual/{name}` is copied
- **AND** "Copy Full Branch Name" of the menu shows the key
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should copy the full branch name with ⌘C on the focused row, the key hint of Copy Full Branch Name`)

#### Scenario: Text is selected
- **WHEN** text is selected and the user presses ⌘C
- **THEN** the branch name is not copied

### Requirement: Actions cell: The row has no copy button

The row MUST have no copy button: a button on every row would hold its place in every row, and 35 copy icons down the
table would be the loudest thing in it.

#### Scenario: The buttons of a row
- **WHEN** a row shows its Actions cell
- **THEN** it has no copy button, and no place is held for one
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should handle branch with commits state`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should line up the columns of the table, the island and its header, whatever each card shows`)

### Requirement: Actions cell: The push button is compact

The push button MUST follow [Outgoing Commits](../../repository/outgoing/spec.md), compact: an up arrow and the count of
the commits it sends ("↑ 3", the arrow alone without a count), "45%" after the spinner while it pushes.

#### Scenario: A branch with commits to push
- **WHEN** a branch has commits to push
- **THEN** its push button is an up arrow and the count of the commits it sends, "↑ 3"
  [@test] src/presenters/push-button.test.ts (`counts the commits the push sends in every label, the user's share in the description`)

#### Scenario: A running push
- **WHEN** the push runs
- **THEN** the button shows "45%" after the spinner
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should show the percent on the push button and git's last line on hover while the push runs`)

### Requirement: Actions cell: The label of the push is its name and its description

The label of the push button ("Push Branch (2)", "Force Push Branch (1)", "Push Stack (3 branches)", "Pushing 45%"; one
name for the first push too) MUST be its accessible name and the first line of its description on hover, which shows
also while it is disabled, with the remote commits a force push drops, the question before a push that drops them, and
the progress of a running push.

#### Scenario: The description of a push
- **WHEN** the user hovers the push button
- **THEN** its description starts with its label, and names the remote commits a force push drops and the question before a push that drops them
- **AND** the description of a first push says "The branch isn't on the remote yet: the push creates it."
  [@test] src/presenters/push-button.test.ts (`counts the commits the push sends in every label, the user's share in the description`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`)

#### Scenario: A running push and a disabled one
- **WHEN** the user hovers the button of a running push, or a disabled one
- **THEN** the description shows the progress of the push, or the reason of a disabled push
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should show the percent on the push button and git's last line on hover while the push runs`, `should describe a disabled push on hover`)

### Requirement: Actions cell: A click pushes and leaves the hover as it is

A click on the push button MUST push and leave the hover as it is ([hover descriptions](../../../../docs/design/ui-theme.md)):
open while the pointer stays, also after a click well after it opened.

#### Scenario: A click on the push button
- **WHEN** the user clicks the push button while its description is open
- **THEN** the branch is pushed and the description stays open while the pointer stays, also after a click well after it opened

### Requirement: Actions cell: The push button is in the push colour of the branch

The colour of the push button MUST be the push colour of the branch as the text of a ghost button (the arrow for
`primary`), neutral when landed.

#### Scenario: The colour of the push
- **WHEN** a branch has a push, or landed
- **THEN** its push button is in the push colour of the branch, or neutral when landed

### Requirement: Actions cell: A branch up to date with its remote has no push button

A branch up to date with its remote MUST have no button: nothing is to push, and a disabled arrow in every pushed row
would say it on every row (while branches stacked on it have work, its button is Push with Children, "↑ 3 with
children", [Stacked Branches](../stacked-branches/spec.md#pull-requests)).

#### Scenario: A branch up to date with its remote
- **WHEN** a branch is up to date with its remote and no branch stacked on it has work
- **THEN** its row has no push button

#### Scenario: Branches stacked on it have work
- **WHEN** a branch is up to date with its remote and branches stacked on it have work
- **THEN** its button is Push with Children, "↑ 3 with children"

### Requirement: Actions cell: A branch a running Safe Push holds has no push button

A branch a running Safe Push holds MUST have no push button, as its Status cell shows the robot or "lands with oauth",
and its Safe Push slot is empty too ([Patronus](../patronus/spec.md#pushing-into-a-branch-whose-safe-push-runs)).

#### Scenario: A running Safe Push holds the branch
- **WHEN** a running Safe Push holds the branch
- **THEN** it has no push button and its Safe Push slot is empty
- **AND** its Status cell shows the robot or "lands with oauth"

### Requirement: Actions cell: A first push that the landing makes has no button

A branch the remote lacks whose landing pushes it (`isFirstPushInLanding`) MUST have no push button: where Patronus
answers or GitHub lists the pull requests and the branch has no live pull request, its Safe Push and Create Pull Request
push it first, so its first push is a step of them, not a move of its own; the menu and P offer it, and Push with
Children of a root.

#### Scenario: The landing pushes the branch
- **WHEN** the remote lacks the branch, Patronus answers or GitHub lists the pull requests, and it has no live pull request
- **THEN** it has no push button: Safe Push or Create Pull Request pushes it first
- **AND** the menu and P offer the push, and Push with Children of a root
  [@test] src/presenters/push-button.test.ts (`leaves the first push of a branch to its landing where Safe Push or Create Pull Request pushes it`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should push a stack parents first and create each pull request into master`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`)

#### Scenario: A push that updates the remote branch, a push nothing else makes
- **WHEN** the push updates the remote branch, or nothing else pushes the branch
- **THEN** the push keeps its button
  [@test] src/presenters/push-button.test.ts (`keeps a push that updates the remote branch, and the first push where nothing else pushes`)

#### Scenario: A branch a Safe Push holds
- **WHEN** a Safe Push holds a branch the remote lacks
- **THEN** its push is no first push of its landing: the row shows no push button, as for every branch a running Safe Push holds
- **AND** where the push keeps its place (the branch preview, the review window) it is disabled, and its description says why it won't land and offers the robot
  [@test] src/presenters/push-button.test.ts (`keeps the push of a branch a Safe Push holds: its description says why it won't land and offers the robot`)

### Requirement: Actions cell: No push button until the landing answers are in

Until Patronus and GitHub answered whether its landing pushes it, such a branch MUST have no button either: one shown
before their answers would leave with them and move the columns under the pointer; where it stays (a repository with
neither), the settled layout of the view keeps its track from the next start on.

#### Scenario: The facts come back after a start
- **WHEN** the app starts and Your Turn, the pull requests and the approvals come back
- **THEN** the rows and the columns stay where the last start left them
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the rows and the columns where the last start left them while Your Turn, the pull requests and the approvals come back`)

### Requirement: Actions cell: A branch with a live pull request keeps its push button

A branch with a live pull request MUST keep its button, as the push updates it, and so MUST a root whose Push with
Children takes the stack branch of the open pull request of its whole stack along, and every branch of a repository with
neither Patronus nor GitHub.

#### Scenario: A root of a whole stack with an open pull request
- **WHEN** the pull request of the whole stack is open
- **THEN** the root keeps its button, and its Push with Children takes the stack branch along, from the push and the menu
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should take the whole stack branch along in Push with Children of the root once its pull request is open, from the push and the menu`)

#### Scenario: A live pull request, a repository with neither
- **WHEN** a branch has a live pull request, or its repository has neither Patronus nor GitHub
- **THEN** it keeps its push button

### Requirement: Actions cell: A running or blocked push keeps its button

A running push MUST keep its button until it finished, and a blocked branch MUST keep the disabled button with its
reason. The focus in the button or its description when it leaves goes to the row, so P and Space keep working.

#### Scenario: A running push
- **WHEN** a push runs
- **THEN** its button stays until it finished, and the focus in it goes to the row when it leaves
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should show the percent on the push button and git's last line on hover while the push runs`)

#### Scenario: A blocked branch
- **WHEN** a branch is blocked
- **THEN** it keeps the disabled push button with its reason
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should describe a disabled push on hover`)

### Requirement: Actions cell: The preview footer keeps the labelled push button

The preview footer MUST keep the labelled push button.

#### Scenario: The kind of push by the remote status
- **WHEN** the remote status says whether a push creates, pushes, forces or has nothing to send
- **THEN** the button and the preview footer follow it, the preview footer with the labelled button
  [@test] crates/domain-types/src/remote_status_test.rs (`a_push_creates_pushes_forces_or_has_nothing_to_send_by_the_remote_status`)
  [@test] src/presenters/push-button.test.ts (`counts the commits the push sends in every label, the user's share in the description`)
  [@test] tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`, `should show the percent on the push button and git's last line on hover while the push runs`)

### Requirement: Actions cell: A branch with an error or without commits has no push button

A branch with an error or without commits MUST have no push button.

#### Scenario: A branch with an error or without commits
- **WHEN** a branch has an error or no commits
- **THEN** its row has no push button

### Requirement: Actions cell: On Space the Safe Push slot shows the next step

On Space (Patronus answered), the Safe Push slot MUST show the next step of the branch, the first row of the table of
[Next Step](../next-step/spec.md) that matches. The robot itself is in the Status cell. Nothing in the slot changes on
the row hover.

#### Scenario: The steps of the slot
- **WHEN** Patronus answered and the slot shows the next step of a branch
- **THEN** it is "Safe Push" for a branch with a push button and nothing in its way (a click starts it at once when it has nothing to say, else opens its popover at the slot,
  [Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run))
- **AND** the step after a robot ("Restart Safe Push")
- **AND** a quiet wait that opens the pull request it waits in ("Awaiting Review", "Changes Requested", "Draft")
- **AND** the action that helps ("Create Pull Request…", "Request Review…", "Retarget #7")
- **AND** a stacked branch's own step ("Safe Push · 3"), its popover at its own row
- **AND** nothing while a robot runs, after a merge, for a landed or held branch ([Patronus](../patronus/spec.md#robot-of-a-row))
  [@test] tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the Status cell, and the next step in the Safe Push slot`, `should restart a failed robot from its row, the state staying in the Status cell`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)

### Requirement: Actions cell: Safe Push is the one labelled action

The product lands through Safe Push, so it MUST be the one labelled action and not only behind the right-click; every
other "Safe Push" (the menu, S, the preview, the review) MUST be disabled with the reason of the step.

#### Scenario: A start from the slot and from the menu
- **WHEN** the user starts a Safe Push from the slot or from the "⋯" menu
- **THEN** it starts, and where the step is another the menu item is disabled with the reason of the step
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] tests/e2e/next-step/next-step.spec.ts (`should wait for the code owner the pull request asks, never offer a Safe Push that fails, and say why on S`)

### Requirement: Actions cell: On a cached frame the cell shows what the frame says

On the cached frame of an earlier run (the table the session seeded when it opened, `RepositoryView.cached`, until the
first sync of this run commits; or a fact of it no fresh answer replaced yet, `FactState::Cached`) the cell MUST show
what the frame says (the push button, the step in the Safe Push slot).

#### Scenario: A warm start
- **WHEN** the window opens on a cached frame
- **THEN** the cell shows its steps and its push button as the frame says them
  [@test] crates/repository-model/src/derive_test.rs (`a_cached_frame_shows_its_steps_and_holds_its_remote_actions_back_until_the_first_sync_commits`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_frame_is_unchecked_while_its_table_or_a_fact_the_actions_read_is_the_cached_one`)

### Requirement: Actions cell: On a cached frame every remote action waits for the first sync

On a cached frame every remote action of the row, of its whole stack, of its robot and of a pull request to review MUST
be disabled with "Waits for the first sync" (`ActionBlocker::Cached`), whatever else the frame says of it: a press on a
frame nobody checked never acts on what a sync may change. The rewrites, the park, the reviews and the links stay as the
frame offers them; no robot of the frame needs the user ([Patronus](../patronus/spec.md#landing-overview)).

#### Scenario: The remote actions of a frame nobody checked
- **WHEN** the frame is cached
- **THEN** a push, a start, on the merge route the merge of the pull request and Cancel Auto-Merge, a restart or a cancel, a pull request, a retarget, a review request, a review, and "Close Pull Request #12…" of a pull request of the whole stack that no longer matches it are disabled with "Waits for the first sync"
- **AND** the rewrites, the park, the reviews and the links stay as the frame offers them
  [@test] crates/domain-policy/src/actions_test.rs (`on_a_frame_nobody_checked_every_remote_action_waits_for_the_first_sync_and_the_rest_stays`, `a_frame_is_unchecked_while_its_table_or_a_fact_the_actions_read_is_the_cached_one`)
  [@test] crates/repository-model/src/derive_test.rs (`a_cached_frame_shows_its_steps_and_holds_its_remote_actions_back_until_the_first_sync_commits`)
  [@test] src/presenters/actions.test.ts (`words every blocker of the catalog as the page says it`)

#### Scenario: The merge of a pull request on a warm start
- **WHEN** the frame of a warm start offers Merge #7
- **THEN** it says it waits for the first sync, and merges once that sync committed
  [@test] tests/e2e/landing/merge-pull-request.spec.ts (`should say Merge #7 of the frame of a warm start waits for the first sync, and merge once that sync committed`)

#### Scenario: The close of a pull request of a stack that is no more
- **WHEN** the frame shows a pull request of a whole stack that is no more
- **THEN** its close waits for the first sync on the rows of the frame
  [@test] crates/repository-model/src/state_test.rs (`the_pull_request_of_a_stack_that_is_no_more_closes_and_waits_for_the_first_sync_on_the_frame`)
  [@test] tests/e2e/startup/warm-start.spec.ts (`says the close of a pull request of a stack that is no more waits for the first sync on the rows of the frame`)

### Requirement: Actions cell: The engine refuses a remote intent on a cached frame

The engine MUST refuse a remote intent on a cached frame (`plan_changed`) for a caller that did not read the action; once
the first sync failed and left the frame (or git deferred it), such an intent, a push that waited for that sync too, MUST
be refused with the error of that sync (`no_identity`, …), which says what to fix: no sync runs that it waits for.

#### Scenario: An intent on a seeded table
- **WHEN** a caller that did not read the action sends a remote intent on the seeded table
- **THEN** the engine refuses it before it is admitted, `plan_changed`
  [@test] crates/operations/src/rewrite_test.rs (`a_remote_intent_on_a_seeded_table_is_refused_before_it_is_admitted`)

#### Scenario: The first sync failed
- **WHEN** a push waited for the first sync, and that sync failed and left the frame
- **THEN** the push is refused with the error of that sync and sends nothing
  [@test] crates/operations/src/rewrite_test.rs (`a_push_that_waited_for_a_first_sync_that_failed_is_refused_and_sends_nothing`)

### Requirement: Actions cell: The menu button opens the menu of the right-click

"⋯" MUST open the branch menu under the button: exactly the menu of the right-click, so its actions are found without
knowing the right-click. It is disabled while the row runs an inline action, as the right-click opens no menu then.

#### Scenario: A click on the menu button
- **WHEN** the user clicks "⋯"
- **THEN** the branch menu opens under it, the menu of the right-click, and the row does not toggle
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the branch menu from the more actions button without toggling the row`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)

#### Scenario: An inline action runs
- **WHEN** the row runs an inline action
- **THEN** "⋯" is disabled, as the right-click opens no menu then

### Requirement: Hover and click: A row shows the pointer and a tint on hover

A row MUST show the pointer and `bg-muted` on hover; an expanded row keeps `bg-muted/50`.

#### Scenario: The pointer over a row
- **WHEN** the pointer is over a row
- **THEN** the row shows the pointer and `bg-muted`
- **AND** an expanded row keeps `bg-muted/50`

### Requirement: Hover and click: A click on the row toggles its commits

A click on the row or on its folder button MUST toggle its commits, in the table, a stack island and the Parked card
alike: a collapsed row shows no commit of its branch below it. A click on another button, a chip or a link in the row
MUST NOT.

#### Scenario: A click on the folder button
- **WHEN** the user clicks the row or its folder button, in the table, a stack island or the Parked card
- **THEN** the row expands and shows its commits, and a second click collapses it
- **AND** a collapsed row shows no commit of its branch below it
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should show expanded branch row with commits`)
  [@test] tests/e2e/stack-island/stack-island.spec.ts (`should show a big stack as an island below the table, the commits of a row on its lane only while it is expanded, oldest first`)

#### Scenario: A click on another button, a chip or a link
- **WHEN** the user clicks another button, a chip or a link in the row
- **THEN** the row does not toggle

### Requirement: Hover and click: Cmd-click and Shift-click select the row

Cmd/Ctrl-click and Shift-click MUST select the row instead (`data-selected`, `bg-primary/15`), for "Move to…" of the
selected branches ([Move Commits](../move-commits/spec.md)).

#### Scenario: Selected branches moved together
- **WHEN** the user Cmd/Ctrl-clicks or Shift-clicks rows
- **THEN** the rows are selected instead of toggled, and "Move to…" moves the selected branches
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should move the selected branches to a new branch with Move to…`)

### Requirement: Hover and click: A right-click opens the branch menu at the pointer

A right-click MUST open the branch menu at the pointer, the same as "⋯" in the Actions cell. A row running an inline
action has no menu. A right-click outside a row (a commit under it, an inline form) is not the branch's.

#### Scenario: A right-click on a row
- **WHEN** the user right-clicks a row
- **THEN** the branch menu opens at the pointer, with the items of "⋯", the copy options among them
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display context menu with copy options`)

#### Scenario: A right-click outside a row
- **WHEN** the user right-clicks outside a row (a commit under it, an inline form), or a row running an inline action
- **THEN** no branch menu opens
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the popups of a row from its first hover or focus, and the branch menu only from the row`)

### Requirement: Hover and click: The rows of a table share one menu

The rows of a table MUST share one menu (`TableContextMenu`: the branch table, its stack islands and the Parked card one,
the archived and the unapplied tables one each), which opens with the items of the row under the pointer.

#### Scenario: A right-click on two rows of a table
- **WHEN** the user right-clicks one row of a table, then another
- **THEN** the one menu of the table opens with the items of the row under the pointer

### Requirement: Hover and click: The popups of a row mount at their first hover or focus

The tooltips, popovers and the "⋯" menu of a row MUST mount at the first hover or focus of their trigger, not with the
row (`useLazyRoot`, [Rendering Performance](../../../../docs/design/rendering-performance.md) Key Decision 7): until then
the trigger has the same look, name and ARIA; one hover opens its popup after the delay it always had, the focus stays
on the trigger, a key or a press acts on it as before, and nothing asks the backend.

#### Scenario: The first hover or focus of a trigger
- **WHEN** the pointer or the focus reaches a trigger of a row for the first time
- **THEN** its popup opens after the delay it always had, and the focus stays on the trigger
- **AND** a key or a press acts on it as before, the menu on the row's ⋯ too
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the popups of a row from its first hover or focus, and the branch menu only from the row`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show an unstack started from the menu on the row's ⋯ and let the other rewrites queue behind it, from a key too`)
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say "Needs review" for a robot that failed for missing approvals, and start a new Safe Push from it`)

### Requirement: Hover and click: The first hover or focus replays once

That first hover or focus MUST replay once: the branches that show again open no popup of a row and give no trigger the
focus ([Main Views](../../repository/main-views/spec.md)).

#### Scenario: The view hides and shows again
- **WHEN** the view of the branches hides and shows again
- **THEN** the popups of the branches close as it hides, none opens by itself as it shows, and the focus is left on no hidden control
  [@test] tests/e2e/landing/statistics.spec.ts (`should close the popups of the branches as the view hides, open none by itself as it shows, and leave the focus on no hidden control`)

### Requirement: Hover and click: In a popover the roots mount with their triggers

In the content of a popover (the robot's details, the landing overview, the Safe Push popover) the roots MUST mount with
their triggers, so a press in a popup a hover opened keeps it open.

#### Scenario: A press in a popup a hover opened
- **WHEN** the user presses a control in a popup a hover opened
- **THEN** the popup stays open
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should say "Needs review" for a robot that failed for missing approvals, and start a new Safe Push from it`)

### Requirement: Hover and click: A hover opens a popup only once the pointer moved

A hover MUST open a popup only once the pointer moved over its trigger (`hoverOpensOnMove` of `src/lib/pointer-moves.ts`,
in the roots of `src/components/ui`): a popup whose trigger came under a still pointer stays closed when the delay of its
hover runs out, and the next move over the trigger opens it after that delay.

#### Scenario: A tooltip or a hover card under a still pointer
- **WHEN** the trigger of a tooltip or a hover card (the description of a push) came under a still pointer (a row expanded or collapsed above it, a new branch on top, a sync that reorders, a scroll)
- **THEN** it stays closed when the delay of its hover runs out, and the next move over the trigger opens it after that delay
- **AND** WebKit tells at once that the pointer is over an element that came under it: the description of session's push opened so, Push session Only in it
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open no hover card or tooltip of a trigger that came under a still pointer, and open it at the next move`)

#### Scenario: A badge before the trigger left
- **WHEN** a badge before a trigger left, and the trigger slid under a still pointer
- **THEN** its tooltip stays closed until the next move
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open no tooltip of a stack chip that slid into the place of a fixup badge under a still pointer, and open it at the next move`)

#### Scenario: A submenu of the row menu
- **WHEN** an item of a submenu of the row menu (Copy) came under a still pointer
- **THEN** its submenu stays closed until the next move
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open no submenu of the row menu whose item came under a still pointer, and open it at the next move`)

### Requirement: Hover and click: The pointer back over a trigger opens its popup

The pointer back over the trigger of a popup that closed otherwise than by its hover (its view hid, a press of an item)
MUST open the popup after the delay of its hover, whether the closed popup still runs its exit or went meanwhile: a
tooltip, a hover card and a submenu alike.

#### Scenario: A tooltip and a hover card that the hidden view closed
- **WHEN** a tooltip or a hover card (the description of a push) was open as its view hid, and the pointer comes back over its trigger as the view shows, before the closed popup went at the end of its exit
- **THEN** the popup opens after the delay of its hover
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the tooltip and the hover card whose trigger the pointer came back to while the popup the hidden view closed still went`)
  [@test] tests/e2e/landing/statistics.spec.ts (`should close the popups of the branches as the view hides, open none by itself as it shows, and leave the focus on no hidden control`)

#### Scenario: A submenu that a press of its item closed
- **WHEN** a press of an item of a submenu (Copy Branch Name) closed the menus, and the pointer comes back over the item of the submenu (Copy) in the row menu opened again, before the closed submenu went
- **THEN** the submenu opens after the delay of its hover
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the submenu of the row menu whose item the pointer came back to while the submenu a press closed still went`)

### Requirement: Hover and click: A tooltip enabled under the pointer opens after its delay

A tooltip disabled while it has nothing to say MUST open after the delay of its hover when it gets its words with the
pointer that moved over its trigger resting on it, as for a pointer that came then; a trigger the pointer did not move
over, which came under a still pointer, stays closed until the next move.

#### Scenario: The description of Approve that came late
- **WHEN** the pointer rests on Approve in a review window while the code-owner review of the pull request has not come, so Approve has no description, and the delay of its hover ran out
- **AND** the review comes, the pointer still
- **THEN** the description of Approve opens after the delay of its hover
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should open the description of Approve that came while the pointer rested on it, after the delay of its hover`)

### Requirement: Hover and click: A control is the same control only by its identity

Another element MUST be the same control only by its identity, never by its place (`registerControl`): a root of
`src/components/ui` names its trigger by its own key, so a trigger that renders as another element in the same root (a
text or a badge by the state, the plain trigger of a lazy root and the one it mounts) keeps the hover.

#### Scenario: A trigger that renders as another element
- **WHEN** a trigger renders as another element in the same root, a text or a badge by the state, or the plain trigger of a lazy root and the one it mounts
- **THEN** it keeps the hover
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open no tooltip of a stack chip that slid into the place of a fixup badge under a still pointer, and open it at the next move`)

#### Scenario: A control that renders its root anew
- **WHEN** a control renders its root anew
- **THEN** it names it by its action and its subject (`controlKey`: `more-actions:<branch>` of the plain "⋯" and the one of its roots; the robot of a row as it ends), and keeps the hover

### Requirement: Hover and click: The buttons of the actions keep one element

The buttons of the actions (`MenuItemButton`, the step of the slot, the actions of a robot) MUST keep one root whether
they have a reason to say or not, and a Stop Waiting or a lost answer comes beside a button (the Safe Push of the slot, a
button of a surface) in a wrapper that is there without them too, so their element stays as their action is enabled,
disabled, runs or waits.

#### Scenario: A move that waits
- **WHEN** a move started from a menu on the row's ⋯ waits, and its Stop Waiting comes beside it
- **THEN** the element of the button stays, and the menu stops the wait, nothing moved
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should show the wait of a move started from a menu on the row's ⋯, whose menu stops it, nothing moved`)

### Requirement: Hover and click: Space over a row opens the branch preview

Space over a row MUST open the [branch preview](../branch-preview/spec.md).

#### Scenario: Space over a row
- **WHEN** the pointer is over a row and the user presses Space
- **THEN** the branch preview of the hovered branch opens, and the arrows step through the branches
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the hovered branch with Space and step through the branches with the arrows`)

### Requirement: Hover and click: A row with commits is draggable

A row with commits MUST be draggable: it drags the branch, or all selected branches; a collapsed stack root drags its own
commits, not the stack, and a drop on it moves the commits into it.

#### Scenario: A drag of a row
- **WHEN** the user drags a row with commits
- **THEN** it drags the branch, or all selected branches

#### Scenario: A collapsed stack root
- **WHEN** the user drags a collapsed stack root, or drops commits on it
- **THEN** it drags its own commits, not the stack, and a drop on it moves the commits into it

### Requirement: Hover and click: The rows that take dragged commits show it

While commits are dragged, every row that takes them MUST show a dashed outline, and the row under the pointer the
primary tint and a solid outline; its rows below take the drop too.

#### Scenario: A drag over the rows
- **WHEN** the user drags commits, or a branch, over the rows
- **THEN** every row that takes them shows a dashed outline, the row under the pointer the primary tint and a solid outline
- **AND** its own branch does not take them, and its rows below take the drop too
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should drag the selection onto a branch, but not onto its own branch`)
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should drag an issue branch onto a branch: the issue joins it without a rewrite, and Undo moves it back`)

### Requirement: Hover and click: A reveal expands the row and scrolls it into view

A reveal (a branch clicked in the landing overview, "Show auth", moved commits) MUST expand the row and scroll it into
view; a row the filter hides shows again first ([Branch Filter](../branch-filter/spec.md)), a row a collapsed stack hides
expands its stack.

#### Scenario: A reveal of a hidden row
- **WHEN** a branch the filter hides is revealed
- **THEN** every branch shows again, and the row expands and scrolls into view
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show every branch again for a reveal of a hidden one`)

#### Scenario: A reveal in a collapsed stack
- **WHEN** a branch deep in a collapsed stack is revealed
- **THEN** its stack expands
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show a match deep in a collapsed stack, and expand the stack for a reveal`)

### Requirement: Hover and click: A row shown only for a match stacked on it is dimmed

A row the filter shows only because a match is stacked on it MUST be dimmed and stay a row: it expands, opens its menu
and takes the keys ([Branch Filter](../branch-filter/spec.md#rows)).

#### Scenario: A search that matches a stacked branch
- **WHEN** a search matches a branch stacked on another
- **THEN** the branch it stacks on shows dimmed, and still expands, opens its menu and takes the keys
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should show the matches of a search and, dimmed, the branches they stack on`)

### Requirement: Keyboard: The keys of a row act only where the row has the focus

The keys of a row MUST act while the focus is on the branch row (one of its buttons too) or on the table, never in an
input, a menu, a popover, an inline form or the commit list of an expanded row; and only without a modifier, but ⌥← and
⌥→, which collapse and expand the stack of the row ([Stacked Branches](../stacked-branches/spec.md#ui)).

#### Scenario: A key in an input or a menu
- **WHEN** the focus is in an input, a menu, a popover, an inline form or the commit list of an expanded row, or the key has a modifier
- **THEN** the key of the row does nothing

#### Scenario: ⌥← and ⌥→
- **WHEN** the user presses ⌥← or ⌥→ on a row
- **THEN** the stack of the row collapses or expands

### Requirement: Keyboard: A click focuses the row, which is not a tab stop

A click on a row MUST focus it; the row is focusable, not a tab stop.

#### Scenario: A click on a row
- **WHEN** the user clicks a row
- **THEN** the row has the focus, and Tab passes it by

### Requirement: Keyboard: The arrows move the focus between the branch rows

↑ and ↓ MUST move the focus to the previous and the next branch row, past the issue rows, the rows below a branch (its
commits, an inline form) and the rows a collapsed stack hides, scrolling it into view; from the table itself to the
first or the last row. The row they focus is the one Space falls back on.

#### Scenario: The arrows on the rows
- **WHEN** the user presses ↓ or ↑ on the table or a row
- **THEN** the focus moves to the next or the previous branch row, from the table to the first or the last, and stops at the ends
- **AND** Space previews the focused row
  [@test] src/lib/branch-row-keys.test.ts (`moves to the next or the previous branch row, from the table to the first or the last, and stops at the ends`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should walk the branch rows with the arrows and preview the focused one with Space`)

### Requirement: Keyboard: The focused row shows the focus outline inside its box

The row the keys focused MUST show the focus outline of the app inside its box (`bd-focus-inset`,
[UI Theme](../../../../docs/design/ui-theme.md)), the rows around in their place; a button of that row draws its outline
outside its own box, the inset is not inherited; a click shows no outline.

#### Scenario: The outline of the focus
- **WHEN** the arrows focus a row, then a button of the row, then a click focuses a row
- **THEN** the row has the outline inside it, the button outside it, and the row a click focused none
  [@test] tests/e2e/branch-row-layout/focus-outline.spec.ts (`should outline the row the arrows focused inside it, a button of the row outside it, and nothing a click focused`)

### Requirement: Keyboard: Space opens the preview of the focused row

Space MUST open the preview of the focused row while the pointer hasn't moved since the focus moved there, else of the
row under the pointer ([Branch Preview](../branch-preview/spec.md)).

#### Scenario: Space after the arrows
- **WHEN** the arrows focused a row and the pointer hasn't moved since, and the user presses Space
- **THEN** the preview of the focused row opens
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should walk the branch rows with the arrows and preview the focused one with Space`)

#### Scenario: Space after the pointer moved
- **WHEN** the pointer moved since the focus moved, and the user presses Space
- **THEN** the preview of the row under the pointer opens

### Requirement: Keyboard: The side arrows expand, collapse and go up the tree

→ MUST expand the focused row. ← MUST collapse it, or on a collapsed row move the focus to the branch row it is nested
under (the branch it is stacked on, or the branch of its issue), as in a tree; nothing under an issue row or at the top
level.

#### Scenario: ← on a collapsed row
- **WHEN** the user presses ← on a collapsed row
- **THEN** the focus moves to the branch it is stacked on, or the branch of its issue
  [@test] src/lib/branch-row-keys.test.ts (`moves from a row to the branch it is nested under, as in a tree`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should walk the branch rows with the arrows and preview the focused one with Space`)

#### Scenario: Under an issue row or at the top level
- **WHEN** the user presses ← on a collapsed row under an issue row or at the top level
- **THEN** nothing happens
  [@test] src/lib/branch-row-keys.test.ts (`has no parent row under an issue row`)

#### Scenario: → and ← on an expandable row
- **WHEN** the user presses → on the focused row, then ←
- **THEN** the row expands, then collapses

### Requirement: Keyboard: Enter toggles the focused row

Enter on the focused row MUST toggle it; with selected rows, Enter opens "Move to…" instead
([Move Commits](../move-commits/spec.md)). A focused button of the row keeps its Enter.

#### Scenario: Enter on a row
- **WHEN** the user presses Enter on the focused row
- **THEN** the row toggles, or with selected rows "Move to…" opens
- **AND** a focused button of the row keeps its Enter

### Requirement: Keyboard: The hint keys run the item of the branch menu

P, D, O and M MUST run the item of the branch menu they are the hints of (`selectRowItem`): the same action, enabled as
the item is, a running one joined. A disabled item says why in an `info` toast, its reason as the title; a branch without
the item (O without a pull request) does nothing.

#### Scenario: A key of an item that runs
- **WHEN** the user presses the key of an item whose action runs
- **THEN** the key joins the running one
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show an unstack started from the menu on the row's ⋯ and let the other rewrites queue behind it, from a key too`)

#### Scenario: A key of a disabled item
- **WHEN** the user presses the key of a disabled item
- **THEN** an `info` toast says why, its reason as the title ("The
  remote branch is up to date", "Unstacking oauth…")

#### Scenario: A branch without the item
- **WHEN** the branch has no such item (O without a pull request)
- **THEN** the key does nothing

### Requirement: Keyboard: P pushes the focused branch as its push button does

P MUST push the focused branch as its push button does (`pushOrConfirm`, the stack where the button pushes it, the
question before a push that drops remote commits), when that button is enabled (the `push` Rust decides with a running
push over it, as P in the preview).

#### Scenario: P on a branch with commits to push
- **WHEN** the user presses P on the focused row whose push button is enabled
- **THEN** the branch is pushed as its push button does, and it stays under Your Move until the filter changes
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should keep a branch under Your Move until the filter changes, once its move is made`)

### Requirement: Keyboard: S opens the Safe Push popover of the focused row

S MUST open the Safe Push popover at the slot of the focused row when its "Safe Push" is enabled (Patronus answered, the
row has a push, its start is not refused (`landing.start`), no inline action running): S never starts by itself
(`requestSafePush` with `via: "key"`). Otherwise S says why in an `info` toast, the reason the button's hover says.

#### Scenario: S and Enter
- **WHEN** the user presses S on the focused row whose "Safe Push" is enabled, then Enter
- **THEN** the popover opens at the slot and focuses its start button once it can start, so Enter starts the robot
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should open the Safe Push popover of the focused row with S and start it with Enter, which takes it out of Your Move`)

#### Scenario: S where the start is not possible
- **WHEN** the user presses S on a row whose "Safe Push" is disabled
- **THEN** an `info` toast says the reason the hover of the button says

### Requirement: Keyboard: D reviews the branch in its review window

D MUST review the branch in its review window when its "Review Changes" is enabled. D is a key that looks at a row, as
Space (`usePreviewHotkeys`): the focused row while the pointer hasn't moved since, else the row under the pointer; never
from a button, a checkbox or a field of an inline form in the row. The keys that act on a row (P, S, O, M) need its focus.

#### Scenario: D on a row
- **WHEN** the user presses D on a row whose "Review Changes" is enabled
- **THEN** the review of the branch opens in its window
  [@test] tests/e2e/branch-review/branch-review.spec.ts (`should open the review of a branch in its window from its row, its menu and its preview`)

#### Scenario: D in a field of an inline form
- **WHEN** the focus is on a button, a checkbox or a field of an inline form in the row
- **THEN** D does nothing

### Requirement: Keyboard: O opens the pull request of the focused row

O MUST open the pull request of the focused row, as "Open Pull Request #7" of its menu; nothing for a branch without one.

#### Scenario: O on a row with a pull request
- **WHEN** the user presses O on the focused row of a branch with a pull request
- **THEN** its pull request opens
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)

### Requirement: Keyboard: M opens Move to for the focused row

M without selected rows MUST select the focused row and open "Move to…" for it, as the item of its menu does, where the
menu offers it (a synced branch with commits, no error); with selected rows M opens it for them
([Move Commits](../move-commits/spec.md)).

#### Scenario: M without a selection
- **WHEN** the user presses M on the focused row of a synced branch with commits and no selection
- **THEN** the row is selected and "Move to…" opens for it
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should open Move to… for the focused row with M, without a selection`)

#### Scenario: M with selected rows
- **WHEN** rows are selected and the user presses M
- **THEN** "Move to…" opens for them

### Requirement: Keyboard: F2 steps through the user's moves by urgency

F2 and Shift+F2 MUST move the focus to the next or the previous branch row that is the user's move (the rows of Your
Move, [Branch Filter](../branch-filter/spec.md#state-chips)), the most urgent first: the fixes, then the steps of the
user, then the branches ready to land, each in table order (`movesByUrgency`), around the ends, from anywhere in the
window but a text field, a dialog or a menu.

#### Scenario: F2 and Shift+F2
- **WHEN** the user presses F2 or Shift+F2
- **THEN** the focus moves to the next or the previous move by urgency, a fix first, around the ends, as F2 steps through the problems in IntelliJ
- **AND** a row in a collapsed stack expands its stack; under a filter only the rows it shows count
  [@test] src/lib/branch-row-keys.test.ts (`steps to the next or the previous move by urgency, around the ends`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should step through the user's moves with F2 and Shift+F2, a fix first, into a collapsed stack`)

#### Scenario: No focused row, or one that is no move
- **WHEN** no row has the focus, or the focused row is no move
- **THEN** F2 goes to the first move and Shift+F2 to the last one
- **AND** nothing happens without a move
  [@test] src/lib/branch-row-keys.test.ts (`starts at the most urgent or the least urgent move from no row or a row that is no move, and finds none without a move`)

#### Scenario: The tooltip of Your Move
- **WHEN** the user hovers the Your Move chip
- **THEN** its tooltip tells the keys

### Requirement: Keyboard: The hint keys work in any layout and show in the menu

P, S, D, O and M MUST work in any keyboard layout (the physical key). The menu items show them (the push `P`, "Safe Push"
`S`, "Review Changes" `D`, "Open Pull Request" `O`, "Move to…" `M`), and so does the hover of "Safe Push" in the row
(`S`): a key shown in the menu works on the row.

#### Scenario: The keys in the menu
- **WHEN** the user opens the branch menu, and presses a key it shows on the row
- **THEN** the item shows its key, and the key runs it on the row as the item does
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should start a Safe Push from the row button and from the "⋯" menu, and say why a start is not possible`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should push from the branch menu as P does, and say why a pushed branch can't be pushed`)

### Requirement: Branch menu: The actions of a subject and their blockers are Rust's

Which actions a branch, a whole stack and a robot offer, and why each can't run now, MUST be Rust's
(`domain_policy::actions`, the `actions` of the view by subject: the branch, `stack:<root>`, `robot:<id>`): every surface
presents them by their stable ids in one look and one set of words (`src/presenters/actions.ts`: label, key, icon, a
blocker in words); an action is disabled with its reason, never hidden for it.

#### Scenario: A ready branch
- **WHEN** a branch is ready to land
- **THEN** it offers every action enabled, and off Space no start of Patronus
  [@test] crates/domain-policy/src/actions_test.rs (`a_ready_branch_offers_every_action_enabled_and_off_space_no_start_of_patronus`)

#### Scenario: An action that can't run now
- **WHEN** an action of the branch or of its whole stack can't run now
- **THEN** it is disabled with its reason, never hidden for it
  [@test] crates/domain-policy/src/actions_test.rs (`the_push_waits_for_parked_blocked_failed_held_a_rebuild_and_nothing_to_push_in_this_order`, `a_rewrite_needs_commits_and_a_build_never_a_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest`, `a_park_waits_for_a_robot_that_lands_the_branch_and_a_parked_branch_unparks`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_whole_stack_starts_as_its_step_says_and_lands_in_pieces_while_its_stack_branch_is_not_built`)

#### Scenario: Every surface by the stable ids
- **WHEN** a surface shows the actions of a subject: the menu, the "⋯" of an island, the branch preview, the review window, the robot's details and the robots indicator, the command palette, the toasts of a robot and the drops of a drag
- **THEN** it presents the actions Rust offers by their stable ids, in one look and one set of words, no item of an action Rust doesn't offer and none hidden that it does
- **AND** a patch of the view brings only the actions that changed
  [@test] crates/repository-model/src/derive_test.rs (`the_view_offers_the_actions_of_each_subject_and_a_patch_only_those_that_changed`)
  [@test] crates/domain-policy/src/action_catalog_test.rs (`every_action_and_every_blocker_code_is_in_the_action_catalog_the_page_reads`)
  [@test] src/presenters/actions.test.ts (`presents every action of the catalog Rust writes by its id, with its look`, `words every blocker of the catalog as the page says it`)
  [@test] src/hooks/useBranchContextActions.test.ts (`offers only what Rust offers: no item of an action it doesn't offer, none hidden it does`)

### Requirement: Branch menu: What runs lies over the actions in Rust

What runs MUST lie over the actions in Rust too, from the operations of the session (`with_operations`): the item of an
intent that runs or waits in this process is running, a rewrite that can start says what its start waits behind, a
remote action whose resource an operation holds is disabled with it; no sync disables a rewrite.

#### Scenario: The operations of the session over the items
- **WHEN** an operation of the session runs
- **THEN** its own item is running, a rewrite waits behind HEAD, and a remote action whose resource it holds is busy
  [@test] src/presenters/actions.test.ts (`lays the operations of the session over an item as Rust decides it: its own runs, a rewrite waits behind HEAD, a remote action is busy`)

### Requirement: Branch menu: Presence by what only the page holds stays the page's

Presence by what only the page holds MUST stay the page's: "Squash N Fixups…" (the autosquash plan), "Ungroup" (the
prefix to remove).

#### Scenario: The items the page decides
- **WHEN** the menu of a branch opens
- **THEN** the page decides by its autosquash plan whether "Squash N Fixups…" is there, and by the prefix to remove whether "Ungroup" is

### Requirement: Branch menu: Rust offers the close of a pull request of the whole stack

"Close Pull Request #12…" of a pull request of the whole stack MUST be Rust's (`close-stack-pull-request`): offered only
while that pull request is open and no longer matches the stack (a landing took the root, or the stack no longer forks,
also of a root no branch is built on any more), and whether it can run.

#### Scenario: A pull request of a whole stack that no longer matches it
- **WHEN** the pull request of a whole stack is open and no longer matches the stack
- **THEN** Rust offers "Close Pull Request #12…", which runs once and waits for what holds its state
  [@test] crates/domain-policy/src/actions_test.rs (`the_close_of_the_pull_request_of_a_whole_stack_runs_once_and_waits_for_what_holds_its_state`)

### Requirement: Branch menu: The groups come in the order of how often they are wanted

The groups MUST come in the order of how often their actions are wanted, Safe Push first.

#### Scenario: The order of the groups
- **WHEN** the menu of a branch opens
- **THEN** it leads with "Fix Conflict…" for a branch with a conflict (its next step, above the rest)
- **AND** the landing: where Patronus answers "Safe Push" (`S`; "Safe
  Push · 3" for a stacked branch), then the actions of the branch's robot as every surface of the robot lists them
  (`getRobotActionItems`, [Patronus](../patronus/spec.md#restart-and-cancel): "Restart Safe Push" (it pushes first while the
  remote lacks work), "Cancel Safe Push", "Open in Patronus", and "Open Pull Request #12" for a robot of a pull request
  other than the branch's own), then "Patronus ›" with the rarer starts: "Safe Push With Profile…" (always the popover,
  its profile to choose; disabled for a start that comments on a pull request, which runs the default checks), "Dry
  Run" and "Check Approvals Again"
- **AND** then the push of the branch, named as its push button, "Push oauth Only" after it
  while it pushes the stack ([Stacked Branches](../stacked-branches/spec.md#pull-requests)), on a branch with branches
  stacked on it "Push with Children (4 branches)" beside it (disabled with the reason while it pushes nothing beyond
  the branch: [Stacked Branches](../stacked-branches/spec.md#pull-requests)), and "Mark IJAI-1 Fixed" for a YouTrack issue
- **AND** the review ("Review Changes", "Open Pull Request #7" or "Create Pull Request…", "Request Review…", "Retarget #7 to
  master", "Link GitHub Stack" of a layer whose stack of GitHub is out of line:
  [Stacks of GitHub](../stacked-branches/spec.md#stacks-of-github))
- **AND** the shape ("Rename…" or "Name This Branch…", "Move to…", "Stack On… ›", "Unstack from …", "Ungroup")
- **AND** on every branch of a stack the stack ("Collapse Stack" `⌥←` or "Expand Stack" `⌥→`, "Whole Stack ›", the actions of the
  whole stack of its root: [Whole Stack](../whole-stack/spec.md))
- **AND** the commits ("Amend Changes…", "Add Issue Reference…", "Squash N Fixups…")
- **AND** the order of landing ("Add Need…", "Remove Need: docs": [Declared Needs](../declared-needs/spec.md))
- **AND** what sets the branch aside ("Park Branch…": [Parked Branches](../parked-branches/spec.md), "Unapply…")
- **AND** last "Copy ›" ("Copy Full Branch Name", "Copy Branch Name", "Copy Pull Request Link" for a branch
  with a pull request, "Copy Retarget Command" for an open one into a virtual branch)
  [@test] src/hooks/useBranchContextActions.test.ts (`leads with Safe Push, keeps every keyed item on top and puts the rarer starts and the copies into submenus`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_review_the_pull_request_actions_and_the_rarer_starts_follow_their_facts`)
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request with O on the focused row, and copy its link from the branch menu and the popover`)
  [@test] src/presenters/robot-actions.test.ts (`cancels a running robot through the engine, and opens it in Patronus and the pull request it was started from`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request into a parent's branch to master before the push`)

### Requirement: Branch menu: An item with a key is never in a submenu

An item with a key (P, S, D, O, M) MUST NOT be in a submenu; a key of an item in a submenu (⌘C of "Copy Full Branch
Name") MUST work on the row as well.

#### Scenario: The keyed items and the submenus
- **WHEN** the menu of a branch opens
- **THEN** every keyed item is on top, the rarer starts and the copies in submenus
  [@test] src/hooks/useBranchContextActions.test.ts (`leads with Safe Push, keeps every keyed item on top and puts the rarer starts and the copies into submenus`)

#### Scenario: The key of an item in a submenu
- **WHEN** the user presses ⌘C, the key of "Copy Full Branch Name" in "Copy ›", on the focused row
- **THEN** it copies the full branch name as the item does

### Requirement: Branch menu: Off Space the landing starts with the push

Off Space the landing MUST start with the push; before Patronus answers whether the repository is on Space its starts
stay, disabled with "Waits for the answer of Patronus" ([Next Step](../next-step/spec.md#facts-without-an-answer)).

#### Scenario: A repository off Space
- **WHEN** Patronus answered that the repository is not on Space
- **THEN** the landing group starts with the push, and Rust offers no start of Patronus
  [@test] src/hooks/useBranchContextActions.test.ts (`starts with the push off Space, where Rust offers no start of Patronus`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_ready_branch_offers_every_action_enabled_and_off_space_no_start_of_patronus`)

#### Scenario: Before Patronus answers
- **WHEN** Patronus has not answered yet whether the repository is on Space
- **THEN** its starts stay, disabled with "Waits for the answer of Patronus", and Create Pull Request… waits until GitHub lists the pull requests, as the review window does
  [@test] src/hooks/useBranchContextActions.test.ts (`offers Safe Push disabled until Patronus answers, and Create Pull Request… until GitHub lists the pull requests, as the review window does`)

### Requirement: Branch menu: A repository that lands by a push offers Push to master

A repository that lands by a push into the target MUST offer "Push to master" (`push-to-baseline`, S; "Push to master ·
3") in the place of the starts of the robot, on Space or off it, disabled with the reason of its step, its slot shows the
step, and "Whole Stack ›" of a root offers the push of the whole stack ([Landing Routes](../landing-routes/spec.md#the-page)).

#### Scenario: A push into master from the menu
- **GIVEN** a repository that lands by a push into the target
- **WHEN** the user chooses "Push to master" in the menu of a row
- **THEN** the branch is pushed into master, and the menu offers no start of the robot
  [@test] crates/domain-policy/src/actions_test.rs (`a_push_into_the_target_offers_its_push_with_the_reason_of_its_step_and_no_start_of_the_robot`)
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push into master from the menu of the row`)

#### Scenario: The whole stack into master
- **WHEN** the user chooses the push of the whole stack in "Whole Stack ›" of the menu of a root
- **THEN** the whole stack is pushed into master
  [@test] tests/e2e/landing/push-to-master.spec.ts (`should push a whole stack into master from Whole Stack of the menu of its root`)

### Requirement: Branch menu: The menu of a branch that can't change keeps every item

The menu of a branch that can't change now (no commits, a sync error) MUST keep every item: the rewrites are disabled
with the reason, as Rust decides them, except the fixes of a stack problem, which run while it blocks the rest
([Stacked Branches](../stacked-branches/spec.md#ui)); what rewrites nothing on the branch stays as it is.

#### Scenario: The rewrites of a branch that can't change
- **WHEN** the menu of a branch without commits or with a sync error opens
- **THEN** the rewrites (Rename…, Move
  to…, Ungroup, Stack On…, Unstack, Add Issue Reference…, Amend Changes…, Unapply…, Squash Fixups…) are disabled with
  the reason ("The branch failed to sync", the reason of a stack problem, "The branch has no commits")
- **AND** the fixes of a stack problem run while it blocks the rest
  [@test] crates/domain-policy/src/actions_test.rs (`a_rewrite_needs_commits_and_a_build_never_a_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest`)

#### Scenario: What rewrites nothing on the branch
- **WHEN** the menu of such a branch opens
- **THEN** Review Changes, the pull request and its review request, Push Branch and Push with Children (they wait for the sync themselves), Safe Push, Safe Push With Profile… and Dry Run (disabled with the reasons of the [next step](../next-step/spec.md)), the robot's actions stay as they are
  [@test] crates/domain-policy/src/actions_test.rs (`push_with_children_pushes_more_than_the_branch_or_says_what_it_lacks`)

### Requirement: Branch menu: A sync that runs disables no rewrite

A sync that runs MUST disable no rewrite: its start goes to the session at once, named on the table the window
shows, and runs once HEAD is as the session knows it; its row shows the rewrite on its "⋯" meanwhile, no info toast
refuses it, and each rewrite asks one sync after it.

#### Scenario: A rewrite from the menu during a sync
- **WHEN** the user opens the menu of a branch during a sync and starts a rewrite
- **THEN** every item is kept, the rewrites enabled, and the rest stays as it is
- **AND** the rewrite runs once that sync ended, without a refusal, with one sync for the rewrite
  [@test] src/hooks/useBranchContextActions.test.ts (`keeps every item during a sync, the rewrites enabled: the session admits them at once; the rest stays as it is`)
  [@test] crates/domain-policy/src/actions_test.rs (`a_rewrite_needs_commits_and_a_build_never_a_sync_and_the_fixes_of_a_stack_problem_run_while_it_blocks_the_rest`)
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep the actions of the branch menu during a sync, the rewrites enabled`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should unstack from the menu during a held sync once that sync ended, without a refusal, one sync for the rewrite`)

### Requirement: Branch menu: On a cached frame the rewrites stay enabled

On the cached frame of an earlier run (`RepositoryView.cached`) the rewrites MUST stay enabled too, and a start of one
MUST never be refused: it goes to the session at once and waits there, holding HEAD and writing nothing, until the first
sync of the session commits (the step `WaitingForSync`, on the sync the session started as it opened), then runs on the
table that sync read.

#### Scenario: A rewrite on a seeded table
- **WHEN** the user starts a rewrite on the cached frame
- **THEN** it waits for the first sync of the session, holding HEAD and writing nothing, and runs once that sync committed
  [@test] crates/operations/src/rewrite_test.rs (`an_unstack_on_a_seeded_table_waits_for_the_first_sync_and_runs_once_it_committed`)

#### Scenario: Stop Waiting and a failed sync
- **WHEN** the user chooses Stop Waiting, or the first sync fails
- **THEN** Stop Waiting ends the wait with `canceled` and the sync goes on; a failed sync answers its error
  [@test] crates/operations/src/rewrite_test.rs (`a_stop_waiting_for_the_first_sync_is_canceled_and_the_sync_goes_on`)

### Requirement: Branch menu: A start on a cached frame finds the commits the frame named

The commits the frame named MUST be found there as after any HEAD another process moved: the same ids on the same HEAD,
else the commits of the same author, author time and subject (the frame keeps each commit's email and subject), and the
plan changed when none is.

#### Scenario: HEAD moved since the frame
- **WHEN** a move starts on the frame and HEAD moved since
- **THEN** it moves the commit of the same author, author time and subject, or the plan changed when none is
  [@test] crates/operations/src/rewrite_test.rs (`a_move_on_a_seeded_table_whose_head_moved_since_moves_the_commit_of_its_key`)
  [@test] crates/repository-model/src/warm_start_test.rs (`a_frame_reads_back_from_the_object_cache_as_it_was_written`)

### Requirement: Branch menu: An action of the menu runs once

An action of the menu MUST run once ([Running Operations](../../../../docs/design/operations.md)): while its intent runs or
waits in the session, its item is running on every surface and in every window, as Rust lays the operations of the
session over the actions (`domain_policy::actions::with_operations`, `Running`, `Queued`); a second start joins it in the
session.

#### Scenario: The item of a running action
- **WHEN** the intent of an action runs or waits in the session
- **THEN** its item is running: disabled, a spinner in place of its icon, its busy state as the reason:
  "Unstacking oauth…", "Waits for Moving 1 commit to docs…", whom it waits for while another holder has the repository
  lock
- **AND** so on every surface and in every window, from the admission of a rewrite to its end
  [@test] crates/domain-policy/src/actions_test.rs (`the_operations_of_the_session_lie_over_the_actions_its_own_runs_a_rewrite_waits_a_push_behind_its_ref_is_busy`, `the_restart_and_the_cancel_of_a_robot_run_once_and_hold_each_other_back`)
  [@test] crates/repository-model/src/derive_test.rs (`the_view_lays_the_operations_of_the_session_over_the_actions_from_the_admission_of_a_rewrite_to_its_end`)
  [@test] src/presenters/actions.test.ts (`lays the operations of the session over an item as Rust decides it: its own runs, a rewrite waits behind HEAD, a remote action is busy`)
  [@test] src/hooks/useBranchContextActions.test.ts (`shows the rewrite that runs in the session running, and says what the start of every other rewrite waits behind`)
  [@test] src/presenters/next-step.test.ts (`lays what runs on the row over the step: its own action runs, the others wait, never a quiet step nor another field of the pull request`)

#### Scenario: A second start
- **WHEN** the user starts the action again while it runs (a double click)
- **THEN** the second start joins it in the session: a double click runs one unstack
- **AND** the surface that started it shows its spinner until its answer, after the sync that shows its result
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should unstack once from the alert, its spinner on the button until the sync shows the branch`)

### Requirement: Branch menu: A start with other commits is a start of its own

A start of it with other commits, another target or another message (a drag of another commit of the branch onto the
same branch, a reword of another commit) MUST be a start of its own, which waits in the queue; a menu item names no
commits, so Move to…, Amend Changes… and Add Issue Reference… only wait.

#### Scenario: A second commit of the same branch dragged onto the same target
- **WHEN** the user drags a second commit of the same branch onto the same target while the first move runs
- **THEN** it is a move of its own, never joined to the first
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should move a second commit of the same branch dragged onto the same target while the first move runs: a move of its own, never joined to the first`)

#### Scenario: A menu item while its action runs
- **WHEN** the user chooses Move to…, Amend Changes… or Add Issue Reference… while that action runs
- **THEN** it only waits

### Requirement: Branch menu: Every other rewrite stays offered and waits in the queue

Every other rewrite of the repository, in every branch and commit menu, by a drag, a key or a toast's Undo, MUST stay
offered and MUST never be refused: its item says what its start waits behind ("Waits for Unstacking oauth…", `Waits`),
its start goes to the session at once (the page queues none) and waits for the running one in the queue of the session,
in the order of the starts, its rows busy and its "⋯" saying what it waits for.

#### Scenario: A rewrite started while another runs
- **WHEN** the user starts a rewrite while another rewrite of the repository runs, from a menu, a drag, a key or a toast's Undo
- **THEN** its item says what its start waits behind, and it waits in the queue of the session, in the order of the starts
- **AND** its rows are busy and its "⋯" says what it waits for
  [@test] src/hooks/useBranchContextActions.test.ts (`shows the rewrite that runs in the session running, and says what the start of every other rewrite waits behind`)
  [@test] src/lib/rewrite-run.test.ts (`says behind which rewrite of this window it waits in the queue of the session`)
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should take a second drag while the first move runs, wait for it in the queue and move the commit as the first move left it`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show an unstack started from the menu on the row's ⋯ and let the other rewrites queue behind it, from a key too`)
  [@test] crates/operations/src/rewrite_test.rs (`a_reword_and_an_unstack_from_two_windows_at_once_are_both_applied_in_order`)

### Requirement: Branch menu: A queued rewrite runs on what the one before left

A queued rewrite MUST run on what the one before left (a move, a squash, a reword, an amend, an added issue reference and
an unapply take the commits by the ids that one gave them, also a commit whose subject it changed; an unapply once the
sync built their copies), in the repository of its start, whichever the window shows by then.

#### Scenario: A rewrite queued behind one that changed its commit
- **WHEN** a reword, a move or an unapply is queued behind a rewrite that gave its commits other ids or another subject
- **THEN** it takes the commits by the ids that rewrite gave them, and rewrites them as that one left them
  [@test] tests/e2e/amend-changes/amend-changes.spec.ts (`should reword a commit as the issue reference before it in the queue left it: another id, another subject`)
  [@test] src/stores/branch-sync.test.ts (`finds a commit whose subject a message rewrite changed by the committer it kept, and one an amend replayed by its subject`, `finds no commit by a key two commits share`)
  [@test] src/hooks/useRewordCommit.test.ts (`rewords the commit as the rewrite before it in the queue left it, its preview and its Undo on that commit`)
  [@test] crates/operations/src/rewrite_test.rs (`a_reword_queued_behind_the_issue_reference_of_its_commit_names_it_through_the_new_id`)
  [@test] crates/operations/src/rewrite_test.rs (`an_unapply_queued_behind_a_reword_of_its_branch_unapplies_what_the_reword_left`)

#### Scenario: The window switched to another repository
- **WHEN** the window switched to another repository while the rewrite waited
- **THEN** it runs in the repository of its start
  [@test] tests/e2e/amend-changes/amend-changes.spec.ts (`should amend in the repository of its start once the rewrite before it ended, though the window switched to another one meanwhile`, `should add the issue reference in the repository of its start once the rewrite before it ended, though the window switched to another one meanwhile`)

### Requirement: Branch menu: A remote action whose resource is held is busy

A remote action whose resource an operation holds (a push of a branch a push of the stack sends, a cancel during a
restart, an action another process runs) MUST be disabled with its busy state ("Pushing branch login…", "brd: Pushing
branch auth…", `Busy`), and a key says so in an info toast.

#### Scenario: A push of a branch the push of the stack sends
- **WHEN** a push of the stack runs and sends the branch
- **THEN** the item runs while its push of the stack runs in the session, and waits for another push that holds the branch
  [@test] src/hooks/useBranchContextActions.test.ts (`runs while its push of the stack runs in the session, and waits for another push that holds the branch`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_operations_of_the_session_lie_over_the_actions_its_own_runs_a_rewrite_waits_a_push_behind_its_ref_is_busy`)

#### Scenario: A cancel during a restart
- **WHEN** a restart of a robot runs and the user cancels it
- **THEN** the restart and the cancel run once and hold each other back
  [@test] crates/domain-policy/src/actions_test.rs (`the_restart_and_the_cancel_of_a_robot_run_once_and_hold_each_other_back`)

### Requirement: Branch menu: A retarget and a review request run side by side

A retarget and a review request of one pull request MUST run side by side, as they write other fields of it, unless the
review request pushes first, which may retarget it too.

#### Scenario: A retarget during a review request
- **WHEN** a retarget and a review request of one pull request run at once, and the review request does not push first
- **THEN** they run side by side, and neither holds back another field of the pull request
  [@test] src/presenters/next-step.test.ts (`lays what runs on the row over the step: its own action runs, the others wait, never a quiet step nor another field of the pull request`)

### Requirement: Branch menu: A success toast with Undo comes after the sync

A success toast with Undo MUST come after the sync.

#### Scenario: A rewrite succeeds
- **WHEN** a rewrite of the menu succeeds
- **THEN** its success toast with Undo comes after the sync

### Requirement: Branch menu: An action started from the menu shows its spinner on the row

An action started from the menu, a key or a toast MUST show its spinner on the "⋯" of its row (`aria-busy`, its hover the
busy state); one started from a button that stays (the notice's "Unstack from login", a form, a dialog, the Safe Push
popover) shows it there, and "⋯" is only disabled.

#### Scenario: An unstack from the menu, a key or the alert
- **WHEN** an unstack starts from the menu on the row's "⋯", or from a key
- **THEN** its spinner shows on the "⋯" of its row, its hover the busy state
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should show an unstack started from the menu on the row's ⋯ and let the other rewrites queue behind it, from a key too`)

#### Scenario: A start from a button that stays
- **WHEN** an unstack starts from the button of the alert
- **THEN** its spinner is on the button until the sync shows the branch, and "⋯" is only disabled
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should unstack once from the alert, its spinner on the button until the sync shows the branch`)

### Requirement: Branch menu: Every row the operation works on is busy

Every row the operation works on MUST be busy meanwhile, as the view of the session has it in every window
(`RepositoryView.operations`: a rewrite of the branch or of its commits, the retarget of its pull request;
`useRowOperation`): no right-click menu, "⋯" disabled, no drag, no push.

#### Scenario: A row an operation works on
- **WHEN** an operation rewrites the branch or its commits, or retargets its pull request
- **THEN** its row has no right-click menu, its "⋯" is disabled, and it takes no drag and no push, in every window
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should unstack once from the alert, its spinner on the button until the sync shows the branch`, `should show an unstack started from the menu on the row's ⋯ and let the other rewrites queue behind it, from a key too`)

### Requirement: Branch menu: The menu button is one element through the states of its action

The "⋯" MUST be one element through the states of the action it shows (its menu; the spinner, no menu; the wait for
another holder of the lock, its menu Stop Waiting): disabled it keeps the focus (`aria-disabled`), and an item of its menu
gives the focus back to it, but for one that opens an inline form, which takes it (`focusBackFromMenu`).

#### Scenario: A rewrite chosen by key that waits for the lock
- **WHEN** the user chooses a rewrite from the menu of "⋯" by key, and it waits for the lock
- **THEN** the focus stays on the "⋯": Enter opens Stop Waiting, Escape closes it again, and the keys of the row go on
- **AND** before, the focus fell to the page, at the item and at each state, and the keys stopped until a click
  [@test] tests/e2e/repository/repository-lock.spec.ts (`should keep the focus on the row's ⋯ through a rewrite started from its menu by key that waits for the lock, and stop it by key`)

### Requirement: Branch menu: The menu gives the focus back only from itself or the page

The menu of a "⋯" MUST give the focus back only from itself (its popup, a submenu of it) or from the page: a menu that
opened as it closed keeps the focus.

#### Scenario: A right-click on the row within the exit of the menu
- **WHEN** the user presses Escape on a "⋯" and right-clicks the row within the exit of its menu
- **THEN** the menu of the row keeps the focus
- **AND** before, the end of that exit gave the focus to the "⋯", whose tooltip took the next Escape: the menu of the row stayed open over the page, its keys dead, and every click under it waited (a test of the island ran into its timeout under load)
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should keep the focus in the menu of a right-click on the row that opened as the menu of its ⋯ closed`)

### Requirement: Branch menu: The same error toast does not stack

The same error toast MUST NOT stack: an error with the title and the description of an open error toast updates it in
place; Activity records each.

#### Scenario: The same error twice
- **WHEN** an error comes with the title and the description of an open error toast
- **THEN** it updates that toast in place, and Activity records each

### Requirement: Branch menu: An item that opens a form ends with an ellipsis

An item that opens a form MUST end with "…"; an item that acts at once MUST NOT: "Safe Push" and "Dry Run" start at once
and open their popover only when they have something to say, "Safe Push With Profile…" always opens it.

#### Scenario: The names of the items
- **WHEN** the menu of a branch opens
- **THEN** the items that open a form end with "…", and "Safe Push" and "Dry Run" do not
  [@test] src/hooks/useBranchContextActions.test.ts (`says why an item doesn't apply in its description, never in its name, and names every item by a stable id`)

### Requirement: Branch menu: An item that doesn't apply now stays and says why

An item that doesn't apply now MUST stay, disabled, and say why on hover, never in its name; it is left out only where it
never applies (no pull request, no robot). Every item has a stable `id`, what the surfaces and the review window name it
by.

#### Scenario: An item that doesn't apply now
- **WHEN** an item doesn't apply now
- **THEN** it stays, disabled, and says why on hover: "Add Issue Reference…" with "Every commit of the branch has an
  issue reference"
- **AND** every item has a stable `id`
  [@test] src/hooks/useBranchContextActions.test.ts (`says why an item doesn't apply in its description, never in its name, and names every item by a stable id`)

#### Scenario: An item that never applies
- **WHEN** the branch has no pull request, or no robot
- **THEN** the items of the pull request or of the robot are left out

### Requirement: Branch menu: The push item is the push button of the row

The push item MUST be the push button of the row as a menu item (`getPushButtonNow`), also where the row has no button for
a first push: its name, what it pushes (`pushOrConfirm`: the stack only where the button pushes it, the question in the
button's description before a push that drops remote commits), enabled as that button (the `push` Rust decides), else
disabled with the reason.

#### Scenario: The name and the push of the item
- **WHEN** the menu of a branch with a push opens
- **THEN** the push item is named as the push button ("Push Branch (2)", "Force Push Branch (1)", "Push Stack (3 branches)" with "Pushes auth, login and oauth, parents first" on
  hover), and pushes the stack as far as the button does
- **AND** above an ancestor whose push drops remote commits it pushes the branch alone: that one asks first in its own row
  [@test] src/hooks/useBranchContextActions.test.ts (`is one item named as the push button, pushing the stack as far as the button does`, `pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should push from the branch menu as P does, and say why a pushed branch can't be pushed`)

#### Scenario: A push that can't run
- **WHEN** the push of the branch can't run
- **THEN** the item is disabled with the reason ("The remote branch is up to date", the blocked parent, "The branch failed to sync: merge conflict" for a
  branch the last sync did not build, the running Safe Push that holds it, "Wait for the sync to finish" while a sync
  rebuilds the branch)
  [@test] src/hooks/useBranchContextActions.test.ts (`is disabled with the reason Rust gives: a branch the last sync did not build names the status of its row, not up to date`)
  [@test] crates/domain-policy/src/actions_test.rs (`the_push_waits_for_parked_blocked_failed_held_a_rebuild_and_nothing_to_push_in_this_order`)
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should push from the branch menu as P does, and say why a pushed branch can't be pushed`)

#### Scenario: A running push
- **WHEN** its push runs in the session, or another push that sends the branch holds it
- **THEN** the item is running ("Pushing branch login…"), or disabled ("Pushing branch login…" of the stack push of a branch built on it)
  [@test] src/hooks/useBranchContextActions.test.ts (`runs while its push of the stack runs in the session, and waits for another push that holds the branch`)

### Requirement: Branch menu: The push item is the one push of the branch in the menu

The push item MUST be the one push of the branch in the menu: no second "Push Stack" pushes an ancestor past the question
its own push button asks. The branch preview and the review list the same item.

#### Scenario: The push of a branch in a stack
- **WHEN** the menu of a branch in a stack opens
- **THEN** it has one push item, and no second "Push Stack" pushes an ancestor past the question its own push button asks
  [@test] src/hooks/useBranchContextActions.test.ts (`pushes the branch alone above an ancestor whose push drops remote commits: that one asks first in its own row`)

### Requirement: Branch menu: An item shows its key only where the key works

An item MUST show its key only where the key works: the push `P`, "Safe Push" `S`, "Review Changes" `D`, "Open Pull
Request" `O` and "Move to…" `M`, the keys of the row ([Keyboard](#keyboard)).

#### Scenario: The keys of the items
- **WHEN** the menu of a branch opens, and the user chooses "Open Pull Request" or presses M on the row
- **THEN** the items show their keys, and the pull request opens, or "Move to…" opens for the focused row
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request from the branch context menu`)
  [@test] tests/e2e/move-commits/combine-branches.spec.ts (`should open Move to… for the focused row with M, without a selection`)

### Requirement: Branch menu: Review Changes reviews the branch in its window

"Review Changes" MUST review the branch in its review window ([Branch Review](../branch-review/spec.md)), disabled with
the reason for a branch without commits, a failed or a blocked one.

#### Scenario: Review Changes of a branch that can't be reviewed
- **WHEN** the branch has no commits, failed or is blocked
- **THEN** "Review Changes" is disabled with the reason

### Requirement: Branch menu: Two menus of one branch at once each follow the stores

Every surface of the menu of a branch (the right-click, its "⋯", the "⋯" of the preview, the command palette) MUST read
it on its own: two of them on one branch at once, a menu that still fades out beside the palette opened on its row,
each show the menu as the stores are now, and the window keeps its rows. The palette MUST read the menu of the branch at
hand only while it shows.

#### Scenario: The palette opens on a row whose menu still fades out
- **GIVEN** the menu of a row still fades out after its Unstack was pressed, the first sync of a warm start held
- **WHEN** the palette opens on the row, and the sync of the unstack comes
- **THEN** the window keeps its rows, the branch at the root
- **AND** the palette lists the menu of the branch as it is now, without the Unstack
  [@test] tests/e2e/startup/warm-start.spec.ts (`keeps the window when the palette opens on a row whose menu still fades out and the sync of the unstack pressed in it comes`)

#### Scenario: The menu of the row the palette listed last, during a sync
- **GIVEN** the palette listed the actions of a row and closed
- **WHEN** the menu of that row is open while a sync runs
- **THEN** it keeps its items, and the window its rows
  [@test] tests/e2e/command-palette/command-palette.spec.ts (`should keep the window when the row the palette listed last opens its menu and a sync runs`)

### Requirement: Inline forms: A form renders in a row below the branch row

A form opened from the menu MUST render in a row below the branch row, one form at a time, with a visible title ("Rename
auth") and, where it adds something, a muted description.

#### Scenario: A form opened from the menu
- **WHEN** the user opens a form from the menu of a row, then another
- **THEN** it renders in a row below the branch row with its visible title, and one form shows at a time

### Requirement: Inline forms: A form focuses its input or its button

A form with an input MUST focus the input. A confirmation without one (Unapply, Ungroup) MUST focus its button once the
button is enabled, so Enter confirms. Escape cancels.

#### Scenario: A form with an input
- **WHEN** a form with an input opens
- **THEN** its input has the focus
  [@test] tests/e2e/issue-reference.spec.ts (`should focus input when opened`)

#### Scenario: A confirmation without an input
- **WHEN** a confirmation without an input opens (Unapply, Ungroup)
- **THEN** its button takes the focus once it is enabled, so Enter confirms, and Escape cancels
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)

### Requirement: Inline forms: A Safe Push or a Dry Run is no inline form

A Safe Push or a Dry Run MUST NOT be an inline form: its popover opens at the Safe Push slot
([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)).

#### Scenario: A Safe Push from the menu
- **WHEN** the user starts a Safe Push or a Dry Run that has something to say
- **THEN** its popover opens at the Safe Push slot, not a form below the row

### Requirement: Inline forms: A running form is a progress row

While its action runs, the form MUST be a progress row that says what runs ("Adding ABC-123 to auth…"), and close when the
action is done, after the sync that shows its result (a deletion: the patch of the session).

#### Scenario: The issue reference is added
- **WHEN** the user adds an issue reference from the menu
- **THEN** the form is a progress row "Adding ABC-123 to auth…" while it runs, and closes after the sync that shows its result
  [@test] tests/e2e/issue-reference.spec.ts (`should add issue reference to branch via context menu`)

### Requirement: Inline forms: A failed form with an input shows again with the typed text

A failure of a form with an input (rename, issue reference, the archived delete) MUST show the form again with the typed
text, focused; the error is a toast.

#### Scenario: Adding an issue reference fails
- **WHEN** the action of a form with an input fails
- **THEN** the form shows again with the typed text, focused, and the error is a toast
  [@test] tests/e2e/issue-reference.spec.ts (`should keep the typed reference when adding it fails`)

### Requirement: Inline forms: Adding an issue reference waits for the holder of the lock

Adding an issue reference MUST reword the branch's commits, computed before the repository lock and written under it after
the witness of HEAD: while another process holds the lock (an agent's `brd pick --follow`, `brd`) or the app's own push,
the reword waits as long as the holder lives, the form's progress row saying whom for with Stop Waiting.

#### Scenario: A live holder of the lock
- **WHEN** another process or the app's own push holds the lock as the reword is to be written
- **THEN** the reword waits as long as the holder lives, and runs after it
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_waits_for_the_holder`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`an_amend_and_an_issue_reference_lock_only_the_witness_and_the_write`)

#### Scenario: Stop Waiting
- **WHEN** the user chooses Stop Waiting on the progress row
- **THEN** the wait ends with nothing changed and the form kept
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

#### Scenario: A holder no record names
- **WHEN** a holder no record names holds the lock
- **THEN** the reword is refused after 10 s, with nothing changed, and the form comes back with the error toast that another operation holds the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

### Requirement: Inline forms: Closing a form gives the focus back to its opener

Closing a form (Escape, Cancel, done) MUST give the focus back to the element that opened it, or to the row when a menu
item opened it or the opener is gone. The row is focusable, not a tab stop. A focus the user moved elsewhere meanwhile
stays.

#### Scenario: Escape in a form opened from the menu
- **WHEN** the user closes a form with Escape
- **THEN** the form closes and the focus goes back to its opener, or to the row when a menu item opened it
  [@test] tests/e2e/issue-reference.spec.ts (`should cancel with Escape key`)

#### Scenario: The focus moved elsewhere
- **WHEN** the user moved the focus elsewhere while the form was open, and the form closes
- **THEN** the focus stays where the user moved it

### Requirement: Inline forms: The form row is tinted and its content is narrow

The form row MUST be `bg-muted/50` with `bd-padding-content`. Its content MUST be `flex max-w-3xl flex-col gap-4` (`gap-2`
for a form with one input), so it doesn't stretch across a wide window.

#### Scenario: A form in a wide window
- **WHEN** a form opens below a row in a wide window
- **THEN** its row is `bg-muted/50`, and its content doesn't stretch across the window

### Requirement: Inline forms: The title of a form is medium and its description muted

The title MUST be `text-sm font-medium`, the description below it `text-sm text-muted-foreground`; the progress row keeps
the title for the screen reader only.

#### Scenario: A form and its progress row
- **WHEN** a form shows, then its action runs
- **THEN** the form has its title and description, and the progress row keeps the title for the screen reader only

### Requirement: Inline forms: An input is followed by Cancel and the submit button

An input MUST be `max-w-md`, followed by Cancel and the submit button (`xs`); the submit of a delete is `destructive`.

#### Scenario: The buttons of a delete
- **WHEN** a form of a delete shows
- **THEN** its input is followed by Cancel and the `destructive` submit button

### Requirement: Inline forms: A focused row has the outline and no tint of its own

A focused row MUST have the focus outline inside it (`bd-focus-inset focus-visible:bd-focus`, also when the arrows moved
the focus there) and no tint of its own: `bg-muted` is the hover, `bg-primary/15` the selection, so a row under the
pointer and the row of the keys never look the same.

#### Scenario: The row of the keys and the row under the pointer
- **WHEN** the arrows focus one row and the pointer is over another
- **THEN** the focused row has the outline inside it, the hovered one `bg-muted`, and they never look the same

### Requirement: Inline forms: A dimmed row is faded until hover or focus

A dimmed row MUST be `opacity-55`, full on hover or focus.

#### Scenario: A dimmed row
- **WHEN** a row is dimmed
- **THEN** it is `opacity-55`, full on hover or focus

### Requirement: Archived table: The table has four columns and shows the last segment

The archived table MUST have the columns Branch Name, Status, Integrated and Actions. The name shows the last segment of
the archived branch path; the full name is copied.

#### Scenario: The archived branches card
- **WHEN** the archived table shows
- **THEN** it has the columns Branch Name, Status, Integrated and Actions, the name the last segment of the archived branch path
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should render archived branches card`)

### Requirement: Archived table: The status is one of three words after its icon

The status MUST be one of three words, in muted text after its icon: "Merged" with the merge icon in `info` (the merged
colour of the chips), "Partially merged · 1 of 2" with the pie icon, muted, and "Not merged" with the warning triangle
in `warning`.

#### Scenario: The words of the status
- **WHEN** the integration of an archived branch is detected
- **THEN** its status is "Merged", "Partially merged · 1 of 2" or "Not merged", in muted text after its icon
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should name the integration status in words, with the counts in the tooltip`)

### Requirement: Archived table: The detection and the commits load in their places

A spinner in the Status column MUST show while the integration is detected; the Integrated column stays empty until then.
Expanding a row MUST load its commits once, "Loading commits…" with its spinner below it; the row itself shows no
progress.

#### Scenario: The detection runs
- **WHEN** the integration of an archived branch is being detected
- **THEN** a spinner shows in the Status column, and the Integrated column stays empty

#### Scenario: A row expands
- **WHEN** the user expands an archived row
- **THEN** its commits load once, "Loading commits…" with its spinner below it, and the row itself shows no progress
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should show the loading of the commits of an expanded row once, below it`)

### Requirement: Archived table: A branch without an integration result says Not checked

A branch the finished sync reported no integration for (its detection failed, or the whole detection did) MUST show "Not
checked" with the dashed circle, muted, and the tooltip "Integration could not be checked, see the log", instead of
spinning until the next sync; its commits still expand. A later result replaces it.

#### Scenario: The detection failed
- **WHEN** a sync finished without an integration result for an archived branch
- **THEN** it shows "Not checked" with the dashed circle and its tooltip, and its commits still expand
- **AND** a later result replaces it
  [@test] src/stores/archived-branches.test.ts (`marks the placeholders a finished sync left without a result as unknown`)

### Requirement: Archived table: The tooltip of the status holds the counts and the merge commit

The tooltip of the status MUST hold the counts and the merge commit. An exact match adds a `success` "Exact confidence"
badge.

#### Scenario: The tooltips of the status
- **WHEN** the user hovers the status of an archived branch
- **THEN** the tooltip says "3 commits, merged as abc12345 by Safe Push
  Robot", "1 of 2 commits merged; the original commits are no longer in HEAD", or "1 commit, none merged; the original
  commits are no longer in HEAD"
- **AND** it says what it knows when the count or the merge commit is unknown
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should name the integration status in words, with the counts in the tooltip`)
  [@test] src/lib/integration-commit.test.ts (`names the commit count and the merge commit`, `says what it knows when the count or the merge commit is unknown`, `names the merged part, or says that none merged`)

#### Scenario: An exact match
- **WHEN** the integration is an exact match
- **THEN** a `success` "Exact confidence" badge is added

### Requirement: Archived table: The Integrated column shows when the branch was integrated

The Integrated column MUST show when the branch was integrated.

#### Scenario: An integrated branch
- **WHEN** an archived branch was integrated
- **THEN** its Integrated column shows when

### Requirement: Archived table: An expanded integrated branch shows the merged-by line

Expanded, an integrated branch MUST show the merged-by line above its commits: the commit icon in `info`, "Merged as
abc12345 by Safe Push Robot" (the hash in the monospace font, no time: the Integrated column shows it), the author on
hover, and a button that copies the full hash.

#### Scenario: An expanded integrated branch
- **WHEN** the user expands an integrated archived branch
- **THEN** the merged-by line above its commits names the short hash and the committer, not the time the row already shows
  [@test] src/lib/integration-commit.test.ts (`names the short hash and the committer, not the time the row already shows`)
  [@test] src/stores/archived-branches.test.ts (`keeps the commit that merged the branch`)

### Requirement: Archived table: The header counts the branches not merged

The header of the archived card MUST say, after its count, how many archived branches the baseline has none of the
commits of: "3 not merged" in `warning`, with the tooltip "3 archived branches have commits the baseline has none of", so
work that may be lost shows without expanding the card; nothing while none is.

#### Scenario: Archived branches the baseline has none of
- **WHEN** archived branches have commits the baseline has none of
- **THEN** the header says "3 not merged" in `warning` after its count, with its tooltip
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should name the integration status in words, with the counts in the tooltip`)

#### Scenario: Every archived branch merged
- **WHEN** no archived branch is without commits in the baseline
- **THEN** the header says nothing of it

### Requirement: Archived table: The help after the count explains archiving

The help after the count of the archived card MUST be titled "Archived Branches" and say when a sync archives a branch and
which archived branches it deletes. It is no part of the toggle, which covers the rest of the header row.

#### Scenario: The help of the archived card
- **WHEN** the user opens the help after the count of the archived card
- **THEN** it says "A sync archives a virtual branch when none of your commits group
  into it anymore. A merged one is deleted by the first sync 7 days after it was archived; the others stay until you
  delete them."
- **AND** the rest of the header row still toggles the card
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should explain archiving and cleanup in the help after the title, the header still toggling around it`)

### Requirement: Archived table: A branch archived in this session is marked New

A branch a sync of this session archived MUST show an `info` "New" badge after its name until the Landed island is
dismissed.

#### Scenario: A branch archived in this session
- **WHEN** a sync of this session archived a branch
- **THEN** it shows the "New" badge after its name until the Landed island is dismissed
  [@test] src/stores/archived-branches.test.ts (`marks the branches archived in this session until cleared`)
  [@test] tests/e2e/update-project/landed.spec.ts (`should lead the toast with what landed and keep the island until it is dismissed`)

### Requirement: Archived table: The menu of an archived row offers its review and its delete

The menu of an archived row MUST offer Review Changes (D, [Review Window](../../review/review-window/spec.md#kept-branches)),
the copy items and "Delete Archived Branch…" in the `destructive` colour, which asks for the branch name; its Delete button
is `destructive` too. Rust offers them (`domain_policy::kept`, the subject `archived:<full name>` of the view).

#### Scenario: The menu of an archived row
- **WHEN** the user opens the menu of an archived row
- **THEN** it offers Review Changes, the copy items and "Delete Archived Branch…", which asks for the branch name
  [@test] crates/domain-policy/src/kept_test.rs (`an_archived_branch_offers_its_review_and_its_delete`)
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should delete an archived branch from its menu, the row gone with no sync after it`)

### Requirement: Archived table: The Actions cell has the menu and ⌘C copies the full name

The Actions cell MUST have "⋯" with the same menu; ⌘C on the focused row MUST copy the full name, as "Copy Full Branch
Name".

#### Scenario: The more actions button of an archived row
- **WHEN** the user clicks "⋯" of an archived row
- **THEN** the same menu opens
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should open the menu of an archived row from its more actions button`)

#### Scenario: ⌘C on an archived row
- **WHEN** the user presses ⌘C on the focused archived row
- **THEN** the full name of the archived branch is copied
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should copy the full name of an archived branch with ⌘C on its focused row`)

### Requirement: Archived table: The delete removes the branch only from the tip it read

The delete MUST remove the archived branch only from the tip it read (one ref transaction with compare-and-swap), and MUST
refuse a branch checked out in any worktree, naming that worktree, with the branch left as it is.

#### Scenario: The delete of an archived branch
- **WHEN** the user deletes an archived branch
- **THEN** only the archived branch is deleted, from the tip it read
  [@test] crates/sync-core/src/delete_archived_branch_test.rs (`deletes_the_archived_branch_only`)

#### Scenario: A branch checked out in a worktree
- **WHEN** the archived branch is checked out in a worktree
- **THEN** the delete is refused, naming that worktree, and the branch is left as it is
  [@test] crates/sync-core/src/delete_archived_branch_test.rs (`refuses_a_branch_checked_out_in_a_worktree`)

### Requirement: Archived table: The delete is a rewrite of the engine that holds one ref

The delete MUST be the rewrite `deleteArchived` of the engine, by the full name of the branch: it holds only the ref it
drops, so it runs beside a rewrite and a sync, and no Undo follows it.

#### Scenario: The delete through the engine
- **WHEN** the user deletes an archived branch while a rewrite or a sync runs
- **THEN** the delete runs through the engine beside them, and no Undo follows it
  [@test] crates/operations/src/rewrite_test.rs (`an_unapply_a_reapply_and_the_delete_of_the_unapplied_branch_run_through_the_engine`)

### Requirement: Archived table: A deleted archived branch leaves the table as a patch

A deleted archived branch MUST leave the table as a patch of the session, with no sync after it; a sync that listed it
before the delete, or one that fails, does not bring it back.

#### Scenario: The row is gone with no sync after it
- **WHEN** an archived branch is deleted
- **THEN** its row is gone as a patch of the session, and no sync runs after it
  [@test] crates/repository-model/src/session_test.rs (`deleting_an_archived_or_unapplied_branch_answers_a_patch_and_runs_no_sync`)
  [@test] crates/test-server/src/session_routes_test.rs (`a_deletion_of_an_archived_branch_answers_the_version_of_its_patch_and_runs_no_sync`)
  [@test] tests/e2e/archived-branches/archived-branches.spec.ts (`should delete an archived branch from its menu, the row gone with no sync after it`)

#### Scenario: A sync that listed it before
- **WHEN** a sync that listed the branch before the delete ends, or a sync fails
- **THEN** the branch stays deleted
  [@test] crates/repository-model/src/table_test.rs (`a_deleted_archived_or_unapplied_branch_stays_deleted_by_the_sync_that_listed_it_before_and_by_one_that_fails`)

### Requirement: Archived table: The status is a size-4 icon and a muted word, New an info badge

The status icon MUST be `size-4`, the word `text-sm text-muted-foreground`. "New" MUST be an `info` badge.

#### Scenario: An archived row
- **WHEN** an archived row shows its status, and is new
- **THEN** its icon is `size-4`, its word `text-sm text-muted-foreground`, and "New" is an `info` badge

### Requirement: Archived table: An expanded archived row is tinted and its merged-by line muted

An expanded archived row MUST be `bg-muted`; its merged-by line MUST be `text-sm text-muted-foreground`, behind the rail of
the commits.

#### Scenario: An expanded archived row
- **WHEN** the user expands an archived row
- **THEN** it is `bg-muted`, and its merged-by line is `text-sm text-muted-foreground`, behind the rail of the commits

### Requirement: Row: The cells have the vertical padding of a dense list

Every cell MUST have the vertical padding `--bd-cell-y` (6px), so a row is 36px around the 24px folder button, as dense
as an IntelliJ list.

#### Scenario: The height of a row
- **WHEN** a branch row shows
- **THEN** its cells have the padding `--bd-cell-y` (6px), and the row is 36px around the 24px folder button

### Requirement: Row: The name cell and the other cells have their paddings and tracks

The name cell MUST have `pl-(--bd-lane-start)` (1.25rem, the left edge of a lane-0 node under the header text) and
`pr-6`, the folder button at `--bd-tree-x` after the lane column. The other cells MUST have `px-4`, `w-px` and
`whitespace-nowrap`.

#### Scenario: The items of the Status and Actions cells
- **WHEN** the Status and Actions cells show their items
- **THEN** each item is a `ColumnTrack`
  (`min-w-(--bd-track-<kind>)`), the Status and Actions items in a `flex` at least as wide as the label of their column

### Requirement: Row: The table header is quiet

The table header MUST be quiet: `text-xs font-medium text-muted-foreground` on the card, with a bottom border and no fill.

#### Scenario: The header of the table
- **WHEN** the branch table shows
- **THEN** its header is `text-xs font-medium text-muted-foreground`, with a bottom border and no fill

### Requirement: Row: An expanded row fades its commits in

An expanded row MUST fade its commits in (150ms, opacity only).

#### Scenario: A row expands
- **WHEN** the user expands a row
- **THEN** its commits fade in over 150ms, opacity only

### Requirement: Row: Under reduced motion nothing animates in or out

Under reduced motion (`prefers-reduced-motion: reduce`, `src/index.css`) nothing MUST animate in or out: the expanded rows,
nor the menus, popovers, tooltips and dialogs, which zoom and slide by a variant of `animate-in` and `animate-out`
(`data-open:animate-in`, `data-closed:animate-out`). The progress indicators keep running.

#### Scenario: The branch menu under reduced motion
- **GIVEN** reduced motion
- **WHEN** the branch menu closes
- **THEN** it closes without its zoom, and the progress indicators keep running
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should close the branch menu without its zoom under reduced motion`)

### Requirement: Row: The folder button carries no accent

The folder button MUST be a ghost `icon-xs` button in `text-muted-foreground`, `text-foreground` while expanded: it repeats
on every row, so it carries no accent. The name is `text-sm font-medium`, the summary `text-xs text-muted-foreground`, the
prefix of a subject `font-mono`, muted as the summary.

#### Scenario: The folder button, the name and the summary
- **WHEN** a row shows, collapsed and expanded
- **THEN** its folder button is `text-muted-foreground`, `text-foreground` while expanded
- **AND** its name is `text-sm font-medium`, its summary `text-xs text-muted-foreground`, the prefix of a subject `font-mono`

### Requirement: Row: The family guide is a line over the cell

The family guide MUST be a `w-px bg-border` line over the cell (`--bd-family-step` in `src/index.css`).

#### Scenario: A member of an issue family
- **WHEN** a member of an issue family shows
- **THEN** its guide is a `w-px bg-border` line over the cell

### Requirement: Row: The lanes are opaque 1px strokes stretched over the row

The lanes (`StackLanes.tsx`) MUST be 1px lines in `--bd-lane-line` (`--bd-lane-line-width`), the muted foreground at
40% over the card: opaque, so the curve of a fork doesn't darken the line it leaves, and stronger than a separator. A
straight line MUST be a box at the x of its lane (`bd-lane-x`), as the rail draws its line; a fork an SVG stroke over
the row. The lines into a node stop at its edge, or at the outer edge of the ring of its robot, in px from its centre
(`--bd-node-cut`).

#### Scenario: The fork of a side child
- **WHEN** a side child forks off its parent's lane
- **THEN** the fork is a quadratic curve from the parent's lane at the top of the row into the left edge of the node, and the lines end at the edge of the node

#### Scenario: A running robot around the node
- **WHEN** the robot of the branch runs
- **THEN** the line up, the line down and the fork curve stop at the outer edge of its ring, whatever the height of the
  row, and the short line of a collapsed root starts below the ring

#### Scenario: A lane across the border of a row
- **WHEN** a lane runs from a branch row into the row below it (its rail, the next branch row), or a fork leaves it
- **THEN** it is one line: the same device pixels in the same colour just above and just below the border, a plain lane
  and the line of a robot alike; a box snaps to whole device pixels, where an antialiased stroke next to it looks
  thinner and lighter, and the SVG of a fork starts where the box of its parent's lane does, so the curve leaves that
  line in its pixels (`preserveAspectRatio="none"`, `vector-effect: non-scaling-stroke`, the mask `bd-node-cut`)
  [@test] tests/e2e/stacked-branches/stacked-branches.spec.ts (`should carry the commits of a branch on its lane, into the lines of its children`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should start the line of the robot at the ring of its node, in the colour of the ring`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should run the line of the robot of a side branch from the fork through its node into its commits`)

### Requirement: Row: The node is a small circle in the colour of its kind

The node MUST be `size-2 rounded-full` with a 1.5px border in the colour of its kind (`currentColor`), `bg-current` when
filled, centred in a `size-4` hover target (`LaneNodeDot`, which the legend and the island header draw too).

#### Scenario: A running robot around the node
- **WHEN** the robot of the branch runs
- **THEN** its `RobotRing` is around the node (`lane-ring`: `size-4`, the lane's width, a thinner and wider
  ring than in the slot, so the node keeps its gap; `text-primary`, turning)

### Requirement: Row: The landing span and the path to the root are 1.5px strokes

The segments of a landing span MUST be a line of `--bd-lane-span-width` in `--bd-robot-line` over the lines
(`landing-lanes`), the colour of the ring and of the robot's progress line: `--color-primary`, `--color-destructive`
while the robot will fail. The path to the root MUST be a line as wide in `--bd-lane-path`, the foreground at 70% over
the card, under the landing span (`stack-path-lanes`).

#### Scenario: The width of a highlighted line
- **WHEN** the window is at a device pixel ratio of 2 or more
- **THEN** a highlighted line is 1.5px, whole device pixels; below 2 it is 2px, as a box of 1.5px snaps to 2 there, so the
  curve of a fork keeps the width of the line it leaves

#### Scenario: The path of a hovered branch
- **WHEN** the path to the root of a branch is highlighted
- **THEN** it fades in over 120 ms (`bd-path-in`,
  `@starting-style`; none under reduced motion) and is gone at once
- **AND** a node the hovered push sends gets a 1.5px `outline-current` 2px off it

#### Scenario: The span of a robot that will fail
- **WHEN** a required check of the robot that lands the span failed
- **THEN** its segments are `destructive` (`data-failing`), as its ring
  [@test] src/lib/stack-lanes.test.ts (`marks the span of a robot that will fail, the lanes passing the rows between too, and leaves the others`)
  [@test] tests/e2e/patronus/robot-coverage.spec.ts (`should draw the line and the dots of a robot that will fail in the colour of its ring`)

### Requirement: Row: The commit dots and the rows below a branch sit on the rail

The commit dots MUST be 5px in `--bd-lane-dot` (`bd-lane-rail`), the content of the rows below a branch row MUST start at
`--bd-row-inset`. The running robot that tests the branch draws its line and its dots over them
([Patronus](../patronus/spec.md#commits-a-running-robot-covers)).

#### Scenario: The commits of an expanded branch
- **WHEN** a branch is expanded
- **THEN** its commit dots are 5px in `--bd-lane-dot`, and the content of its rows starts at `--bd-row-inset`
- **AND** a running robot that tests the branch draws its line and its dots over them

### Requirement: Row: The commit count stands at the end of its track

The commit count MUST be `text-sm text-muted-foreground tabular-nums`, at the end of its track (`justify-end`), under the
head "Commits" that labels the track.

#### Scenario: The count of a row
- **WHEN** a row shows its commit count
- **THEN** it is `text-sm text-muted-foreground tabular-nums`, `justify-end` under the head "Commits"

### Requirement: Status: The items are capped tracks after a gutter

The items MUST be the tracks `state` (capped at `max-w-40`, its text truncates), `pr` and `landed` (capped at `max-w-48`),
each after an 8px gutter while its column has content; `landed` is a `flex items-center gap-2` of the Landed chip, then the
`text-xs text-muted-foreground` "lands with oauth" or "on master".

#### Scenario: A running robot in the state track
- **WHEN** a running robot is in the `state` track
- **THEN** it is at least `13ch` wide ("59m · ~1h 59m"), so its ticking time moves no column

### Requirement: Status: The status text is muted after a dot and the robot reads as it

The status text MUST be `text-sm text-muted-foreground`, truncated in its column, after a `size-1.5` dot. The badge is the
subtle badge of its colour. The robot in its place is its chip at `text-sm`, `h-6` without padding, a `size-3.5` icon (the
ring while it runs) where the dot is; the progress line under its time is `h-0.5`, in the padding of the cell.

#### Scenario: The status text and the robot
- **WHEN** the first column shows a status text, or a robot
- **THEN** the text is `text-sm text-muted-foreground` after a `size-1.5` dot
- **AND** the robot is its chip at `text-sm`, `h-6`, a `size-3.5` icon where the dot is, its progress line `h-0.5`

### Requirement: Status: The mark of the covered facts follows the first column

The mark of the facts the first column covers MUST follow it after `gap-2`: a `size-1.5` dot in the colour of its strongest
fact inside a `size-4` target (`row-status-mark`), its lines in its tooltip and in the hover of the node (`lane-node-also`,
muted).

#### Scenario: A first column that covers facts
- **WHEN** the first column covers other facts
- **THEN** a `size-1.5` dot in the colour of the strongest follows it after `gap-2`, its lines in its tooltip and in the hover of the node

### Requirement: Status: A chip is a small rounded button without a border

A chip (`StatusChip`) MUST be `h-5`, `text-xs` with tabular figures and a `size-3.5` icon, `rounded-2xl` as buttons and
badges (its focus outline), without a border or an outline, and underlined on hover. A dot is `size-1.5 rounded-full
shrink-0`. The details popover is `w-80` with `p-3`.

#### Scenario: The pull request chip
- **WHEN** the pull request chip shows
- **THEN** it colours its
  icon alone; its number is muted ([Pull Requests](../pull-requests/spec.md#chip))

### Requirement: Status: The colours say merged, attention, failure, approval and push

The colours MUST be: `info` merged and landed; `warning` attention (blocked, a pull request or a robot behind the branch,
"Needs review"); `destructive` a failure or a conflict; `success` approved or passed; the push colour (`primary`, or
`warning` for a push that drops remote commits) for the push only.

#### Scenario: The colour of a state
- **WHEN** a state shows in the row
- **THEN** it takes the colour of its kind, and only the push takes the push colour

### Requirement: Status: The alerts of a blocked branch and of a changed base

The blocked alert MUST be the `warning` alert with the warning triangle; its "Show auth" link is `text-warning`. The alert
of a changed base MUST be `info` with the info icon for a landed parent, `warning` with the triangle for a parent without
commits, the neutral alert for an unapplied one; its "Unstack from login" link takes the alert's colour.

#### Scenario: The alerts of an expanded stacked branch
- **WHEN** a blocked stacked branch, or one whose base changed, is expanded
- **THEN** its alert takes the colour and the icon of its case, and its link the colour of the alert

### Requirement: Actions: The push button is a ghost button in the push colour

The push button MUST be a ghost `xs` button, `min-w-16 justify-start` (so "⟳ 100%" fits and the arrows stand in a column),
with the `ArrowUpIcon` and the count in tabular figures, in the push colour (`ghostButtonColorClass`). Its description
starts with the label in `font-medium`.

#### Scenario: The push button of a row
- **WHEN** a row has a push
- **THEN** its button is a ghost `xs` button with the `ArrowUpIcon` and the count, in the push colour, and the arrows stand in a column

### Requirement: Actions: Push with Children says with children in a wide view

Push with Children in its place MUST add "with children" in `text-muted-foreground` after the count where the branch view
is at least 56rem wide (`data-wide` of `BranchColumnsRoot`, which the column registry sets as the width of the view
crosses 56rem; no container query: in WebKit a query container restyles every row at each change of its width,
[UI Theme](../../../../docs/design/ui-theme.md#branch-columns)).

#### Scenario: A view at least 56rem wide and a pixel narrower
- **WHEN** the button of a row is Push with Children, and the branch view is at least 56rem wide
- **THEN** it says "with children" after the count
- **AND** a pixel narrower, the count alone, the name the same
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should say with children after the count of Push with Children while the branch view is at least 56rem wide, and only the count below`)

### Requirement: Actions: The Safe Push slot is a capped track of ghost buttons

The Safe Push slot MUST be the track `step`, capped at `max-w-40` ([Patronus](../patronus/spec.md#user-experience)).
"Safe Push" is a ghost `xs` button with the `ShieldCheckIcon` in the foreground colour, "Restart Safe Push" the same with
the `RotateCcwIcon`, the pull request step with its icon; disabled, they are dimmed to `opacity-50` and keep their hover
(`focusableWhenDisabled`).

#### Scenario: A disabled step
- **WHEN** the step of the slot is disabled
- **THEN** it is dimmed to `opacity-50` and keeps its hover

### Requirement: Actions: A starting Safe Push shows its step with a spinner

While a start of the branch runs, "Safe Push" MUST show its step with a spinner instead of the shield, `aria-busy`, not
dimmed, at the width it had at rest (`min-w-26` covers "Checking…").

#### Scenario: The steps of a start
- **WHEN** a start of the branch runs
- **THEN** "Safe Push" shows "Checking…", "45%" of its push as the compact push
  shows it (named "Pushing 45%"), "Starting…", with a spinner, at the width it had at rest

### Requirement: Actions: The menu button is a muted ghost button

"⋯" MUST be a ghost `icon-xs` button with the `EllipsisIcon`, muted, named "More actions for feature-auth", with the
tooltip "More actions". Its menu is the shadcn `DropdownMenu` (Base UI Menu), `align="end"`, as wide as its items
(`w-auto min-w-36`, as the context menu); one renderer, `MenuItems` with the primitives of either menu, draws both.

#### Scenario: An action of the row runs
- **WHEN** an action of the row started from a menu, a key or a toast runs
- **THEN** a `Spinner` is in place of the icon, `aria-busy`, the tooltip its busy state ("Unstacking oauth…")

### Requirement: Actions: The cell lays out its tracks as the island header does

The cell MUST be `ActionTracks`: a `flex items-center` of the tracks `push`, `step` and `more`, the push and the step each
with an 8px gutter after it while its column has content; the header of a stack island draws the same.

#### Scenario: The tracks of the cell
- **WHEN** a row or the header of a stack island shows its actions
- **THEN** the tracks `push`, `step` and `more` follow each other, the push and the step each with an 8px gutter after it while its column has content

### Requirement: Dark theme: The rows use only semantic tokens

The rows MUST use only shadcn semantic tokens and no `dark:` variants.

#### Scenario: The dark theme
- **WHEN** the app shows in the dark theme
- **THEN** the rows take their colours from the shadcn semantic tokens, with no `dark:` variants

## Testing / Local Run

- `pnpm test:unit`: `src/stores/branch-table-rows.test.ts`, `src/lib/stack-lanes.test.ts`,
  `src/lib/branch-row-keys.test.ts`, `src/stores/branch-sync.test.ts`, `src/presenters/row-status.test.ts`,
  `src/presenters/landing-node.test.ts`,
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
  by the vertical clip of the rail (`clip-path: inset(0 -100vw)`), which also cuts a focus outline above or below it.
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

- [Commit Row](../../commit/commit-row/spec.md): the commits of an expanded row and their rail
- [Outgoing Commits](../../repository/outgoing/spec.md): the push labels, the push colours and the push
- [Pull Requests](../pull-requests/spec.md), [Patronus](../patronus/spec.md), [Landed Commits](../../repository/landed-commits/spec.md):
  the chips
- [Stacked Branches](../stacked-branches/spec.md): the order of the rows and the lane layout
- [Branch Preview](../branch-preview/spec.md), [Move Commits](../move-commits/spec.md)
- [UI Theme: Islands Layering and the Radius Scale](../../../../docs/design/ui-theme.md)
