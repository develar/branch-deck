---
name: Branch Preview
description: Requirements for the branch preview that Space opens on a branch row, with the commits of the branch and their whole messages, the branch navigation and the push.
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
  - ../../src/lib/git/push.ts
---

# Branch Preview

Status: Implemented
Date: 2026-10-14

## Summary

Space on a branch row opens a centered preview of the branch, as it opens the preview of a commit row
(see [Commit Row](../commit/commit-row.spec.md)).
The preview shows the branch name and its status, then the commits of the branch newest first with their whole
messages, so the branch reads like the description of a pull request.
The arrows step through the branches of the table, and P pushes the shown branch.

## Goals

- Read what a branch contains without expanding its row and opening every commit.
- Review the branches one after another from the keyboard before pushing them.
- Behave as the commit preview does: the same keys, the same frame, the same focus rules.

## Non-goals

- The preview does not show a diff or a file list.
- The preview does not edit the commit messages; the commit preview does.
- The preview does not resolve a merge conflict; the expanded row shows the conflict viewer.

## Requirements

### Opening and closing

- Space must open the preview of the branch row under the pointer.
- Space must not expand or collapse the row.
- ↑ and ↓ must show the previous and the next branch in the order of the table, and the preview must show the
  position as `2 / 7`. Issue rows are not branches and are skipped.
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

- The header must show the branch name, the issue of a prefix branch as a chip, and the summary of the row.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should show the issue of a prefix branch`)
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
- P must work in any keyboard layout (the physical key).
- The push must be disabled while a sync or a push of the branch runs, while an inline action runs on the row, and when
  everything is pushed.
- The preview must stay open, and its status and last push time must follow the push result.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should push the branch with P`)
- A branch with a conflict or a sync error must not offer the push.
  [@test] ../../tests/e2e/branch-preview/branch-preview.spec.ts (`should show the merge conflict above the commits`)

## User Experience

- The preview uses the frame of the commit preview (`PreviewDialogFrame`): a centered dialog on the blurred backdrop,
  at most 56rem wide and `min(80vh, 48rem)` high, the header and the footer below and above a `border-border` rule.
- Only the body scrolls, and it has the focus, so PageUp and PageDown scroll it. A newly shown branch starts at the top.
- The header shows the name in `text-sm font-medium`, then the issue chip and the summary in `text-xs
  text-muted-foreground`, truncated. The meta line below uses `text-xs text-muted-foreground` and `•` separators.
- The status in the meta line is the muted text of the status column with its colour dot.
- The commits are separated by `border-border` rules. The bodies use the 14/24px text of the commit preview.
- The conflict notice is a `destructive` alert on `bg-destructive/10`, without a border.
- The footer shows the position on the left and `↑ ↓ Previous/next`, `Space Close` and a ghost `xs` button `P Push` on
  the right. A table of one branch has no `↑ ↓` hint. The button shows a spinner instead of the key while pushing.
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
- [Patronus](patronus.spec.md): the robot section above the commits and the robot chip in the meta line.
- [Commit Row](../commit/commit-row.spec.md): the commit preview, its keys and the focus rules of Space.
- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
- The preview follows macOS Quick Look, as the commit preview does.
