# Proposal

## Why

The memory of the app kept an answer of `accio renames` only when every pair of it was complete with the uses of both
trees, a pair accio could not ask, or two places that are not one declaration. A pair with one use tied by its name
alone, or a side cut by the work, is the same answer whenever the same trees are asked, yet the window started an
accio process for it each time it showed the subject again. accio now says per pair whether the same trees give the
answer again (`final`, its decision 2026-11-13-rename-answer-says-final, JetBrains/accio#742): false only when the
clock of the call cut a pair of it, and then for every pair of the call that read anything.

## What Changes

- `RenameAnswer` reads `final` (`is_final`) and `cut`; `RenameAnswer::is_final` is the field, false when an accio
  before it writes none.
- `FileDiffs::rename_proofs` keeps each final answer by the trees and the questions of the subject, and asks again only
  the pairs whose answer was not final, in one call (`ask_again`); memory holds an empty answer in their place.
- The fixtures of the index (`FixtureIndex`, the `commit_marks` template, `answerStoreRename`) answer `final`.
- The notes of `commit/quiet-changes` (Open Questions) no longer ask for a flag of accio: `final` decides what is kept.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: a final answer of the index is kept complete or not, and only the pairs whose answer was
  not final are asked again.
- `commit/commit-files`: an answer of the index the clock of its query cut, or of an accio before `final`, is that of
  its question alone; an incomplete final one is kept.

## Impact

- `crates/diff-marks/src/index.rs`, `crates/file-diffs/src/service.rs`, `crates/file-diffs/src/accio.rs`, the test
  fixtures.
- `docs/design/diff-view.md` ("Marks"), `docs/design/git-operations.md` (the index of the diffs).
