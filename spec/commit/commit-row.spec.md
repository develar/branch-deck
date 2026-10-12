---
name: Commit Row
description: Requirements for the commit row, its inline message body and the commit hash popover.
style: plain-1
targets:
  - ../../src/components/commit/CommitList.tsx
  - ../../src/components/commit/CommitHeader.tsx
  - ../../src/components/commit/CommitSubject.tsx
  - ../../src/components/commit/CommitMetadata.tsx
  - ../../src/components/commit/CommitMessage.tsx
  - ../../src/components/commit/CommitHashPopover.tsx
  - ../../src/components/commit/LinkedText.tsx
  - ../../src/lib/commit.ts
  - ../../src/lib/commit-message.ts
  - ../../src/lib/inline-code.ts
  - ../../src/components/unassigned/UnassignedCommitListCard.tsx
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/branch/ArchivedBranchExpanded.tsx
  - ../../src/components/conflict/ConflictCommitContext.tsx
  - ../../src/components/conflict/MergeConflictViewer.tsx
  - ../../src/components/conflict/ConflictResolutionHint.tsx
---

# Commit Row

Status: Implemented
Date: 2026-10-12

## Summary

A commit row shows one commit in a branch table, in the unassigned commits card or in a conflict view.
The row has a subject line above a meta line.
An icon button expands the full commit message inline, below the subject.
The commit hash popover shows the same header and body.

## Goals

- Show every commit with one row anatomy in every view.
- Keep a collapsed row to two lines: the subject and the metadata.
- Make a long commit message easy to read inline, without a dialog.
- Show exceptions as badges. Routine states stay quiet.
- Give a hover tint and a selection tint only to rows that react to a click.

## Non-goals

- The row does not render Markdown. It recognizes git conventions and code tokens only.
- The row does not show a diff or a file list.
- This spec does not cover the branch status column or the branch actions.

## Requirements

### Row anatomy

- A commit row must show the subject line above the meta line.
- The meta line must start with the short hash.
- The meta line may then show the author, the time, the file count and extra items, in this order.
- The meta line must separate its items with `•`.
- A synced commit must show `hash → newHash` when the two hashes differ.
- The subject must wrap to two lines at most.
- Issue keys in the subject must become links, as the issue navigation settings define.
- A synced commit must not show a "Created" status text.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should show expanded branch row with commits`)
  [@test] ../../tests/e2e/issue-navigation/issue-navigation.spec.ts (`should render issue links in commit messages`, `should render issue links in unassigned commits`, `should show plain text for commits without issue references`)
- The subject line must show only exception badges: Pending, Merge conflict, Error, Blocked and Amend target.
- A commit with a merge conflict must show the "Merge conflict" badge.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`)
- The Amend target badge must explain its meaning in a tooltip.
- The first commit of a branch must show the Amend target badge while the amend form is open.
  [@test] ../../tests/e2e/amend-changes/amend-changes.spec.ts (`should mark the first commit as the amend target while the form is open`)

### Subject prefix

- A conventional-commit prefix (`feat:`, `feat(model):`, `fix!:`, `docs air:`) must stay in the subject.
- A grouping prefix `(feature-auth)` must show as `feature-auth`, without the parentheses.
- A subject without a prefix must show unchanged.
  [@test] ../../src/lib/commit.test.ts (`splits the conventional-commit prefix off %j`, `returns a grouping prefix without the parentheses`, `keeps %j without a prefix`)
- The row must show the prefix dimmed, before the subject text.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should dim a conventional-commit prefix of the subject`)
- A row must show the stripped subject when the backend supplies one.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`, `should expand/collapse file accordions in missing commits details`)

### Message body

- A row must show the body toggle only when the message has more lines than the subject.
- The body must start at the first line after the subject that is not blank.
- The toggle must be an icon button with `aria-expanded`.
- The toggle label must be "Show full commit message" or "Hide full commit message".
- The row must render the body only while the body is expanded.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should expand the body inline and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/commit.test.ts (`is true for a message with lines after the subject`, `is false for a message that equals the subject`, `is false for a one-line message that differs from a stripped subject`, `is false for an empty message`, `skips the blank lines after the subject`, `returns an empty body for a one-line message`)
- The body must join hard-wrapped lines into one paragraph.
- A blank line must start a new block.
- Lines that start with `- `, `* `, `• `, `1. ` or `1) ` must form a list without these markers.
- A change between bullets and numbers must start a new list.
- An ordered list must keep the number of its first item.
- A continuation line must stay in its list item.
- Lines indented by four spaces or a tab must stay verbatim, without the common indent.
- A fenced block must stay verbatim, blank lines included, without the fences.
- A fenced block without a closing fence must run to the end of the body.
- A last block of git trailers must show as a separate trailer block.
- A block of trailers that is not the last block must stay prose.
- The body must treat CRLF line endings as LF.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should expand the body inline and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/commit-message.test.ts (`joins hard-wrapped lines into one paragraph`, `starts a new block after a blank line`, `turns lines with -, * and • markers into one bullet list without the markers`, `turns lines with 1. markers into an ordered list`, `keeps the number of the first item of an ordered list with 1) markers`, `starts a new list when bullets change to numbers`, `keeps a continuation line in its list item`, `keeps lines indented by four spaces verbatim, without the common indent`, `keeps lines indented by a tab verbatim, without the tab`, `keeps a fenced block verbatim with its blank lines and without the fences`, `runs an unclosed fence to the end of the body`, `turns a last block of git trailers into a trailers block`, `joins an indented trailer continuation line to its trailer`, `accepts a cherry-pick note as a trailer`, `keeps a trailer-shaped block that is not the last block as a paragraph`, `keeps a last block as a paragraph when one of its lines is not a trailer`, `treats CRLF line endings like LF`)
- A paragraph that is only `Label:` must become a section label above the next blocks.
- A paragraph that starts with a short `Label: ` must become a section label above its remaining text.
- A `Label:` paragraph without a following content block must stay a paragraph.
- The trailers must form the last section, without a label.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should expand the body inline and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/commit-message.test.ts (`turns a paragraph that is only Label: into the label of the next blocks`, `turns a short run-in Label: into a label above the rest of the paragraph`, `keeps a run-in label longer than 20 characters as prose`, `keeps a Label: paragraph without a following block as a paragraph`, `keeps a Label: paragraph followed only by trailers as a paragraph`, `puts the trailers into the last, unlabelled section`, `adds unlabelled blocks to the current section`)
- Issue keys in the body must become links.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)

### Code tokens

- Text in backticks must show as code, without the backticks.
- The body must show detected paths, file names, flags, calls, CONSTANT_CASE, snake_case and camelCase as code.
- Names from the stoplist (`JavaScript`, `GitHub`, `macOS`) must stay prose.
- Preformatted blocks and trailers must not detect code.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should expand the body inline and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/inline-code.test.ts (`shows text in backticks as explicit code without the backticks`, `detects a %s as code`, `keeps %s as prose`)
- A PascalCase name must be code only when it has three or more humps.
- A token in double quotes must stay prose.
- Code tokens that only single spaces separate must form one span, such as `bt.cmd --lane`.
- Punctuation around a token must stay prose.
- An issue key inside a code token must stay part of the code.
  [@test] ../../src/lib/inline-code.test.ts (`keeps %s as prose`, `keeps a token in double quotes as prose`, `joins code tokens separated by a single space into one span`, `ends a code span at trailing punctuation`, `keeps the punctuation around a token as prose`, `keeps the punctuation after a call as prose`, `keeps an issue key inside a code token`)
- A long path must break before a `/` or a non-final `.`, never before a short final extension.
  [@test] ../../src/lib/inline-code.test.ts (`breaks a path before each slash`, `does not break before a short final extension`, `breaks before a dot that is not the final extension`, `does not break inside a leading ./`, `does not break inside // or ...`, `keeps a CONSTANT_CASE name whole`)
- The subject must not detect code.

### Selection

- A click on the body toggle must not change the selection.
- A click in the expanded body must not change the selection.
- A click on another part of the row must select the row.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)
- Cmd/Ctrl+click must add the row to the selection.
- The card header must show "N selected" and a "Group into Branch" button while rows are selected.
  [@test] ../../tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should verify commit selection states with ARIA snapshots`)
- Enter must open the inline branch form when rows are selected.
  [@test] ../../tests/e2e/inline-branch-creator/inline-branch-creator.spec.ts (`should open with Enter key when commits are selected`)
- Only the rows of the unassigned commits card must be selectable.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should not select commit rows of a branch`)
- Shift+click must select a range of rows.
- Cmd/Ctrl+A must select all rows, and Escape must clear the selection.
- The context menu of a selected row must act on all selected commits.
- The context menu of a row that is not selected must act on that commit only.
- The context menu must not change the selection.
  [@test] ../../tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should select a range of rows with Shift+click`, `should select all rows with Cmd/Ctrl+A and clear the selection with Escape`, `should act on all selected commits from the context menu of a selected row`, `should act on that commit only from the context menu of a row that is not selected`)
- The card header must hide the selection actions while the inline branch form is open.

### Commit hash popover

- A hover on a short hash must open a popover with the commit header.
- The popover header must show the subject and the same short hash.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show the commit header on hover over the common ancestor hash`)
- The popover must show the message body below the header, as the commit row does.
- The popover must limit its height and scroll its content.
- Without a subject, the popover must show the first message line or "No message available".

### Conflict view

- A row with a merge conflict must show the conflict viewer below its header.
- The viewer must start with its title, the missing commits insight and a collapsed "How to resolve" hint.
- The conflicting files must follow, and then the missing commits details.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`, `should expand/collapse file accordions in missing commits details`)
- A row with a generic error must show the error text in a destructive alert.
- The conflict commit context must use the commit header, with the branch name as an extra meta item.
- The conflict pages in a separate window must also show the "How to resolve" hint.

## User Experience

### Collapsed row

- The subject uses `text-sm font-medium text-foreground`.
- The prefix uses `font-mono text-xs text-muted-foreground`.
- The meta line uses `text-xs text-muted-foreground`. The hashes use the monospace font.
- The body toggle follows the subject as a ghost `icon-xs` button. It keeps the 20px line height of the subject.
- The toggle is muted. It becomes `text-foreground` on hover and while the body is expanded.
- Issue links are muted and become `text-foreground` on hover.
- Badges follow the toggle: `secondary` Pending with a spinner, `destructive` conflict or error, `warning` Blocked, `info` Amend target.

### Expanded body

- The body has no panel. It stays on the row surface and shares the left edge of the subject.
- The body is at most `90ch` wide.
- The body text is 13/20px `text-foreground/85` with the `ss04` font feature.
- A section label is a small `font-medium text-foreground/70` heading above its blocks.
- Sections have a 16px gap. Blocks in a section have an 8px gap.
- Lists have a 20px indent and muted markers.
- Preformatted text uses `font-mono text-xs` on `bg-muted` and scrolls horizontally.
- Trailers use `text-xs text-muted-foreground` below a `border-border` rule.
- Detected code uses the monospace font at `0.875em` without a background.
- Code in backticks also gets a `bg-muted` chip.
- The body shows the text cursor, and its text is selectable.

### Hover and selection

- A selectable row shows the pointer cursor and `bg-muted` on hover.
- A selected row uses `bg-primary/10`, and `bg-primary/15` on hover.
- A selected row has no accent bar.
- A row that is not selectable has no hover tint and keeps the default cursor.
- The selection actions show `N selected` in muted text, then an outline "Group into Branch" button in the primary colour.

### Branch table

- An expanded branch shows its commits behind a neutral `border-l border-border` rail.
- The rail sits under the folder icon (`ml-8`). The subjects start at the left edge of the branch name.
- An archived branch uses the same rail.

### Commit hash popover

- The popover opens on hover over the monospace short hash.
- The popover is at most 24rem high and 42rem wide, and at least 300px wide.

### Dark theme

- The row uses only shadcn semantic tokens and no `dark:` variants.
- The body colour and the row tints follow the theme tokens in both themes.

## Testing / Local Run

- `pnpm test:unit` runs the unit tests of the parser, the code detection and the subject prefix in `src/lib`.
- `pnpm e2e` runs the E2E tests. The tests in `tests/e2e/commit-message` cover the row body.

## Open Questions / Risks

- No test covers the Pending, Blocked and generic Error badges. No backend path in the mock server emits them.
- No test covers the body, the height limit or a commit without a subject in the commit hash popover.
- No test checks that the subject does not detect code.
- No test checks that the card header hides the selection actions while the inline branch form is open.
- No test covers the generic error alert or the conflict pages in a separate window.
- Code detection is heuristic. A false positive sets a prose word in the monospace font. Add such words to the stoplist.
- Reflow can join lines that the author aligned by hand. An indent of four spaces keeps such lines verbatim.

## References

- [UI Theme: Islands Layering and the Radius Scale](../../docs/design/ui-theme.md)
- [Virtual Branches](../../docs/design/virtual-branches.md)
- GitHub, GitLab and Gerrit show a commit body as monospace `pre-wrap` text.
- Branch Deck reflows the body into proportional prose, lists and code blocks.
- A layout like the man page `.TP` macro put the section labels in a side gutter. That layout was tried and dropped.
- The section labels now stack above their blocks.
