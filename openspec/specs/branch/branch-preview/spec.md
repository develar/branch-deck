---
name: Branch Preview
description: Requirements for the branch preview that Space opens on a branch row, with the commits of the branch and their whole messages, the branch navigation, the push and the Safe Push.
style: plain-1
targets:
  - src/components/branch/BranchPreviewDialog.tsx
  - src/stores/branch-preview.ts
  - src/components/branch/BranchTableCard.tsx
  - src/components/branch/BranchRowElement.tsx
  - src/components/branch/BranchRow.tsx
  - src/components/branch/BranchNameCell.tsx
  - src/components/shared/PreviewDialogFrame.tsx
  - src/hooks/usePreviewHotkeys.ts
  - src/lib/preview-keys.ts
  - src/presenters/push-button.ts
  - src/hooks/useBranchPushes.ts
  - src/lib/push-run.ts
  - src/components/branch/BranchPushButton.tsx
  - src/stores/push-confirm.ts
  - src/components/branch/SafePushButton.tsx
  - src/hooks/useSafePush.ts
---

# Branch Preview

Status: Implemented
Date: 2026-10-14

## Purpose

Space on a branch row opens a centered preview of the branch, as it opens the preview of a commit row
(see [Commit Row](../../commit/commit-row/spec.md)).
The preview shows the branch name and its status, then the commits of the branch newest first with their whole
messages, so the branch reads like the description of a pull request.
The arrows step through the branches of the table, P pushes the shown branch and S opens its Safe Push popover at
its row.

## Goals

- Read what a branch contains without expanding its row and opening every commit.
- Review the branches one after another from the keyboard before pushing them.
- Behave as the commit preview does: the same keys, the same frame, the same focus rules.

## Non-goals

- The preview does not show a diff or a file list. The meta line of each commit counts its files and lines, coloured
  as in the commit preview: the commit is a heading there (`CommitCard` with its `heading` header, [Commit Row](../../commit/commit-row/spec.md#commit-card)) ([Commit Files](../../commit/commit-files/spec.md)); D and the footer's "Review Changes" open the review
  window of the branch and close the preview once it opened ([Branch Review](../branch-review/spec.md)).
- The preview does not edit the commit messages; the commit preview does.
- The preview does not resolve a merge conflict; the expanded row shows the conflict viewer.

## Requirements

### Requirement: Opening and closing: Space opens the preview of the branch row at the pointer

Space MUST open the preview of the branch row under the pointer, or of the branch row the arrows focused while the
pointer hasn't moved since ([Branch Row](../branch-row/spec.md#keyboard)).

#### Scenario: Space over a branch row
- **WHEN** the pointer is over a branch row and the user presses Space
- **THEN** the preview of that branch opens
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the hovered branch with Space and step through the branches with the arrows`)

#### Scenario: Space on the row the arrows focused
- **GIVEN** the arrows focused a branch row and the pointer hasn't moved since
- **WHEN** the user presses Space
- **THEN** the preview of the focused branch row opens
  [@test] tests/e2e/branch-filter/branch-filter.spec.ts (`should walk the branch rows with the arrows and preview the focused one with Space`)

### Requirement: Opening and closing: Space does not toggle the row

Space MUST NOT expand or collapse the row.

#### Scenario: The row stays as it was
- **WHEN** Space opens the preview of a collapsed branch row
- **THEN** the row stays collapsed
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the hovered branch with Space and step through the branches with the arrows`)

### Requirement: Opening and closing: The arrows step through the branches of the table

↑ and ↓ MUST show the previous and the next branch in the order of the table, and the preview MUST show the position
as `2 / 7`. Issue rows are not branches and MUST be skipped; under a filter, the arrows MUST walk the rows shown
([Branch Filter](../branch-filter/spec.md)), the matches in a collapsed stack too, as a filter wins over the collapse,
and without one, MUST NOT walk the rows a collapsed stack hides.

#### Scenario: ↓ and ↑ show the next and the previous branch with the position
- **GIVEN** the preview is open
- **WHEN** the user presses ↓, then ↑
- **THEN** the preview shows the next branch in the order of the table, then the previous one
- **AND** it shows the position as `2 / 7`
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the hovered branch with Space and step through the branches with the arrows`)

#### Scenario: Issue rows are skipped
- **WHEN** the next row of the table is an issue row
- **THEN** ↓ skips it, as it is not a branch

#### Scenario: Under a filter
- **GIVEN** a filter of the table is set, and a branch it matches is in a collapsed stack
- **WHEN** the user steps with ↑ and ↓
- **THEN** the preview walks the rows the filter shows, the matched branch of the collapsed stack among them

#### Scenario: Without a filter, a collapsed stack
- **GIVEN** no filter is set and a collapsed stack hides some rows
- **WHEN** the user steps with ↑ and ↓
- **THEN** the preview does not walk the rows the collapsed stack hides

### Requirement: Opening and closing: A collapse that hides the shown branch closes the preview

The preview of a branch a collapse hides MUST close.

#### Scenario: The stack of the shown branch collapses
- **WHEN** a collapse hides the branch the preview shows
- **THEN** the preview closes

### Requirement: Opening and closing: Space and Escape close the preview

Space and Escape MUST close the preview.

#### Scenario: Space closes it
- **GIVEN** the preview is open
- **WHEN** the user presses Space
- **THEN** the preview closes
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the hovered branch with Space and step through the branches with the arrows`)

#### Scenario: Escape closes it
- **GIVEN** the preview is open
- **WHEN** the user presses Escape
- **THEN** the preview closes
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the branch header and its commits newest first with their messages`)

### Requirement: Opening and closing: A close focuses the row of the branch shown last

A close (Escape, Space, D that opens the review window) MUST focus the row of the branch the preview showed last, also
after ↑/↓, so D, S and P act on that branch, as Quick Look leaves the selection where the arrows went; the row itself,
not its expand toggle. Without the row (a filter hides it), the focus MUST go back where it was.

#### Scenario: Escape after ↓, and D after ↑
- **GIVEN** the preview opened on one branch and ↑/↓ showed another
- **WHEN** the user closes it with Escape, or with D that opens the review window
- **THEN** the row of the branch it showed last has the focus, the row itself, not its expand toggle
- **AND** D, S and P act on that branch, though the pointer is over another row
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should give the focus to the row of the branch it showed last, so D and Space go on from there`)

#### Scenario: A filter hides the row
- **GIVEN** a filter hides the row of the branch the preview showed last
- **WHEN** the preview closes
- **THEN** the focus goes back where it was

### Requirement: Opening and closing: Space opens the last toggled branch while the table has the focus

Without a row under the pointer, Space MUST open the preview of the last toggled branch while the focus is in the
branch table.

#### Scenario: The pointer away from every row
- **GIVEN** the user toggled a branch row and the focus is in the branch table
- **WHEN** the pointer is away from every row and the user presses Space
- **THEN** the preview of the last toggled branch opens
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the last toggled branch with Space while the table has the focus`)

### Requirement: Opening and closing: Space over a commit row opens the commit preview only

Space over a commit row of an expanded branch MUST open the commit preview only.

#### Scenario: Space over a commit row of an expanded branch
- **GIVEN** a branch is expanded
- **WHEN** the pointer is over one of its commit rows and the user presses Space
- **THEN** the commit preview opens and the branch preview does not
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should leave Space to a commit list nested in the branch table`)

### Requirement: Opening and closing: Space belongs to a focused nested commit list

Space MUST belong to the commit list nested in the branch table while that list has the focus.

#### Scenario: The focus in the nested commit list
- **GIVEN** the commit list nested in the branch table has the focus
- **WHEN** the pointer is away from every row and the user presses Space
- **THEN** the commit list handles Space and the branch preview does not open
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should leave Space to a commit list nested in the branch table`)

### Requirement: Opening and closing: Space keeps its own meaning in fields, menus and dialogs

Space MUST keep its own meaning in a text field, a menu, another dialog and a button focused from the keyboard, as for
the commit rows.

#### Scenario: Space in a text field, a menu, another dialog or a focused button
- **WHEN** the focus is in a text field, a menu, another dialog or on a button focused from the keyboard
- **AND** the user presses Space
- **THEN** Space does what it does there and the preview does not open

### Requirement: Opening and closing: The preview keeps its branch across a sync

The preview MUST keep showing its branch after a sync.

#### Scenario: A sync while the preview is open
- **GIVEN** the preview shows a branch
- **WHEN** a sync runs
- **THEN** the preview still shows that branch at its position
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should keep the preview on its branch across a sync`)

### Requirement: Opening and closing: A sync that removes the branch closes the preview

The preview MUST close when a sync removes its branch, also when the branch is renamed.

#### Scenario: A sync removes or renames the branch
- **GIVEN** the preview shows a branch
- **WHEN** a sync removes that branch, also by renaming it
- **THEN** the preview closes
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should close the preview when a sync removes its branch`)

### Requirement: Opening and closing: The preview belongs to its repository

The preview belongs to its repository: another one selected MUST close it, and the first one selected again MUST NOT
show it again.

#### Scenario: Another repository is selected, then the first one again
- **GIVEN** the preview of a branch is open
- **WHEN** another repository is selected
- **THEN** the preview closes
- **AND** when the first repository is selected again, the preview does not show again
  [@test] src/stores/branch-preview.test.ts (`closes when another repository is selected, and doesn't come back with the first one`)

### Requirement: Content: The header shows the name, the issue chip and the summary

The header MUST show the branch name, the issue of a prefix branch as a chip, and the summary of the row (the title
of the branch's description, [Branch Row](../branch-row/spec.md#name-cell)).

#### Scenario: The name and the summary of the row
- **WHEN** the preview of a branch opens
- **THEN** its header shows the branch name and the summary of the row, the title of the branch's description
  [@test] tests/e2e/branch-row-layout/description-title.spec.ts (`shows the description title of a branch as written, in the row and the preview`)
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the branch header and its commits newest first with their messages`)

#### Scenario: The issue of a prefix branch
- **WHEN** the preview of a prefix branch opens
- **THEN** its header shows the issue of the branch as a chip
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the issue of a prefix branch`)

### Requirement: Content: The meta line shows the count, the times and the status

The meta line MUST show the commit count, the time of the latest commit, the status of the row and the time of the
last push, if the branch was pushed.

#### Scenario: The meta line of a branch
- **WHEN** the preview of a branch opens
- **THEN** its meta line shows the commit count, the time of the latest commit and the status of the row
- **AND** the time of the last push, if the branch was pushed
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the branch header and its commits newest first with their messages`)

### Requirement: Content: The status shows its full description on hover

The status MUST show its full description on hover, as the status column does.

#### Scenario: Hover over the status
- **WHEN** the pointer is over the status in the meta line
- **THEN** its full description shows, as in the status column

### Requirement: Content: The body lists the commits newest first with their messages

The body MUST list the commits newest first: the whole subject, the short hash and the time, then the body of the
message, reflowed as in the commit preview.

#### Scenario: The commits of a branch
- **WHEN** the preview of a branch with several commits opens
- **THEN** the body lists them newest first, each with its whole subject, its short hash and its time
- **AND** then the body of its message, reflowed as in the commit preview
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the branch header and its commits newest first with their messages`)

### Requirement: Content: A commit without a body shows its subject only

A commit without a body MUST show its subject only.

#### Scenario: A commit whose message has no body
- **WHEN** a commit of the shown branch has no body
- **THEN** the preview shows its subject only
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the branch header and its commits newest first with their messages`)

### Requirement: Content: The pull request and the Safe Push each have a section above the commits

Above the commits, the pull request of the branch and its Safe Push (the details of its robot) MUST each stand in a
section of its own.

#### Scenario: A branch with a pull request and a Safe Push
- **GIVEN** a branch has a pull request and a Safe Push
- **WHEN** its preview opens
- **THEN** the pull request and the details of its robot each show in a section of its own above the commits
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the sections and the push of the preview in their places while Patronus and GitHub answer`)

### Requirement: Content: A section that waits for its answer holds its place

While GitHub hasn't listed the pull requests, or Patronus hasn't answered with its robots, a pushed branch (which may
have them) MUST show the section with one muted line in its place (`PendingSection`), which the answer fills: the
commits MUST NOT be pushed down by a section that comes late.

#### Scenario: GitHub and Patronus haven't answered yet
- **GIVEN** a pushed branch, and GitHub hasn't listed the pull requests and Patronus hasn't answered with its robots
- **WHEN** its preview opens
- **THEN** each section shows one muted line in its place: "Checking GitHub for a pull request of ABC-123…", "Asking Patronus for the Safe Push of ABC-123…"
- **AND** the answer fills the place of its section, and the commits are not pushed down
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the sections and the push of the preview in their places while Patronus and GitHub answer`)

### Requirement: Content: A merge conflict shows a notice with its fix above the commits

A branch with a merge conflict MUST show a notice above the commits that names the commit that does not apply and
where the commits it misses are (the explanation of the conflict view), and below it the fix of
[Conflict Fix](../conflict-fix/spec.md), with Apply Fix.

#### Scenario: A branch with a merge conflict
- **GIVEN** a commit of the branch does not apply
- **WHEN** its preview opens
- **THEN** a notice above the commits names that commit and where the commits it misses are
- **AND** below it the fix, with Apply Fix
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the merge conflict above the commits`)

### Requirement: Content: The conflicting commit shows the merge conflict badge

The conflicting commit MUST show the "Merge conflict" badge.

#### Scenario: The commit that does not apply
- **GIVEN** a branch with a merge conflict
- **WHEN** its preview opens
- **THEN** the conflicting commit shows the "Merge conflict" badge
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the merge conflict above the commits`)

### Requirement: Content: A failed sync shows its error in the notice

A branch whose sync failed MUST show the error in the notice.

#### Scenario: The sync of the branch failed
- **GIVEN** the sync of a branch failed
- **WHEN** its preview opens
- **THEN** the notice shows the error

### Requirement: Push: P or the footer button pushes the shown branch as the row does

P or the push button in the footer MUST push the shown branch, as the push button of the row does, with the same
label ("Push Branch (2)", "Force Push Branch (1)").

#### Scenario: P pushes the shown branch
- **GIVEN** the preview shows a branch with commits to push
- **WHEN** the user presses P
- **THEN** the shown branch is pushed, as the push button of its row would
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should push the branch with P`)

#### Scenario: The label of the row's push button
- **WHEN** the preview shows a branch
- **THEN** the push button in its footer has the label of the row's push button, "Push Branch (2)" or "Force Push Branch (1)"
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should push the branch with P`, `should ask before P force pushes over a commit the branch lacks`)

### Requirement: Push: A first push of the landing keeps the footer's push in place, disabled

A first push whose landing pushes the branch (`isFirstPushInLanding`, [Branch Row](../branch-row/spec.md#actions-cell))
is P and "⋯", beside Create Pull Request… and Safe Push: the footer MUST keep its button in place, disabled with why,
and so MUST it while Patronus or GitHub haven't answered whether they push it (`firstPushWait`): the answers MUST NOT
move a button of the footer.

#### Scenario: The landing pushes the branch
- **GIVEN** an unpushed branch whose Safe Push and Create Pull Request… push it
- **WHEN** its preview opens
- **THEN** the footer keeps its push button in place, disabled
- **AND** its hover says why ("Safe Push and Create Pull Request… push JIRA-456 first: "⋯" pushes it alone")
  [@test] tests/e2e/pull-requests/pull-requests.spec.ts (`should offer to create the pull request of a branch in the push toast, the branch menu and the preview`)
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the sections and the push of the preview in their places while Patronus and GitHub answer`)

#### Scenario: Patronus or GitHub haven't answered yet
- **GIVEN** an unpushed branch, and Patronus or GitHub haven't answered whether they push it
- **WHEN** its preview opens
- **THEN** the footer's push button is disabled, its hover says "Waits for the answer of Patronus"
- **AND** when the answer comes, the button stays where it was
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the sections and the push of the preview in their places while Patronus and GitHub answer`)

### Requirement: Push: A branch a Safe Push holds keeps its disabled push

A branch a Safe Push holds MUST keep its disabled push, whose description MUST offer the robot.

#### Scenario: A Safe Push holds the branch
- **GIVEN** a Safe Push holds the shown branch
- **WHEN** the preview shows it
- **THEN** the footer keeps its push, disabled, and its description offers the robot

### Requirement: Push: P works in any keyboard layout

P MUST work in any keyboard layout (the physical key).

#### Scenario: Another keyboard layout
- **GIVEN** a keyboard layout where the physical key of P types another letter
- **WHEN** the user presses that key in the preview
- **THEN** the preview pushes as on P

### Requirement: Push: P on the focused row pushes by the same rule

P on the focused row of the table MUST push by the same rule (the `push` Rust decides, a running push over it,
[Branch Row](../branch-row/spec.md#keyboard)).

#### Scenario: P on the focused row of the table
- **GIVEN** a branch row of the table has the focus
- **WHEN** the user presses P
- **THEN** the push follows the same rule as in the preview: the `push` Rust decides, a running push over it

### Requirement: Push: The push is disabled while it cannot run

The push MUST be disabled while a sync or a push of the branch runs, while an inline action runs on the row, and when
everything is pushed.

#### Scenario: A sync or a push of the branch runs
- **WHEN** a sync or a push of the shown branch runs
- **THEN** the push of the preview is disabled

#### Scenario: An inline action runs on the row
- **WHEN** an inline action runs on the row of the shown branch
- **THEN** the push of the preview is disabled

#### Scenario: Everything is pushed
- **WHEN** everything of the shown branch is pushed
- **THEN** the push of the preview is disabled
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should push the branch with P`)

### Requirement: Push: The preview stays open and follows the push result

The preview MUST stay open, and its status and last push time MUST follow the push result.

#### Scenario: After a push from the preview
- **WHEN** a push from the preview succeeds
- **THEN** the preview stays open
- **AND** its status and its last push time show the result
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should push the branch with P`)

### Requirement: Push: The footer's push button opens the description of the row's

The push button in the footer is the push button of the row: it MUST open the same description on hover, also while
it is disabled.

#### Scenario: Hover over the footer's push button
- **WHEN** the pointer is over the push button in the footer, enabled or disabled
- **THEN** it opens the description the push button of the row opens
  [@test] tests/e2e/branch-row-layout/late-facts.spec.ts (`should keep the sections and the push of the preview in their places while Patronus and GitHub answer`)

### Requirement: Push: A push that drops remote commits asks first

A push that drops remote commits MUST ask first ([Outgoing Commits](../../repository/outgoing/spec.md#push)): P or a
click MUST open that description with the dropped commits and a Force Push button, which pushes; P again MUST push
nothing.

#### Scenario: P over a commit the branch lacks
- **GIVEN** the remote branch has a commit the shown branch lacks
- **WHEN** the user presses P or clicks the push button
- **THEN** its description opens with the dropped commits and a Force Push button, and nothing is pushed
- **AND** the Force Push button pushes
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should ask before P force pushes over a commit the branch lacks`)

#### Scenario: P again while the description is open
- **GIVEN** the description with the dropped commits is open
- **WHEN** the user presses P again
- **THEN** nothing is pushed

### Requirement: Push: A branch with a conflict or a sync error offers no push

A branch with a conflict or a sync error MUST NOT offer the push, as its row doesn't.

#### Scenario: A branch with a merge conflict
- **WHEN** the preview shows a branch with a merge conflict
- **THEN** it offers no push
  [@test] tests/e2e/branch-preview/branch-preview.spec.ts (`should show the merge conflict above the commits`)

#### Scenario: A branch whose sync failed
- **WHEN** the preview shows a branch whose sync failed
- **THEN** it offers no push

### Requirement: Safe Push: S and the footer button ask the start as the row does

S and "Safe Push" in the footer MUST ask the start of the shown branch as its row does (`requestSafePush`), when that
button is enabled: Patronus answers, the branch has a push, its start is not refused (`landing.start`), no inline
action runs on the row. The preview MUST close first: the start is the row's, its popover opens at the row's slot
(`useClosePreview`).

#### Scenario: S on a branch whose Safe Push can start
- **GIVEN** the "Safe Push" button of the preview is enabled
- **WHEN** the user presses S or clicks the button
- **THEN** the preview closes first
- **AND** the start is asked as the row asks it, its popover at the row's slot
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)

### Requirement: Safe Push: S opens the popover, the button starts when there is nothing to say

S MUST always open the popover; the button MUST start at once when there is nothing to say, else open it
([Patronus](../patronus/spec.md#starting-a-safe-push-or-a-dry-run)).

#### Scenario: S opens the popover
- **GIVEN** the "Safe Push" button of the preview is enabled
- **WHEN** the user presses S
- **THEN** the popover of the start opens at the row
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)

#### Scenario: A click with nothing to say starts at once
- **GIVEN** the start has nothing to say
- **WHEN** the user clicks "Safe Push" in the footer
- **THEN** the Safe Push starts at once, without the popover
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)

#### Scenario: A click with something to say opens the popover
- **GIVEN** the start has something to say
- **WHEN** the user clicks "Safe Push" in the footer
- **THEN** the popover opens

### Requirement: Safe Push: A start that cannot run keeps the preview and says why

Otherwise the button MUST stay, disabled, its hover saying why, and S MUST keep the preview and say it in an `info`
toast, as S on the row does.

#### Scenario: S while the start cannot run
- **GIVEN** the "Safe Push" button of the preview is disabled, as a robot runs on the commit of the branch
- **WHEN** the user presses S
- **THEN** the preview stays open and an `info` toast says why
- **AND** the button stays, disabled, its hover saying why
  [@test] tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push popover from the branch preview with S, and start from its footer button`)

### Requirement: Safe Push: S works in any keyboard layout

S MUST work in any keyboard layout, as on the focused row ([Branch Row](../branch-row/spec.md#keyboard)).

#### Scenario: Another keyboard layout
- **GIVEN** a keyboard layout where the physical key of S types another letter
- **WHEN** the user presses that key in the preview
- **THEN** it acts as S, as on the focused row

## User Experience

- The preview uses the frame of the commit preview (`PreviewDialogFrame`): a centered dialog on the blurred backdrop,
  at most 56rem wide and `min(80vh, 48rem)` high, the header and the footer below and above a `border-border` rule.
- Only the body scrolls, and it has the focus, so PageUp and PageDown scroll it. A newly shown branch starts at the top.
- The header shows the name in `text-base font-medium`, the dialog title, then the issue chip and the summary in `text-xs
  text-muted-foreground`, truncated. The meta line below uses `text-xs text-muted-foreground` and `·` separators.
- The status in the meta line is the muted text of the status column with its colour dot; the dot stands in for the
  separator before it, so the line never reads "· ● not pushed".
- The commits are separated by `border-border` rules. Each subject is `text-md font-medium`, the size of its
  body, and the bodies use the 14/22px reading text of the commit preview.
- The conflict notice is a `destructive` alert on `bg-destructive/10`, without a border.
- The footer shows the position on the left and `↑ ↓ Previous/next`, `Space Close`, a ghost `xs` button `D Review
  Changes` (disabled with the reason for a branch that can't be reviewed), `O Open Pull Request` and "Create Pull
  Request…" (which closes the preview for its dialog; both beside a closed pull request), `S Safe Push` where
  Patronus answers (the key in place of the shield, as the other buttons show theirs), a ghost `xs` button `P Push Branch`,
  the push in the text colour, not the push colour of the row. The header has "⋯" before the close button, the whole
  menu of the row (`getBranchContextMenuItems`, `BranchPreviewMenu`): the expanded form offers what the compact one
  does. Its key hints are only the keys
  of the preview (P, O, D, S); an item that opens a form of the row (Rename…, Move to…, Amend Changes…) closes the
  preview first and shows the row, a copy keeps the preview (`toPreviewMenu`).
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should offer the menu of the row in the branch preview, and close the preview for a form of the row`)
  A table of one branch has no `↑ ↓` hint. While pushing, the button shows a spinner instead of the key and the label of
  the row's push button, "Pushing 45%".
- The preview is loaded on its first use.

## Testing / Local Run

- `pnpm e2e` runs the E2E tests; `tests/e2e/branch-preview` covers the preview.
- The push tests push to the remote of the test repository, a bare `.git/upstream.git`, and assert what it took
  (`remotePushes`, [The remote of a repository](../../../../docs/design/e2e-testing.md#the-remote-of-a-repository)).

## Open Questions / Risks

- No test covers a sync error notice or the disabled push during a sync.
- Space right after a popover closes with Escape belongs to the trigger that got the focus back, until the pointer moves.
- A long branch with many commits renders every message at once; the body is not virtualized.

## References

- [Pull Requests](../pull-requests/spec.md): the pull request section above the commits, the chip in the meta line and `O`.
- [Patronus](../patronus/spec.md): the robot section above the commits, the robot chip in the meta line and "Safe Push" in
  the footer.
- [Commit Row](../../commit/commit-row/spec.md): the commit preview, its keys and the focus rules of Space.
- [UI Theme: Islands Layering and the Radius Scale](../../../../docs/design/ui-theme.md)
- The preview follows macOS Quick Look, as the commit preview does.
