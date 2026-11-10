# Proposal

## Why

The diffs of a review ended at the end of the last card. In a short document J, K, N, P and a row brought a section
near the end only as far up as the end let it: the bar, the footer and the tree named the section J showed while the
top of the view showed the card above it, and the selection had a rule of its own for "a last file that cannot reach
the top". IntelliJ's editor and diff viewer scroll past the end of a file, so a line reaches the top; the user chose
that.

## What Changes

- The review windows scroll past the end of their diffs by as much empty space as the view needs for its last stop
  to reach the top: the card of the last file, else, below an epilogue, its end less a row. The space is part of the
  virtual layout (`DiffLayout.pastEnd`, counted by `height`), sized from the height of the view, laid out again on a
  resize and never on a scroll; none where the last card is as high as the view.
- J, K, N, P and a row bring the section they show to the top; the section at the top is the one the bar, the footer,
  the tree and the map name. The rule that kept the section the view showed last while it was on screen applies only
  where a view ends at its end.
- A diff that is a block of another scroller (the preview of a commit, the conflict viewer) and the view of a file
  outside the review end at their end, as before.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-review`: the diffs scroll past their end; a last file reaches the top.
- `branch/review-tour`: the bar at the end of a short document.

## Impact

- `src/lib/diff/layout.ts` (`pastEnd`, `contentHeight`, `lastStopTop`), `src/components/diff/DiffView.tsx` (the
  height of the view, the section at the top); the e2e of the parts of a file and of the map.
