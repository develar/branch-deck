# Proposal

## Why

A function moved to another file with its parameters wrapped, or a type moved out of its owner and reformatted, is
read today as deleted and added lines: the moves between files take exact lines alone, so the reader compares the two
ends by hand. accio answers which declarations a change moved whole with the same words (`accio moves`, answer
revision 2), whatever their layout; Branch Deck asked it nothing.

## What Changes

- The window's question of the index (`get_rename_proofs`, now `get_index_marks`: one command, `COMMAND_COUNT` the
  same) answers in stages on a channel (`IndexMarks`): the renames first, as before, then `accio moves --no-build`
  between the two trees of the subject, of an accio whose `moves` is of revision 2, where a file deletes a line and
  one adds one, kept by the trees; the moves never delay a proof of a rename.
- `Subject::with_moves`: a declaration that moved whole is a move between files (`FileMove::declaration`: its name, the
  lines of the other end), its layout another too, where each end is the changed lines of one block that no mark of
  its pair took. A declaration a row of the same lines holds as it is stays that row; of a row that shares lines with
  a declaration of another layout, the parts beside it that are a row by themselves stay moves.
- The owner in front of the uses of a declaration that moved under another owner (`Ledger.title()` to
  `Report.title()`, a name that changed so at every place of its file) follows the move (`UsedRename::moved`) once
  `accio renames` of the moved pair, the last stage, lists every use behind an owner in both trees: its runs are
  quiet, and the note says "Follows a move". Unproven, it says nothing: the call may be of another declaration.
- The diffs of two trees say the index is there to be asked (`asksIndex` of their `files` event), so the window asks it
  of a subject that renamed nothing.
- The note of such an end names the declaration ("Moved summary to Report.kt:12").
- `brd commits --mark-stats --index` counts the declarations apart from the rows, the owners that follow a move, and
  the time of the moves apart from that of the renames.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: a declaration the index says moved whole is a move between files at both ends, asked once
  per subject after the renames, and its note names it; the owner in front of its uses follows the move.
- `commit/commit-files`: the answer of the index comes in stages.

## Impact

- `crates/diff-marks/src/{index,model,marks_subject}.rs`, `crates/text-diff/src/model.rs`, `crates/file-diffs/src/{accio,model,service}.rs`,
  `crates/app-api` (the command renamed, a channel of stages), `src/stores/{file-stream,file-diffs}.ts`,
  `src/lib/diff/block-parts.ts`, `src/presenters/diff-marks.ts`, `crates/bd-rewrite/src/mark_stats.rs`, the template
  `declaration_moves` and the e2e helpers.
- `docs/design/diff-view.md` ("Marks"), `docs/design/change-review.md` (Next, item 2), `docs/design/git-operations.md`.
