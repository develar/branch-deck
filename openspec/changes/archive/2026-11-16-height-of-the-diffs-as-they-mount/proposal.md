# The diffs of a review read their height as they mount

## Why

`quiet-changes.spec.ts:484` failed 2 of 5 runs of the suite on the Mac Studio, 5 of 40 alone under load: the first N
of the test found "no header at the top of the diffs". A probe that logged every write of `scrollTop` showed N set the
diffs to the top of its stop (115 px) and no scroll followed: the view was not tall enough to scroll there. The space
past the end of a review window is sized from the height of the view (`DiffLayout.pastEnd`), which only the first
report of its `ResizeObserver` set; that report comes in a rendering update, a frame later or two when the observation
waits for the end of a delivery, so a key in between laid out no space past the end, the scroll stopped at the end of
the cards, and the change cursor went on from a stop the reader never saw.

## What Changes

- `DiffView` reads the content box of its scroller as it mounts (`laidOutBoxes`, the box the observer reports), so the
  first layout has the space past its end; the reports that follow change nothing unless the size changed.
- Capability `commit/commit-review`: the space past the end is sized from the height of the view as it mounts; a
  scenario of a key before the first report.
