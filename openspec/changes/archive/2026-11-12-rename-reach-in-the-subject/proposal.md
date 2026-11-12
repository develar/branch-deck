# Proposal

## Why

The note at the declaration of a renamed name counted the places of its own file beside a proof that speaks of the
whole change: "Renamed openStore → connectStore · 1 site · all uses in this change" for three places in two files.
A reader of the declaring file could not tell how many uses "all uses" covers, or that other files hold the name.

## What Changes

- The pass over a subject says, for each name a file declares that is not local, how many places of the files of
  the subject hold it and in how many files (`RenameCheck.sites`, `files`).
- The note at the declaration words them where another file holds the name: "Renamed openStore → connectStore · 3
  sites in 2 files"; the note of a file that uses the name still counts its own places.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: one requirement added to the group Subject, one to the group Views.

## Impact

- `crates/text-diff/src/model.rs`, `crates/file-diffs/src/marks_subject.rs`, `src/lib/bindings.ts`.
- `src/lib/diff/block-parts.ts` (`RenameReach.across`), `src/presenters/diff-marks.ts`,
  `src/components/diff/diff-painter.ts`.
