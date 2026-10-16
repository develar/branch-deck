---
name: Branch Preview
description: Requirements for the branch preview that Space opens on a branch row, with the commits of the branch and their whole messages, the branch navigation, the push and the Safe Push form.
style: plain-1
targets:
  - ../../src/components/branch/BranchPreviewDialog.tsx
  - ../../src/components/branch/BranchTableCard.tsx
  - ../../src/components/branch/BranchContextMenuRow.tsx
  - ../../src/components/branch/BranchRow.tsx
  - ../../src/components/branch/BranchNameCell.tsx
  - ../../src/components/shared/PreviewDialogFrame.tsx
  - ../../src/hooks/useSpacePreviewHotkey.ts
  - ../../src/lib/preview-keys.ts
  - ../../src/hooks/usePushButton.ts
  - ../../src/lib/git/push.ts
  - ../../src/components/branch/BranchPushButton.tsx
  - ../../src/stores/push-confirm.ts
  - ../../src/components/branch/SafePushButton.tsx
---

# Branch Preview

Status: Implemented
Date: 2026-10-14

## Summary

Space on a branch row opens a centered preview of the branch, as it opens the preview of a commit row
(see [Commit Row](../commit/commit-row.spec.md)).
The preview shows the branch name and its status, then the commits of the branch newest first with their whole
messages, so the branch reads like the description of a pull request.
The arrows step through the branches of the table, P pushes the shown branch and S opens its Safe Push form.

## Goals

- Read what a branch contains without expanding its row and opening every commit.
- Review the branches one after another from the keyboard before pushing them.
- Behave as the commit preview does: the same keys, the same frame, the same focus rules.

## Non-goals

- The preview does not show a diff or a file list. The meta line of each commit counts its files and lines, as in a
  commit row ([Commit Files](../commit/commit-files.spec.md)); D and the footer's "Review Changes" open the review
  window of the branch ([Branch Review](branch-review.spec.md)).
- The preview does not edit the commit messages; the commit preview does.
- The preview does not resolve a merge conflict; the expanded row shows the conflict viewer.

## Requirements

### Opening and closing

- Space must open the preview of the branch row under the pointer, or of the branch row the arrows focused while the
  pointer hasn't moved since ([Branch Row](branch-row.spec.md#keyboard)).
  [@test] ../../tests/e2e/branch-filter/branch-filter.spec.ts (`should walk the branch rows with the arrows and preview the focused one with Space`)
- Space must not expand or collapse the row.
- ↑ and ↓ must show the previous and the next branch in the order of the table, and the preview must show the
  position as `2 / 7`. Issue rows are not branches and are skipped; under a filter, the rows shown
  ([Branch Filter](branch-filter.spec.md)), and without one, not the rows a collapsed stack hides. The preview of a
  branch a collapse hides closes.
- Space and Escape must close the preview.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the hovered branch with Space and step through the branches with the arrows`)
- Without a row under the pointer, Space must open the preview of the last toggled branch while the focus is in the
  branch table.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should open the preview of the last toggled branch with Space while the table has the focus`)
- Space over a commit row of an expanded branch must open the commit preview only.
- Space must belong to the commit list nested in the branch table while that list has the focus.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should leave Space to a commit list nested in the branch table`)
- Space must keep its own meaning in a text field, a menu, another dialog and a button focused from the keyboard,
  as for the commit rows.
- The preview must keep showing its branch after a sync.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should keep the preview on its branch across a sync`)
- The preview must close when a sync removes its branch, also when the branch is renamed.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should close the preview when a sync removes its branch`)

### Content

- The header must show the branch name, the issue of a prefix branch as a chip, and the summary of the row (the title
  of the branch's description, [Branch Row](branch-row.spec.md#name-cell)).
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should show the issue of a prefix branch`)
  [@test] ../../tests/e2e/branch-row-layout/description-title.spec.ts (`shows the description title of a branch as written, in the row and the preview`)
- The meta line must show the commit count, the time of the latest commit, the status of the row and the time of the
  last push, if the branch was pushed.
- The status must show its full description on hover, as the status column does.
- The body must list the commits newest first: the whole subject, the short hash and the time, then the body of the
  message, reflowed as in the commit preview.
- A commit without a body must show its subject only.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should show the branch header and its commits newest first with their messages`)
- A branch with a merge conflict must show a notice above the commits that names the commit that does not apply.
- The conflicting commit must show the "Merge conflict" badge.
- A branch whose sync failed must show the error in the notice.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should show the merge conflict above the commits`)

### Push

- P or the push button in the footer must push the shown branch, as the push button of the row does, with the same
  label ("Push (2)", "Create Remote (3)", "Force Push (1)").
- P must work in any keyboard layout (the physical key). P on the focused row of the table pushes by the same rule
  (`canPushNow`, [Branch Row](branch-row.spec.md#keyboard)).
- The push must be disabled while a sync or a push of the branch runs, while an inline action runs on the row, and when
  everything is pushed.
- The preview must stay open, and its status and last push time must follow the push result.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should push the branch with P`)
- The push button in the footer is the push button of the row: it opens the same description on hover, also while it
  is disabled. A push that drops remote commits asks first ([Outgoing Commits](../repository/outgoing.spec.md#push)):
  P or a click opens that description with the dropped commits and a Force Push button, which pushes; P again pushes
  nothing.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should ask before P force pushes over a commit the branch lacks`)
- A branch with a conflict or a sync error must not offer the push.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should show the merge conflict above the commits`)

### Safe Push

- S or "Safe Push…" in the footer must open the Safe Push form below the row of the shown branch, and the preview closes
  for it (the form is a form of the row), when that button is enabled: Patronus answers, the branch has a push,
  `canStartRobot`, no inline action runs on the row. Otherwise S does nothing and the button's hover says why. S works
  in any keyboard layout, as on the focused row ([Branch Row](branch-row.spec.md#keyboard)).
  [@test] ../../tests/e2e/patronus/patronus-actions.spec.ts (`should open the Safe Push form from the branch preview with S, where its button is enabled`)

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
  Changes` (not for a branch that can't be reviewed), `O Open Pull Request` (or "Create Pull Request"), `S Safe Push…`
  where Patronus answers (the key in place of the shield, as the other buttons show theirs) and a ghost `xs` button `P
  Push` on the right, the push in the text colour, not the push colour of the row. A table of one branch has no `↑ ↓` hint. While
  pushing, the button shows a spinner instead of the key and the label of the row's push button, "Pushing 45%".
- The preview is loaded on its first use.

## Testing / Local Run

- `pnpm e2e` runs the E2E tests; `tests/e2e/branch-preview` covers the preview.
- The push test answers `push_branch` in the page: the E2E server has no remote to push to.

## Open Questions / Risks

- No test covers a sync error notice or the disabled push during a sync.
- Space right after a popover closes with Escape belongs to the trigger that got the focus back, until the pointer moves.
- A long branch with many commits renders every message at once; the body is not virtualized.

## References

- [Pull Requests](pull-requests.spec.md): the pull request section above the commits, the chip in the meta line and `O`.
- [Patronus](patronus.spec.md): the robot section above the commits, the robot chip in the meta line and "Safe Push…" in
  the footer.
- [Commit Row](../commit/commit-row.spec.md): the commit preview, its keys and the focus rules of Space.
- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
- The preview follows macOS Quick Look, as the commit preview does.
