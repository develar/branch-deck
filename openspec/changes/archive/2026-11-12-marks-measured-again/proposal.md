# Proposal

## Why

The measurement of the marks had three gaps the review named.

- It read what the thresholds of a move let in and never what they leave out: no value but the one in use could
  be asked, so "the thresholds stand" rested on the moves found alone.
- Its check of itself (`differ`: the marks read again are the marks of the view) took a run the index confirmed
  for a difference, and no test asked the index at all: the 45 commits asked of accio ran on a build from before
  the kind `renamed` was switched on.
- `--index` on a machine without accio measured nothing and said nothing, and a commit id or a filter of `brd
  commits` beside `--mark-stats` was accepted and ignored.

## What Changes

- `brd commits --mark-stats --min-move-chars <N>` and `--max-edited <N/D>` search each file once more under those
  thresholds (`text_diff::moves::find_moves_under`) and count and sample what that adds to the moves of the views
  and what it makes of them.
- Measured so, a quarter of edited lines cut 47 moves of 3,129 short or left them out, 46 of them the lines that
  left with an edit in each: a move holds a third of edited lines now (`MAX_EDITED_LINES`). 16 characters in
  place of 24 add 363 moves, most of them a short line that stands again in another record or another function:
  the 24 stay.
- A run the index confirmed is proven and no difference; `--index` is refused without accio; `--mark-stats` takes
  no commit id and no filter.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/quiet-changes`: the groups Moves and Measurement.

## Impact

- `crates/text-diff/src/moves.rs` (`MAX_EDITED_LINES`, `MoveThresholds`, `find_moves_under`),
  `crates/file-diffs/src/marks_measure.rs` (`MeasuredText::other_moves`, the check of the runs),
  `crates/bd-rewrite/src/mark_stats.rs` (`mark_stats_with`, the flags, `OtherMoveCounts`).
- `docs/design/diff-view.md` ("Measured on real history"), `docs/design/text-diff.md` ("Moves").
