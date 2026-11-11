# Proposal

## Why

The old lines of a quiet run that Inline shows once could be looked at only with the pointer: the note that opens
them is not in the Tab order, as a fold, and no key opened it, so from the keys a mark could be checked only by
switching the whole view to Unified or Split. And a name of the old side shown on an unchanged line, which one column
shows once by its new line, was selected at the columns it has in the old line: on a line the whitespace policy
ignores (another indent) that is other text of the new line; after a press on the old side in Split the one column
was not selectable at all until the next press in the diffs.

## What Changes

- H in a review window shows the old lines of every note in view that opens, and hides them again once they all
  show, from the diffs and from the tree of the files. Such a note names the key after its words and to a screen
  reader, only where the key works: the preview of a commit shows none. The key is the key of no action of a window
  that holds a review (`REVIEW_KEYS` of `src/lib/review-keys.ts`): O stays the pull request's, also with a note in
  view.
- A name of the old side on an unchanged line of one column is selected in the line the row shows, at the columns
  the name has there, by the characters that are no whitespace.
- The one column of a row is always selectable, whatever side a press in Split kept the selection on.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the key of the notes, in the group Views.
- `commit/symbol-navigation`: the name of a declaration shown, on a line one column shows by its new line.

## Impact

- `src/components/diff/DiffView.tsx` (`DiffViewHandle.toggleNotes`, `notesKey`), `CommitReviewCode.tsx`,
  `src/components/commit/CommitReview.tsx` (the key), `src/lib/review-keys.ts` (the keys of the review, one table),
  `diff-painter.ts` (the hint, `selectRevealedName`), `diff-view.css`.
- `src/lib/diff/rows.ts` (`FileRows.noteRows`), `block-parts.ts` (`partsNotes`), `line-parts.ts` (`sameTextOffset`).
- `tests/e2e/commit-files/quiet-changes.spec.ts`, `symbol-navigation.spec.ts`.
