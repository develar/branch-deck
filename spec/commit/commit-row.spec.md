---
name: Commit Row
description: Requirements for the commit row, the commit preview with the message body and its editing, and the commit hash popover.
style: plain-1
targets:
  - ../../crates/file-diffs/src/merge.rs
  - ../../crates/text-diff/src/merge_model.rs
  - ../../src/components/conflict/CollapsibleFileHeader.tsx
  - ../../src/components/files/FileHeaderParts.tsx
  - ../../src/components/commit/CommitList.tsx
  - ../../src/components/commit/CommitHeader.tsx
  - ../../src/components/commit/CommitSubject.tsx
  - ../../src/components/commit/CommitMetadata.tsx
  - ../../src/components/commit/CommitMessage.tsx
  - ../../src/components/commit/CommitPreviewDialog.tsx
  - ../../src/components/shared/PreviewDialogFrame.tsx
  - ../../src/hooks/usePreviewHotkeys.ts
  - ../../src/lib/preview-keys.ts
  - ../../src/hooks/useRewordCommit.ts
  - ../../src/hooks/useStackActions.ts
  - ../../src/components/commit/PrefixChangeHint.tsx
  - ../../src/lib/stack.ts
  - ../../src/lib/rewrite-run.ts
  - ../../crates/operations/src/rewrite.rs
  - ../../crates/history-rewrite/src/reword.rs
  - ../../crates/history-rewrite/src/fixup_targets.rs
  - ../../crates/history-rewrite/src/repository_lock.rs
  - ../../crates/git-ops/src/reword_commits.rs
  - ../../src/components/commit/LinkedText.tsx
  - ../../src/lib/commit.ts
  - ../../src/lib/commit-message.ts
  - ../../src/hooks/useCommitBodyBlocks.ts
  - ../../src/hooks/usePreviewGroup.ts
  - ../../src/hooks/useDebouncedRead.ts
  - ../../src-tauri/src/commands/grouping.rs
  - ../../crates/sync-utils/src/commit_body.rs
  - ../../crates/sync-utils/src/commit_subject.rs
  - ../../crates/sync-utils/src/issue_pattern.rs
  - ../../crates/domain-types/src/commit_body.rs
  - ../../crates/history-rewrite/src/commit_grouper.rs
  - ../../src/lib/inline-code.ts
  - ../../src/components/unassigned/UnassignedCommitListCard.tsx
  - ../../src/components/branch/ActiveBranchExpanded.tsx
  - ../../src/components/branch/StackLanes.tsx
  - ../../src/components/commit/MovableCommitList.tsx
  - ../../src/components/commit/PushUpToMark.tsx
  - ../../src/presenters/push.ts
  - ../../src/components/branch/ArchivedBranchExpanded.tsx
  - ../../src/components/conflict/ConflictCommitContext.tsx
  - ../../src/components/conflict/MergeConflictViewer.tsx
  - ../../src/components/conflict/ResolvedMergeViewer.tsx
  - ../../src/components/conflict/ConflictResolutionHint.tsx
  - ../../src/components/conflict/ConflictExplanation.tsx
  - ../../src/components/conflict/ConflictFixPanel.tsx
  - ../../src/components/conflict/MissingCommitsDetails.tsx
  - ../../src/hooks/useMissingCommitOwners.ts
  - ../../crates/trial-check/src/missing_commits.rs
  - ../../crates/sync-core/src/commit_owners.rs
  - ../../src/components/conflict/ConflictingFilesSection.tsx
  - ../../src/components/diff/DiffView.tsx
  - ../../src/stores/file-merges.ts
  - ../../src/lib/diff/file-merge-model.ts
  - ../../src/lib/diff/row-cells.ts
  - ../../src/components/conflict/SubjectFileDiffList.tsx
  - ../../src/components/conflict/ConflictWindowPage.tsx
  - ../../src/components/conflict/DiffLoadState.tsx
  - ../../src/components/shared/NestedPanel.tsx
  - ../../src/lib/merge-conflict.ts
  - ../../crates/sync-core/src/conflict_chain.rs
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
- A synced commit must show the hash of the user's commit, the one of their log, and name the hash of its copy in the
  virtual branch on hover ("Rebuilt in the virtual branch as 6959c2bd"), which Patronus and the remote name; an
  unexplained `29cf0a82 → 6959c2bd` in every row was the mechanism, not the commit.
- The subject must wrap to two lines at most.
- Issue keys in the subject must become links, as the issue navigation settings define. Signed in to YouTrack, a key
  of its issues shows the issue on hover ([YouTrack](../repository/youtrack.spec.md#the-issue-popover)).
- In the commits of a branch, a subject that starts with the issue key of the branch must leave it out, with the
  separator after it, as the title of the branch does: the branch row (its name, or its issue chip) or the issue row above
  it shows the key, as a link. A subject that is the key alone keeps it; other lists (unassigned, landed, the previews)
  show the subject as written. Rust splits the subject (`subjectParts` of a commit: its autosquash markers, its
  `(prefix)`, the rest with its issue key, and what the row shows split at the key it starts with, `issue_lead`); the
  page has no grammar of its own.
  [@test] ../../tests/e2e/branch-row-layout/description-title.spec.ts (`leaves out the issue key the row shows already, in the title and in the commits`)
  [@test] ../../crates/sync-utils/src/commit_subject.rs (`the_parts_of_a_subject_are_its_markers_its_prefix_its_rest_and_the_key_a_row_shows_it_led_by`)
  [@test] ../../crates/sync-utils/src/issue_pattern.rs (`a_title_led_by_an_issue_key_splits_at_it_and_its_separator`)
- A synced commit must not show a "Created" status text.
  [@test] ../../tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should show expanded branch row with commits`)
  [@test] ../../tests/e2e/issue-navigation/issue-navigation.spec.ts (`should render issue links in commit messages`, `should render issue links in unassigned commits`, `should show plain text for commits without issue references`)
- The subject line must show only exception badges: Pending, Merge conflict, Error, Blocked and Amend target.
- In the commits of a branch, a commit the remote lacks must show the push arrow after its subject (`PushUpToMark`):
  a click opens Push Up to Here of it, not the selection of the row
  ([Stacked Branches](../branch/stacked-branches.spec.md#push-up-to-here)); a commit the remote has, or one a merged
  Safe Push landed, has none (`offersPushUpTo`). While the button of an open Push Up to Here is hovered, the rows of
  the commits it sends are tinted (`data-sent`).
  [@test] ../../tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`, `should show no push arrow on the commits a merged Safe Push landed, only on a commit added after them`)
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
- D over a row reviews its commit, as Space opens its preview: a commit of a branch in the review window of its
  branch, at the commit, another one in the review window of the commit; the row's menu starts with "Review Changes"
  (D) ([Commit Review](commit-review.spec.md)).
  [@test] ../../tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a row in its window with D and with the item of its menu`)
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
  The menu of its row offers "Reword…" (one commit), which opens the preview with the message in edit, also while a sync
  or another rewrite runs: the save goes to the session at once, named on the table the preview shows, and waits there
  (C14). Escape ends that edit as any other, the preview stays.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should reword a commit from the menu of its row, in its preview`)
- The preview of a commit that is not on the current branch (an archived or a missing commit) must not offer it.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should open one preview for a missing commit nested in a conflict row`)
- The edit must show the raw message in a text field, the grouping prefix included, and focus the field.
- Cmd/Ctrl+Enter or Save must save the message. The preview must then show the reworded commit at the same position.
- After the save, the branch tables must show the new message.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should save the edited message and stay on the reworded commit`)
- Escape must cancel the edit and keep the preview open. A click outside must keep the edit.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should cancel the edit with Escape and keep the preview open`)
- A changed grouping must show a warning that names the new branch, or the unassigned commits. Rust groups the
  message as it was and as typed (`preview_group`: the subject as git reads it, by `CommitGrouper`'s rule with the
  stacked branches, issue groups and stack parents of the last grouping; the typed one once the typing pauses, again
  after a sync): an explicit prefix, else the issue group of the subject's key, else the key. With
  [stacked branches](../branch/stacked-branches.spec.md) on, it also names the parent and warns about a restack and a
  stack problem; a stack problem the save would introduce (parents that conflict, an empty level) is said to be
  refused.
  [@test] ../../src/lib/stack.test.ts (`warns about a parent the other commits of the branch don't declare`, `warns about an empty level of the prefix, which a reword refuses`)
- A save that moves the commit to another list must close the preview.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should warn that a changed grouping prefix moves the commit, and move it`)
  [@test] ../../crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_of_a_subject_is_its_prefix_else_the_issue_group_of_its_key_else_the_key`, `the_preview_with_stacked_branches_reads_the_parent_of_the_prefix_else_the_stack_parent`)
- The edit and the save must stay enabled while a sync runs: the reword goes to the session at once, named on the
  table the preview shows, and waits there (C14).
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should keep the edit enabled while a sync runs: the save goes to the session at once`)
- A failed save must keep the edit open with the draft and show the error in a toast.
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should refuse a reword that strands a fixup, and keep the draft`)
- A save of a commit that changed meanwhile (reworded in a terminal) must change nothing: the session brings its table
  to HEAD to find the commit, finds it neither by its id nor by its key, and refuses the reword as a changed plan
  ("Cannot edit the commit message" with the reason); the preview of the commit that is gone closes, as after any sync
  that removes it.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should save nothing when the commit changed meanwhile, say so, and close the preview of the commit that is gone`)
- E must work in any keyboard layout (the physical key).
- The save hint must name the platform shortcut: `⌘↵` on macOS, `Ctrl+Enter` elsewhere.
- The save must be the reword of `bd reword` ([Agent CLI](../cli/agent-cli.spec.md), `bd reword`): one regroup of the
  commits ahead of the baseline, with its checks. It must replace only the message: the commits after it are recreated
  with the same trees, and the working tree and the index stay as they are.
- The save must keep the author and the committer of every recreated commit: name, email, date and timezone.
  [@test] ../../crates/sync-core/src/reword_test.rs (`reword_preserves_committer`)
- The message must be cleaned up as `git commit` does: LF line endings, no trailing whitespace, no blank lines around.
- An unchanged message must keep the commit.
- A message without a subject line must be refused.
- The save must be refused while a rebase, merge, cherry-pick or revert is in progress, or on a detached HEAD.
- The save must be refused if the commit left the current branch meanwhile (reworded or dropped in a terminal): "Commit
  <short id> is no longer on the current branch, sync and try again".
- The save must fail, and keep the new commit unreferenced, if the branch moved meanwhile (a commit made in a terminal).
  [@test] ../../crates/sync-core/src/reword_test.rs (`rewords_a_middle_commit_and_keeps_the_commits_after_it`, `a_subject_keeps_the_body_and_a_message_equal_after_cleanup_rewrites_nothing`, `refuses_ids_and_texts_it_cannot_reword`, `refuses_to_reword_while_a_merge_is_in_progress`, `rewrite_refused_on_detached_head`, `refuses_a_commit_that_left_the_current_branch`, `rewords_several_commits_in_one_pass`)
  [@test] ../../crates/git-ops/src/ref_transaction_test.rs (`refuses_to_move_a_branch_that_moved_meanwhile`)
- A new prefix of a commit must move its fixups with it.
  [@test] ../../crates/sync-core/src/reword_test.rs (`a_new_prefix_moves_the_commit_with_its_fixups`)
- The save must be refused, with the edit and its draft kept, when a fixup would fold into another target or none
  afterwards, when it adds or removes a `fixup!`, `squash!` or `amend!` marker, when it gives a new id to a target that a
  fixup names by its id, or when an `amend!` of the commit waits; the reason says to squash the fixups into their target
  first (Squash Fixups).
  [@test] ../../tests/e2e/squash-fixups/squash-fixups.spec.ts (`should refuse a reword that strands a fixup, and keep the draft`)
  [@test] ../../crates/sync-core/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)
- The save must be refused when the new prefix introduces a stack problem (parents that conflict, a cycle, an empty
  level).
  [@test] ../../crates/sync-core/src/reword_test.rs (`a_reword_that_breaks_a_stack_is_refused`)
- After a save that changed the commit, a toast "Reworded <short id of the old commit>" with the new subject must offer
  Undo for 10 s, and Activity must record the reword with the same Undo, which runs once. The Undo must move HEAD back
  to what the save found (the old messages, and with them the old virtual commits), as the Undo of a move does
  (the rewrite `undo` of the engine). A click on the toast must keep the preview open; the preview on the reworded commit must follow the
  Undo back to the old commit. A preview stepped to another commit stays there, and one on a commit that the Undo
  removes closes.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should undo a reword from its toast and follow the preview back`, `should undo a reword from Activity`)
  [@test] ../../src/hooks/useRewordCommit.test.ts (`records a reword with its Undo for the window of the toast, once`, `offers no Undo past the window of the toast`)
  [@test] ../../crates/sync-core/src/reword_test.rs (`undo_of_a_reword_brings_back_the_old_virtual_commits`)
  [@test] ../../crates/test-server/src/reword_test.rs (`a_reword_answers_the_undo_that_brings_the_old_head_back`)
- The Undo must be refused, with nothing changed, once the branch moved after the save (another rewrite, a commit in a
  terminal): "Cannot undo the reword" with the reason.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should refuse to undo a reword after the branch moved`)
- A save that changed nothing (the same message after the cleanup) must show no toast, record nothing and run no sync.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should save an unchanged message without a toast`)
  [@test] ../../src/hooks/useRewordCommit.test.ts (`records nothing for an unchanged message`)
- The save must be the rewrite `reword` of the engine, named on the table the window shows, and run under the
  repository lock: while another process holds it (an agent's `bd pick --follow`, `bd`) or the app's own push, the save
  waits as long as the holder lives, the editor saying whom for (the step of its operation) with Stop Waiting
  (`cancel_operation`), which ends the wait with nothing changed; only a holder no record names refuses it after 10 s,
  with nothing changed and the toast "Cannot edit the commit message" saying that another operation holds the lock. A
  save while another rewrite runs (an Unstack from the main window, an issue reference of the same commit) waits
  behind it in the queue of the session and rewords the commit as that one left it, its new id included.
  [@test] ../../crates/operations/src/rewrite_test.rs (`a_reword_and_an_unstack_from_two_windows_at_once_are_both_applied_in_order`, `a_reword_queued_behind_the_issue_reference_of_its_commit_names_it_through_the_new_id`)
  [@test] ../../crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`, `a_rewrite_waits_for_the_holder`)
  [@test] ../../crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`, `a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

### Message body

The body is parsed in Rust (`commit_body_blocks`, `sync_utils::commit_body`), the grammar the body of a pull request is
written from ([Pull Requests](../branch/pull-requests.spec.md)); the page asks once per message and keeps the answer
for a message shown again.

- The body must start at the first line after the subject that is not blank.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/commit.test.ts (`is true for a message with lines after the subject`, `is false for a message that equals the subject`, `is false for a one-line message that differs from a stripped subject`, `is false for an empty message`)
  [@test] ../../crates/sync-utils/src/commit_body_test.rs (`the_body_of_a_message_is_after_its_subject_line`)
- The body must join hard-wrapped lines into one paragraph; a blank line must start a new block. A line after a hard
  break (two spaces or `\` at the end), a quote line and a table row must stay a line of their own; a heading and a
  rule a paragraph of their own.
- Lines that start with `- `, `* `, `+ `, `• `, `1. ` or `1) ` must form a list without these markers (a rule is no
  item). A change between bullets and numbers must start a new list. An ordered list must keep the number of its first
  item. A continuation line, wrapped, indented or not, must stay in its list item until a heading, a quote, a table row
  or a rule.
- Lines indented by four spaces or a tab must stay verbatim, without the common indent.
- A fenced block (```` ``` ```` or `~~~`) must stay verbatim, blank lines included, without the fences; one without a
  closing fence must run to the end of the body.
- A last block of git trailers (`Token: value` lines, an indented line continuing one, `(cherry picked from commit …)`)
  must show as a separate trailer block when one of its lines is a token of words joined by `-` or the cherry-pick
  note; a block of one-word tokens alone (`Note: …`) and a block that is not the last stay prose.
- The body must treat CRLF line endings as LF.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
  [@test] ../../crates/sync-utils/src/commit_body_test.rs (`joins_hard_wrapped_lines_into_one_paragraph_and_starts_a_block_after_a_blank_line`, `keeps_a_line_after_a_hard_break_a_quote_line_and_a_table_row_its_own_and_a_heading_and_a_rule_a_paragraph`, `reads_lists_without_their_markers_each_kind_its_own`, `keeps_code_verbatim_without_the_common_indent_and_the_fences`, `reads_a_last_block_of_trailers_with_one_that_only_a_trailer_names`, `keeps_as_prose_a_trailer_shaped_block_that_is_not_the_last_one_has_prose_or_only_one_word_tokens`)
- A paragraph that is only `Label:` must become a section label above the next blocks.
- A paragraph that starts with a short `Label: ` must become a section label above its remaining text.
- A `Label:` paragraph without a following content block must stay a paragraph.
- The trailers must form the last section, without a label.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/commit-message.test.ts (`turns a paragraph that is only Label: into the label of the next blocks`, `turns a short run-in Label: into a label above the rest of the paragraph`, `keeps a run-in label longer than 20 characters as prose`, `keeps a Label: paragraph without a following block as a paragraph`, `keeps a Label: paragraph followed only by trailers as a paragraph`, `puts the trailers into the last, unlabelled section`, `keeps the lines of a run-in paragraph after its label`, `adds unlabelled blocks to the current section`)
- Issue keys in the body must become links.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)

### Code tokens

- Text in backticks must show as code, without the backticks.
- The body must show detected paths, file names, dotted module and package names of three or more parts, flags, calls,
  CONSTANT_CASE, snake_case and camelCase as code. A dotted name of two parts (`example.com`, `e.g.`) stays prose.
- Names from the stoplist (`JavaScript`, `GitHub`, `macOS`) must stay prose.
- Preformatted blocks and trailers must not detect code.
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
  [@test] ../../src/lib/inline-code.test.ts (`shows text in backticks as explicit code without the backticks`, `detects a %s as code`, `keeps %s as prose`)
- A PascalCase name must be code only when it has three or more humps.
- A token in double quotes must stay prose.
- Code tokens that only single spaces separate must form one span, such as `bt.cmd --lane` or
  `--module intellij.air.tests --filter com.intellij.air`.
- Punctuation around a token must stay prose.
- An issue key inside a code token must stay part of the code.
  [@test] ../../src/lib/inline-code.test.ts (`keeps %s as prose`, `keeps a token in double quotes as prose`, `joins code tokens separated by a single space into one span`, `joins a flag and its dotted value into one span`, `ends a code span at trailing punctuation`, `keeps the punctuation around a token as prose`, `keeps the punctuation after a call as prose`, `keeps an issue key inside a code token`)
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

### Conflict view

- A row with a merge conflict must show the conflict viewer below its header.
- The viewer must start with its title, the explanation and the fix of [Conflict Fix](../branch/conflict-fix.spec.md);
  a conflict with the amended commit has the collapsed "How to resolve" hint instead.
- The badge must say "Merge conflict". The title must name the commit and what it is copied onto: "“<subject>” does
  not apply cleanly onto <base>", where the base is the parent of a stacked branch or the baseline, and "the amended
  commit" for an amend conflict. The subject leaves out the grouping prefix. The sub-windows and the branch preview say
  the same.
  [@test] ../../src/lib/merge-conflict.test.ts (`names the subject without its grouping prefix and the branch it is copied onto`, `says "the baseline" before the first sync names it`)
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`)
- The missing commits are the older commits that change a conflicting file and are neither in the branch nor in the
  branches it is built on, each with the branch it is in (`MissingCommit.branch`, none for an unassigned commit). The copies of the branch's
  own commits and of its stack ancestors are not missing.
  [@test] ../../crates/trial-check/src/missing_commits_test.rs (`the_missing_commits_leave_out_the_branches_it_is_built_on_and_name_their_branch`)
- The explanation must be one callout in the row and in the missing-commits window. Its title says where the missing
  commits are: "18 commits of IJAI-4047 changed these files first", "42 commits of IJAI-4047 and 11 branches it is
  built on …" when one branch is built on all the others, "… of 12 branches …" otherwise, ", as did 2 unassigned
  commits" for the unassigned ones, and "No commit outside the branch changed these files first" for none. A sentence
  says what the branch lacks: "<branch> is built on <base>, which doesn't have them." (not for an amend conflict).
  [@test] ../../src/lib/merge-conflict.test.ts (`groups the commits by branch, the most first, and finds the branch built on all the others`, `has no stack top when the branches are on two stacks`, `names a single branch`, `says that no commit outside the branch changed the files`, `says what the branch is built on`, `says nothing for a conflict with the amended commit`)
- The explanation must show a chip per branch with its number of missing commits, the most first, at most 8 and then
  "+N branches", all 9 of 9 (`cutList`: never "+1 branch"). In the main window a chip shows the branch with its missing commits (`requestReveal`); the window
  only names them. The common ancestor and the divergence counts are not shown.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`, `should show the branch of the missing commits from the explanation`)
- The missing commits details must group the commits by branch, in the order of the chips, the unassigned ones last
  under "Unassigned"; the header of a group names its branch and count, and in the main window shows the branch.
- The conflicting files must follow, and then the missing commits details.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`, `should expand/collapse file accordions in missing commits details`)
- The conflicting files are to come from `get_file_merges`, as IntelliJ's merge view shows them: git's `merge-tree`
  decides which files conflict, and the engine merges the three stages of each file again. Each block has its kind,
  the sides that changed it, its word ranges and its resolution. A conflict that its words resolve says so: "Git
  stops here; the conflict resolves by words" (`conflicts > 0`, `unresolved == 0`, the `resolvedText` of each of its
  blocks). A modify/delete shows the diff of the base to the side that kept the file.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`shows the merge in three columns, with the result the engine proposes and the base`)
  [@test] ../../crates/file-diffs/src/merge_test.rs (`each_conflict_type_has_its_merge`, `a_merge_costs_one_merge_tree_and_one_cat_file_then_nothing`, `the_stream_lists_the_files_then_the_file_in_view_first`, `the_model_is_json_with_a_kind`)
  [@test] ../../crates/text-diff/src/merge_model_test.rs (`a_conflict_of_other_words_resolves_by_its_words`, `a_conflict_of_the_same_words_stays_with_the_user`, `a_block_is_ignored_where_its_changed_sides_are_equal_ignoring_whitespace`, `blocks_partition_the_three_texts`)
  [@test] ../../crates/test-server/src/commit_files_test.rs (`commit_files_routes_read_the_fixture_repository`)
- The conflicting files must be one list, a collapsible item per file, the first open, each with the number of its
  conflicts ("1 conflict", "3 conflicts") or "modify/delete". The merge of a file shows in three columns, as
  IntelliJ's merge viewer, read-only: left the branch the commit is copied onto (ours), right the commit (theirs),
  named above them. The middle column shows "Result (proposed)" by default: each block as it resolves (the side that
  changed it, or its resolve by words), and the base for a conflict that does not resolve. "Base" shows the base. A
  block has the colour of its kind (inserted green, deleted grey, modified blue, conflict red) in the middle and on
  each side that changed it; a side that did not change it stays plain. The tints follow the rule of a diff, with the
  word ranges of each side: a block with inner ranges, also empty ones, is pale with its changed words in the full
  colour, and an insertion or a deletion without them is in the full colour. A conflict that the result resolves has
  a dashed bar there. Every block is a change that N and P stop at; a block the whitespace choice ignores is pale,
  never drawn as unchanged lines, as in IntelliJ's merge viewer. The equal lines fold as in a diff. The merge takes
  the one Whitespace choice of the diffs; IntelliJ's merge has no Ignore Whitespace and Blank Lines, so that choice
  merges as Ignore Whitespace, and the conflict view has no menu of its own. A resolve action is not part of this
  view.
  [@test] ../../src/lib/diff/rows.test.ts (`aligns the blocks of a merge in three columns, the middle the result, without the empty last line`, `counts the lines of a resolve by words in the middle, and moves the middle lines after it`, `folds the equal lines of a merge, labelled by the scopes of the left side under the result`, `draws a block the policy ignores pale, and keeps it a change that N and P stop at, as IntelliJ's merge viewer`, `tints the commit_whitespace merge by IntelliJ's rule under each policy`, `draws the lines of a merge block with inner ranges pale`)
  [@test] ../../src/lib/diff/row-cells.test.ts (`colours a block in the middle and on the sides that changed it, and marks a resolved conflict in the result`, `leaves a side that did not change a block plain`)
  [@test] ../../src/lib/diff/file-merge-model.test.ts (`takes the side that changed a block, the resolve by words, or the base of a conflict that does not resolve`, `composes the result text with the line indices of the middle column`)
  [@test] ../../crates/git-ops/src/merge_conflict_tests.rs (`the_conflicts_of_a_merged_file_are_counted_by_their_start_markers`)
  [@test] ../../tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`shows the merge in three columns, with the result the engine proposes and the base`)
- A file that one side deleted and the other changed must show the change of the side that kept it, under a line
  that says what happened: "<branch> does not have the file, the commit changes it: the change of the commit", or
  "The commit deletes the file, <branch> changed it: that change". So the list is never empty under a non-zero file
  count.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should list every conflicting file, the first open, and say that a file without conflict markers is a modify/delete`)
- A sync must report a conflict without its contents: the commits, each conflicting file with its number of conflicts,
  and the missing commits with the files they change. Its events stay a few kilobytes whatever the size of the files.
  A view reads the merges of its conflicting files (`get_file_merges`) when it shows the conflict, one read per
  conflict and whitespace choice, the open file first, and keeps the last 8; an open item shows "Loading the
  conflict…" until its merge arrives. The merge works after `git gc` too, while the branch conflicts, also after a
  sync without the object cache: the sync keeps the copies of the branch's commits before the conflict, which the
  conflict merges onto, on a ref of its own.
  [@test] ../../crates/sync-core/src/conflict_report_test.rs (`a_sync_reports_a_conflict_without_the_contents_of_its_files`)
  [@test] ../../crates/sync-core/src/process_budget_test.rs (`the_target_of_a_conflict_survives_gc`)
  [@test] ../../crates/sync-core/src/conflict_chain_test.rs (`a_conflicting_branch_keeps_its_copies_while_it_conflicts`, `the_refs_move_with_compare_and_swap_and_a_move_by_another_process_counts`, `a_branch_and_a_branch_below_it_have_refs_of_their_own`)
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should read the merges of a conflict when its view shows it, once for all its files`)
  [@test] ../../src/stores/file-diffs.test.ts (`reads the merges of a conflict once per policy, the open file first, and the late scopes keep the lines it expanded`, `says why a read failed`)
- A failed read must make each open item say "Cannot load the conflict: <reason>" under its header, which keeps the
  file and its conflict count. A failed read is read again when the conflict is shown anew.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should say why the merges of a conflict did not load`)
- The windows of the conflicting files and of the missing commits must get the conflict without its contents, and the
  repository, and read the merges and the diffs themselves.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should load the contents of the conflict in the windows of its files and missing commits`)
- The diffs of the missing commits must be the diffs of the engine of their commit (one
  `get_file_diffs` per commit, its texts by blob id), so the unchanged lines between hunks expand. The missing commits
  header has the split/unified/inline toggle, persisted: a stored choice keeps winning over the Split default.
  [@test] ../../src/lib/diff/view-files.test.ts (`says why a file shows no lines: binary, too large, a submodule, a blob git cannot read, a text that cannot be read, no changed lines`)
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should keep a stored diff view mode over the Split default`)
- A commit whose conflict of git the sync resolved by words (`branchdeck.resolveByWords`, on by default; see
  [Agent CLI](../cli/agent-cli.spec.md), "`bd sync`") must not show a conflict: its row says "Resolved 1 conflict by
  words" ("Resolved 3 conflicts by words"), a button. A click shows "Resolved by Words" under the row: what git stopped
  at, that the sync copied the commit with the result as IntelliJ resolves simple conflicts, and the merge of each
  file in three columns, each file with "1 conflict resolved", the middle "Result (committed)", the blob that the sync
  committed. A second click hides it. A copy that a later sync reused gives the counts of its mapping note, and its
  files come with the merges.
  [@test] ../../tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`copies the commit with the result, says so on its row and shows what it committed`)
  [@test] ../../crates/file-diffs/src/merge_test.rs (`a_resolved_copy_names_the_blob_of_each_file`)
- A row with a generic error must show the error text in a destructive alert.
- The conflict commit context of a sub-window must show the title over the meta line of the commit, with the branch
  name as an extra meta item. An unknown branch must be left out, never shown as "Unknown".
- The conflict pages in a separate window must show the explanation and the steps of the fix the main window found
  when it opened them, under "Apply the fix in the main window" (the window shows a snapshot), or the "How to resolve"
  hint without one. Their window titles must be "Conflicting files · <branch>" and "Missing commits · <branch>".

## User Experience

### Collapsed row

- The subject uses `text-sm text-foreground` in regular weight: weight marks names and titles, so a long list does not
  outweigh the branch names ([UI Theme](../../docs/design/ui-theme.md#rules-for-ui-changes), "Type hierarchy").
- The prefix uses `font-mono text-xs text-muted-foreground`.
- The meta line uses `text-xs text-muted-foreground`. The hashes use the monospace font.
- The message button follows the subject as a ghost `icon-xs` button. It keeps the 20px line height of the subject.
- The message button is muted. It becomes `text-foreground` on hover.
- Issue links are muted and become `text-foreground` on hover.
- The push arrow follows the message button: a 16px button with a 12px `ArrowUpIcon` in `text-primary`,
  `bg-primary/10` on hover. A row its Push Up to Here sends is `bg-primary/5`; the selection tint wins over it.
- Badges follow the message button: `secondary` Pending with a spinner, `destructive` conflict or error, `warning` Blocked, `primary` Amend target (the target of the user's amend).

### Commit preview

- The preview is a centered dialog on the blurred backdrop, at most 56rem wide and `min(80vh, 48rem)` high.
- The header shows the whole subject in `text-base font-medium` (16/24px), the title of the dialog, without the
  two-line clamp, above the meta line. A `border-border` rule separates it.
- Only the body scrolls. The body has the focus, so PageUp and PageDown scroll it. ↑ and ↓ belong to the navigation.
- A newly shown commit starts at the top of its body.
- The footer shows the position on the left and the `Kbd` hints `↑ ↓ Previous/next` and `Space Close` on the right,
  in `text-xs text-muted-foreground` below a `border-border` rule. A list of one commit has no `↑ ↓` hint.
- The footer then shows the ghost `xs` buttons `D Review Changes`, which reviews the commit in its window and closes
  the preview ([Commit Review](commit-review.spec.md)), and `F Files`, which shows or hides the changed files.
- An editable preview ends the footer with a ghost `xs` button `E Edit`, enabled during a sync too; "D Review Changes"
  of a commit without files is disabled with "The commit changes no files". A preview of a list with a row menu has "⋯" in its header, before the close button: the menu of the row (Move to…, Squash into, Push Up to
  Here, the copies), its key hints only for the keys of the preview; an item that opens a form of the row closes the
  preview first, a copy keeps it (`toPreviewMenu`). One builder lists the menu of commits for the row, its preview and
  the commit in the review window of its branch (`getCommitMenuGroups`), every item by a stable id
  ([Branch Review](../branch/branch-review.spec.md#title-bar-footer-and-actions)).
  [@test] ../../tests/e2e/commit-message/commit-message.spec.ts (`should offer the menu of the commit's row in its preview`)
- While editing, the body is a monospace 13/20px text field at least 12rem high that grows with the message.
  The footer shows "The first line is the subject", or the prefix warning in `text-warning`, on the left, and
  `Esc Cancel` and a primary `xs` button `Save ⌘↵` (`Save Ctrl+Enter` outside macOS) on the right. The button shows a spinner while saving.
- The selection tint of a row never grows with the message: the row stays two lines.

### Message body

- The body is at most `max-w-prose` (65ch, about 80 characters) wide.
- The body text is `text-foreground` with the `ss04` font feature: the 14/22px reading text (`text-md`,
  `size="reading"`) in the preview, the 13/20px UI text (`text-sm`) in the hash popover.
- A section label is a small `font-medium text-muted-foreground` heading above its blocks.
- Sections have a 16px gap. Blocks in a section have an 8px gap.
- Lists have a 20px indent and muted markers.
- Preformatted text uses `font-mono` one step below the body (`text-sm` in the preview, `text-xs` in the hash popover)
  on `bg-muted` and scrolls horizontally.
- Trailers use `text-xs text-muted-foreground` below a `border-border` rule.
- Detected code uses the monospace font at the size of the text, without a background: JetBrains Mono has the x-height
  and cap height of Inter, so a smaller size only shrinks it.
- Code in backticks also gets a `bg-muted` chip.
- The body shows the text cursor, and its text is selectable.

### Hover and selection

- A selectable row shows the pointer cursor and `bg-muted` on hover.
- A selected row uses `bg-primary/15` (the IntelliJ light selection), and `bg-primary/20` on hover.
- A selected row has no accent bar.
- A row that is not selectable has no hover tint and keeps the default cursor.
- The selection actions show `N selected` in muted text, then an outline "Move to…" button in the primary colour.

### Conflict view

- In the expanded row the viewer is three `NestedPanel`s (`rounded-lg border border-border bg-card`, no ring and no
  shadow), 16px apart: the title with the explanation and the fix, the conflicting files, the missing commits. A sub-window
  shows the same sections as `Card` islands in a `Container`, 8px apart.
- The title is `text-sm font-medium` after a `destructive` `TriangleAlertIcon`.
- The explanation is a `warning` `Callout` with `FileDiffIcon`; its chips are `secondary` `xs` buttons, the count muted
  and tabular.
- A conflicting file is a `rounded-lg border` item with the file header of a review (`FileHeaderParts`: `h-9`, the
  chevron first, turned down when open, the name `FilePathLabel` `emphasis`, Copy Path on hover; a click anywhere on it
  toggles) and an `outline` badge with its conflict count at its end; the unresolved file has no file header of its
  own.
- The file without conflict markers states it in a muted `text-xs` line above its diff.

### Branch table

- An expanded branch of the branch table shows its commits on its lane: a 5px dot (`--bd-lane-dot`) per
  commit at the middle of its subject line, on the line of the lane, which runs on into the children of the branch or
  ends at its last commit, beside the other lanes that run on ([Branch Row](../branch/branch-row.spec.md#tree)). The
  highlighted path to the root of a stack runs on beside them as well.
- Without stacks the lane is the one lane of the node of each branch, and the commits are newest first.
- The subjects start at the left edge of the branch name.
- An archived branch uses the rail.

### Commit hash popover

- The popover opens on hover over the monospace short hash.
- The popover is at most 24rem high and 42rem wide, and at least 300px wide.

### Dark theme

- The row uses only shadcn semantic tokens and no `dark:` variants.
- The body colour and the row tints follow the theme tokens in both themes.

## Testing / Local Run

- `cargo nextest run -p sync-utils -p history-rewrite` runs the tests of the body parser, the subject parts and the
  grouping preview; `pnpm test:unit` those of the sections, the code detection and the subject prefix in `src/lib`.
- `pnpm e2e` runs the E2E tests. The tests in `tests/e2e/commit-message` cover the commit preview and its body.

## Open Questions / Risks

- No test covers the Pending, Blocked and generic Error badges. No backend path in the mock server emits them.
- No test covers the body, the height limit or a commit without a subject in the commit hash popover.
- No test checks that the subject does not detect code.
- Base UI moves the focus into the preview a frame after it opens. Until then, the list ignores its keys, so a fast
  ↓ right after Space is lost.
- No test checks that the card header hides the selection actions while the inline branch form is open.
- No test covers the generic error alert, or the explanation and the steps of the fix in the conflict pages of a
  separate window.
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
