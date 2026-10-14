---
name: Commit Row
description: Requirements for the commit row, the commit preview with the message body and its editing, and the commit hash popover.
style: plain-1
targets:
  - ../../src/components/commit/CommitList.tsx
  - ../../src/components/commit/CommitHeader.tsx
  - ../../src/components/commit/CommitSubject.tsx
  - ../../src/components/commit/CommitMetadata.tsx
  - ../../src/components/commit/CommitMessage.tsx
  - ../../src/components/commit/CommitPreviewDialog.tsx
  - ../../src/components/shared/PreviewDialogFrame.tsx
  - ../../src/hooks/useSpacePreviewHotkey.ts
  - ../../src/lib/preview-keys.ts
  - ../../src/hooks/useRewordCommit.ts
  - ../../crates/sync-core/src/reword_commit.rs
  - ../../crates/git-ops/src/reword_commits.rs
  - ../../src/components/commit/CommitHashPopover.tsx
  - ../../src/components/commit/LinkedText.tsx
  - ../../src/lib/commit.ts
  - ../../src/lib/commit-message.ts
  - ../../src/lib/inline-code.ts
  - ../../src/components/unassigned/UnassignedCommitListCard.tsx
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/commit/MovableCommitList.tsx
  - ../../src/components/branch/ArchivedBranchExpanded.tsx
  - ../../src/components/conflict/ConflictCommitContext.tsx
  - ../../src/components/conflict/MergeConflictViewer.tsx
  - ../../src/components/conflict/ConflictResolutionHint.tsx
  - ../../src/components/conflict/DivergenceInsight.tsx
  - ../../src/components/conflict/ConflictingFilesSection.tsx
  - ../../src/components/conflict/GitDiffMergeView.tsx
  - ../../src/components/conflict/ConflictMarkerExtension.tsx
  - ../../src/components/conflict/ConflictWindowPage.tsx
  - ../../src/components/shared/NestedPanel.tsx
  - ../../src/lib/merge-conflict.ts
---

# Commit Row

Status: Implemented
Date: 2026-10-13

## Summary

A commit row shows one commit in a branch table, in the unassigned commits card or in a conflict view.
The row has a subject line above a meta line.
Space or an icon button opens the full commit message in a centered preview, as Quick Look does.
The preview also edits the message of a commit on the current branch.
The commit hash popover shows the same header and body.

## Goals

- Show every commit with one row anatomy in every view.
- Keep a collapsed row to two lines: the subject and the metadata.
- Make a long commit message easy to read in a focused preview, from the keyboard or the mouse.
- Keep the list layout still: reading a message must not push the other rows down.
- Fix a commit message without leaving the app and without a rebase in a terminal.
- Show exceptions as badges. Routine states stay quiet.
- Give a hover tint and a selection tint only to rows that react to a click.

## Non-goals

- The row does not render Markdown. It recognizes git conventions and code tokens only.
- The row does not show a diff or a file list. Its meta line counts the files, and the preview lists them:
  see [Commit Files](commit-files.spec.md).
- This spec does not cover the branch status column or the branch actions: see [Branch Row](../branch/branch-row.spec.md).

## Requirements

### Row anatomy

- A commit row must show the subject line above the meta line.
- The meta line must start with the short hash.
- The meta line may then show the author, the time, the file count with the line counts
  ([Commit Files](commit-files.spec.md)) and extra items, in this order.
- The meta line must separate its items with `·`.
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

### Commit preview

- A row must show the message button only when the message has more lines than the subject.
- The message button must be an icon button with `aria-haspopup="dialog"`, the label "Show full commit message" and the
  same text as its tooltip.
- The message button must open the preview of its commit.
- The row must not render the body.
- The preview must show the whole subject, the meta line and the body, then the changed files
  ([Commit Files](commit-files.spec.md)).
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
- Space must open the preview of the commit row under the pointer.
- ↑ and ↓ must show the previous and the next commit of the same list, and the preview must show the position as `2 / 3`.
- The preview of a commit without a body must show its subject and "No description".
- Space and Escape must close the preview.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the hovered commit with Space and step through the list with the arrows`)
- Without a row under the pointer, Space must open the preview of the last clicked row of the focused list.
- The preview must not change the selection.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the last clicked unassigned row with Space and keep the selection`)
- Space must keep its own meaning in a text field, a menu and another dialog.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should keep Space in a text field`)
- A focused button must keep Space unless the pointer moved after the button got the focus.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should leave Space to a button focused from the keyboard`, `should open the preview of the hovered commit with Space and step through the list with the arrows`)
- Space over a row of a list nested in a conflict view must open one preview, for the nested row.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should open one preview for a missing commit nested in a conflict row`)
- The preview must keep showing its commit after a sync.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should keep the preview on its commit across a sync`)
- The preview must close when a sync removes its commit.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should close the preview when a sync removes its commit`)

### Message editing

- The preview of a commit in a branch table or in the unassigned commits card must offer the edit: E or the Edit button.
- The preview of a commit that is not on the current branch (an archived or a missing commit) must not offer it.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should open one preview for a missing commit nested in a conflict row`)
- The edit must show the raw message in a text field, the grouping prefix included, and focus the field.
- Cmd/Ctrl+Enter or Save must save the message. The preview must then show the reworded commit at the same position.
- After the save, the branch tables must show the new message.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should save the edited message and stay on the reworded commit`)
- Escape must cancel the edit and keep the preview open. A click outside must keep the edit.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should cancel the edit with Escape and keep the preview open`)
- A changed grouping must show a warning that names the new branch, or the unassigned commits. The grouping follows
  the backend rule: an explicit prefix, else the issue key of the subject. With
  [stacked branches](../branch/stacked-branches.spec.md) on, it also names the parent and warns about a restack and a
  stack problem.
- A save that moves the commit to another list must close the preview.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should warn that a changed grouping prefix moves the commit, and move it`)
  [@test] ../../src/lib/commit.test.ts (`returns the explicit prefix of the subject`, `skips the blank lines before the subject`, `groups %j into the prefix branch %j`, `groups %j into the issue branch %j`, `returns null for %j`)
- The edit and the save must be disabled while a sync runs.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should disable the edit while a sync runs`)
- A failed save must keep the edit open with the draft and show the error in a toast.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should keep the edit open with an error when the commit changed meanwhile`)
- E must work in any keyboard layout (the physical key).
- The save hint must name the platform shortcut: `⌘↵` on macOS, `Ctrl+Enter` elsewhere.
- The save must replace only the message: the commits after it are recreated with the same trees, and the working
  tree and the index stay as they are.
- The save must keep the author and the committer of every recreated commit: name, email, date and timezone.
  [@test] ../../crates/sync-core/src/reword_commit_test.rs (`reword_preserves_committer`)
- The message must be cleaned up as `git commit` does: LF line endings, no trailing whitespace, no blank lines around.
- An unchanged message must keep the commit.
- A message without a subject line must be refused.
- The save must be refused while a rebase, merge, cherry-pick or revert is in progress.
- The save must be refused if the commit left the current branch meanwhile (reworded or dropped in a terminal).
- The save must fail, and keep the new commit unreferenced, if the branch moved meanwhile (a commit made in a terminal).
  [@test] ../../crates/sync-core/src/reword_commit_test.rs (`rewords_a_middle_commit_and_keeps_the_commits_after_it`, `cleans_up_the_message_like_git_commit`, `keeps_the_commit_when_the_message_does_not_change`, `refuses_a_message_without_a_subject`, `refuses_to_reword_while_a_merge_is_in_progress`, `refuses_a_commit_that_left_the_current_branch`, `refuses_to_move_a_branch_that_moved_meanwhile`, `reword_refuses_an_empty_subject_in_any_rewrite`)

### Message body

- The body must start at the first line after the subject that is not blank.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
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
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/commit-message.test.ts (`joins hard-wrapped lines into one paragraph`, `starts a new block after a blank line`, `turns lines with -, * and • markers into one bullet list without the markers`, `turns lines with 1. markers into an ordered list`, `keeps the number of the first item of an ordered list with 1) markers`, `starts a new list when bullets change to numbers`, `keeps a continuation line in its list item`, `keeps lines indented by four spaces verbatim, without the common indent`, `keeps lines indented by a tab verbatim, without the tab`, `keeps a fenced block verbatim with its blank lines and without the fences`, `runs an unclosed fence to the end of the body`, `turns a last block of git trailers into a trailers block`, `joins an indented trailer continuation line to its trailer`, `accepts a cherry-pick note as a trailer`, `keeps a trailer-shaped block that is not the last block as a paragraph`, `keeps a last block as a paragraph when one of its lines is not a trailer`, `treats CRLF line endings like LF`)
- A paragraph that is only `Label:` must become a section label above the next blocks.
- A paragraph that starts with a short `Label: ` must become a section label above its remaining text.
- A `Label:` paragraph without a following content block must stay a paragraph.
- The trailers must form the last section, without a label.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/commit-message.test.ts (`turns a paragraph that is only Label: into the label of the next blocks`, `turns a short run-in Label: into a label above the rest of the paragraph`, `keeps a run-in label longer than 20 characters as prose`, `keeps a Label: paragraph without a following block as a paragraph`, `keeps a Label: paragraph followed only by trailers as a paragraph`, `puts the trailers into the last, unlabelled section`, `adds unlabelled blocks to the current section`)
- Issue keys in the body must become links.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)

### Code tokens

- Text in backticks must show as code, without the backticks.
- The body must show detected paths, file names, flags, calls, CONSTANT_CASE, snake_case and camelCase as code.
- Names from the stoplist (`JavaScript`, `GitHub`, `macOS`) must stay prose.
- Preformatted blocks and trailers must not detect code.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
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

- A click on the message button must not change the selection.
- A click in the preview body must not change the selection.
- A click on another part of the row must select the row.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)
- Cmd/Ctrl+click must add the row to the selection.
- The card header must show "N selected" and a "Move to…" button while rows are selected.
  [@test] ../../tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should verify commit selection states with ARIA snapshots`)
- Enter must open the inline "Move to…" form when rows are selected.
  [@test] ../../tests/e2e/inline-branch-creator/inline-branch-creator.spec.ts (`should open with Enter key when commits are selected`)
- The rows of the unassigned commits card and of an expanded branch must be selectable, one list at a time
  (see [Move Commits](../branch/move-commits.spec.md)).
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should select commit rows of a branch, one list at a time`)
- Shift+click must select a range of rows.
- Cmd/Ctrl+A must select all rows, and Escape must clear the selection.
- The context menu of a selected row must act on all selected commits.
- The context menu of a row that is not selected must act on that commit only.
- The context menu must not change the selection.
  [@test] ../../tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should select a range of rows with Shift+click`, `should select all rows with Cmd/Ctrl+A and clear the selection with Escape`, `should act on all selected commits from the context menu of a selected row`, `should act on that commit only from the context menu of a row that is not selected`)
- The card header must hide the selection actions while the inline "Move to…" form is open.

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
- The badge must say "Merge conflict". The title must name the commit and what it is copied onto: "“<subject>” does
  not apply cleanly onto <base>", where the base is the parent of a stacked branch or the baseline, and "the amended
  commit" for an amend conflict. The subject leaves out the grouping prefix. The sub-windows and the branch preview say
  the same.
  [@test] ../../src/lib/merge-conflict.test.ts (`names the subject without its grouping prefix and the branch it is copied onto`, `says "the baseline" before the first sync names it`)
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`)
- The missing commits insight must be one callout in the row and in the missing-commits window: "N missing commits
  modified the conflicting files", then the common ancestor and how far the source and the target moved from it.
- The conflicting files must follow, and then the missing commits details.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`, `should expand/collapse file accordions in missing commits details`)
- "Conflicts only" must never hide a conflicting file. A file without conflict markers (a modify/delete conflict) must
  show its whole diff under the line "No conflict markers in this file: showing the whole diff", so the list is never
  empty under a non-zero file count.
  [@test] ../../src/lib/merge-conflict.test.ts (`keeps only the hunks with conflict markers`, `keeps the whole diff of a file without conflict markers, so no conflicting file disappears`)
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should list every conflicting file in the Diff View, also one without conflict markers`)
- The 3-way tab must start with a collapsed "How to read this view".
- A row with a generic error must show the error text in a destructive alert.
- The conflict commit context of a sub-window must show the title over the meta line of the commit, with the branch
  name as an extra meta item. An unknown branch must be left out, never shown as "Unknown".
- The conflict pages in a separate window must also show the "How to resolve" hint. Their window titles must be
  "Conflicting files · <branch>" and "Missing commits · <branch>".

## User Experience

### Collapsed row

- The subject uses `text-sm font-medium text-foreground`.
- The prefix uses `font-mono text-xs text-muted-foreground`.
- The meta line uses `text-xs text-muted-foreground`. The hashes use the monospace font.
- The message button follows the subject as a ghost `icon-xs` button. It keeps the 20px line height of the subject.
- The message button is muted. It becomes `text-foreground` on hover.
- Issue links are muted and become `text-foreground` on hover.
- Badges follow the message button: `secondary` Pending with a spinner, `destructive` conflict or error, `warning` Blocked, `primary` Amend target (the target of the user's amend).

### Commit preview

- The preview is a centered dialog on the blurred backdrop, at most 56rem wide and `min(80vh, 48rem)` high.
- The header shows the whole subject, without the two-line clamp, above the meta line. A `border-border` rule separates it.
- Only the body scrolls. The body has the focus, so PageUp and PageDown scroll it. ↑ and ↓ belong to the navigation.
- A newly shown commit starts at the top of its body.
- The footer shows the position on the left and the `Kbd` hints `↑ ↓ Previous/next` and `Space Close` on the right,
  in `text-xs text-muted-foreground` below a `border-border` rule. A list of one commit has no `↑ ↓` hint.
- The footer then shows a ghost `xs` button `F Files`, which shows or hides the changed files.
- An editable preview ends the footer with a ghost `xs` button `E Edit`.
- While editing, the body is a monospace 13/20px text field at least 12rem high that grows with the message.
  The footer shows "The first line is the subject", or the prefix warning in `text-warning`, on the left, and
  `Esc Cancel` and a primary `xs` button `Save ⌘↵` (`Save Ctrl+Enter` outside macOS) on the right. The button shows a spinner while saving.
- The selection tint of a row never grows with the message: the row stays two lines.

### Message body

- The body is at most `90ch` wide.
- The body text is 14/24px `text-foreground/85` with the `ss04` font feature in the preview, and 13/20px in the hash popover.
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
- The selection actions show `N selected` in muted text, then an outline "Move to…" button in the primary colour.

### Conflict view

- In the expanded row the viewer is three `NestedPanel`s (`rounded-lg border border-border bg-card`, no ring and no
  shadow), 16px apart: the title with the insight and the hint, the conflicting files, the missing commits. A sub-window
  shows the same sections as `Card` islands in a `Container`, 24px apart.
- The title is `text-sm font-medium` after a `destructive` `TriangleAlertIcon`.
- The insight is a `warning` `Callout` with `FileDiffIcon`; the common ancestor is a hash with its popover.
- The file without conflict markers states it in a muted `text-xs` line above its diff.
- The 3-way panels are `min(600px, 60vh)` high; a divider turns `bg-border` on hover.
- The conflict marker annotations are `text-xs`: the current side (HEAD, what the base already has) in `info`, the
  separator in `warning`, the incoming side in `success`, the common ancestor muted.

### Branch table

- An expanded branch shows its commits behind a neutral `border-l border-border` rail.
- The rail sits under the left wall of the folder icon: 2rem from the cell edge, one `--bd-nesting-step` further per
  nesting level, where it continues into the rule of the branch's children
  ([Branch Row](../branch/branch-row.spec.md)). The subjects start at the left edge of the branch name.
- An archived branch uses the same rail.

### Commit hash popover

- The popover opens on hover over the monospace short hash.
- The popover is at most 24rem high and 42rem wide, and at least 300px wide.

### Dark theme

- The row uses only shadcn semantic tokens and no `dark:` variants.
- The body colour and the row tints follow the theme tokens in both themes.

## Testing / Local Run

- `pnpm test:unit` runs the unit tests of the parser, the code detection and the subject prefix in `src/lib`.
- `pnpm e2e` runs the E2E tests. The tests in `tests/e2e/commit-message` cover the commit preview and its body.

## Open Questions / Risks

- No test covers the Pending, Blocked and generic Error badges. No backend path in the mock server emits them.
- No test covers the body, the height limit or a commit without a subject in the commit hash popover.
- No test checks that the subject does not detect code.
- Base UI moves the focus into the preview a frame after it opens. Until then, the list ignores its keys, so a fast
  ↓ right after Space is lost.
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
- The body was first expanded inline below the subject. It pushed the other rows down, and the selection tint of a
  selected row covered the whole body. The preview replaced it.
- The preview follows macOS Quick Look (Space opens and closes it, the arrows step through the items) and the peek
  of Linear (Space acts on the hovered item).
