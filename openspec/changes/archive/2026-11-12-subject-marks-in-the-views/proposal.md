# Proposal

## Why

The diffs of a subject say what its files say of each other (the lines that moved from one file to another, the name
one file declares and another uses) and what the index of the repository proved of such a name, but no view worded
it: a function that left `Ledger.kt` for `Report.kt` was deleted lines in one file and added lines in the other, with
no word that they are the same lines, and the calls of a function another file renamed were changes as any, each
under a note "Renamed only" of its own. A reader could not tell a rename that is complete in the change from one
that leaves uses behind.

The marks of a subject come after the rows of a file were shown (the files are compared first, the index answers
seconds later), so a view must take them without moving what is read.

## What Changes

- A move between two files has a note at each end, "Moved to Report.kt:18" and "Moved from Ledger.kt:55", and its
  lines in the colour of a move, every line in every layout. The place is a button that shows the other end where a
  view shows that file: in a review window in its card, with Back; in the preview of a commit and in the uncommitted
  changes, where each file has a view of its own, by opening that file. G and the menu of the code take it as the
  note of any move. Plain words where no view shows the file.
- A name another file declares is a rename of the file that uses it: one note at its first site, with "declared in
  Store.kt", a button that shows the declaration, and the name in its mark at every site.
- The note of a rename that is not local says what the index proved: "all uses in this change", "uses outside this
  change", "changes what another name means", "more than the name changed", "uses not all checked".
- A run the index proved is passed by N and P and played down, and keeps every line in every layout.
- Marks that come late add rows of words and tints alone, and the row at the top of the view stays where it is, also
  where the view is a block of another scroller and where it lies above the viewport.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the Views requirements of the marks of a subject.

## Impact

- `src/lib/diff/block-parts.ts`, `rename-sites.ts`, `row-cells.ts`, `layout.ts`, `code-menu.ts`,
  `file-diff-model.ts`; `src/presenters/diff-marks.ts`; `src/components/diff/diff-painter.ts`, `DiffView.tsx`,
  `diff-view.css`; `src/stores/file-stream.ts`, `file-diffs.ts`; `src/components/commit/CommitFilesSection.tsx`,
  `InlineCommitFileDiff.tsx`, `src/components/conflict/SubjectFileDiffList.tsx`.
- `tests/e2e/commit-files/subject-marks.spec.ts` (new), `quiet-changes.spec.ts`, `moved-lines.spec.ts`,
  `renamed-names.spec.ts`, `tests/e2e/helpers/marks-helpers.ts`, `diff-helpers.ts`.
