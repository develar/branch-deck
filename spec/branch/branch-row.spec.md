---
name: Branch Row
description: Requirements for a row of the branch table and of the archived table, the name cell with the tree of stacked and issue branches, the commit count, the Status cell with the status text and the pull request, Patronus and Landed chips in fixed columns, the Actions cell with the push button, "Safe Push…" and the "⋯" menu, hover and click, the branch menu and the inline forms below a row (title, progress, focus); and for the archived table, its status words and colours, the merged-by line and the "New" mark.
style: plain-1
targets:
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchContextMenuRow.tsx
  - ../../src/hooks/useBranchContextActions.ts
  - ../../src/components/shared/InlineForm.tsx
  - ../../src/components/shared/InlineInputDialog.tsx
  - ../../src/stores/inline-row-action.ts
  - ../../src/components/branch/BranchNameCell.tsx
  - ../../src/components/branch/IssueFamilyRow.tsx
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
  - ../../src/components/shared/RowActions.tsx
  - ../../src/components/shared/MoreActionsMenu.tsx
  - ../../src/components/shared/MenuItems.tsx
  - ../../src/components/shared/ContextMenuItems.tsx
  - ../../src/components/shared/DropdownMenuItems.tsx
  - ../../src/components/ui/dropdown-menu.tsx
  - ../../src/components/shared/TableHeader.tsx
  - ../../src/hooks/useStatusBadge.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/hooks/useRowExpansion.ts
  - ../../src/lib/remote-status.ts
  - ../../src/lib/push-summary.ts
  - ../../src/lib/status-chip.ts
  - ../../src/lib/ui-types.ts
  - ../../src/stores/branch-sync.ts
  - ../../src/lib/branch.ts
  - ../../crates/sync-core/src/sync.rs
  - ../../src/lib/stack-lanes.ts
  - ../../src/index.css
  - ../../src/components/branch/ArchivedBranchTableCard.tsx
  - ../../src/components/branch/ArchivedBranchStatusBadge.tsx
  - ../../src/components/branch/ArchivedDateCell.tsx
  - ../../src/components/branch/ArchivedBranchExpanded.tsx
  - ../../src/lib/integration-commit.ts
  - ../../src/stores/archived-branches.ts
---

# Branch Row

Status: Implemented
Date: 2026-10-14

## Summary

The branch table lists the active virtual branches, one row per branch; the archived table below it lists the branches
a sync archived. A row reads from left to right: the folder that expands the commits and the branch name, the commit
count, the Status cell and the actions. The Status cell has one status text and up to three chips (the pull request,
the Patronus robot, the landed commits), each kind in its own column, so a look down the table compares like with
like. Stacked branches and the branches of an issue hang under their parent as a tree, and the rail of an expanded
parent's commits runs into the rule of its children.

## Goals

- Read the state of every branch in one look: the status text, then each chip kind, in the same column of every row.
- Keep the table still: chips that load after the sync do not reflow it, and a long subject truncates.
- One meaning per colour, in the row and in its chips: `info` merged or landed, `warning` attention, `destructive` a
  failure, the push colour for the push.
- Show a stack as one tree: the rail of a parent's commits and the rule of its children are one line.

## Non-goals

- The commit rows of an expanded branch: [Commit Row](../commit/commit-row.spec.md).
- What the chips say and when they refresh: [Pull Requests](pull-requests.spec.md), [Patronus](patronus.spec.md),
  [Landed Commits](../repository/landed-commits.spec.md).
- The push labels, colours, descriptions and the push itself: [Outgoing Commits](../repository/outgoing.spec.md).
- What each menu item and inline form does: [Move Commits](move-commits.spec.md), [Unapply](unapply.spec.md),
  [Patronus](patronus.spec.md), [Squash Fixups](../commit/fixups.spec.md), [Amend Changes](../commit/amend.spec.md).
- The order of the rows and the placement of stacked branches: [Stacked Branches](stacked-branches.spec.md).

## Requirements

### Columns

- The branch table must have the columns Branch Name, Commits, Status and Actions. The name must take the free width,
  the other columns their content width. Commits and Status hide below `sm`.
- The Status column must be 16rem wide at least, so chips that arrive after the sync do not reflow the table.
- A long name or subject must truncate, not wrap: the row stays one line high.
  [@test] ../../tests/e2e/branch-row-layout/column-widths.spec.ts (`gives the free width to the branch name and keeps the other columns at content width`)
- A subject with less than 6rem of free width must drop out instead of truncating to a letter or two.
  [@test] ../../tests/e2e/branch-row-layout/column-widths.spec.ts (`drops a subject that has no room to be read`)

### Name cell

- The name cell must show, in this order: the folder button that expands the commits, the branch name with its issue
  keys as links, the issue chip of a branch whose issue has no row above it, the fixup count, a mark (the archived
  "New"), then the summary, muted.
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
  [@test] ../../crates/sync-core/src/sync_test.rs (`test_branches_grouped_carries_the_description_titles`)
- A branch without commits and without an error must have no folder button.

### Tree

- A branch nested under the branch of its issue, under its issue row or under the branch it stacks on must be indented
  one step per level (`--bd-nesting-step`, 1.75rem) and draw one rule per level over the whole row height. The rule of
  level n sits 2rem + (n − 1) steps from the cell edge, under the left wall of the folder icon of the row above.
  [@test] ../../src/stores/branch-table-rows.test.ts (`puts each branch under its parent, depth-first, wherever the parent is in the order`)
  [@test] ../../src/stores/branch-sync.test.ts (`nests the branches of an issue under the issue branch, wherever it is in the order`, `heads the branches of an issue without a branch of its own with an issue row`)
- The commits of an expanded branch at depth d must sit behind a rail at the rule of level d + 1, with the rules of
  the branch's own levels beside them, so the tree continues past them. When the next rows are its children, the rail
  must run to the bottom of the cell, into their rule.
- The commit subjects must start at the branch name.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should continue the commit rail of a parent into the rule of its children`)
- An issue row without a branch of its own heads its branches with the issue key and the count of its branches. Its
  icon box has the size of the folder button, so the rule of its branches sits under the icon.

### Commits cell

- The commits cell must show "1 commit" or "N commits", muted.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display correct branch row structure`)

### Status cell

- The Status cell must be a grid of three columns: the status text (7rem at least), the pull request chip, then the
  Patronus chip and the Landed chip, in this order, and the muted "on login" of a stacked branch whose parent has work
  for the remote ([Stacked Branches](stacked-branches.spec.md#pull-requests)). A missing chip leaves its column empty,
  and the next chip keeps its own column.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show each pull request state in the status cell`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show each robot state in the status cell`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
- The status text must follow the first that applies: the sync progress ("syncing 3/10…"), an error or a conflict
  ("merge conflict"), "analyzing conflict…", a blocked stacked branch ("blocked by auth"), a branch the fetched
  baseline has all of ("landed", [Landed Commits](../repository/landed-commits.spec.md#before-the-update)), the remote
  status ("not pushed", "up to date", "3 (5)↑ · 2↓"), then the result of the sync ("created", "updated", "unchanged").
- Only an error or a conflict must be a badge (`destructive`; `warning` while the conflict is analysed). Every other
  status is muted text after a dot of its colour; the dot of a remote status has the push colour of the branch
  ([Push colours](../repository/outgoing.spec.md#push-colours)).
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should display correct branch row structure`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
  [@test] ../../src/lib/push-summary.test.ts (`takes the hue from the risk: warning only for a push that drops remote commits`, `is neutral for a safe push of no commit of the user`)
- The status text must show as written: the texts are lower case already, and an issue key or an error text keeps its
  case ("blocked by IJPL-123"). Nothing changes the case in CSS; the branch preview shows the same text.
- Hovering the status text must open its description: the remote status in a sentence and the last push time, the
  reason of a block, the error.
- A chip must be a text-sized button. Hovering it opens its details; a click opens its item (the pull request, the
  robot), or the details when it has none (Landed), and never toggles the row.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should show the details on hover and open the pull request on click`)
  [@test] ../../tests/e2e/patronus/patronus.spec.ts (`should show the details on hover and open the robot on click`)
- The details of every chip must have one anatomy: the state line (the icon and the state in the hue of the chip), the
  title or a sentence on what it means, `dl` rows, the notices, then the actions.
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should name what the fetched baseline already has before the update`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should show the profile, the checks and the problems in the popover`)

### Blocked

- A blocked stacked branch must have one treatment: the muted status "blocked by <parent>" after a `warning` dot, a
  disabled push button, and, expanded, a `warning` alert with the reason and "Show <parent>", which reveals the parent.
  Its commits carry the `warning` Blocked badge.
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should block the children of a parent that doesn't build`)
- A stacked branch whose declared parent has no commits is not blocked but built elsewhere ("Base changed"): the muted
  "on master" in the third column of the Status cell, the reason in its tooltip ("login landed · oauth is built on
  master"), an enabled push button, and, expanded, an alert "Built on master: login landed. The pull request base is
  master." with "Unstack from login" ([Stacked Branches](stacked-branches.spec.md#ui)).
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)

### Actions cell

- The Actions cell must show, in this order: the copy button at its start, then, at its end, the push button,
  "Safe Push…" and "⋯". So the last two line up across the rows, whatever the push label; the column is as wide as its
  widest row and doesn't wrap.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should handle branch with commits state`)
  [@test] ../../tests/e2e/branch-row-layout/column-widths.spec.ts (`gives the free width to the branch name and keeps the other columns at content width`)
- A click in the cell must not toggle the row: on a button, on a disabled one (it passes the click on) and in the
  menu "⋯" opens (its popup is in the row's React tree; a right-click in it doesn't open the row menu either).
- The copy button must always be visible and copy the full branch name, `{prefix}/virtual/{name}`.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should copy via copy button in actions column`, `should handle branch with commits state`)
- The push button must follow [Outgoing Commits](../repository/outgoing.spec.md): its label ("Push (2)", "Create
  Remote (3)", "Force Push (1)"), its colour (the push colour of the branch as the text of a ghost button, neutral when
  up to date or landed) and its description on hover, also while it is disabled, with the remote commits a force push
  drops, the question before a push that drops them, and the progress of a running push. A branch with an error or
  without commits has no push button.
  [@test] ../../src/lib/push-summary.test.ts (`follows the rules of the push button label`)
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should force push a branch the remote has commits for, leased on the remote head the row shows`, `should show the percent on the push button and git's last line on hover while the push runs`)
- On Space (Patronus answered), a branch with a push button must have "Safe Push…": it opens the Safe Push form below
  the row, disabled with the reason on hover when a start is not possible
  ([Patronus](patronus.spec.md#starting-a-safe-push-or-a-dry-run)). The product lands through Safe Push, so it is not
  only behind the right-click.
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push form from the row button and from the "⋯" menu, and say why a start is not possible`)
- "⋯" must open the branch menu under the button: exactly the menu of the right-click, so its actions are found without
  knowing the right-click. It is disabled while the row runs an inline action, as the right-click opens no menu then.
  The outgoing overview keeps only the push button.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should open the branch menu from the more actions button without toggling the row`)
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push form from the row button and from the "⋯" menu, and say why a start is not possible`)

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
- A row with commits must be draggable: it drags the branch, or all selected branches.
- While commits are dragged, every row that takes them must show a dashed outline, and the row under the pointer the
  primary tint and a solid outline; its rows below take the drop too.
  [@test] ../../tests/e2e/move-commits/move-commits.spec.ts (`should drag the selection onto a branch, but not onto its own branch`)
  [@test] ../../tests/e2e/move-commits/combine-branches.spec.ts (`should drag an issue branch onto a branch: the issue joins it without a rewrite, and Undo moves it back`)
- A reveal (a branch clicked in the outgoing overview, "Show auth", moved commits) must expand the row and scroll it
  into view.
  [@test] ../../tests/e2e/outgoing/outgoing.spec.ts (`should show and expand the row of a branch clicked in the overview`)

### Branch menu

- The groups must come in this order: copy ("Copy Full Branch Name", "Copy Branch Name", with stacked branches "Copy PR
  Base" and "Copy Retarget Command"), the name ("Rename…" or "Name This Branch…", "Ungroup"), the stack, the commits
  ("Add Issue Reference…", "Amend Changes…", "Unapply…"), the history ("Squash Fixups (N)…", "Open Pull Request") and
  Patronus ("Safe Push…", "Dry Run…", "Restart Safe Push", "Cancel Safe Push", "Open in Patronus").
- An item that opens a form must end with "…"; an item that acts at once must not.
- "Open Pull Request" must show `O`, its key in the branch preview.
  [@test] ../../tests/e2e/pull-requests/pull-requests.spec.ts (`should open the pull request from the branch context menu`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should retarget a pull request whose base is not the expected one before the push`)

### Inline forms

- A form opened from the menu must render in a row below the branch row, one form at a time, with a visible title
  ("Rename auth") and, where it adds something, a muted description.
- A form with an input must focus the input. A confirmation without one (Unapply, Ungroup, Safe Push, Dry Run) must
  focus its button once the button is enabled, so Enter confirms. Escape cancels.
  [@test] ../../tests/e2e/issue-reference.spec.ts (`should focus input when opened`)
  [@test] ../../tests/e2e/stacked-branches/stacked-branches.spec.ts (`should build a child on the baseline once its parent has no commits, and unstack it`)
- While its action runs, the form must be a progress row that says what runs ("Adding ABC-123 to auth…"), and close
  when the action is done. A failure of a form with an input (rename, issue reference, the archived delete) must show
  the form again with the typed text, focused; the error is a toast.
  [@test] ../../tests/e2e/issue-reference.spec.ts (`should add issue reference to branch via context menu`, `should keep the typed reference when adding it fails`)
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
  warning triangle in `warning`. A spinner shows while the integration is detected.
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
- A branch a sync of this session archived must show an `info` "New" badge after its name until the Landed island is
  dismissed.
  [@test] ../../src/stores/archived-branches.test.ts (`marks the branches archived in this session until cleared`)
  [@test] ../../tests/e2e/update-project/landed.spec.ts (`should lead the toast with what landed and keep the island until it is dismissed`)
- The menu of an archived row must offer the copy items and "Delete Archived Branch…" in the `destructive` colour,
  which asks for the branch name; its Delete button is `destructive` too. The Actions cell has the copy button and "⋯"
  with the same menu.
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should delete archived branch via context menu`)
  [@test] ../../tests/e2e/archived-branches/archived-branches.spec.ts (`should open the menu of an archived row from its more actions button`)

## User Experience

### Row

- Every cell has the vertical padding `--bd-cell-y` (6px), so a row is 36px around the 24px folder button, as dense as
  an IntelliJ list. The name cell has `px-6`, a nested one `pl-8 pr-6`; the other cells `px-4`, `w-px` and
  `whitespace-nowrap`. The Status `<th>` has `min-w-[16rem]`.
- The table header is quiet: `text-xs font-medium text-muted-foreground` on the card, with a bottom border and no fill.
- An expanded row fades its commits in (150ms, opacity only).
- The folder button is a ghost `icon-xs` button in `text-muted-foreground`, `text-foreground` while expanded: it
  repeats on every row, so it carries no accent. The name is `text-sm font-medium`, the summary
  `text-xs text-muted-foreground`, the prefix of a subject `font-mono text-muted-foreground/70`.
- The tree rules and the commit rail are neutral `border-l border-border` lines (`bd-nesting-level`,
  `bd-nesting-leaf` and `--bd-nesting-step` in `src/index.css`).
- The commit count is `text-sm text-muted-foreground`.

### Status

- The grid is `grid-cols-[minmax(7rem,auto)_auto_auto] items-center justify-start gap-2`; the third column is a
  `flex items-center gap-2` of the Patronus and the Landed chip, then the `text-xs text-muted-foreground` "on login".
- The status text is `text-sm text-muted-foreground`, at most `max-w-40` and truncated, after a `size-1.5` dot. The
  badge is the subtle badge of its colour.
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

- The copy button is a ghost `icon-xs` button, muted.
- The push button is a ghost `xs` button with the upload icon, its text in the push colour (`ghostButtonColorClass`).
- "Safe Push…" is a ghost `xs` button with the `ShieldCheckIcon` in the foreground colour; disabled, it is dimmed to
  `opacity-50` and keeps its hover (`focusableWhenDisabled`).
- "⋯" is a ghost `icon-xs` button with the `EllipsisIcon`, muted, named "More actions for feature-auth", with the
  tooltip "More actions". Its menu is the shadcn `DropdownMenu` (Base UI Menu), `align="end"`, as wide as its items
  (`w-auto min-w-36`, as the context menu); one renderer, `MenuItems` with the primitives of either menu, draws both.
- The cell is a `flex items-center gap-2` (`RowActions`); the copy button has `mr-auto`.

### Inline forms

- The form row is `bg-muted/50` with `bd-padding-content`. Its content is `flex max-w-3xl flex-col gap-4` (`gap-2` for
  a form with one input), so it doesn't stretch across a wide window.
- The title is `text-sm font-medium`, the description below it `text-sm text-muted-foreground`; the progress row keeps
  the title for the screen reader only.
- An input is `max-w-md`, followed by Cancel and the submit button (`xs`); the submit of a delete is `destructive`.
- A focused row is `bg-muted` with an inset `ring-3 ring-ring/30` (`focus-visible`).

### Archived table

- The status icon is `size-4`, the word `text-sm text-muted-foreground`. "New" is an `info` badge.
- An expanded archived row is `bg-muted`; its merged-by line is `text-sm text-muted-foreground`, behind the rail of the
  commits.

### Dark theme

- The rows use only shadcn semantic tokens and no `dark:` variants.

## Testing / Local Run

- `pnpm test:unit`: `src/stores/branch-table-rows.test.ts`, `src/lib/stack-lanes.test.ts`,
  `src/stores/branch-sync.test.ts`, `src/lib/push-summary.test.ts`, `src/lib/integration-commit.test.ts`,
  `src/stores/archived-branches.test.ts`.
- `pnpm e2e`: `tests/e2e/branch-row-layout`, `tests/e2e/stacked-branches`, `tests/e2e/archived-branches`, the chip
  specs and `tests/e2e/issue-reference.spec.ts` for the inline forms (a failure is a `page.route` of the command with
  status 500). E2E reads the status text by `data-testid="branch-status"`, the tree rules by `nesting-rule`, the rail by
  `commit-rail` and the archived status by `archived-branch-status` (`data-status`: `merged`, `partial`, `not-merged`).

## Open Questions / Risks

- A status text longer than 7rem ("blocked by IJPL-123", "2 (3)↑ · 1↓") moves the chips of its row to the right, and the
  third column follows the width of the pull request chip of its row: the chips line up across rows with short texts
  and the same chips only.
- 16rem fits the status text and one or two chips; a row with all three widens the column once, when they load.
- The inline form row and the amend conflict row of a nested branch draw no tree rules: the tree pauses while a form is
  open.
- The commit subjects start at the name from `sm` up only: below it the commit rows have `px-4`, the name cell `px-6`.
- No E2E test clicks the row itself (only its folder button) or checks the hover tint and the drop outlines' colours.
- A branch without commits has no folder button, so its name starts 2rem further left than the others.
- "Safe Push…" arrives with the first answer of Patronus, after the sync: the Actions column widens once then, and the
  name column gives the room.

## References

- [Commit Row](../commit/commit-row.spec.md): the commits of an expanded row and their rail
- [Outgoing Commits](../repository/outgoing.spec.md): the push labels, the push colours and the push
- [Pull Requests](pull-requests.spec.md), [Patronus](patronus.spec.md), [Landed Commits](../repository/landed-commits.spec.md):
  the chips
- [Stacked Branches](stacked-branches.spec.md): the tree
- [Branch Preview](branch-preview.spec.md), [Move Commits](move-commits.spec.md)
- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
