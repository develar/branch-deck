# Proposal

## Why

The marks of a diff were those of a pair of blobs alone: a function that moved from one file to another was a deleted
block and an added block with no word that they are the same lines, and a function renamed in one file was read again
at each of its calls in the other files of the change, which a pair cannot explain (the file of a call declares
nothing). Only the files of a subject together, and for a name the index of the repository, say what happened.

## What Changes

- A pair keeps, beside its diff, what it leaves to the other files of a subject: the changed lines no mark took, as
  hashed keys in rows, the names it declares none of with the runs their blocks have were they renames, and the
  places its declarations are asked of the index by. The pass over a subject reads no blob.
- The pass over the files of a subject (`marks_subject`), a pure function of what its pairs are and left: the moves
  between files, exact lines from 48 characters and two lines, labels at both ends; a name another file declares as a
  rename of the file that uses it, the lines that changed in it alone as runs that are labels.
- The declarations of the renamed names that are not local are asked of the index once per subject (`accio renames`).
  A complete answer whose every use the subject explains proves the rename: its runs are named as confirmed, quiet
  once their kind is switched on; every other answer names why not.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the group Subject.

## Impact

- `crates/text-diff/src/moves_across.rs` (new), `model.rs` (`DiffMarks::subject`, `SubjectMarks`, `FileMove`,
  `RenameCheck`, `UsedRename`, `RenameProof`).
- `crates/file-diffs/src/leftovers.rs`, `marks_subject.rs` (new), `marks.rs`, `marks_renames.rs` (the third reading),
  `accio.rs` (`renames`), `service.rs`.
- `crates/object-cache/src/file_diff.rs` (the `file_leftovers` store).
- `docs/design/diff-view.md` ("Marks"), `docs/design/text-diff.md` ("Moves between files"),
  `docs/design/git-operations.md`, `docs/design/change-review.md`.
