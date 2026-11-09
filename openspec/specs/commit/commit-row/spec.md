---
name: Commit Row
description: Requirements for the commit row, the commit card, the commit preview with the message body and its editing.
style: plain-1
targets:
  - crates/file-diffs/src/merge.rs
  - crates/text-diff/src/merge_model.rs
  - src/components/conflict/CollapsibleFileHeader.tsx
  - src/components/files/FileHeaderParts.tsx
  - src/components/commit/CommitList.tsx
  - src/components/commit/CommitRow.tsx
  - src/components/commit/CommitHeader.tsx
  - src/components/commit/CommitSubject.tsx
  - src/components/commit/CommitMetadata.tsx
  - src/components/commit/CommitBodyView.tsx
  - src/presenters/commit-body.ts
  - src/components/commit/CommitCard.tsx
  - src/components/commit/CommitRowMark.tsx
  - src/components/review/ReviewPrologue.tsx
  - src/hooks/useHeadingCard.ts
  - src/components/shared/RowCard.tsx
  - src/stores/commit-card.ts
  - src/components/preview/CommitsPreviewBody.tsx
  - crates/git-ops/src/commit_list.rs
  - src/components/commit/CommitPreviewDialog.tsx
  - src/components/shared/PreviewDialogFrame.tsx
  - src/hooks/usePreviewHotkeys.ts
  - src/lib/preview-keys.ts
  - src/hooks/useRewordCommit.ts
  - src/hooks/useStackActions.ts
  - src/components/commit/PrefixChangeHint.tsx
  - src/lib/stack.ts
  - src/lib/rewrite-run.ts
  - crates/operations/src/rewrite.rs
  - crates/history-rewrite/src/reword.rs
  - crates/history-rewrite/src/regroup.rs
  - crates/history-rewrite/src/fixup_targets.rs
  - crates/history-rewrite/src/repository_lock.rs
  - crates/git-ops/src/reword_commits.rs
  - src/components/commit/LinkedText.tsx
  - src/lib/commit.ts
  - src/hooks/useCommitBody.ts
  - src/hooks/usePreviewGroup.ts
  - src/hooks/useDebouncedRead.ts
  - crates/app-api/src/session/view.rs
  - crates/app-api/src/reads/grammar.rs
  - crates/sync-utils/src/commit_body.rs
  - crates/sync-utils/src/markdown_table.rs
  - crates/sync-utils/src/markdown_inline.rs
  - crates/sync-utils/src/markdown_comments.rs
  - crates/sync-utils/src/commit_subject.rs
  - crates/sync-utils/src/issue_pattern.rs
  - crates/domain-types/src/commit_body.rs
  - crates/history-rewrite/src/commit_grouper.rs
  - src/lib/inline-code.ts
- ../../crates/sync-utils/src/code_spans.rs
- ../../src/components/shared/CodeText.tsx
  - src/components/unassigned/UnassignedCommitListCard.tsx
  - src/components/branch/ActiveBranchExpanded.tsx
  - src/components/branch/StackLanes.tsx
  - src/components/commit/MovableCommitList.tsx
  - src/components/commit/PushUpToMark.tsx
  - src/presenters/push.ts
  - src/components/branch/ArchivedBranchExpanded.tsx
  - src/components/conflict/ConflictCommitContext.tsx
  - src/components/conflict/MergeConflictViewer.tsx
  - src/components/conflict/ResolvedMergeViewer.tsx
  - src/components/conflict/ConflictResolutionHint.tsx
  - src/components/conflict/ConflictExplanation.tsx
  - src/components/conflict/ConflictFixPanel.tsx
  - src/components/conflict/MissingCommitsDetails.tsx
  - src/hooks/useMissingCommitOwners.ts
  - crates/trial-check/src/missing_commits.rs
  - crates/sync-core/src/commit_owners.rs
  - src/components/conflict/ConflictingFilesSection.tsx
  - src/components/diff/DiffView.tsx
  - src/stores/file-merges.ts
  - src/lib/diff/file-merge-model.ts
  - src/lib/diff/row-cells.ts
  - src/components/conflict/SubjectFileDiffList.tsx
  - src/components/conflict/ConflictWindowPage.tsx
  - src/components/conflict/DiffLoadState.tsx
  - src/components/shared/NestedPanel.tsx
  - src/lib/merge-conflict.ts
  - crates/sync-core/src/conflict_chain.rs
  - src/presenters/rewrite.ts
  - crates/git-ops/src/word_resolve.rs
  - crates/git-ops/src/attributes.rs
  - crates/git-ops/src/conflict_analysis.rs
  - crates/git-ops/src/merge_conflict.rs
  - crates/domain-types/src/conflict.rs
---

# Commit Row

Status: Implemented
Date: 2026-10-13

## Purpose

A commit row shows one commit in a branch table, in the unassigned commits card or in a conflict view.
The row has a subject line above a meta line.
Space or a click on the mark of a row opens the full commit message in a centered preview, as Quick Look does.
The preview also edits the message of a commit on the current branch.
The hover of the mark, or I, opens the commit card: the one card of a commit, its header and its body, which every
place that shows a commit shows the same way.

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
  see [Commit Files](../commit-files/spec.md).
- This spec does not cover the branch status column or the branch actions: see [Branch Row](../../branch/branch-row/spec.md).

## Requirements

### Requirement: Row anatomy: The subject line stands above the meta line

A commit row MUST show the subject line above the meta line.

#### Scenario: A commit row
- **WHEN** a commit row is shown
- **THEN** its subject line stands above its meta line

### Requirement: Row anatomy: The meta line starts with the short hash

The meta line MUST start with the short hash.

#### Scenario: The first item of the meta line
- **WHEN** a commit row shows its meta line
- **THEN** the meta line starts with the short hash

### Requirement: Row anatomy: The items of the meta line and their order

The meta line SHALL allow, after the short hash, the author, the time, the file count with the line counts
([Commit Files](../commit-files/spec.md)) and extra items, in this order.

#### Scenario: The meta line with its items
- **WHEN** the meta line shows the author, the time, the file count with the line counts and extra items
- **THEN** they come after the short hash in this order: the author, the time, the file count with the line counts, the
  extra items

### Requirement: Row anatomy: The meta line separates its items with a dot

The meta line MUST separate its items with `·`.

#### Scenario: The separator of the meta line
- **WHEN** the meta line shows several items
- **THEN** `·` separates them

### Requirement: Row anatomy: A synced commit shows the hash of the user's commit

A synced commit MUST show the hash of the user's commit, the one of their log, and name the hash of its copy in the
virtual branch on hover ("Rebuilt in the virtual branch as 6959c2bd"), which Patronus and the remote name; an
unexplained `29cf0a82 → 6959c2bd` in every row was the mechanism, not the commit.

#### Scenario: The hash of a synced commit and of its copy
- **GIVEN** a synced commit whose copy in the virtual branch has another hash
- **WHEN** its row is shown
- **THEN** the meta line shows the hash of the user's commit, the one of their log
- **AND** the hover of the hash names the hash of its copy: "Rebuilt in the virtual branch as 6959c2bd"
- **AND** the row shows no unexplained `29cf0a82 → 6959c2bd`

### Requirement: Row anatomy: The subject wraps to two lines at most

The subject MUST wrap to two lines at most.

#### Scenario: A long subject
- **WHEN** a subject is longer than the row is wide
- **THEN** it wraps to two lines at most

### Requirement: Row anatomy: Issue keys in the subject are links

Issue keys in the subject MUST become links, as the issue navigation settings define.

#### Scenario: A subject with an issue key
- **GIVEN** the issue navigation settings define the pattern of a key
- **WHEN** a subject contains such a key
- **THEN** the key is a link, as the settings define it
  [@test] tests/e2e/issue-navigation/issue-navigation.spec.ts (`should render issue links in unassigned commits`)

#### Scenario: A subject without an issue key
- **WHEN** a subject contains no issue key
- **THEN** it shows as plain text, without a link
  [@test] tests/e2e/issue-navigation/issue-navigation.spec.ts (`should show plain text for commits without issue references`)

### Requirement: Row anatomy: Signed in to YouTrack, a key shows its issue on hover

Signed in to YouTrack, a key of its issues SHALL show the issue on hover
([YouTrack](../../repository/youtrack/spec.md#the-issue-popover)).

#### Scenario: The hover of a key of a YouTrack issue
- **GIVEN** the user is signed in to YouTrack
- **WHEN** the pointer rests on a key of its issues in a subject
- **THEN** the issue shows on hover

### Requirement: Row anatomy: A commit of a branch leaves out the issue key of the branch

In the commits of a branch, a subject that starts with the issue key of the branch MUST leave it out, with the
separator after it, as the title of the branch does: the branch row (its name, or its issue chip) or the issue row above
it shows the key, as a link. A subject that is the key alone keeps it; other lists (unassigned, landed, the previews)
show the subject as written.

#### Scenario: A subject led by the issue key of its branch
- **GIVEN** a commit of a branch whose subject starts with the issue key of the branch
- **WHEN** the commits of the branch are shown
- **THEN** the subject leaves out the key with the separator after it, as the title of the branch does
- **AND** the branch row (its name, or its issue chip) or the issue row above it shows the key, as a link
  [@test] tests/e2e/branch-row-layout/description-title.spec.ts (`leaves out the issue key the row shows already, in the title and in the commits`)
  [@test] tests/e2e/issue-navigation/issue-navigation.spec.ts (`should render issue links in commit messages`)

#### Scenario: A subject that is the key alone
- **WHEN** the subject of a commit of a branch is the issue key of the branch alone
- **THEN** it keeps the key

#### Scenario: Other lists show the subject as written
- **WHEN** a commit shows in another list (unassigned, landed, the previews)
- **THEN** its subject shows as written

### Requirement: Row anatomy: Rust splits the subject, the page has no grammar

Rust MUST split the subject (`subjectParts` of a commit: its autosquash markers, its `(prefix)`, the rest with its
issue key, and what the row shows split at the key it starts with, `issue_lead`); the page MUST NOT have a grammar of
its own.

#### Scenario: The parts of a subject
- **WHEN** Rust reads the subject of a commit
- **THEN** `subjectParts` holds its autosquash markers, its `(prefix)` and the rest with its issue key
- **AND** what the row shows is split at the key it starts with, `issue_lead`
  [@test] crates/sync-utils/src/commit_subject.rs (`the_parts_of_a_subject_are_its_markers_its_prefix_its_rest_and_the_key_a_row_shows_it_led_by`)
  [@test] crates/sync-utils/src/issue_pattern.rs (`a_title_led_by_an_issue_key_splits_at_it_and_its_separator`)

### Requirement: Row anatomy: A synced commit shows no Created status text

A synced commit MUST NOT show a "Created" status text.

#### Scenario: The rows of an expanded branch
- **WHEN** a branch with synced commits is expanded
- **THEN** its commit rows show no "Created" status text
  [@test] tests/e2e/branch-row-layout/branch-row-layout.spec.ts (`should show expanded branch row with commits`)

### Requirement: Row anatomy: The subject line shows only exception badges

The subject line MUST show only exception badges: Pending, Merge conflict, Error, Blocked and Amend target.

#### Scenario: The badges of the subject line
- **WHEN** a commit row shows a badge on its subject line
- **THEN** it is one of Pending, Merge conflict, Error, Blocked and Amend target

### Requirement: Row anatomy: A commit the remote lacks shows the push arrow

In the commits of a branch, a commit the remote lacks MUST show the push arrow after its subject (`PushUpToMark`): a
click opens Push Up to Here of it, not the selection of the row
([Stacked Branches](../../branch/stacked-branches/spec.md#push-up-to-here)); a commit the remote has, or one a merged
Safe Push landed, has none (`offersPushUpTo`).

#### Scenario: A click on the push arrow
- **GIVEN** a commit of a branch that the remote lacks
- **WHEN** the user clicks the push arrow after its subject (`PushUpToMark`)
- **THEN** Push Up to Here of it opens, not the selection of the row
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`)

#### Scenario: Commits a merged Safe Push landed have no arrow
- **WHEN** a commit is one the remote has, or one a merged Safe Push landed (`offersPushUpTo`)
- **THEN** it has no push arrow
- **AND** a commit added after them has one
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should show no push arrow on the commits a merged Safe Push landed, only on a commit added after them`)

### Requirement: Row anatomy: The button of Push Up to Here tints the commits it sends

While the button of an open Push Up to Here is hovered, the rows of the commits it sends MUST be tinted (`data-sent`).

#### Scenario: The hover of the button of Push Up to Here
- **GIVEN** Push Up to Here of a commit is open
- **WHEN** the pointer hovers its button
- **THEN** the rows of the commits it sends are tinted (`data-sent`)
  [@test] tests/e2e/stacked-branches/push-up-to.spec.ts (`should push the stack up to a commit, the branches it is built on first, the rest left to push`)

### Requirement: Row anatomy: A commit with a merge conflict shows its badge

A commit with a merge conflict MUST show the "Merge conflict" badge.

#### Scenario: A commit with a merge conflict
- **WHEN** a commit has a merge conflict
- **THEN** its row shows the "Merge conflict" badge
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`)

### Requirement: Row anatomy: The Amend target badge explains itself in a tooltip

The Amend target badge MUST explain its meaning in a tooltip.

#### Scenario: The tooltip of the Amend target badge
- **WHEN** the pointer rests on the Amend target badge
- **THEN** a tooltip explains its meaning

### Requirement: Row anatomy: The first commit is the Amend target while the form is open

The first commit of a branch MUST show the Amend target badge while the amend form is open.

#### Scenario: The amend form is open
- **WHEN** the amend form of a branch is open
- **THEN** the first commit of the branch shows the Amend target badge
  [@test] tests/e2e/amend-changes/amend-changes.spec.ts (`should mark the first commit as the amend target while the form is open`)

### Requirement: Subject prefix: A conventional-commit prefix stays in the subject

A conventional-commit prefix (`feat:`, `feat(model):`, `fix!:`, `docs air:`) MUST stay in the subject.

#### Scenario: A subject with a conventional-commit prefix
- **WHEN** a subject starts with a conventional-commit prefix (`feat:`, `feat(model):`, `fix!:`, `docs air:`)
- **THEN** the prefix stays in the subject, split off the rest of it
  [@test] src/lib/commit.test.ts (`splits the conventional-commit prefix off %j`)

### Requirement: Subject prefix: A grouping prefix shows without its parentheses

A grouping prefix `(feature-auth)` MUST show as `feature-auth`, without the parentheses.

#### Scenario: A subject with a grouping prefix
- **WHEN** a subject starts with the grouping prefix `(feature-auth)`
- **THEN** it shows as `feature-auth`, without the parentheses
  [@test] src/lib/commit.test.ts (`returns a grouping prefix without the parentheses`)

### Requirement: Subject prefix: A subject without a prefix shows unchanged

A subject without a prefix MUST show unchanged.

#### Scenario: A subject without a prefix
- **WHEN** a subject has no prefix
- **THEN** it shows unchanged
  [@test] src/lib/commit.test.ts (`keeps %j without a prefix`)

### Requirement: Subject prefix: The prefix shows dimmed before the subject text

The row MUST show the prefix dimmed, before the subject text.

#### Scenario: A row of a subject with a prefix
- **WHEN** a row shows a subject with a prefix
- **THEN** the prefix is dimmed, before the subject text
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should dim a conventional-commit prefix of the subject`)

### Requirement: Subject prefix: A row shows the stripped subject the backend supplies

A row MUST show the stripped subject when the backend supplies one.

#### Scenario: The backend supplies a stripped subject
- **WHEN** the backend supplies the stripped subject of a commit
- **THEN** its row shows the stripped subject
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`, `should expand/collapse file accordions in missing commits details`)

### Requirement: Commit card: A row shows the mark of its card only for a message with a body

A row MUST show the mark of its card only when its message has a body, as Rust tells it (`hasBody` of a commit: a
block after the subject other than the trailers); a body of trailers alone (`Co-Authored-By:` of an agent) has no
mark.

#### Scenario: A message with a block after its subject
- **WHEN** the message of a commit has a block after the subject other than the trailers
- **THEN** Rust tells that it has a body (`hasBody` of a commit)
- **AND** its row shows the mark of its card
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_message_has_a_body_with_a_block_after_its_subject_that_is_no_trailers`)

#### Scenario: A message of its subject and its trailers alone
- **WHEN** the message of a commit is its subject alone, or its subject and a body of trailers alone (`Co-Authored-By:`
  of an agent)
- **THEN** it has no body, and its row has no mark
  [@test] crates/git-ops/src/commit_list_test.rs (`a_commit_has_a_body_unless_its_message_is_its_subject_and_its_trailers`)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should say No description above the trailers of a body of trailers alone, which has no mark`, `should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape`)

### Requirement: Commit card: The page reads no grammar of the body

The page MUST NOT read a grammar of its own to tell whether a message has a body; it takes `hasBody` from Rust.

#### Scenario: The page decides on the mark
- **WHEN** the page decides whether a row shows the mark of its card
- **THEN** it reads `hasBody` of the commit, no grammar of its own

### Requirement: Commit card: The mark is named and its click opens the preview

The mark MUST be named "Show full commit message", and a click on it MUST open the preview of its commit.

#### Scenario: A click on the mark
- **WHEN** the user clicks the mark "Show full commit message" of a row
- **THEN** the preview of its commit opens
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)

### Requirement: Commit card: The hover of the mark opens the card

The hover of the mark (300 ms, only after a move of the pointer) MUST open the commit card, under the whole row so its
meta line stays in view; the pointer leaving the mark closes it.

#### Scenario: The pointer rests on the mark and leaves it
- **WHEN** the pointer moves onto the mark of a row and rests there 300 ms
- **THEN** the commit card opens under the whole row, its meta line in view
- **AND** the pointer leaving the mark closes it
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape`)

### Requirement: Commit card: The card shows the subject, the meta line and the body folded

The commit card MUST show the whole subject over the meta line with its line counts, then the body folded as the
opening of a review folds it (the parts Rust shows, the author's "Look closely:" always, then one line that names the
folded parts, verification among them, each its own disclosure), its trailers last.

#### Scenario: The card of a short body
- **WHEN** the card of a commit opens
- **THEN** it shows the whole subject over the meta line with its line counts, then the body
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape`)

#### Scenario: The card of a long body
- **GIVEN** a long body with a "Look closely:" part and verification
- **WHEN** its card opens
- **THEN** the body shows folded as the opening of a review folds it: the parts Rust shows, the author's "Look closely:"
  always
- **AND** one line names the folded parts, verification among them, each its own disclosure
- **AND** its trailers come last
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should fold a long body in the card as the review folds it, open each folded part, keep the Look closely, and fade the card while more is below`)

### Requirement: Commit card: The hover of the subject opens the card too

The hover of the subject of the row MUST open the card too, after a longer rest (600 ms, only after a move), and the
pointer that leaves the subject without reaching the card closes it: the mark is the visible hint and the target of a
click, not the only way in.

#### Scenario: The pointer rests on the subject and leaves it
- **WHEN** the pointer moves onto the subject of a row and rests there 600 ms
- **THEN** the commit card opens
- **AND** the pointer that leaves the subject without reaching the card closes it
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape`)

### Requirement: Commit card: The bottom of the card fades while more is below

While more of the card is below its bottom edge, the bottom MUST fade into the popover, so a body the height cuts
never reads as the whole message.

#### Scenario: A body taller than the card
- **WHEN** more of the card is below its bottom edge
- **THEN** its bottom fades into the popover
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should fold a long body in the card as the review folds it, open each folded part, keep the Look closely, and fade the card while more is below`)

### Requirement: Commit card: I opens the card and Escape gives the focus back

I on the row Space would preview MUST open its card with the focus in it. Escape MUST close the card alone and give
the focus back to the list, whose keys go on from that row; a card opened by I that goes otherwise gives it back too.

#### Scenario: I and then Escape
- **GIVEN** the row Space would preview
- **WHEN** the user presses I
- **THEN** its card opens with the focus in it
- **AND** Escape closes the card alone and gives the focus back to the list, whose keys go on from that row
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape`)

#### Scenario: A card opened by I goes otherwise
- **WHEN** a card opened by I closes otherwise than by Escape
- **THEN** it gives the focus back to the list too

### Requirement: Commit card: The Escape of a hovered card keeps the selection

The Escape of a card the hover opened MUST close it alone too: the selection of the list stays.

#### Scenario: Escape of a card the hover opened
- **GIVEN** a card the hover opened and a selected row of the list
- **WHEN** the user presses Escape
- **THEN** the card closes alone and the selection of the list stays
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape`)

### Requirement: Commit card: The window keeps one card open

The window MUST keep one card open: a card that opens in one list closes the card of another.

#### Scenario: A card opens in another list
- **GIVEN** a card is open in one list
- **WHEN** a card opens in another list
- **THEN** the card of the first list closes, and one card stays open
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape`)

### Requirement: Commit card: The mark is no tab stop

The mark MUST NOT be a tab stop: Space previews the row, I opens its card.

#### Scenario: The keys of a row with a mark
- **WHEN** the user moves through a list from the keyboard
- **THEN** Tab never stops on the mark
- **AND** Space previews the row and I opens its card

### Requirement: Commit card: A commit of a review sidebar is one row

A commit on a row of a sidebar of a review MUST be one row (`CommitRow`, the `compact` header): the commit column and
the commits of a pull request clamp its subject to two lines; a commit row of the tree shows its whole title (the title
Rust gives it, as the tree and the openings name it, its subject without a plan).

#### Scenario: The commit column and the commits of a pull request
- **WHEN** a sidebar of a review shows a commit in the commit column or in the commits of a pull request
- **THEN** it is one row (`CommitRow`, the `compact` header) and its subject is clamped to two lines
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should make the row of a step its commit and its rounds rows of it, their titles whole at any width, and open the card of the commit by the hover of its mark or subject and by I`)

#### Scenario: A commit row of the tree
- **WHEN** the tree of a review shows a commit row
- **THEN** it shows its whole title, the title Rust gives it, as the tree and the openings name it, its subject without
  a plan
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should show every step and commit title of the tree whole at the default width, its files one line each`)

### Requirement: Commit card: A commit row of the tree has its subject and hash in its card

A commit row of the tree MUST show its subject and hash in its card, or in the native hover of a commit without a
body, never a native hover over a card.

#### Scenario: A commit with and without a body
- **WHEN** the pointer rests on a commit row of the tree
- **THEN** its subject and hash show in its card, or in the native hover of a commit without a body
- **AND** no native hover shows over a card

### Requirement: Commit card: A commit row of the tree says what it is since the review

A commit row of the tree MUST show what it is since the review ("New", "Rewritten", "Your review") and its branch.

#### Scenario: A commit on top since Mark Reviewed
- **WHEN** a commit came on top since the review
- **THEN** its row of the tree shows what it is since the review ("New", "Rewritten", "Your review") and its branch
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should mark the step of a commit on top since Mark Reviewed New, and the steps reviewed before nothing`)

### Requirement: Commit card: A commit row of a review has the mark and card of the main window

A commit row of a sidebar of a review MUST have the same mark and card as the main window, which the hover of the mark
or of the subject and `I` on the focused row open
([Review Tour](../../branch/review-tour/spec.md#in-the-review-window)).

#### Scenario: The card of a commit row of a review
- **WHEN** the pointer rests on the mark or the subject of a commit row of a review, or the user presses `I` on the
  focused row
- **THEN** the same card as in the main window opens
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should make the row of a step its commit and its rounds rows of it, their titles whole at any width, and open the card of the commit by the hover of its mark or subject and by I`)

### Requirement: Commit card: The heading of a review of one commit carries the mark

A review of one commit lists it in no row (a range of one commit, a pull request of one commit; its tree lists its
tasks, or its files for a commit of one task, whose heading tells its whole body): the heading of the commit at the top
of its overview MUST then carry the same mark and card as its row anywhere else (`CommitRowMark`), after its subject.

#### Scenario: The heading of a range of one commit
- **WHEN** a review shows a range of one commit
- **THEN** the heading of the commit at the top of its overview carries the mark of its card (`CommitRowMark`), after its
  subject
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should give the heading of a range of one commit the mark of its card, which the hover and I open, Escape giving the focus back`)

#### Scenario: A range of one commit of one task
- **WHEN** a review shows a range of one commit of one task, whose tree lists its files
- **THEN** its heading tells its whole body and its mark opens its card
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell the whole body of a range of one commit of one task in its heading, whose mark opens its card, name a removed declaration after the heading of its block, and give a commit without a body no mark and no tab stop`)

#### Scenario: The heading of a pull request of one commit
- **WHEN** a review shows a pull request of one commit
- **THEN** the heading of its commit carries the mark of its card
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should give the heading of the commit of a pull request of one commit the mark of its card, which the hover and I open, Escape giving the focus back`)

### Requirement: Commit card: The hover and I open the card of the heading

On the heading of a review of one commit, the hover of the mark or of the subject MUST open the card under the subject
and the meta line, I on the focused heading (a tab stop) opens it with the focus in it, and Escape closes it and gives
the focus back to the heading.

#### Scenario: The hover, I and Escape on the heading
- **WHEN** the pointer rests on the mark or the subject of the heading
- **THEN** the card opens under the subject and the meta line
- **AND** I on the focused heading opens it with the focus in it, and Escape closes it and gives the focus back to the
  heading
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should give the heading of a range of one commit the mark of its card, which the hover and I open, Escape giving the focus back`)
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should give the heading of the commit of a pull request of one commit the mark of its card, which the hover and I open, Escape giving the focus back`)

### Requirement: Commit card: The card of the heading stays as the diffs load

The card and the focus of the heading MUST stay as the overview moves into the diffs while they load.

#### Scenario: The diffs load under an open card
- **GIVEN** the card of the heading is open, or the heading has the focus
- **WHEN** the overview moves into the diffs while they load
- **THEN** the card and the focus stay

### Requirement: Commit card: A heading without a body has no mark

A commit without a body MUST have no mark on its heading, and the heading MUST NOT be a tab stop.

#### Scenario: The heading of a commit without a body
- **WHEN** a review of one commit shows a commit without a body
- **THEN** its heading has no mark and is no tab stop
  [@test] tests/e2e/branch-review/review-tour.spec.ts (`should tell the whole body of a range of one commit of one task in its heading, whose mark opens its card, name a removed declaration after the heading of its block, and give a commit without a body no mark and no tab stop`)

### Requirement: Commit card: Every header with a meta line shows the menu of the commit

Every header of a commit with a meta line (`row` aside: the card, the heading of a commit shown alone, the title of the
preview) MUST show the "⋯" of the commit at the end of its meta line, the menu the right-click of its row opens: one
place, wherever the subject ends.

#### Scenario: The menu at the end of the meta line of the card
- **WHEN** the card of a commit opens
- **THEN** the "⋯" of the commit is at the end of its meta line, the menu the right-click of its row opens
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the commit card on the hover of the mark, of the subject and by I, with the menu of the commit, and give the focus back on Escape`)

### Requirement: Commit card: The menu of a header is built as it opens

The items of the "⋯" of a header MUST be built as it opens, of the selection then, as the right-click builds them (a
selected row acts on the whole selection); no row builds its menu as it renders.

#### Scenario: The menu of the card acts on the selection
- **GIVEN** two selected rows, selected after the menus of their cards were rendered
- **WHEN** the user opens the "⋯" of the card of one of them
- **THEN** its items act on the whole selection, as the right-click of its row does
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should act from the menu of the card on the selection as the right-click of its row does`)

### Requirement: Commit card: One card for every place a commit is shown

The card MUST be one for every place a commit is shown (`CommitCard`: `CommitHeader` and `CommitBodyView`): the card of
a row's mark, the commits of the preview of a branch and of a kept branch, the commit a review shows alone; the commit
preview shows the same header as its title and the same body, whole.

#### Scenario: The places that show a commit
- **WHEN** a commit shows in the card of a row's mark, the preview of a branch or of a kept branch, or alone in a review
- **THEN** it shows as `CommitCard`: `CommitHeader` and `CommitBodyView`
- **AND** the commit preview shows the same header as its title and the same body, whole

### Requirement: Commit card: One view of a body

There MUST be one view of a body (`BodyPartsView`): the card, the preview and the opening of a step in a review set the
same parts, labels and callouts, folded or whole ([Review Tour](../../branch/review-tour/spec.md#in-the-review-window)).

#### Scenario: The same body in the card, the preview and a review
- **WHEN** the card, the preview and the opening of a step in a review show the same body
- **THEN** they set the same parts, labels and callouts (`BodyPartsView`), folded or whole

### Requirement: Commit card: The row does not render the body

The row MUST NOT render the body.

#### Scenario: A row of a commit with a body
- **WHEN** a row shows a commit with a body
- **THEN** the row does not render the body

### Requirement: Commit card: The card is a popover under the whole row

The card MUST be a popover under the whole row of its mark, at the start of the row (above it when there is no room
below), never over the meta line of the row (its hash, files and line counts), 36rem wide at most and 24rem high at
most; its body scrolls under its header, which stays at the top of the card, and a 40px gradient of `popover` fades its
bottom while more is below.

#### Scenario: The place and size of the card
- **WHEN** the card of a row opens
- **THEN** it is a popover under the whole row of its mark, at the start of the row, or above it when there is no room
  below
- **AND** it never covers the meta line of the row (its hash, files and line counts), and is 36rem wide at most and
  24rem high at most

#### Scenario: A body taller than the card
- **WHEN** the body is taller than the card
- **THEN** it scrolls under its header, which stays at the top of the card
- **AND** a 40px gradient of `popover` fades its bottom while more is below

### Requirement: Commit card: The header of the card

The header MUST show the whole subject in `text-sm font-medium` above the meta line, the line counts coloured
(`success`, `destructive`), as every header but a row's (`CommitHeader`: `compact` and `row`, `card`, `heading` in
`text-md`, `title` in `text-base`), the "⋯" of the commit at the end of the meta line.

#### Scenario: The header of a card
- **WHEN** the card shows its header
- **THEN** it shows the whole subject in `text-sm font-medium` above the meta line, the line counts coloured (`success`,
  `destructive`), the "⋯" of the commit at the end of the meta line

#### Scenario: The kinds of headers
- **WHEN** a header of a commit shows (`CommitHeader`)
- **THEN** it is one of `compact` and `row`, `card`, `heading` in `text-md`, `title` in `text-base`
- **AND** every header but a row's colours its line counts

### Requirement: Commit card: The body of the card

The body of the card MUST be `text-sm` (`card`), its sections 12px apart, preformatted text `text-xs`; the trailers
below a rule as in the preview.

#### Scenario: The body in the card
- **WHEN** the card shows a body
- **THEN** it is `text-sm` (`card`), its sections 12px apart, preformatted text `text-xs`
- **AND** the trailers are below a rule as in the preview

### Requirement: Commit preview: The preview shows the whole message, then the files

The preview MUST show the whole subject, the meta line and the body, then the changed files
([Commit Files](../commit-files/spec.md)).

#### Scenario: The preview of a commit with a body
- **WHEN** the preview of a commit opens
- **THEN** it shows the whole subject, the meta line and the body
- **AND** the changed files after them
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)

### Requirement: Commit preview: Space opens the preview of the row under the pointer

Space MUST open the preview of the commit row under the pointer.

#### Scenario: Space over a commit row
- **WHEN** the user presses Space with the pointer over a commit row
- **THEN** the preview of that commit opens
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the hovered commit with Space and step through the list with the arrows`)

### Requirement: Commit preview: The arrows step through the same list

↑ and ↓ MUST show the previous and the next commit of the same list, and the preview MUST show the position as `2 / 3`.

#### Scenario: Stepping through the list
- **GIVEN** the preview of a commit of a list is open
- **WHEN** the user presses ↑ or ↓
- **THEN** the preview shows the previous or the next commit of the same list
- **AND** it shows the position as `2 / 3`
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the hovered commit with Space and step through the list with the arrows`)

### Requirement: Commit preview: A commit without a body says No description

The preview of a commit without a body MUST show its subject and "No description"; of a body of trailers alone, "No
description" above its trailers, which every place that shows the body lists last.

#### Scenario: A commit of its subject alone
- **WHEN** the preview of a commit without a body opens
- **THEN** it shows its subject and "No description"
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the hovered commit with Space and step through the list with the arrows`)

#### Scenario: A body of trailers alone
- **WHEN** the preview of a commit whose body is trailers alone opens
- **THEN** it shows "No description" above its trailers
- **AND** every place that shows the body lists the trailers last
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should say No description above the trailers of a body of trailers alone, which has no mark`)

### Requirement: Commit preview: Space and Escape close the preview

Space and Escape MUST close the preview.

#### Scenario: Closing the preview from the keyboard
- **GIVEN** the preview is open
- **WHEN** the user presses Space or Escape
- **THEN** the preview closes
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the hovered commit with Space and step through the list with the arrows`)

### Requirement: Commit preview: Without a row under the pointer, Space previews the last clicked row

Without a row under the pointer, Space MUST open the preview of the last clicked row of the focused list.

#### Scenario: Space with the pointer elsewhere
- **GIVEN** a row of the focused list was clicked last
- **WHEN** the user presses Space without a row under the pointer
- **THEN** the preview of the last clicked row of the focused list opens
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the last clicked unassigned row with Space and keep the selection`)

### Requirement: Commit preview: The preview does not change the selection

The preview MUST NOT change the selection.

#### Scenario: The selection across a preview
- **GIVEN** a selection in a list
- **WHEN** the preview opens and closes
- **THEN** the selection is as it was
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the last clicked unassigned row with Space and keep the selection`)

### Requirement: Commit preview: D reviews the commit of a row

D over a row SHALL review its commit, as Space opens its preview: a commit of a branch in the review window of its
branch, at the commit, another one in the review window of the commit; the row's menu starts with "Review Changes"
(D) ([Commit Review](../commit-review/spec.md)).

#### Scenario: D over a commit of a branch
- **WHEN** the user presses D over a commit row of a branch
- **THEN** the review window of its branch opens at the commit
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a row in its window with D and with the item of its menu`)

#### Scenario: D over another commit
- **WHEN** the user presses D over a row of a commit of no branch
- **THEN** the review window of the commit opens

#### Scenario: The first item of the row's menu
- **WHEN** the user opens the menu of a commit row
- **THEN** it starts with "Review Changes" (D), which reviews the commit as D does
  [@test] tests/e2e/commit-files/commit-review.spec.ts (`should review a commit of a row in its window with D and with the item of its menu`)

### Requirement: Commit preview: Space keeps its meaning in a text field, a menu and a dialog

Space MUST keep its own meaning in a text field, a menu and another dialog.

#### Scenario: Space in a text field
- **WHEN** the user presses Space in a text field, a menu or another dialog
- **THEN** Space keeps its own meaning there and opens no preview
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should keep Space in a text field`)

### Requirement: Commit preview: A focused button keeps Space

A focused button MUST keep Space unless the pointer moved after the button got the focus.

#### Scenario: A button focused from the keyboard
- **WHEN** the user presses Space on a button focused from the keyboard, the pointer still
- **THEN** the button keeps Space
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should leave Space to a button focused from the keyboard`)

#### Scenario: The pointer moved after the focus
- **GIVEN** a focused button
- **WHEN** the pointer moved over a commit row after the button got the focus and the user presses Space
- **THEN** Space opens the preview of the row
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the preview of the hovered commit with Space and step through the list with the arrows`)

### Requirement: Commit preview: Space over a nested row opens one preview

Space over a row of a list nested in a conflict view MUST open one preview, for the nested row.

#### Scenario: A missing commit nested in a conflict row
- **WHEN** the user presses Space over a row of a list nested in a conflict view
- **THEN** one preview opens, for the nested row
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should open one preview for a missing commit nested in a conflict row`)

### Requirement: Commit preview: The preview keeps its commit across a sync

The preview MUST keep showing its commit after a sync.

#### Scenario: A sync under an open preview
- **GIVEN** the preview of a commit is open
- **WHEN** a sync keeps the commit
- **THEN** the preview keeps showing it
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should keep the preview on its commit across a sync`)

### Requirement: Commit preview: The preview closes when a sync removes its commit

The preview MUST close when a sync removes its commit.

#### Scenario: A sync removes the commit of the preview
- **GIVEN** the preview of a commit is open
- **WHEN** a sync removes the commit
- **THEN** the preview closes
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should close the preview when a sync removes its commit`)

### Requirement: Commit preview: The preview is a centered dialog of a bounded size

The preview MUST be a centered dialog on the blurred backdrop, at most 56rem wide and `min(80vh, 48rem)` high.

#### Scenario: The dialog of the preview
- **WHEN** the preview opens
- **THEN** it is a centered dialog on the blurred backdrop, at most 56rem wide and `min(80vh, 48rem)` high

### Requirement: Commit preview: The header shows the whole subject

The header MUST show the whole subject in `text-base font-medium` (16/24px), the title of the dialog, without the
two-line clamp, above the meta line. A `border-border` rule separates it.

#### Scenario: The header of the preview
- **WHEN** the preview shows a commit
- **THEN** its header shows the whole subject in `text-base font-medium` (16/24px), the title of the dialog, without the
  two-line clamp, above the meta line
- **AND** a `border-border` rule separates it

### Requirement: Commit preview: Only the body scrolls, and it has the focus

Only the body MUST scroll. The body MUST have the focus, so PageUp and PageDown scroll it. ↑ and ↓ belong to the
navigation.

#### Scenario: The keys of a long body
- **WHEN** the body of the preview is longer than the dialog
- **THEN** only the body scrolls, and PageUp and PageDown scroll it, as it has the focus
- **AND** ↑ and ↓ step through the list

### Requirement: Commit preview: A newly shown commit starts at the top

A newly shown commit MUST start at the top of its body.

#### Scenario: Stepping to another commit
- **GIVEN** the body of the preview is scrolled
- **WHEN** the preview shows another commit
- **THEN** it starts at the top of its body

### Requirement: Commit preview: The footer shows the position and the key hints

The footer MUST show the position on the left and the `Kbd` hints `↑ ↓ Previous/next` and `Space Close` on the right,
in `text-xs text-muted-foreground` below a `border-border` rule. A list of one commit has no `↑ ↓` hint.

#### Scenario: The footer of a list of several commits
- **WHEN** the preview of a commit of a list of several commits shows
- **THEN** its footer shows the position on the left and the `Kbd` hints `↑ ↓ Previous/next` and `Space Close` on the
  right, in `text-xs text-muted-foreground` below a `border-border` rule

#### Scenario: A list of one commit
- **WHEN** the preview of a list of one commit shows
- **THEN** its footer has no `↑ ↓` hint

### Requirement: Commit preview: The footer offers Review Changes and Files

The footer MUST then show the ghost `xs` buttons `D Review Changes`, which reviews the commit in its window and closes
the preview ([Commit Review](../commit-review/spec.md)), and `F Files`, which shows or hides the changed files.

#### Scenario: D Review Changes
- **WHEN** the user clicks `D Review Changes` in the footer
- **THEN** the commit is reviewed in its window and the preview closes

#### Scenario: F Files
- **WHEN** the user clicks `F Files` in the footer
- **THEN** the changed files show or hide

### Requirement: Commit preview: An editable preview ends the footer with Edit

An editable preview MUST end the footer with a ghost `xs` button `E Edit`, enabled during a sync too; "D Review
Changes" of a commit without files is disabled with "The commit changes no files".

#### Scenario: The footer of an editable preview
- **WHEN** the preview of a commit that can be edited shows, also during a sync
- **THEN** its footer ends with an enabled ghost `xs` button `E Edit`

#### Scenario: A commit without files
- **WHEN** the preview of a commit without files shows
- **THEN** "D Review Changes" is disabled with "The commit changes no files"

### Requirement: Commit preview: The header has the menu of the row

A preview of a list with a row menu MUST have "⋯" in its header, before the close button: the menu of the row (Move to…,
Squash into, Push Up to Here, Drop Commit…, the copies), its key hints only for the keys of the preview; an item that
opens a form of the row closes the preview first, a copy keeps it (`toPreviewMenu`).

#### Scenario: Move to from the preview
- **WHEN** the user chooses an item of the "⋯" of the preview that opens a form of the row
- **THEN** the preview closes first and the form of the row opens
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should offer the menu of the commit's row in its preview`)

#### Scenario: A copy from the preview
- **WHEN** the user chooses a copy in the "⋯" of the preview
- **THEN** the preview stays open

### Requirement: Commit preview: One builder of the menu of commits

One builder MUST list the menu of commits for the row, its preview and the commit in the review window of its branch
(`getCommitMenuGroups`), every item by a stable id
([Branch Review](../../branch/branch-review/spec.md#title-bar-footer-and-actions)).

#### Scenario: The menus of a commit
- **WHEN** the row, its preview or the commit in the review window of its branch opens the menu of a commit
- **THEN** `getCommitMenuGroups` lists it, every item by a stable id

### Requirement: Commit preview: The field of the edit grows with the message

While editing, the body MUST be a monospace 13/20px text field at least 12rem high that grows with the message.

#### Scenario: A long message in the edit
- **WHEN** the user edits a message
- **THEN** the body is a monospace 13/20px text field at least 12rem high that grows with the message

### Requirement: Commit preview: The footer of the edit

While editing, the footer MUST show "The first line is the subject", or the prefix warning in `text-warning`, on the
left, and `Esc Cancel` and a primary `xs` button `Save ⌘↵` (`Save Ctrl+Enter` outside macOS) on the right. The button
shows a spinner while saving.

#### Scenario: The footer while editing
- **WHEN** the user edits a message
- **THEN** the footer shows "The first line is the subject", or the prefix warning in `text-warning`, on the left
- **AND** `Esc Cancel` and a primary `xs` button `Save ⌘↵` (`Save Ctrl+Enter` outside macOS) on the right

#### Scenario: Saving
- **WHEN** the save runs
- **THEN** the button shows a spinner

### Requirement: Commit preview: The selection tint of a row stays two lines

The selection tint of a row MUST NOT grow with the message: the row stays two lines.

#### Scenario: A selected row with a long message
- **WHEN** a selected row has a long message
- **THEN** its selection tint stays two lines high, as the row

### Requirement: Message editing: The preview of a commit of the current branch offers the edit

The preview of a commit in a branch table or in the unassigned commits card MUST offer the edit: E or the Edit button.

#### Scenario: E in the preview of a commit of a branch
- **GIVEN** the preview of a commit in a branch table or in the unassigned commits card
- **WHEN** the user presses E or clicks the Edit button
- **THEN** the edit of its message starts
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should save the edited message and stay on the reworded commit`)

### Requirement: Message editing: Reword in the menu of a row opens the preview in edit

The menu of a commit row MUST offer "Reword…" (one commit), which opens the preview with the message in edit, also while
a sync or another rewrite runs: the save goes to the session at once, named on the table the preview shows, and waits
there (C14). Escape ends that edit as any other, the preview stays.

#### Scenario: Reword from the menu of a row
- **WHEN** the user chooses "Reword…" in the menu of a commit row
- **THEN** the preview of the commit opens with the message in edit
- **AND** Escape ends that edit as any other, and the preview stays
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should reword a commit from the menu of its row, in its preview`)

#### Scenario: Reword while a sync or another rewrite runs
- **GIVEN** a sync or another rewrite runs
- **WHEN** the user chooses "Reword…" and saves
- **THEN** the preview opens with the message in edit
- **AND** the save goes to the session at once, named on the table the preview shows, and waits there (C14)

### Requirement: Message editing: A commit off the current branch offers no edit

The preview of a commit that is not on the current branch (an archived or a missing commit) MUST NOT offer the edit.

#### Scenario: The preview of a missing commit
- **WHEN** the preview of an archived or a missing commit opens
- **THEN** it offers no edit
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should open one preview for a missing commit nested in a conflict row`)

### Requirement: Message editing: The edit shows the raw message in a focused field

The edit MUST show the raw message in a text field, the grouping prefix included, and focus the field.

#### Scenario: The edit starts
- **WHEN** the edit of a message starts
- **THEN** a text field shows the raw message, the grouping prefix included, and has the focus
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should save the edited message and stay on the reworded commit`)

### Requirement: Message editing: The save stays on the reworded commit

Cmd/Ctrl+Enter or Save MUST save the message. The preview MUST then show the reworded commit at the same position.

#### Scenario: Cmd/Ctrl+Enter saves
- **WHEN** the user presses Cmd/Ctrl+Enter or clicks Save in the edit
- **THEN** the message is saved
- **AND** the preview shows the reworded commit at the same position
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should save the edited message and stay on the reworded commit`)

### Requirement: Message editing: The branch tables show the new message after the save

After the save, the branch tables MUST show the new message.

#### Scenario: The table after a save
- **WHEN** a save of a new message is done
- **THEN** the branch tables show the new message
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should save the edited message and stay on the reworded commit`)

### Requirement: Message editing: Escape cancels the edit and keeps the preview

Escape MUST cancel the edit and keep the preview open.

#### Scenario: Escape in the edit
- **GIVEN** a changed message in the edit
- **WHEN** the user presses Escape
- **THEN** the edit is cancelled and the preview stays open on the commit as it was
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should cancel the edit with Escape and keep the preview open`)

### Requirement: Message editing: A click outside keeps the edit

A click outside MUST keep the edit.

#### Scenario: A click outside the edit
- **GIVEN** an edit is open
- **WHEN** the user clicks outside it
- **THEN** the edit stays

### Requirement: Message editing: A changed grouping shows a warning

A changed grouping MUST show a warning that names the new branch, or the unassigned commits.

#### Scenario: A new prefix or none
- **WHEN** the typed message groups into another branch, or into none
- **THEN** a warning names the new branch, or the unassigned commits
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should warn that a changed grouping prefix moves the commit, and move it`)

### Requirement: Message editing: Rust groups the message as it was and as typed

Rust MUST group the message as it was and as typed (`preview_group`: the subject as git reads it, by `CommitGrouper`'s
rule with the stacked branches, issue groups and stack parents of the last grouping; the typed one once the typing
pauses, again after a sync): an explicit prefix, else the issue group of the subject's key, else the key.

#### Scenario: The group of a subject
- **WHEN** Rust groups a subject (`preview_group`)
- **THEN** its group is its explicit prefix, else the issue group of the subject's key, else the key
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_of_a_subject_is_its_prefix_else_the_issue_group_of_its_key_else_the_key`)

#### Scenario: The typed message
- **WHEN** the user types a message
- **THEN** Rust groups the typed one once the typing pauses, and again after a sync

### Requirement: Message editing: With stacked branches the warning names the parent

With [stacked branches](../../branch/stacked-branches/spec.md) on, the warning MUST also name the parent and warn about a
restack and a stack problem; a stack problem the save would introduce (parents that conflict, an empty level) is said
to be refused.

#### Scenario: The parent of the new prefix
- **GIVEN** stacked branches are on
- **WHEN** the typed message groups into a stacked branch
- **THEN** the warning also names the parent, and warns about a restack
  [@test] crates/history-rewrite/src/commit_grouper_test.rs (`the_preview_with_stacked_branches_reads_the_parent_of_the_prefix_else_the_stack_parent`)

#### Scenario: A stack problem the save would introduce
- **GIVEN** stacked branches are on
- **WHEN** the typed message introduces a stack problem (parents that conflict, an empty level)
- **THEN** the warning warns about the stack problem and says that the save would be refused
  [@test] src/lib/stack.test.ts (`warns about a parent the other commits of the branch don't declare`, `warns about an empty level of the prefix, which a reword refuses`)

### Requirement: Message editing: A save that moves the commit closes the preview

A save that moves the commit to another list MUST close the preview.

#### Scenario: The commit leaves the list
- **WHEN** a save moves the commit to another list
- **THEN** the preview closes
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should warn that a changed grouping prefix moves the commit, and move it`)

### Requirement: Message editing: The edit stays enabled while a sync runs

The edit and the save MUST stay enabled while a sync runs: the reword goes to the session at once, named on the table
the preview shows, and waits there (C14).

#### Scenario: A save during a sync
- **GIVEN** a sync runs
- **WHEN** the user edits and saves a message
- **THEN** the reword goes to the session at once, named on the table the preview shows, and waits there (C14)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should keep the edit enabled while a sync runs: the save goes to the session at once`)

### Requirement: Message editing: A failed save keeps the draft

A failed save MUST keep the edit open with the draft and show the error in a toast.

#### Scenario: The save fails
- **WHEN** a save fails
- **THEN** the edit stays open with the draft
- **AND** a toast shows the error
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should refuse a reword that strands a fixup, and keep the draft`)

### Requirement: Message editing: A save of a commit changed meanwhile changes nothing

A save of a commit that changed meanwhile (reworded in a terminal) MUST change nothing: the session brings its table to
HEAD to find the commit, finds it neither by its id nor by its key, and refuses the reword as a changed plan ("Cannot
edit the commit message" with the reason); the preview of the commit that is gone closes, as after any sync that
removes it.

#### Scenario: The commit was reworded in a terminal
- **GIVEN** the commit of the edit was reworded in a terminal
- **WHEN** the user saves
- **THEN** the session brings its table to HEAD and finds the commit neither by its id nor by its key
- **AND** it refuses the reword as a changed plan, "Cannot edit the commit message" with the reason, and nothing changes
- **AND** the preview of the commit that is gone closes, as after any sync that removes it
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should save nothing when the commit changed meanwhile, say so, and close the preview of the commit that is gone`)

### Requirement: Message editing: E works in any keyboard layout

E MUST work in any keyboard layout (the physical key).

#### Scenario: Another keyboard layout
- **WHEN** the user presses the physical key E in another keyboard layout
- **THEN** the edit starts

### Requirement: Message editing: The save hint names the platform shortcut

The save hint MUST name the platform shortcut: `⌘↵` on macOS, `Ctrl+Enter` elsewhere.

#### Scenario: The save hint on each platform
- **WHEN** the edit shows its save hint
- **THEN** it names `⌘↵` on macOS and `Ctrl+Enter` elsewhere

### Requirement: Message editing: The save is the reword of brd reword

The save MUST be the reword of `brd reword` ([Agent CLI](../../cli/agent-cli/spec.md), `brd reword`): one regroup of
the commits ahead of the baseline, with its checks.

#### Scenario: Several commits reworded in one pass
- **WHEN** a reword names several commits ahead of the baseline
- **THEN** one regroup of the commits ahead of the baseline rewords them, with its checks
  [@test] crates/sync-core/src/reword_test.rs (`rewords_several_commits_in_one_pass`)

### Requirement: Message editing: The save replaces only the message

The save MUST replace only the message: the commits after it are recreated with the same trees, and the working tree
and the index stay as they are.

#### Scenario: A reword of a middle commit
- **WHEN** the user saves a new message of a commit with commits after it
- **THEN** the commits after it are recreated with the same trees
- **AND** the working tree and the index stay as they are
  [@test] crates/sync-core/src/reword_test.rs (`rewords_a_middle_commit_and_keeps_the_commits_after_it`)

### Requirement: Message editing: The save keeps the author and the committer

The save MUST keep the author and the committer of every recreated commit: name, email, date and timezone.

#### Scenario: The recreated commits
- **WHEN** a save recreates commits
- **THEN** each keeps its author and its committer: name, email, date and timezone
  [@test] crates/sync-core/src/reword_test.rs (`reword_preserves_committer`)

### Requirement: Message editing: The message is cleaned up as git commit does

The message MUST be cleaned up as `git commit` does: LF line endings, no trailing whitespace, no blank lines around.

#### Scenario: A message with CRLF, trailing whitespace and blank lines
- **WHEN** the saved message has CRLF line endings, trailing whitespace or blank lines around
- **THEN** the commit gets it with LF line endings, no trailing whitespace and no blank lines around
  [@test] crates/sync-core/src/reword_test.rs (`a_subject_keeps_the_body_and_a_message_equal_after_cleanup_rewrites_nothing`)

### Requirement: Message editing: An unchanged message keeps the commit

An unchanged message MUST keep the commit.

#### Scenario: A message equal after the cleanup
- **WHEN** the saved message is the same as before after the cleanup
- **THEN** the commit is kept, and nothing is rewritten
  [@test] crates/sync-core/src/reword_test.rs (`a_subject_keeps_the_body_and_a_message_equal_after_cleanup_rewrites_nothing`)

### Requirement: Message editing: A message without a subject line is refused

A message without a subject line MUST be refused.

#### Scenario: A blank message
- **WHEN** the saved message has no subject line
- **THEN** the save is refused and nothing is written
  [@test] crates/sync-core/src/reword_test.rs (`refuses_ids_and_texts_it_cannot_reword`)

### Requirement: Message editing: The save is refused during an operation in progress

The save MUST be refused while a rebase, merge, cherry-pick or revert is in progress, or on a detached HEAD.

#### Scenario: A merge in progress
- **WHEN** the user saves while a rebase, merge, cherry-pick or revert is in progress
- **THEN** the save is refused
  [@test] crates/sync-core/src/reword_test.rs (`refuses_to_reword_while_a_merge_is_in_progress`)

#### Scenario: A detached HEAD
- **WHEN** the user saves on a detached HEAD
- **THEN** the save is refused
  [@test] crates/sync-core/src/reword_test.rs (`rewrite_refused_on_detached_head`)

### Requirement: Message editing: The save is refused when the commit left the branch

The save MUST be refused if the commit left the current branch meanwhile (reworded or dropped in a terminal): "Commit
<short id> is no longer on the current branch, sync and try again".

#### Scenario: The commit was dropped in a terminal
- **GIVEN** the commit was reworded or dropped in a terminal
- **WHEN** the user saves
- **THEN** the save is refused: "Commit <short id> is no longer on the current branch, sync and try again"
  [@test] crates/sync-core/src/reword_test.rs (`refuses_a_commit_that_left_the_current_branch`)

### Requirement: Message editing: The core fails when the branch moved meanwhile

The core of the save MUST fail, and keep the new commit unreferenced, if the branch moved meanwhile (a commit made in a
terminal); the app's save then rewords the commit once more on that HEAD (below).

#### Scenario: A commit made in a terminal
- **GIVEN** the branch moved meanwhile, by a commit made in a terminal
- **WHEN** the core of the save writes its transaction
- **THEN** it fails and the new commit stays unreferenced
- **AND** the app's save rewords the commit once more on that HEAD
  [@test] crates/git-ops/src/ref_transaction_test.rs (`refuses_to_move_a_branch_that_moved_meanwhile`)

### Requirement: Message editing: A new prefix moves the fixups of the commit with it

A new prefix of a commit MUST move its fixups with it.

#### Scenario: A reword to a new prefix
- **GIVEN** a commit with fixups
- **WHEN** a save gives the commit a new prefix
- **THEN** its fixups move with it
  [@test] crates/sync-core/src/reword_test.rs (`a_new_prefix_moves_the_commit_with_its_fixups`)

### Requirement: Message editing: A save that strands a fixup is refused

The save MUST be refused, with the edit and its draft kept, when a fixup would fold into another target or none
afterwards, when it adds or removes a `fixup!`, `squash!` or `amend!` marker, when it gives a new id to a target that a
fixup names by its id, or when an `amend!` of the commit waits; the reason says to squash the fixups into their target
first (Squash Fixups).

#### Scenario: A fixup would fold into another target or none
- **WHEN** after the save a fixup would fold into another target or none
- **THEN** the save is refused, the edit and its draft kept
- **AND** the reason says to squash the fixups into their target first (Squash Fixups)
  [@test] tests/e2e/squash-fixups/squash-fixups.spec.ts (`should refuse a reword that strands a fixup, and keep the draft`)
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)

#### Scenario: A marker added or removed, a target named by its id, a waiting amend
- **WHEN** the save adds or removes a `fixup!`, `squash!` or `amend!` marker, gives a new id to a target that a fixup
  names by its id, or an `amend!` of the commit waits
- **THEN** the save is refused, nothing written
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_that_strands_a_fixup_is_refused`)

### Requirement: Message editing: A save that breaks a stack is refused

The save MUST be refused when the new prefix introduces a stack problem (parents that conflict, a cycle, an empty
level).

#### Scenario: A new prefix of a stack problem
- **WHEN** the new prefix introduces parents that conflict, a cycle or an empty level
- **THEN** the save is refused
  [@test] crates/sync-core/src/reword_test.rs (`a_reword_that_breaks_a_stack_is_refused`)

### Requirement: Message editing: The toast and Activity offer the Undo of a reword

After a save that changed the commit, a toast "Reworded <short id of the old commit>" with the new subject MUST offer
Undo for 10 s, and Activity MUST record the reword with the same Undo, which runs once.

#### Scenario: The Undo of the toast
- **WHEN** a save changed the commit
- **THEN** a toast "Reworded <short id of the old commit>" with the new subject offers Undo for 10 s
- **AND** past the window of the toast no Undo is offered
  [@test] src/hooks/useRewordCommit.test.ts (`records a reword with its Undo for the window of the toast, once`, `offers no Undo past the window of the toast`)

#### Scenario: The Undo of Activity
- **WHEN** a save changed the commit
- **THEN** Activity records the reword with the same Undo, which runs once
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should undo a reword from Activity`)
  [@test] src/hooks/useRewordCommit.test.ts (`records a reword with its Undo for the window of the toast, once`)

### Requirement: Message editing: The Undo moves HEAD back to what the save found

The Undo MUST move HEAD back to what the save found (the old messages, and with them the old virtual commits), as the
Undo of a move does (the rewrite `undo` of the engine).

#### Scenario: The Undo of a reword
- **WHEN** the user runs the Undo of a reword
- **THEN** HEAD moves back to what the save found, the old messages, and with them the old virtual commits
- **AND** it runs as the rewrite `undo` of the engine, as the Undo of a move does
  [@test] crates/sync-core/src/reword_test.rs (`undo_of_a_reword_brings_back_the_old_virtual_commits`)
  [@test] crates/test-server/src/reword_test.rs (`a_reword_answers_the_undo_that_brings_the_old_head_back`)

### Requirement: Message editing: The preview follows the Undo back

A click on the toast MUST keep the preview open; the preview on the reworded commit MUST follow the Undo back to the old
commit. A preview stepped to another commit stays there, and one on a commit that the Undo removes closes.

#### Scenario: Undo from the toast with the preview open
- **GIVEN** the preview is on the reworded commit
- **WHEN** the user clicks Undo on the toast
- **THEN** the preview stays open and follows the Undo back to the old commit
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should undo a reword from its toast and follow the preview back`)

#### Scenario: A preview on another commit
- **WHEN** the Undo runs while the preview was stepped to another commit, or is on a commit that the Undo removes
- **THEN** a preview stepped to another commit stays there, and one on a commit that the Undo removes closes

### Requirement: Message editing: The Undo is refused once the branch moved

The Undo MUST be refused, with nothing changed, once the branch moved after the save (another rewrite, a commit in a
terminal): "Cannot undo the reword" with the reason.

#### Scenario: The branch moved after the save
- **GIVEN** the branch moved after the save, by another rewrite or a commit in a terminal
- **WHEN** the user runs the Undo
- **THEN** it is refused with "Cannot undo the reword" and the reason, nothing changed
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should refuse to undo a reword after the branch moved`)

### Requirement: Message editing: A save that changed nothing is quiet

A save that changed nothing (the same message after the cleanup) MUST show no toast, record nothing and run no sync.

#### Scenario: An unchanged message saved
- **WHEN** the user saves the same message, after the cleanup
- **THEN** no toast shows, nothing is recorded and no sync runs
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should save an unchanged message without a toast`)
  [@test] src/hooks/useRewordCommit.test.ts (`records nothing for an unchanged message`)

### Requirement: Message editing: The save waits for the repository lock while its holder lives

The save MUST be the rewrite `reword` of the engine, named on the table the window shows, and run under the repository
lock: while another process holds it (an agent's `brd pick --follow`, `brd`) or the app's own push, the save waits as
long as the holder lives, the editor saying whom for (the step of its operation) with Stop Waiting
(`cancel_operation`), which ends the wait with nothing changed.

#### Scenario: A live holder of the lock
- **GIVEN** another process holds the repository lock (an agent's `brd pick --follow`, `brd`), or the app's own push
- **WHEN** the user saves
- **THEN** the save waits as long as the holder lives, the editor saying whom for (the step of its operation)
- **AND** it runs after the holder lets go
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_waits_for_the_holder`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_waits_while_a_live_holder_holds_the_lock_and_runs_after`)

#### Scenario: Stop Waiting
- **GIVEN** the save waits for the lock
- **WHEN** the user clicks Stop Waiting (`cancel_operation`)
- **THEN** the wait ends with nothing changed
  [@test] crates/operations/src/rewrite_test.rs (`a_rewrite_waits_while_the_holder_of_the_lock_lives_and_stops_when_told`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)

### Requirement: Message editing: A holder no record names refuses the save after 10 s

Only a holder of the repository lock no record names MUST refuse the save, after 10 s, with nothing changed and the
toast "Cannot edit the commit message" saying that another operation holds the lock.

#### Scenario: A holder without a record
- **GIVEN** a holder of the lock that writes no record
- **WHEN** the save waits 10 s
- **THEN** it is refused with nothing changed, the toast "Cannot edit the commit message" saying that another operation
  holds the lock
  [@test] crates/sync-core/src/repository_lock_test.rs (`a_rewrite_of_the_app_ends_its_wait_when_stopped_and_refuses_an_unnamed_holder`)
  [@test] crates/sync-core/src/repository_lock_test.rs (`every_rewrite_fails_with_locked_while_another_holds_the_lock`)

### Requirement: Message editing: A save behind another rewrite waits in the queue

A save while another rewrite runs (an Unstack from the main window, an issue reference of the same commit) MUST wait
behind it in the queue of the session and reword the commit as that one left it, its new id included.

#### Scenario: A reword and an Unstack from two windows
- **GIVEN** an Unstack from the main window runs
- **WHEN** the user saves a reword
- **THEN** the reword waits behind it and both are applied in order
  [@test] crates/operations/src/rewrite_test.rs (`a_reword_and_an_unstack_from_two_windows_at_once_are_both_applied_in_order`)

#### Scenario: A reword behind the issue reference of its commit
- **GIVEN** an issue reference of the same commit runs
- **WHEN** the user saves a reword of the commit
- **THEN** the reword waits behind it and names the commit through its new id
  [@test] crates/operations/src/rewrite_test.rs (`a_reword_queued_behind_the_issue_reference_of_its_commit_names_it_through_the_new_id`)

### Requirement: Message editing: A HEAD moved during the wait is resolved again

A HEAD another process moved while the save waited for the lock (an agent's `brd update`, a commit) MUST make the core
refuse it, nothing written, and the save is resolved again on that HEAD and rewords the commit as that process left it,
by its author, author time and subject without the prefix; a commit gone is `plan_changed`.

#### Scenario: Another process moved HEAD during the wait
- **GIVEN** the save waits for the lock of another process
- **WHEN** that process moves HEAD (an agent's `brd update`, a commit)
- **THEN** the core refuses the save, nothing written
- **AND** the save is resolved again on that HEAD and rewords the commit as that process left it, by its author, author
  time and subject without the prefix
  [@test] crates/operations/src/rewrite_test.rs (`an_unapply_and_a_reword_waiting_for_the_lock_of_another_process_run_on_the_head_it_left`, `a_rewrite_waiting_for_the_lock_of_another_process_runs_on_the_head_it_left`)

#### Scenario: The commit is gone
- **WHEN** the commit of the save is gone from that HEAD
- **THEN** the save fails with `plan_changed`

### Requirement: Message editing: The reword holds the lock only for the witness and the write

The reword MUST compute before the lock (the snapshot, the new commits on temporary refs) and hold it only for the
witness of HEAD and the store commits and the compare-and-swap transaction.

#### Scenario: The span of the lock of a reword
- **WHEN** a reword runs
- **THEN** it computes the snapshot and the new commits on temporary refs before the lock
- **AND** it holds the lock only for the witness of HEAD and the store commits and the compare-and-swap transaction
  [@test] crates/history-rewrite/src/regroup_test.rs (`a_reword_locks_only_the_witness_and_the_write`)

### Requirement: Message body: Rust reads the body, the one grammar of a commit body

The body MUST be read in Rust (`commit_body`, `sync_utils::commit_body`), the one grammar of a commit body: the commit
preview renders it, the body of a pull request is written from it ([Pull Requests](../../branch/pull-requests/spec.md)),
the description of a pull request to review is read by it ([Review Requests](../../repository/review-requests/spec.md)),
and a review tells it ([Review Tour](../../branch/review-tour/spec.md)).

#### Scenario: The places that read a body
- **WHEN** the commit preview renders a body, a pull request gets its body, a pull request to review shows its
  description, or a review tells a body
- **THEN** each reads it by `sync_utils::commit_body`, the one grammar of a commit body

### Requirement: Message body: The page asks once per message and only lays it out

The page MUST ask for the body once per message, keep the answer for a message shown again, and only lay out what Rust
read.

#### Scenario: A message shown again
- **GIVEN** the page asked for the body of a message
- **WHEN** the message is shown again
- **THEN** the page keeps the answer it had and asks no more
- **AND** it only lays out what Rust read

### Requirement: Message body: The body starts after the subject

The body MUST start at the first line after the subject that is not blank.

#### Scenario: Blank lines after the subject
- **WHEN** blank lines follow the subject
- **THEN** the body starts at the first line after the subject that is not blank
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)
  [@test] crates/sync-utils/src/commit_body_test.rs (`the_body_of_a_message_is_after_its_subject_line`)

### Requirement: Message body: Hard-wrapped lines join into one paragraph

The body MUST join hard-wrapped lines into one paragraph; a blank line MUST start a new block.

#### Scenario: A hard-wrapped paragraph and a blank line
- **WHEN** a body has hard-wrapped lines and a blank line
- **THEN** the lines join into one paragraph
- **AND** the blank line starts a new block
  [@test] crates/sync-utils/src/commit_body_test.rs (`joins_hard_wrapped_lines_into_one_paragraph_and_starts_a_block_after_a_blank_line`)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)

### Requirement: Message body: A hard break, a quote line and a pipe line stay a line

A line after a hard break (two spaces or `\` at the end), a quote line and a line that starts with `|` but is no row of
a table MUST stay a line of their own.

#### Scenario: Lines that stay their own
- **WHEN** a body has a line after a hard break (two spaces or `\` at the end), a quote line, or a line that starts with
  `|` but is no row of a table
- **THEN** each stays a line of its own
  [@test] crates/sync-utils/src/commit_body_test.rs (`keeps_a_line_after_a_hard_break_a_quote_line_and_a_pipe_line_of_no_table_its_own_and_a_heading_and_a_rule_a_block_of_their_own`)

### Requirement: Message body: A Markdown heading and a rule are blocks of their own

A Markdown heading (one to six `#` and a text) MUST be a heading of its level, a rule (`---`, `***`, `___`) a rule, each
a block of its own; a `#` without text stays prose.

#### Scenario: A heading and a rule
- **WHEN** a body has a line of one to six `#` and a text, or a rule (`---`, `***`, `___`)
- **THEN** it is a heading of its level, or a rule, each a block of its own
  [@test] crates/sync-utils/src/commit_body_test.rs (`keeps_a_line_after_a_hard_break_a_quote_line_and_a_pipe_line_of_no_table_its_own_and_a_heading_and_a_rule_a_block_of_their_own`)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should render a Markdown heading, a rule and a label glued to its list as sections of the body`)

#### Scenario: A hash without text
- **WHEN** a line is a `#` without text
- **THEN** it stays prose

### Requirement: Message body: Item lines form a list without their markers

Lines that start with `- `, `* `, `+ `, `• `, `1. ` or `1) ` MUST form a list without these markers (a rule is no item).
A change between bullets and numbers MUST start a new list. An ordered list MUST keep the number of its first item.

#### Scenario: Lists of bullets and of numbers
- **WHEN** lines start with `- `, `* `, `+ `, `• `, `1. ` or `1) `
- **THEN** they form a list without these markers, and a rule is no item
- **AND** a change between bullets and numbers starts a new list, and an ordered list keeps the number of its first item
  [@test] crates/sync-utils/src/commit_body_test.rs (`reads_lists_without_their_markers_each_kind_its_own`)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)

### Requirement: Message body: A continuation line stays in its list item

A continuation line, wrapped, indented or not, MUST stay in its list item until a heading, a quote, a line that starts
with `|`, a table, a rule or a label line right before an item.

#### Scenario: A wrapped list item
- **WHEN** a line continues a list item, wrapped, indented or not
- **THEN** it stays in its item until a heading, a quote, a line that starts with `|`, a table, a rule or a label line
  right before an item
  [@test] crates/sync-utils/src/commit_body_test.rs (`reads_lists_without_their_markers_each_kind_its_own`)

### Requirement: Message body: A table of GitHub Markdown is a table

A table of GitHub Markdown MUST be a table: a header row, then a delimiter row of as many cells, each dashes with a
colon at either end or none, then its rows up to the first line that is no row. It ends the paragraph or the list before
it. Lines of pipes without the delimiter row under the first one stay lines of prose.

#### Scenario: A table with its alignment
- **WHEN** a header row is followed by a delimiter row of as many cells, each dashes with a colon at either end or none
  (`---`, `:--` left, `--:` right, `:-:` centred: the alignment of its column)
- **THEN** it is a table with its rows up to the first line that is no row (a blank line, a line without a pipe, a list
  item, a heading, a quote), and it ends the paragraph or the list before it
- **AND** the pipes at the edges of a row are optional, `\|` is a pipe inside a cell, a short row is filled with empty
  cells and the cells past the header are dropped
  [@test] crates/sync-utils/src/commit_body_test.rs (`reads_a_table_its_header_its_alignment_and_its_rows_as_wide_as_the_header`)

#### Scenario: Lines of pipes without a delimiter row
- **WHEN** lines of pipes have no delimiter row under the first one
- **THEN** they stay lines of prose
  [@test] crates/sync-utils/src/commit_body_test.rs (`reads_a_table_its_header_its_alignment_and_its_rows_as_wide_as_the_header`)

### Requirement: Message body: The page renders a table as a table

The page MUST render a table as a table (`role=table`), the header row in the weight of a label, the rows apart by a
line of the border, a table wider than its column scrolling in it, each cell a line of prose with its code and issue
links; a table folds as a list does, a row a line.

#### Scenario: A table in the description of a pull request
- **WHEN** a body with a table shows
- **THEN** the table renders as a table (`role=table`), the header row in the weight of a label, the rows apart by a line
  of the border, a table wider than its column scrolling in it
  [@test] tests/e2e/review-requests/review-requests.spec.ts (`should show the description of the pull request as its Markdown: its headings and its tables`)

#### Scenario: The cells of a table
- **WHEN** a cell of a table holds code or an issue key
- **THEN** it is a line of prose with its code and issue links
  [@test] crates/sync-utils/src/commit_body_test.rs (`each_cell_of_a_table_carries_its_spans_of_code_by_its_place_row_by_row`)

#### Scenario: A table folds
- **WHEN** a folded body holds a table
- **THEN** the table folds as a list does, a row a line

### Requirement: Message body: An HTML comment is no text

An HTML comment (`<!-- … -->`, also over several lines: the hints of a template of a pull request) MUST be no text,
outside a fence, a line indented as code and a code span; an opening without a `-->` after it is a word. A line a
comment holds alone is blank, so the blocks around it stay apart.

#### Scenario: The hints of a template of a pull request
- **WHEN** a body holds an HTML comment, on one line or over several, outside a fence, a line indented as code and a
  code span
- **THEN** the comment is no text
- **AND** a line the comment holds alone is blank, so the blocks around it stay apart
  [@test] crates/sync-utils/src/commit_body_test.rs (`an_html_comment_is_no_text_outside_code`)

#### Scenario: An opening without a close
- **WHEN** a `<!--` has no `-->` after it
- **THEN** the opening is a word

### Requirement: Message body: Inline Markdown reads as its text

The inline Markdown of a line of prose, a heading or a cell MUST read as its text: an image (`![alt](url)`, an
`<img alt="…">`) as its alt text, a link (`[text](url)`) as its text, strong emphasis (`**text**`, `__text__`) without
its marks, so `**Why:** the text` is a run-in label.

#### Scenario: An image, a link and strong emphasis
- **WHEN** a line of prose, a heading or a cell holds an image (`![alt](url)`, an `<img alt="…">`), a link
  (`[text](url)`) or strong emphasis (`**text**`, `__text__`)
- **THEN** the image reads as its alt text, the link as its text, the strong emphasis without its marks
- **AND** `**Why:** the text` is a run-in label
  [@test] crates/sync-utils/src/commit_body_test.rs (`an_image_a_link_and_strong_emphasis_read_as_their_text`)

### Requirement: Message body: Code spans, escapes and marks around a name or a path stay as written

A code span, an escaped mark (`\*`), `__name__` around one identifier and marks around a path that starts or ends with
`/` (`**/generated/**`) MUST stay as written; a line that then says nothing (an image without an alt text) is no line.

#### Scenario: Marks that are no emphasis
- **WHEN** a line holds a code span, an escaped mark (`\*`), `__name__` around one identifier, or marks around a path
  that starts or ends with `/` (`**/generated/**`)
- **THEN** they stay as written
  [@test] crates/sync-utils/src/commit_body_test.rs (`an_image_a_link_and_strong_emphasis_read_as_their_text`)

#### Scenario: A line that says nothing
- **WHEN** a line says nothing once its inline Markdown reads as its text (an image without an alt text)
- **THEN** it is no line

### Requirement: Message body: The body of a pull request keeps the author's Markdown

The body of a pull request Branch Deck writes MUST keep the inline Markdown as the author wrote it.

#### Scenario: A pull request of a commit with inline Markdown
- **WHEN** Branch Deck writes the body of a pull request from a message with images, links and strong emphasis
- **THEN** the body keeps them as the author wrote them

### Requirement: Message body: Indented lines stay verbatim as code

Lines indented by four spaces or a tab MUST stay verbatim, without the common indent.

#### Scenario: An indented block
- **WHEN** lines are indented by four spaces or a tab
- **THEN** they stay verbatim, without the common indent
  [@test] crates/sync-utils/src/commit_body_test.rs (`keeps_code_verbatim_without_the_common_indent_and_the_fences`)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)

### Requirement: Message body: A fenced block stays verbatim

A fenced block (```` ``` ```` or `~~~`) MUST stay verbatim, blank lines included, without the fences; one without a
closing fence MUST run to the end of the body.

#### Scenario: A closed fence
- **WHEN** a body holds a fenced block (```` ``` ```` or `~~~`)
- **THEN** it stays verbatim, blank lines included, without the fences
  [@test] crates/sync-utils/src/commit_body_test.rs (`keeps_code_verbatim_without_the_common_indent_and_the_fences`)

#### Scenario: A fence without a close
- **WHEN** a fenced block has no closing fence
- **THEN** it runs to the end of the body

### Requirement: Message body: A last block of git trailers is the trailers

A last block of git trailers (`Token: value` lines, an indented line continuing one, `(cherry picked from commit …)`)
MUST be the trailers of the body, apart from its blocks, when one of its lines is a token of words joined by `-` or the
cherry-pick note; a block of one-word tokens alone (`Note: …`) and a block that is not the last stay prose. A body of
trailers alone has no block.

#### Scenario: A last block with a trailer token
- **WHEN** the last block is `Token: value` lines, an indented line continuing one, or `(cherry picked from commit …)`
- **AND** one of its lines is a token of words joined by `-` or the cherry-pick note
- **THEN** the block is the trailers of the body, apart from its blocks
- **AND** a body of trailers alone has no block
  [@test] crates/sync-utils/src/commit_body_test.rs (`reads_a_last_block_of_trailers_with_one_that_only_a_trailer_names`)

#### Scenario: Trailer-shaped blocks that stay prose
- **WHEN** a block is of one-word tokens alone (`Note: …`), or is not the last
- **THEN** it stays prose
  [@test] crates/sync-utils/src/commit_body_test.rs (`keeps_as_prose_a_trailer_shaped_block_that_is_not_the_last_one_has_prose_or_only_one_word_tokens`)

### Requirement: Message body: CRLF line endings read as LF

The body MUST treat CRLF line endings as LF.

#### Scenario: A body with CRLF line endings
- **WHEN** a body has CRLF line endings
- **THEN** it reads as with LF
  [@test] crates/sync-utils/src/commit_body_test.rs (`joins_hard_wrapped_lines_into_one_paragraph_and_starts_a_block_after_a_blank_line`)

### Requirement: Message body: A Label paragraph heads the blocks after it

A paragraph that is only `Label:` (a capital letter of any script, at most 39 characters without a colon) MUST become
the label of the blocks after it when a paragraph, a list, a table or code follows it.

#### Scenario: A label paragraph over its blocks
- **WHEN** a paragraph is only `Label:`, a capital letter of any script, at most 39 characters without a colon
- **AND** a paragraph, a list, a table or code follows it
- **THEN** it becomes the label of the blocks after it
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_paragraph_that_is_only_a_label_heads_the_next_blocks_and_a_short_run_in_label_the_rest_of_its_paragraph`)

### Requirement: Message body: A Label line heads the list or table right after it

A `Label:` line right before a list item or a table MUST become their label, also after prose or a list of its block.

#### Scenario: A label glued to its list
- **WHEN** a `Label:` line stands right before a list item or a table, also after prose or a list of its block
- **THEN** it becomes the label of the list or the table
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_label_line_right_before_a_list_heads_it_after_prose_or_another_list`)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should render a Markdown heading, a rule and a label glued to its list as sections of the body`)

### Requirement: Message body: A Label line heads the prose or code right after it

A `Label:` line that starts its block or follows a list MUST become a label right before a line of prose or code ("Why
not a setting:" over its paragraph with no blank line between), never inside prose, where the author wrapped a
sentence.

#### Scenario: A label line over its paragraph
- **WHEN** a `Label:` line starts its block or follows a list, right before a line of prose or code ("Why not a
  setting:" over its paragraph with no blank line between)
- **THEN** it becomes the label of that paragraph or code
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_label_line_that_starts_its_block_heads_the_paragraph_or_the_code_right_after_it`)

#### Scenario: A label-shaped line inside prose
- **WHEN** a line that ends with a colon stands inside prose, where the author wrapped a sentence
- **THEN** it stays prose

### Requirement: Message body: A short run-in Label heads its paragraph

A paragraph that starts with a short `Label: ` (a capitalised word and up to two lowercase ones of any script, at most
20 characters) MUST become a label above its remaining text, a label written at the start of its paragraph.

#### Scenario: A short run-in label
- **WHEN** a paragraph starts with a short `Label: `, a capitalised word and up to two lowercase ones of any script, at
  most 20 characters
- **THEN** it becomes a label above its remaining text
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_paragraph_that_is_only_a_label_heads_the_next_blocks_and_a_short_run_in_label_the_rest_of_its_paragraph`)

#### Scenario: A longer lead of a sentence
- **WHEN** a paragraph starts with words and a colon longer than a short label, or a sentence
- **THEN** it stays prose
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_label_without_blocks_to_head_a_long_one_and_a_sentence_stay_prose`)

### Requirement: Message body: A run-in Look closely heads its paragraph alone

A run-in `Look closely: text` MUST head that paragraph alone, with the list or the code written right under it (no
blank line between: "the guards against a false merge." over its list of guards): the blocks after a blank line are
the story again, a part of no label up to the next label, so the callout holds the author's question and nothing more.

#### Scenario: A Look closely with its list
- **WHEN** a paragraph starts with `Look closely: text` and a list or code is written right under it, no blank line
  between ("the guards against a false merge." over its list of guards)
- **THEN** the callout holds that paragraph and that list or code
- **AND** the blocks after a blank line are the story again, a part of no label up to the next label
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_run_in_look_closely_heads_its_paragraph_and_the_list_or_code_right_under_it`)

### Requirement: Message body: A Label paragraph without a block stays a paragraph

A `Label:` paragraph without a following block of content (the last one, one before the trailers or a heading) MUST
stay a paragraph.

#### Scenario: The last Label paragraph
- **WHEN** a `Label:` paragraph is the last one, or stands before the trailers or a heading
- **THEN** it stays a paragraph
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_label_without_blocks_to_head_a_long_one_and_a_sentence_stay_prose`)

### Requirement: Message body: The kind of a label comes from its words

A label's kind MUST come from its words, a leading "The", the case and what follows a comma aside: Tests, Test,
Evidence, Gates, Verified, Verification, Results and Proof (also "Gates on the final tree") are a verification, "Look
closely" asks the reviewer to decide, "Left out" says what the change does not do or prove, any other label is plain.

#### Scenario: The kinds of labels
- **WHEN** a label is Tests, Test, Evidence, Gates, Verified, Verification, Results or Proof (also "Gates on the final
  tree"), a leading "The", the case and what follows a comma aside
- **THEN** it is a verification
- **AND** "Look closely" asks the reviewer to decide, "Left out" says what the change does not do or prove, and any
  other label is plain
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_label_is_of_the_kind_its_words_name_the_case_a_leading_the_and_a_comma_aside`)

### Requirement: Message body: The trailers show last without a label

The trailers MUST show last, below the blocks, without a label.

#### Scenario: A body with trailers
- **WHEN** a body with trailers shows
- **THEN** the trailers come last, below the blocks, without a label
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)

### Requirement: Message body: The lead and the summary of a body

The lead of a body MUST be its first block when that is a paragraph, before any label or heading. The summary of a
body (`summary`, the block that sums it up in a line) MUST be its lead, else the first paragraph of a section a plain
label or a heading heads (a description that opens with `## Goal`); a verification, a Look closely or a Left out
section sums up nothing.

#### Scenario: The lead
- **WHEN** the first block of a body is a paragraph, before any label or heading
- **THEN** it is the lead of the body
  [@test] crates/sync-utils/src/commit_body_test.rs (`the_lead_is_a_paragraph_before_any_label_or_heading`)

#### Scenario: The summary without a lead
- **WHEN** a body has no lead (a description that opens with `## Goal`)
- **THEN** its `summary` is the first paragraph of a section a plain label or a heading heads
- **AND** a verification, a Look closely or a Left out section sums up nothing
  [@test] crates/sync-utils/src/commit_body_test.rs (`the_summary_is_the_lead_else_the_first_paragraph_of_a_plain_section`)

### Requirement: Message body: Each part of a body says what it is

The parts of a body (`parts`: the lead alone, a labelled section, a run of the other blocks) MUST each say what it is
(`kind`: `lead`, `section`, `run`; `tone`: the kind of the label of a section), so a view places them without the
grammar.

#### Scenario: The parts of a body
- **WHEN** Rust reads a body
- **THEN** its `parts` are the lead alone, each labelled section and each run of the other blocks
- **AND** each says its `kind` (`lead`, `section`, `run`) and a section its `tone`, the kind of its label
  [@test] crates/sync-utils/src/commit_body_test.rs (`each_part_says_whether_it_is_the_lead_a_section_with_the_kind_of_its_label_or_a_run`)

### Requirement: Message body: A body folds at a block after about 12 lines

Each part MUST also say whether a reader folds it, at a block, never at a fixed row count: the first part shows, a
labelled section whole when the body opens with one, then whole parts while they read in about 12 lines (a line of prose
or an item by 100 characters, a line of code one); a labelled section only whole, so a label never ends what shows.

#### Scenario: A long body
- **WHEN** a body reads in more than about 12 lines, a line of prose or an item by 100 characters, a line of code one
- **THEN** the first part shows, a labelled section whole when the body opens with one
- **AND** whole parts show while they read in about 12 lines, and every part after the first that does not fit folds
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_short_body_shows_whole_and_a_long_one_folds_at_a_block_after_about_twelve_lines`)

#### Scenario: A labelled section at the fold
- **WHEN** a labelled section (a label or a heading and its blocks up to the next one, a rule among them) reaches the
  fold
- **THEN** it shows only whole, so a label never ends what shows
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_fold_never_cuts_a_section_off_its_label_and_a_verification_section_costs_no_line`)

#### Scenario: A short body
- **WHEN** a body reads in about 12 lines
- **THEN** it shows whole
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_short_body_shows_whole_and_a_long_one_folds_at_a_block_after_about_twelve_lines`)

### Requirement: Message body: Verification folds, Look closely and Left out always show

A verification section MUST fold, and a "Look closely:" or a "Left out:" section MUST show wherever it is; none of them
costs a line: the decision the author asks for and what the author says the change does not do or prove are never
behind a fold.

#### Scenario: A verification section
- **WHEN** a body has a verification section
- **THEN** it folds and costs no line of the fold
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_fold_never_cuts_a_section_off_its_label_and_a_verification_section_costs_no_line`)

#### Scenario: A Look closely section
- **WHEN** a body has a "Look closely:" section anywhere
- **THEN** it shows wherever it is and costs no line of the fold
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_look_closely_section_shows_wherever_it_is_and_costs_no_line`)

#### Scenario: A Left out section
- **WHEN** a body has a "Left out:" section anywhere
- **THEN** it shows always and costs no line of the fold
  [@test] crates/sync-utils/src/commit_body_test.rs (`a_left_out_section_shows_always_and_costs_no_line_of_the_fold`)

### Requirement: Message body: Issue keys in the body are links

Issue keys in the body MUST become links.

#### Scenario: An issue key in the body
- **WHEN** a body holds an issue key
- **THEN** it is a link, as in the subject
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)

### Requirement: Message body: The width of the body

The body MUST be at most `max-w-prose` (65ch, about 80 characters) wide.

#### Scenario: A body in a wide preview
- **WHEN** the body shows in a preview wider than its text
- **THEN** it is at most `max-w-prose` (65ch, about 80 characters) wide

### Requirement: Message body: The reading text of the body

The body text MUST be `text-foreground` with the `ss04` font feature, the 14/22px reading text (`text-md`).

#### Scenario: The text of the body
- **WHEN** a body shows
- **THEN** its text is `text-foreground` with the `ss04` font feature, the 14/22px reading text (`text-md`)

### Requirement: Message body: The style of labels, headings and rules

A section label MUST be a `font-semibold` heading above its blocks in the text colour, as the story of a review sets
it; a "Look closely:" section is an info callout, its label the title; a Markdown heading of any level is
`font-semibold` at the size of the text; a rule is a `border-border` line of its own.

#### Scenario: A section label
- **WHEN** a body shows a section label
- **THEN** it is a `font-semibold` heading above its blocks in the text colour, as the story of a review sets it

#### Scenario: A Look closely section
- **WHEN** a body shows a "Look closely:" section
- **THEN** it is an info callout, its label the title

#### Scenario: A Markdown heading and a rule
- **WHEN** a body shows a Markdown heading of any level, or a rule
- **THEN** the heading is `font-semibold` at the size of the text, and the rule a `border-border` line of its own

### Requirement: Message body: The gaps of sections and blocks

Sections MUST have a 16px gap. Blocks in a section MUST have an 8px gap.

#### Scenario: Sections and their blocks
- **WHEN** a body shows several sections of several blocks
- **THEN** the sections are 16px apart and the blocks in a section 8px apart

### Requirement: Message body: The indent and markers of lists

Lists MUST have a 20px indent and muted markers.

#### Scenario: A list of the body
- **WHEN** a body shows a list
- **THEN** it has a 20px indent and muted markers

### Requirement: Message body: Preformatted text is a smaller monospace block that scrolls

Preformatted text MUST use `font-mono` one step below the body (`text-sm`) on `bg-muted` and scroll horizontally.

#### Scenario: A wide preformatted block
- **WHEN** a body shows preformatted text wider than the body
- **THEN** it is `font-mono` one step below the body (`text-sm`) on `bg-muted`, and scrolls horizontally

### Requirement: Message body: The style of the trailers

Trailers MUST use `text-xs text-muted-foreground` below a `border-border` rule.

#### Scenario: The trailers of a body
- **WHEN** a body shows trailers
- **THEN** they are `text-xs text-muted-foreground` below a `border-border` rule

### Requirement: Message body: Detected code is monospace at the size of the text

Detected code MUST use the monospace font at the size of the text, without a background: JetBrains Mono has the
x-height and cap height of Inter, so a smaller size only shrinks it.

#### Scenario: Code Rust detected in prose
- **WHEN** a line of prose shows code Rust detected
- **THEN** it is in the monospace font at the size of the text, without a background

### Requirement: Message body: Code in backticks gets a chip

Code in backticks MUST also get a `bg-muted` chip.

#### Scenario: Code in backticks
- **WHEN** a line of prose shows code in backticks
- **THEN** it is in the monospace font on a `bg-muted` chip

### Requirement: Message body: The text of the body is selectable

The body MUST show the text cursor, and its text MUST be selectable.

#### Scenario: The pointer over the body
- **WHEN** the pointer is over the body
- **THEN** it shows the text cursor, and the user can select the text

### Requirement: Code tokens: The code of a body is Rust's

The code of a body MUST be Rust's (`CommitBody.code`, `sync_utils::code_spans`): each line of its prose carries its
spans of code, which the card, the preview and the story of a review set alike (`CodeText`); the page MUST NOT find code
of its own.

#### Scenario: The spans of code of a line of prose
- **WHEN** Rust reads a body
- **THEN** each line of its prose carries its spans of code (`CommitBody.code`, `sync_utils::code_spans`)
- **AND** the card, the preview and the story of a review set them alike (`CodeText`), the page finding no code of its
  own
  [@test] crates/sync-utils/src/commit_body_test.rs (`each_line_of_prose_carries_its_spans_of_code`)

### Requirement: Code tokens: Text in backticks shows as a chip

Text in backticks MUST show as code without the backticks, a chip (`InlineCode`: `bg-muted` and a halo of the same
colour that takes no width, so "(Frame.paint)" never reads "( Frame.paint )").

#### Scenario: A span in backticks
- **WHEN** a line of prose holds text in backticks
- **THEN** it is explicit code, and shows without the backticks as a chip (`InlineCode`: `bg-muted` and a halo of the
  same colour that takes no width)
- **AND** "(Frame.paint)" never reads "( Frame.paint )"
  [@test] crates/sync-utils/src/code_spans_test.rs (`a_span_in_backticks_is_explicit_code_its_backticks_included`)
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)

### Requirement: Code tokens: Rust finds the code tokens of prose

Rust MUST find paths, file names, dotted module and package names of three or more parts, an `Owner.member` of a
capitalised owner (`Frame.paint`, `SplashScreen.hide`), flags, calls, `CONSTANT_CASE`, `snake_case`, `camelCase` and
`PascalCase` of two humps or more: only the code font, a chip only when Rust joined several tokens into one span
(`CodeSpan.joined`).

#### Scenario: Paths, names, flags and calls
- **WHEN** a line of prose holds a path, a file name, a dotted module or package name of three or more parts, an
  `Owner.member` of a capitalised owner (`Frame.paint`, `SplashScreen.hide`), a flag, a call, `CONSTANT_CASE`,
  `snake_case`, or `camelCase` and `PascalCase` of two humps or more
- **THEN** Rust finds it as code, shown in the code font only
- **AND** a chip only when Rust joined several tokens into one span (`CodeSpan.joined`)
  [@test] crates/sync-utils/src/code_spans_test.rs (`the_grammar_finds_paths_names_flags_and_calls`)

### Requirement: Code tokens: Words of prose that look like code stay prose

A dotted name of two parts led lowercase (`example.com`, `e.g.`), `GenUI`, `AssertJ`, a word with a hyphen
(`follow-up`) and the names of products (`JavaScript`, `GitHub`, `macOS`, `TeamCity`, `PhpStorm`, `WordPress`) and of
libraries (`Node.js`, `Vue.js`: a capitalised word before `.js`) MUST stay prose, and so does a token in double quotes.

#### Scenario: Names that are no code
- **WHEN** a line of prose holds a dotted name of two parts led lowercase (`example.com`, `e.g.`), `GenUI`, `AssertJ`,
  a word with a hyphen (`follow-up`), a name of a product (`JavaScript`, `GitHub`, `macOS`, `TeamCity`, `PhpStorm`,
  `WordPress`) or of a library (`Node.js`, `Vue.js`: a capitalised word before `.js`), or a token in double quotes
- **THEN** it stays prose
  [@test] crates/sync-utils/src/code_spans_test.rs (`a_word_of_prose_stays_prose`)

### Requirement: Code tokens: Preformatted blocks and trailers detect no code

Preformatted blocks and trailers MUST NOT detect code.

#### Scenario: A code token in a preformatted block or a trailer
- **WHEN** a preformatted block or a trailer holds a token Rust would find in prose
- **THEN** it detects no code in it
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should open the full message in a preview and reflow it into paragraphs, lists and code`)

### Requirement: Code tokens: Tokens a single space apart are one span

Code tokens that only single spaces separate MUST form one span, such as `bt.cmd --lane` or
`--module intellij.air.tests --filter com.intellij.air`.

#### Scenario: A command with its flags
- **WHEN** code tokens are separated only by single spaces, such as `bt.cmd --lane` or
  `--module intellij.air.tests --filter com.intellij.air`
- **THEN** they form one span
  [@test] crates/sync-utils/src/code_spans_test.rs (`the_punctuation_around_a_token_stays_prose_and_tokens_one_space_apart_are_one_span`)

### Requirement: Code tokens: Punctuation around a token stays prose

Punctuation around a token MUST stay prose, a possessive (`SplashScreen's`) and the dash of a sentence (`Frame.paint—`)
too.

#### Scenario: A token in a sentence
- **WHEN** punctuation, a possessive (`SplashScreen's`) or the dash of a sentence (`Frame.paint—`) touches a token
- **THEN** it stays prose
  [@test] crates/sync-utils/src/code_spans_test.rs (`the_punctuation_around_a_token_stays_prose_and_tokens_one_space_apart_are_one_span`)

### Requirement: Code tokens: An issue key inside a code token stays code

An issue key inside a code token MUST stay part of the code.

#### Scenario: A path with an issue key
- **WHEN** a code token holds an issue key
- **THEN** the key stays part of the code

### Requirement: Code tokens: The offsets are UTF-16 code units

The offsets of the spans MUST be UTF-16 code units, as the page counts.

#### Scenario: Characters of two code units before a span
- **WHEN** a line of prose holds characters of more than one UTF-16 code unit before a span
- **THEN** the offsets of the span count UTF-16 code units, as the page counts
  [@test] crates/sync-utils/src/code_spans_test.rs (`the_offsets_are_utf16_code_units`)

### Requirement: Code tokens: A long path breaks after each slash or before a dotted name

A long path MUST break after each `/` (keeping the slash with its directory, and `//` together) or before a dotted
name, never before a chain of short final extensions, and never inside a word: a code name set apart (the title of a
step, a link of a story, a name of a fact) breaks there too (`BreakableCode`).

#### Scenario: A path wider than its line
- **WHEN** a long path is wider than its line
- **THEN** it breaks after each `/`, keeping the slash with its directory
  [@test] src/lib/inline-code.test.ts (`breaks a path after each slash`, `keeps the path prefix and each directory slash with the directory`)

#### Scenario: Dotted names, extensions and words
- **WHEN** a long path or code name is wider than its line
- **THEN** it breaks before a dotted name, never before a chain of short final extensions, and never inside a word
- **AND** `//` stays together
  [@test] src/lib/inline-code.test.ts (`does not break before a short final extension`, `breaks before a dotted name but keeps a chain of short final extensions`, `keeps a leading ./ prefix together`, `does not break inside // or ...`, `keeps a CONSTANT_CASE name whole`)

#### Scenario: A code name set apart
- **WHEN** a code name is set apart: the title of a step, a link of a story, a name of a fact
- **THEN** it breaks at the same places (`BreakableCode`)

### Requirement: Code tokens: A long part of a title breaks at its humps

A title of a step or an entry of Possible Breakage MUST break a part of 24 characters or more at its camelCase humps
too, so a name wider than its line breaks between words.

#### Scenario: A long name in the title of a step
- **WHEN** a title of a step or an entry of Possible Breakage holds a part of 24 characters or more
- **THEN** it breaks at its camelCase humps too, and a shorter one only at its dots
  [@test] src/lib/inline-code.test.ts (`breaks a part too long for a line at its humps, a shorter one only at its dots`)

### Requirement: Code tokens: The punctuation that touches code stays on its line

The punctuation that touches code MUST stay on its line (`CodeGlue`): a line never starts with the comma or the period
after code, a link of code too, nor ends with the parenthesis before it.

#### Scenario: A comma after code at the end of a line
- **WHEN** a comma, a period or a closing mark follows code, or a link of code, at the end of a line
- **THEN** it stays on the line of the code, never the words after them
  [@test] src/lib/inline-code.test.ts (`keeps the closing marks and the end of a sentence after code on its line, never the words after them`)

#### Scenario: A parenthesis before code
- **WHEN** an opening parenthesis comes before code at the end of a line
- **THEN** it moves to the line of the code, never the words before it
  [@test] src/lib/inline-code.test.ts (`keeps an opening mark before code on its line, never the words before it`)

### Requirement: Code tokens: The subject detects no code

The subject MUST NOT detect code.

#### Scenario: A code token in the subject
- **WHEN** a subject holds a token Rust would find in prose
- **THEN** it shows as prose, not as code

### Requirement: Selection: A click on the mark or in the card keeps the selection

A click on the mark of a row or in its card MUST NOT change the selection.

#### Scenario: A click on the mark
- **WHEN** the user clicks the mark of a row, or in its card
- **THEN** the selection does not change
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)

### Requirement: Selection: A click in the preview body keeps the selection

A click in the preview body MUST NOT change the selection.

#### Scenario: A click on the text of the body
- **WHEN** the user clicks in the body of the preview, to read or select its text
- **THEN** the selection does not change
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)

### Requirement: Selection: A click on the row selects it

A click on another part of the row MUST select the row; a press of the selection's actions on the row MUST NOT change
the selection.

#### Scenario: A click on the rest of the row
- **WHEN** the user clicks a part of the row other than the mark
- **THEN** the row is selected
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should not change the row selection`)

#### Scenario: A press of the actions of the selection
- **GIVEN** selected rows
- **WHEN** the user presses an action of the selection on the row
- **THEN** the selection does not change
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should keep the rows in place as commits are selected and deselected`)

### Requirement: Selection: Cmd/Ctrl+click adds the row to the selection

Cmd/Ctrl+click MUST add the row to the selection.

#### Scenario: Cmd/Ctrl+click on a row
- **GIVEN** a selected row
- **WHEN** the user Cmd/Ctrl+clicks another row
- **THEN** that row is added to the selection
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should act from the menu of the card on the selection as the right-click of its row does`)

### Requirement: Selection: The lead row shows the count and Move to

The lead row of the selection (the row clicked last) MUST show "N selected" and a "Move to…" button at the end of its
meta line while rows are selected, after its other items and before a "⋯"
([Move Commits](../../branch/move-commits/spec.md)).

#### Scenario: Rows are selected
- **WHEN** rows are selected
- **THEN** the row clicked last shows "N selected" and a "Move to…" button at the end of its meta line, after its other
  items and before a "⋯"
  [@test] tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should verify commit selection states with ARIA snapshots`)

### Requirement: Selection: Enter opens the Move to form

Enter MUST open the inline "Move to…" form when rows are selected.

#### Scenario: Enter with rows selected
- **GIVEN** rows are selected
- **WHEN** the user presses Enter
- **THEN** the inline "Move to…" form opens
  [@test] tests/e2e/inline-branch-creator/inline-branch-creator.spec.ts (`should open with Enter key when commits are selected`)

### Requirement: Selection: The rows of one list at a time are selectable

The rows of the unassigned commits card and of an expanded branch MUST be selectable, one list at a time
(see [Move Commits](../../branch/move-commits/spec.md)).

#### Scenario: A selection in another list
- **GIVEN** a selection in one list
- **WHEN** the user selects a row of another list, the unassigned commits card or an expanded branch
- **THEN** the selection is of that list alone
  [@test] tests/e2e/commit-message/commit-message.spec.ts (`should select commit rows of a branch, one list at a time`)

### Requirement: Selection: Shift+click selects a range

Shift+click MUST select a range of rows.

#### Scenario: Shift+click on a row
- **GIVEN** a selected row
- **WHEN** the user Shift+clicks another row
- **THEN** the rows between them are selected
  [@test] tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should select a range of rows with Shift+click`)

### Requirement: Selection: Cmd/Ctrl+A selects all and Escape clears

Cmd/Ctrl+A MUST select all rows, and Escape MUST clear the selection.

#### Scenario: Select all and clear
- **WHEN** the user presses Cmd/Ctrl+A in a list
- **THEN** all its rows are selected
- **AND** Escape clears the selection
  [@test] tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should select all rows with Cmd/Ctrl+A and clear the selection with Escape`)

### Requirement: Selection: The menu of a selected row acts on the selection

The context menu of a selected row MUST act on all selected commits.

#### Scenario: Right-click on a selected row
- **GIVEN** several selected rows
- **WHEN** the user opens the context menu of one of them
- **THEN** its items act on all selected commits
  [@test] tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should act on all selected commits from the context menu of a selected row`)

### Requirement: Selection: The menu of a row not selected acts on it alone

The context menu of a row that is not selected MUST act on that commit only.

#### Scenario: Right-click on a row outside the selection
- **GIVEN** a selection of other rows
- **WHEN** the user opens the context menu of a row that is not selected
- **THEN** its items act on that commit only
  [@test] tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should act on that commit only from the context menu of a row that is not selected`)

### Requirement: Selection: The context menu keeps the selection

The context menu MUST NOT change the selection.

#### Scenario: Opening and closing the context menu
- **GIVEN** a selection
- **WHEN** the user opens the context menu of a row, selected or not, and closes it
- **THEN** the selection is as it was
  [@test] tests/e2e/unassigned-commits/unassigned-commits.spec.ts (`should act on that commit only from the context menu of a row that is not selected`, `should act on all selected commits from the context menu of a selected row`)

### Requirement: Selection: Drop Commit from the menu and from the keys

The context menu of commits MUST offer "Drop Commit…" ("Drop N Commits…" for a selection), and Backspace or Delete with
rows selected MUST open the same confirmation below the lead row ([Drop Commits](../drop/spec.md)).

#### Scenario: Drop Commit from the menu
- **WHEN** the user opens the context menu of a commit
- **THEN** it offers "Drop Commit…", "Drop N Commits…" for a selection
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop a commit and its file from its menu and bring them back with the Undo of its toast`)

#### Scenario: Backspace or Delete with rows selected
- **GIVEN** rows are selected
- **WHEN** the user presses Backspace or Delete
- **THEN** the same confirmation opens below the lead row
  [@test] tests/e2e/drop-commits/drop-commits.spec.ts (`should drop the selected commit with the key, and say which branch it leaves without a commit`)

### Requirement: Selection: The count shows alone while the Move to form is open

While the inline "Move to…" form is open, the card header MUST show "N selected" alone, as the line above the commits
of a branch does ([Move Commits](../../branch/move-commits/spec.md)).

#### Scenario: The Move to form is open
- **WHEN** the inline "Move to…" form is open
- **THEN** "N selected" shows alone, as the line above the commits of a branch does
  [@test] tests/e2e/move-commits/move-commits.spec.ts (`should keep the rows in place as commits are selected and deselected`)

### Requirement: Conflict view: A row with a merge conflict shows the viewer

A row with a merge conflict MUST show the conflict viewer below its header.

#### Scenario: A commit with a merge conflict
- **WHEN** a row has a merge conflict
- **THEN** the conflict viewer shows below its header

### Requirement: Conflict view: The viewer starts with its title, the explanation and the fix

The viewer MUST start with its title, the explanation and the fix of [Conflict Fix](../../branch/conflict-fix/spec.md);
a conflict with the amended commit has the collapsed "How to resolve" hint instead.

#### Scenario: A conflict of a sync
- **WHEN** the viewer of a conflict of a sync shows
- **THEN** it starts with its title, the explanation and the fix

#### Scenario: A conflict with the amended commit
- **WHEN** the viewer of a conflict with the amended commit shows
- **THEN** it has the collapsed "How to resolve" hint instead of the fix

### Requirement: Conflict view: The badge and the title name the commit and its base

The badge MUST say "Merge conflict". The title MUST name the commit and what it is copied onto: "“<subject>” does not
apply cleanly onto <base>", where the base is the parent of a stacked branch or the baseline, and "the amended commit"
for an amend conflict. The subject leaves out the grouping prefix. The sub-windows and the branch preview say the same.

#### Scenario: A conflict onto the baseline or a parent
- **WHEN** a commit conflicts
- **THEN** the badge says "Merge conflict"
- **AND** the title says "“<subject>” does not apply cleanly onto <base>", the subject without its grouping prefix, the
  base the parent of a stacked branch or the baseline
- **AND** the sub-windows and the branch preview say the same
  [@test] src/lib/merge-conflict.test.ts (`names the subject without its grouping prefix and the branch it is copied onto`, `says "the baseline" before the first sync names it`)
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`)

#### Scenario: An amend conflict
- **WHEN** the conflict is one with the amended commit
- **THEN** the base the title names is "the amended commit"

### Requirement: Conflict view: The missing commits

The missing commits MUST be the older commits that change a conflicting file and are neither in the branch nor in the
branches it is built on, each with the branch it is in (`MissingCommit.branch`, none for an unassigned commit). The
copies of the branch's own commits and of its stack ancestors are not missing.

#### Scenario: The missing commits of a stacked branch
- **WHEN** older commits change a conflicting file of a stacked branch
- **THEN** those neither in the branch nor in the branches it is built on are missing, each with the branch it is in
  (`MissingCommit.branch`, none for an unassigned commit)
- **AND** the copies of the branch's own commits and of its stack ancestors are not missing
  [@test] crates/trial-check/src/missing_commits_test.rs (`the_missing_commits_leave_out_the_branches_it_is_built_on_and_name_their_branch`)

### Requirement: Conflict view: The explanation says where the missing commits are

The explanation MUST be one callout in the row and in the missing-commits window. Its title says where the missing
commits are. A sentence says what the branch lacks: "<branch> is built on <base>, which doesn't have them." (not for an
amend conflict).

#### Scenario: The missing commits of one branch
- **WHEN** the missing commits are of one branch
- **THEN** the title of the callout says "18 commits of IJAI-4047 changed these files first"
  [@test] src/lib/merge-conflict.test.ts (`names a single branch`)

#### Scenario: The missing commits of several branches
- **WHEN** the missing commits are of several branches
- **THEN** the title says "42 commits of IJAI-4047 and 11 branches it is built on …" when one branch is built on all the
  others, "… of 12 branches …" otherwise
- **AND** ", as did 2 unassigned commits" for the unassigned ones
  [@test] src/lib/merge-conflict.test.ts (`groups the commits by branch, the most first, and finds the branch built on all the others`, `has no stack top when the branches are on two stacks`)

#### Scenario: No missing commit
- **WHEN** no commit outside the branch changed the conflicting files
- **THEN** the title says "No commit outside the branch changed these files first"
  [@test] src/lib/merge-conflict.test.ts (`says that no commit outside the branch changed the files`)

#### Scenario: What the branch lacks
- **WHEN** the explanation of a conflict of a sync shows
- **THEN** a sentence says "<branch> is built on <base>, which doesn't have them."
  [@test] src/lib/merge-conflict.test.ts (`says what the branch is built on`)

#### Scenario: The explanation of an amend conflict
- **WHEN** the explanation of an amend conflict shows
- **THEN** it has no sentence of what the branch lacks
  [@test] src/lib/merge-conflict.test.ts (`says nothing for a conflict with the amended commit`)

### Requirement: Conflict view: The explanation shows a chip per branch

The explanation MUST show a chip per branch with its number of missing commits, the most first, at most 8 and then "+N
branches", all 9 of 9 (`cutList`: never "+1 branch"). In the main window a chip shows the branch with its missing
commits (`requestReveal`); the window only names them. The common ancestor and the divergence counts are not shown.

#### Scenario: The chips of the branches
- **WHEN** the explanation shows the branches of the missing commits
- **THEN** a chip per branch shows its number of missing commits, the most first, at most 8 and then "+N branches", all 9
  of 9 (`cutList`: never "+1 branch")
- **AND** the common ancestor and the divergence counts are not shown
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`)

#### Scenario: A click on a chip
- **WHEN** the user clicks a chip in the main window
- **THEN** it shows the branch with its missing commits (`requestReveal`)
- **AND** the chips of a separate window only name the branches
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show the branch of the missing commits from the explanation`)

### Requirement: Conflict view: The missing commits details group by branch

The missing commits details MUST group the commits by branch, in the order of the chips, the unassigned ones last under
"Unassigned"; the header of a group names its branch and count, and in the main window shows the branch.

#### Scenario: The groups of the details
- **WHEN** the missing commits details show
- **THEN** the commits are grouped by branch in the order of the chips, the unassigned ones last under "Unassigned"
- **AND** the header of a group names its branch and count, and in the main window shows the branch
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should expand/collapse file accordions in missing commits details`)

### Requirement: Conflict view: The conflicting files, then the missing commits details

The conflicting files MUST follow, and then the missing commits details.

#### Scenario: The order of the viewer
- **WHEN** the viewer shows
- **THEN** the conflicting files follow, and then the missing commits details
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should show branches with missing commits`, `should expand/collapse file accordions in missing commits details`)

### Requirement: Conflict view: The conflicting files come from get_file_merges

The conflicting files MUST come from `get_file_merges`, as IntelliJ's merge view shows them: git's `merge-tree` decides
which files conflict, and the engine merges the three stages of each file again.

#### Scenario: The merges of the conflicting files
- **WHEN** a view reads the conflicting files
- **THEN** `get_file_merges` gives them as IntelliJ's merge view shows them, git's `merge-tree` deciding which files
  conflict
- **AND** the engine merges the three stages of each file again
  [@test] crates/file-diffs/src/merge_test.rs (`each_conflict_type_has_its_merge`, `a_merge_costs_one_merge_tree_and_one_cat_file_then_nothing`, `the_stream_lists_the_files_then_the_file_in_view_first`)
  [@test] crates/test-server/src/commit_files_test.rs (`commit_files_routes_read_the_fixture_repository`)
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`shows the merge in three columns, with the result the engine proposes and the base`)

### Requirement: Conflict view: Each block has its kind, its sides, its words and its resolution

Each block of a merge MUST have its kind, the sides that changed it, its word ranges and its resolution.

#### Scenario: The model of a merge
- **WHEN** the engine merges a file
- **THEN** each block has its kind, the sides that changed it, its word ranges and its resolution
  [@test] crates/file-diffs/src/merge_test.rs (`the_model_is_json_with_a_kind`)
  [@test] crates/text-diff/src/merge_model_test.rs (`blocks_partition_the_three_texts`, `a_block_is_ignored_where_its_changed_sides_are_equal_ignoring_whitespace`)

### Requirement: Conflict view: A conflict its words resolve says so

A conflict that its words resolve MUST say so: "Git stops here; the conflict resolves by words" (`conflicts > 0`,
`unresolved == 0`, the `resolvedText` of each of its blocks).

#### Scenario: A conflict of other words
- **WHEN** the sides of a conflict changed other words
- **THEN** it resolves by its words, the `resolvedText` of each of its blocks
- **AND** with `conflicts > 0` and `unresolved == 0` the view says "Git stops here; the conflict resolves by words"
  [@test] crates/text-diff/src/merge_model_test.rs (`a_conflict_of_other_words_resolves_by_its_words`)

#### Scenario: A conflict of the same words
- **WHEN** the sides of a conflict changed the same words
- **THEN** its words do not resolve it, and the view does not say that it resolves by words
  [@test] crates/text-diff/src/merge_model_test.rs (`a_conflict_of_the_same_words_stays_with_the_user`)

### Requirement: Conflict view: A modify/delete shows the diff of the side that kept the file

A modify/delete MUST show the diff of the base to the side that kept the file.

#### Scenario: The merge of a modify/delete
- **WHEN** one side deleted a conflicting file and the other changed it
- **THEN** the merge of the file is the diff of the base to the side that kept it
  [@test] crates/file-diffs/src/merge_test.rs (`each_conflict_type_has_its_merge`)

### Requirement: Conflict view: The conflicting files are one list of collapsible items

The conflicting files MUST be one list, a collapsible item per file, the first open, each with the number of its
conflicts ("1 conflict", "3 conflicts") or "modify/delete".

#### Scenario: The list of the conflicting files
- **WHEN** a conflict shows its conflicting files
- **THEN** they are one list, a collapsible item per file, the first open
- **AND** each item has the number of its conflicts ("1 conflict", "3 conflicts") or "modify/delete"
  [@test] crates/git-ops/src/merge_conflict_tests.rs (`the_conflicts_of_a_merged_file_are_counted_by_their_start_markers`)
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)

### Requirement: Conflict view: The merge of a file shows in three read-only columns

The merge of a file MUST show in three columns, as IntelliJ's merge viewer, read-only: left the branch the commit is
copied onto (ours), right the commit (theirs), named above them.

#### Scenario: The columns of a merge
- **WHEN** the item of a conflicting file is open
- **THEN** its merge shows in three columns, read-only, as IntelliJ's merge viewer
- **AND** left the branch the commit is copied onto (ours), right the commit (theirs), named above them
  [@test] src/lib/diff/rows.test.ts (`aligns the blocks of a merge in three columns, the middle the result, without the empty last line`)
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`shows the merge in three columns, with the result the engine proposes and the base`)
  [@test] tests/e2e/conflict-fix/conflict-fix.spec.ts (`should offer the move a trial build verified, apply it and undo it`)

### Requirement: Conflict view: The middle column shows the proposed result or the base

The middle column MUST show "Result (proposed)" by default: each block as it resolves (the side that changed it, or its
resolve by words), and the base for a conflict that does not resolve. "Base" MUST show the base.

#### Scenario: Result (proposed)
- **WHEN** a merge shows
- **THEN** the middle column shows "Result (proposed)": each block as it resolves, the side that changed it or its resolve
  by words, and the base for a conflict that does not resolve
  [@test] src/lib/diff/file-merge-model.test.ts (`takes the side that changed a block, the resolve by words, or the base of a conflict that does not resolve`, `composes the result text with the line indices of the middle column`)
  [@test] src/lib/diff/rows.test.ts (`counts the lines of a resolve by words in the middle, and moves the middle lines after it`)
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`shows the merge in three columns, with the result the engine proposes and the base`)

#### Scenario: Base
- **WHEN** the user chooses "Base"
- **THEN** the middle column shows the base
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`shows the merge in three columns, with the result the engine proposes and the base`)

### Requirement: Conflict view: A block has the colour of its kind where it changed

A block MUST have the colour of its kind (inserted green, deleted grey, modified blue, conflict red) in the middle and on
each side that changed it; a side that did not change it stays plain.

#### Scenario: The colours of a block
- **WHEN** a block of a merge shows
- **THEN** it has the colour of its kind (inserted green, deleted grey, modified blue, conflict red) in the middle and on
  each side that changed it
  [@test] src/lib/diff/row-cells.test.ts (`colours a block in the middle and on the sides that changed it, and marks a resolved conflict in the result`)

#### Scenario: A side that did not change the block
- **WHEN** a side did not change a block
- **THEN** that side stays plain
  [@test] src/lib/diff/row-cells.test.ts (`leaves a side that did not change a block plain`)

### Requirement: Conflict view: The tints of a merge follow the rule of a diff

The tints MUST follow the rule of a diff, with the word ranges of each side: a block with inner ranges, also empty ones,
is pale with its changed words in the full colour, and an insertion or a deletion without them is in the full colour. A
conflict that the result resolves has a dashed bar there.

#### Scenario: A block with inner ranges
- **WHEN** a block has inner ranges, also empty ones
- **THEN** it is pale with its changed words in the full colour
  [@test] src/lib/diff/rows.test.ts (`draws the lines of a merge block with inner ranges pale`, `tints the commit_whitespace merge by IntelliJ's rule under each policy`)

#### Scenario: An insertion or a deletion without inner ranges
- **WHEN** an insertion or a deletion has no inner ranges
- **THEN** it is in the full colour

#### Scenario: A conflict the result resolves
- **WHEN** the result resolves a conflict
- **THEN** the conflict has a dashed bar there
  [@test] src/lib/diff/row-cells.test.ts (`colours a block in the middle and on the sides that changed it, and marks a resolved conflict in the result`)

### Requirement: Conflict view: Every block is a change N and P stop at

Every block MUST be a change that N and P stop at; a block the whitespace choice ignores is pale, never drawn as
unchanged lines, as in IntelliJ's merge viewer.

#### Scenario: A block the whitespace choice ignores
- **WHEN** the whitespace choice ignores a block
- **THEN** the block is pale, never drawn as unchanged lines
- **AND** N and P stop at it, as at every block
  [@test] src/lib/diff/rows.test.ts (`draws a block the policy ignores pale, and keeps it a change that N and P stop at, as IntelliJ's merge viewer`)

### Requirement: Conflict view: The equal lines of a merge fold

The equal lines of a merge MUST fold as in a diff.

#### Scenario: Equal lines between blocks
- **WHEN** a merge has equal lines between its blocks
- **THEN** they fold as in a diff
  [@test] src/lib/diff/rows.test.ts (`folds the equal lines of a merge, labelled by the scopes of the left side under the result`)

### Requirement: Conflict view: The merge takes the one Whitespace choice of the diffs

The merge MUST take the one Whitespace choice of the diffs; IntelliJ's merge has no Ignore Whitespace and Blank Lines,
so that choice merges as Ignore Whitespace, and the conflict view has no menu of its own.

#### Scenario: The whitespace choices
- **WHEN** the user picks a Whitespace choice of the diffs
- **THEN** the merge takes it, and Ignore Whitespace and Blank Lines merges as Ignore Whitespace
- **AND** the conflict view has no menu of its own
  [@test] src/lib/diff/rows.test.ts (`tints the commit_whitespace merge by IntelliJ's rule under each policy`)

### Requirement: Conflict view: The view has no resolve action

A resolve action MUST NOT be part of this view.

#### Scenario: The actions of a merge
- **WHEN** the merge of a conflicting file shows
- **THEN** it offers no resolve action

### Requirement: Conflict view: A modify/delete says what happened

A file that one side deleted and the other changed MUST show the change of the side that kept it, under a line that says
what happened: "<branch> does not have the file, the commit changes it: the change of the commit", or "The commit
deletes the file, <branch> changed it: that change". So the list is never empty under a non-zero file count.

#### Scenario: A file without conflict markers
- **WHEN** one side deleted a conflicting file and the other changed it
- **THEN** its item shows the change of the side that kept it, under "<branch> does not have the file, the commit changes
  it: the change of the commit", or "The commit deletes the file, <branch> changed it: that change"
- **AND** the list is never empty under a non-zero file count
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should list every conflicting file, the first open, and say that a file without conflict markers is a modify/delete`)

### Requirement: Conflict view: A sync reports a conflict without its contents

A sync MUST report a conflict without its contents: the commits, each conflicting file with its number of conflicts,
and the missing commits with the files they change. Its events stay a few kilobytes whatever the size of the files.

#### Scenario: A conflict of large files
- **WHEN** a sync meets a conflict
- **THEN** it reports the commits, each conflicting file with its number of conflicts, and the missing commits with the
  files they change, without the contents of the files
- **AND** its events stay a few kilobytes whatever the size of the files
  [@test] crates/sync-core/src/conflict_report_test.rs (`a_sync_reports_a_conflict_without_the_contents_of_its_files`)

### Requirement: Conflict view: A view reads the merges as it shows the conflict

A view MUST read the merges of its conflicting files (`get_file_merges`) when it shows the conflict, one read per
conflict and whitespace choice, the open file first, and keep the last 8; an open item shows "Loading the conflict…"
until its merge arrives.

#### Scenario: A conflict shows
- **WHEN** a view shows a conflict
- **THEN** it reads the merges of its conflicting files (`get_file_merges`), once for all its files, the open file first
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should read the merges of a conflict when its view shows it, once for all its files`)

#### Scenario: Another whitespace choice
- **WHEN** the whitespace choice changes
- **THEN** the view reads the merges once per conflict and whitespace choice, the open file first, and keeps the last 8
  [@test] src/stores/file-diffs.test.ts (`reads the merges of a conflict once per policy, the open file first, and the late scopes keep the lines it expanded`)

#### Scenario: An open item before its merge
- **WHEN** an item is open before its merge arrives
- **THEN** it shows "Loading the conflict…"

### Requirement: Conflict view: The merge works after git gc while the branch conflicts

The merge MUST work after `git gc` too, while the branch conflicts, also after a sync without the object cache: the
sync keeps the copies of the branch's commits before the conflict, which the conflict merges onto, on a ref of its own.

#### Scenario: A git gc while the branch conflicts
- **GIVEN** a branch conflicts
- **WHEN** `git gc` runs, or a sync without the object cache
- **THEN** the merge still works, onto the copies the sync keeps on a ref of its own
  [@test] crates/sync-core/src/process_budget_test.rs (`the_target_of_a_conflict_survives_gc`)
  [@test] crates/sync-core/src/conflict_chain_test.rs (`a_conflicting_branch_keeps_its_copies_while_it_conflicts`, `the_refs_move_with_compare_and_swap_and_a_move_by_another_process_counts`, `a_branch_and_a_branch_below_it_have_refs_of_their_own`)

### Requirement: Conflict view: A failed read says why

A failed read MUST make each open item say "Cannot load the conflict: <reason>" under its header, which keeps the file
and its conflict count. A failed read is read again when the conflict is shown anew.

#### Scenario: The read of the merges fails
- **WHEN** the read of the merges fails
- **THEN** each open item says "Cannot load the conflict: <reason>" under its header, which keeps the file and its
  conflict count
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should say why the merges of a conflict did not load`)
  [@test] src/stores/file-diffs.test.ts (`says why a read failed`)

#### Scenario: The conflict is shown anew
- **GIVEN** a read failed
- **WHEN** the conflict is shown anew
- **THEN** it is read again

### Requirement: Conflict view: The windows of a conflict read its contents themselves

The windows of the conflicting files and of the missing commits MUST get the conflict without its contents, and the
repository, and read the merges and the diffs themselves.

#### Scenario: A window of the conflicting files opens
- **WHEN** the window of the conflicting files or of the missing commits opens
- **THEN** it gets the conflict without its contents, and the repository
- **AND** it reads the merges and the diffs itself
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should load the contents of the conflict in the windows of its files and missing commits`)

### Requirement: Conflict view: The diffs of the missing commits are the engine's

The diffs of the missing commits MUST be the diffs of the engine of their commit (one `get_file_diffs` per commit, its
texts by blob id), so the unchanged lines between hunks expand.

#### Scenario: The diff of a missing commit
- **WHEN** the missing commits details show the files of a commit
- **THEN** their diffs come from one `get_file_diffs` per commit, its texts by blob id
- **AND** the unchanged lines between hunks expand
  [@test] src/lib/diff/view-files.test.ts (`says why a file shows no lines: binary, too large, a submodule, a blob git cannot read, a text that cannot be read, no changed lines`)

### Requirement: Conflict view: The missing commits header has a persisted diff toggle

The missing commits header MUST have the split/unified/inline toggle, persisted: a stored choice keeps winning over the
Split default.

#### Scenario: A stored diff view mode
- **GIVEN** a stored choice of the diff view mode
- **WHEN** the missing commits header shows
- **THEN** its split/unified/inline toggle has the stored choice, over the Split default
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`should keep a stored diff view mode over the Split default`)

### Requirement: Conflict view: A conflict the sync resolved by words shows no conflict

A commit whose conflict of git the sync resolved by words (`branchdeck.resolveByWords`, on by default; see
[Agent CLI](../../cli/agent-cli/spec.md), "`brd sync`") MUST NOT show a conflict: its row says "Resolved 1 conflict by
words" ("Resolved 3 conflicts by words"), a button. A second click hides what the first shows.

#### Scenario: The row of a commit resolved by words
- **GIVEN** `branchdeck.resolveByWords` is on, by default
- **WHEN** the sync resolved a conflict of git of a commit by words
- **THEN** its row shows no conflict and says "Resolved 1 conflict by words" ("Resolved 3 conflicts by words"), a button
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`copies the commit with the result, says so on its row and shows what it committed`)

#### Scenario: A second click
- **GIVEN** "Resolved by Words" shows under the row
- **WHEN** the user clicks the button again
- **THEN** it hides

### Requirement: Conflict view: Resolved by Words shows what the sync committed

A click on the button MUST show "Resolved by Words" under the row: what git stopped at, that the sync copied the commit
with the result as IntelliJ resolves simple conflicts, and the merge of each file in three columns, each file with "1
conflict resolved", the middle "Result (committed)", the blob that the sync committed.

#### Scenario: A click on the button
- **WHEN** the user clicks "Resolved 1 conflict by words"
- **THEN** "Resolved by Words" shows under the row: what git stopped at, that the sync copied the commit with the result
  as IntelliJ resolves simple conflicts
- **AND** the merge of each file in three columns, each file with "1 conflict resolved", the middle "Result
  (committed)", the blob that the sync committed
  [@test] tests/e2e/conflict-viewer/conflict-viewer.spec.ts (`copies the commit with the result, says so on its row and shows what it committed`)
  [@test] crates/file-diffs/src/merge_test.rs (`a_resolved_copy_names_the_blob_of_each_file`)

### Requirement: Conflict view: A reused copy gives the counts of its mapping note

A copy that a later sync reused MUST give the counts of its mapping note, and its files MUST come with the merges.

#### Scenario: A later sync reuses the copy
- **WHEN** a later sync reuses a copy resolved by words
- **THEN** its row gives the counts of its mapping note, and its files come with the merges

### Requirement: Conflict view: A generic error shows in a destructive alert

A row with a generic error MUST show the error text in a destructive alert.

#### Scenario: A row with an error
- **WHEN** a row has a generic error
- **THEN** it shows the error text in a destructive alert

### Requirement: Conflict view: The commit context of a sub-window

The conflict commit context of a sub-window MUST show the title over the meta line of the commit, with the branch name
as an extra meta item. An unknown branch MUST be left out, never shown as "Unknown".

#### Scenario: The commit context with its branch
- **WHEN** a sub-window shows the conflict commit context
- **THEN** the title stands over the meta line of the commit, the branch name an extra meta item

#### Scenario: An unknown branch
- **WHEN** the branch is unknown
- **THEN** it is left out, never shown as "Unknown"

### Requirement: Conflict view: The conflict pages in a separate window

The conflict pages in a separate window MUST show the explanation and the steps of the fix the main window found when
it opened them, under "Apply the fix in the main window" (the window shows a snapshot), or the "How to resolve" hint
without one. Their window titles MUST be "Conflicting files · <branch>" and "Missing commits · <branch>".

#### Scenario: A separate window with a fix
- **WHEN** the main window opens a conflict page in a separate window with a fix it found
- **THEN** the page shows the explanation and the steps of that fix under "Apply the fix in the main window", a snapshot

#### Scenario: A separate window without a fix
- **WHEN** the main window found no fix
- **THEN** the page shows the "How to resolve" hint

#### Scenario: The titles of the windows
- **WHEN** the windows of the conflicting files and of the missing commits open
- **THEN** their titles are "Conflicting files · <branch>" and "Missing commits · <branch>"

### Requirement: Conflict view: The viewer is three nested panels

In the expanded row the viewer MUST be three `NestedPanel`s (`rounded-lg border border-border bg-card`, no ring and no
shadow), 16px apart: the title with the explanation and the fix, the conflicting files, the missing commits. A
sub-window MUST show the same sections as `Card` islands in a `Container`, 8px apart.

#### Scenario: The viewer in the expanded row
- **WHEN** the viewer shows in an expanded row
- **THEN** it is three `NestedPanel`s (`rounded-lg border border-border bg-card`, no ring and no shadow), 16px apart: the
  title with the explanation and the fix, the conflicting files, the missing commits

#### Scenario: The viewer in a sub-window
- **WHEN** a sub-window shows the conflict
- **THEN** it shows the same sections as `Card` islands in a `Container`, 8px apart

### Requirement: Conflict view: The style of the title

The title MUST be `text-sm font-medium` after a `destructive` `TriangleAlertIcon`.

#### Scenario: The title of the viewer
- **WHEN** the viewer shows its title
- **THEN** it is `text-sm font-medium` after a `destructive` `TriangleAlertIcon`

### Requirement: Conflict view: The style of the explanation and its chips

The explanation MUST be a `warning` `Callout` with `FileDiffIcon`; its chips are `secondary` `xs` buttons, the count
muted and tabular.

#### Scenario: The explanation and its chips
- **WHEN** the viewer shows the explanation
- **THEN** it is a `warning` `Callout` with `FileDiffIcon`
- **AND** its chips are `secondary` `xs` buttons, the count muted and tabular

### Requirement: Conflict view: A conflicting file has the file header of a review

A conflicting file MUST be a `rounded-lg border` item with the file header of a review (`FileHeaderParts`: `h-9`, the
chevron first, turned down when open, the name `FilePathLabel` `emphasis`, Copy Path on hover; a click anywhere on it
toggles) and an `outline` badge with its conflict count at its end; the unresolved file has no file header of its own.

#### Scenario: The item of a conflicting file
- **WHEN** a conflicting file shows
- **THEN** it is a `rounded-lg border` item with the file header of a review (`FileHeaderParts`: `h-9`, the chevron
  first, turned down when open, the name `FilePathLabel` `emphasis`, Copy Path on hover)
- **AND** an `outline` badge with its conflict count is at its end, and the unresolved file has no file header of its own

#### Scenario: A click on the header
- **WHEN** the user clicks anywhere on the header of a conflicting file
- **THEN** the item toggles

### Requirement: Conflict view: A file without conflict markers says so above its diff

The file without conflict markers MUST state it in a muted `text-xs` line above its diff.

#### Scenario: A modify/delete file
- **WHEN** a conflicting file has no conflict markers
- **THEN** a muted `text-xs` line above its diff states it

### Requirement: Collapsed row: The subject is in regular weight

The subject MUST use `text-sm text-foreground` in regular weight: weight marks names and titles, so a long list does not
outweigh the branch names ([UI Theme](../../../../docs/design/ui-theme.md#rules-for-ui-changes), "Type hierarchy").

#### Scenario: The subject of a row
- **WHEN** a collapsed row shows its subject
- **THEN** it is `text-sm text-foreground` in regular weight, so a long list does not outweigh the branch names

### Requirement: Collapsed row: The style of the prefix

The prefix MUST use `font-mono text-xs text-muted-foreground`.

#### Scenario: A subject with a prefix
- **WHEN** a collapsed row shows a prefix
- **THEN** the prefix is `font-mono text-xs text-muted-foreground`

### Requirement: Collapsed row: The style of the meta line

The meta line MUST use `text-xs text-muted-foreground`. The hashes MUST use the monospace font.

#### Scenario: The meta line of a row
- **WHEN** a collapsed row shows its meta line
- **THEN** it is `text-xs text-muted-foreground`, its hashes in the monospace font

### Requirement: Collapsed row: The mark follows the subject

The mark MUST follow the subject: a 24px box with a 12px muted `RectangleEllipsisIcon` that keeps the 20px line height
of the subject, `text-foreground` and `bg-muted` on hover; in the tree of a review a 16px box with a 14px icon, as the
mark of a file.

#### Scenario: The mark in a list
- **WHEN** a row shows the mark of its card
- **THEN** the mark follows the subject: a 24px box with a 12px muted `RectangleEllipsisIcon` that keeps the 20px line
  height of the subject
- **AND** it is `text-foreground` and `bg-muted` on hover

#### Scenario: The mark in the tree of a review
- **WHEN** a commit row of the tree of a review shows the mark
- **THEN** it is a 16px box with a 14px icon, as the mark of a file

### Requirement: Collapsed row: Issue links are muted

Issue links MUST be muted and become `text-foreground` on hover.

#### Scenario: The hover of an issue link
- **WHEN** the pointer rests on an issue link of a row
- **THEN** the muted link becomes `text-foreground`

### Requirement: Collapsed row: The push arrow follows the mark

The push arrow MUST follow the mark: a 16px button with a 12px `ArrowUpIcon` in `text-primary`, `bg-primary/10` on
hover. A row its Push Up to Here sends is `bg-primary/5`; the selection tint wins over it.

#### Scenario: The push arrow
- **WHEN** a row shows the push arrow
- **THEN** it follows the mark: a 16px button with a 12px `ArrowUpIcon` in `text-primary`, `bg-primary/10` on hover

#### Scenario: A row Push Up to Here sends
- **WHEN** Push Up to Here sends a row
- **THEN** the row is `bg-primary/5`, and the selection tint wins over it

### Requirement: Collapsed row: Badges follow the mark

Badges MUST follow the mark: `secondary` Pending with a spinner, `destructive` conflict or error, `warning` Blocked,
`primary` Amend target (the target of the user's amend).

#### Scenario: The badges of a row
- **WHEN** a row shows a badge
- **THEN** it follows the mark: `secondary` Pending with a spinner, `destructive` conflict or error, `warning` Blocked,
  `primary` Amend target (the target of the user's amend)

### Requirement: Hover and selection: A selectable row reacts to the hover

A selectable row MUST show the pointer cursor and `bg-muted` on hover.

#### Scenario: The hover of a selectable row
- **WHEN** the pointer rests on a selectable row
- **THEN** it shows the pointer cursor and `bg-muted`

### Requirement: Hover and selection: The tint of a selected row

A selected row MUST use `bg-primary/15` (the IntelliJ light selection), and `bg-primary/20` on hover.

#### Scenario: A selected row, hovered or not
- **WHEN** a row is selected
- **THEN** it is `bg-primary/15` (the IntelliJ light selection), and `bg-primary/20` on hover

### Requirement: Hover and selection: A selected row has no accent bar

A selected row MUST NOT have an accent bar.

#### Scenario: The edge of a selected row
- **WHEN** a row is selected
- **THEN** it shows no accent bar

### Requirement: Hover and selection: A row that is not selectable has no hover tint

A row that is not selectable MUST NOT have a hover tint and MUST keep the default cursor.

#### Scenario: The hover of a row that is not selectable
- **WHEN** the pointer rests on a row that is not selectable
- **THEN** it has no hover tint and keeps the default cursor

### Requirement: Hover and selection: The style of the selection actions

The selection actions MUST show `N selected` in muted text, then an outline "Move to…" button in the primary colour.

#### Scenario: The selection actions of the lead row
- **WHEN** the lead row shows the selection actions
- **THEN** they show `N selected` in muted text, then an outline "Move to…" button in the primary colour

### Requirement: Branch table: The commits of an expanded branch stand on its lane

An expanded branch of the branch table MUST show its commits on its lane: a 5px dot (`--bd-lane-dot`) per commit at the
middle of its subject line, on the line of the lane, which runs on into the children of the branch or ends at its last
commit, beside the other lanes that run on ([Branch Row](../../branch/branch-row/spec.md#tree)). The highlighted path to
the root of a stack runs on beside them as well.

#### Scenario: An expanded branch with children
- **WHEN** a branch of the branch table is expanded
- **THEN** each commit has a 5px dot (`--bd-lane-dot`) at the middle of its subject line, on the line of the lane
- **AND** the lane runs on into the children of the branch or ends at its last commit, beside the other lanes that run
  on and the highlighted path to the root of a stack

### Requirement: Branch table: Without stacks one lane and the newest commit first

Without stacks the lane MUST be the one lane of the node of each branch, and the commits MUST be newest first.

#### Scenario: An expanded branch without stacks
- **GIVEN** no stacks
- **WHEN** a branch is expanded
- **THEN** its lane is the one lane of the node of the branch, and its commits are newest first

### Requirement: Branch table: The subjects start at the edge of the branch name

The subjects MUST start at the left edge of the branch name.

#### Scenario: The commits under their branch
- **WHEN** a branch is expanded
- **THEN** the subjects of its commits start at the left edge of the branch name

### Requirement: Branch table: An archived branch uses the rail

An archived branch MUST use the rail.

#### Scenario: An expanded archived branch
- **WHEN** an archived branch is expanded
- **THEN** its commits show on the rail

### Requirement: Dark theme: The row uses only semantic tokens

The row MUST use only shadcn semantic tokens and MUST NOT use `dark:` variants.

#### Scenario: The classes of a row
- **WHEN** a row renders, in either theme
- **THEN** its colours are shadcn semantic tokens, and it has no `dark:` variants

### Requirement: Dark theme: The colours follow the theme tokens

The body colour and the row tints MUST follow the theme tokens in both themes.

#### Scenario: The dark theme
- **WHEN** the dark theme or the light theme is on
- **THEN** the body colour and the row tints follow the theme tokens

## Testing / Local Run

- `cargo nextest run -p sync-utils -p history-rewrite` runs the tests of the body parser, the subject parts and the
  grouping preview; `pnpm test:unit` those of the sections, the code detection and the subject prefix in `src/lib`.
- `pnpm e2e` runs the E2E tests. The tests in `tests/e2e/commit-message` cover the commit preview and its body.

## Open Questions / Risks

- No test covers the Pending, Blocked and generic Error badges. No backend path in the mock server emits them.
- No test covers the height limit of the commit card, its header staying as the body scrolls, or a commit without a
  subject in it.
- The preview shows the whole body; the card and the story of a review fold the parts Rust folds. A body whose shown
  parts alone are taller than 24rem still scrolls in the card.
- No test checks that the subject does not detect code.
- Base UI moves the focus into the preview a frame after it opens. Until then, the list ignores its keys, so a fast
  ↓ right after Space is lost.
- No test covers the generic error alert, or the explanation and the steps of the fix in the conflict pages of a
  separate window.
- Code detection is heuristic. A false positive sets a prose word in the monospace font. Add such words to the stoplist.
- Reflow can join lines that the author aligned by hand. An indent of four spaces keeps such lines verbatim.

## References

- [UI Theme: Islands Layering and the Radius Scale](../../../../docs/design/ui-theme.md)
- [Virtual Branches](../../../../docs/design/virtual-branches.md)
- GitHub, GitLab and Gerrit show a commit body as monospace `pre-wrap` text.
- Branch Deck reflows the body into proportional prose, lists and code blocks.
- A layout like the man page `.TP` macro put the section labels in a side gutter. That layout was tried and dropped.
- The section labels now stack above their blocks.
- The body was first expanded inline below the subject. It pushed the other rows down, and the selection tint of a
  selected row covered the whole body. The preview replaced it.
- The preview follows macOS Quick Look (Space opens and closes it, the arrows step through the items) and the peek
  of Linear (Space acts on the hovered item).
