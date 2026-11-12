# Proposal

## Why

A place a note asked of the diff of another file stayed with the surface after it was shown: a diff of that file
that mounted again (its directory folded and unfolded, the diffs read again) scrolled the preview back to it. Back
from "declared in Store.kt" returned to the first site of the rename in the file, not to the site of the note. The
scroller of a preview kept the engine's scroll anchoring off for good once a diff was opened in it, and a diff whose
rows came above the view while it was still read moved what the reader had on screen. "declared in Store.kt" could
be reached by a click alone, and its plain words had no test in a browser.

## What Changes

- A place a note names in another file is shown once: the view tells the surface that it showed it, and the surface
  forgets it. Asked again by its note, it is shown again.
- Back from the declaration returns to the site the note names the rename at.
- The engine's scroll anchoring is off in a scroller only while it holds a view. A file that opens while its diff is
  still read has its view at once, which says "Loading…" in a row; its rows come in that view, so what is on screen
  below it stays where it is.
- Go to Declaration of the menu of the code on the note of a name another file declares shows that declaration.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: five requirements of the group Views changed, one added.
- `commit/commit-files`: the file that expands to its diff under its row.

## Impact

- `src/lib/diff/block-parts.ts` (`RenameNote.sites`, `renameJumpOf`), `layout.ts`, `code-menu.ts`;
  `src/components/diff/DiffView.tsx`, `diff-painter.ts`, `diff-view.css`;
  `src/components/commit/CommitFilesSection.tsx`, `InlineCommitFileDiff.tsx`,
  `src/components/conflict/SubjectFileDiffList.tsx`.
- `tests/e2e/commit-files/subject-marks.spec.ts`, `moved-lines.spec.ts`, `tests/e2e/helpers/diff-helpers.ts`.
