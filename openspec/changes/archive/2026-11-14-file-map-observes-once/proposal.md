# Proposal

## Why

The map of the files of a review window made a new `ResizeObserver` for each layout of the diffs: its effect depended
on the `scroller` the view passes, a new function whenever the layout changes. As a review window opened, the view laid
out several times (the heights it measures render at once, `flushSync`), and each made a new observation of the view
(9 of the track of the map in one opening). In the frame of the first heights that new observation could not be
delivered, which Chromium reports as "ResizeObserver loop completed with undelivered notifications", 4 to 7 times in
110 to 165 e2e tests under load, and the view laid out once more in the next frame.

## What Changes

- The map observes the sizes of its track and of the view once, as it mounts; it reads the scroller through an effect
  event.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `commit/commit-review`: the map observes its track and the view once.

## Impact

- `src/components/diff/DiffFileMap.tsx`; `docs/design/diff-view.md`.
